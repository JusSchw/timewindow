use chrono::{DateTime, Duration, Utc};

use crate::{Window, WindowSource};

/// A single recurring interval definition relative to a shared source anchor.
///
/// Each pattern entry defines its own recurrence schedule:
///
/// - `start` is the offset from the source anchor to the first occurrence
/// - `every` is the recurrence period for subsequent occurrences
/// - `duration` is the window length
/// - `meta` is cloned into each generated occurrence
///
/// If a source has:
///
/// - anchor = `2026-03-20T00:00:00Z`
///
/// and a pattern entry with:
///
/// - start = 9 hours
/// - every = 1 day
/// - duration = 3 hours
///
/// then it produces windows like:
///
/// - `[2026-03-20 09:00, 2026-03-20 12:00)`
/// - `[2026-03-21 09:00, 2026-03-21 12:00)`
/// - `[2026-03-22 09:00, 2026-03-22 12:00)`
///
/// and so on.
///
/// Another entry in the same source could have a completely different cadence,
/// for example:
///
/// - start = 15 minutes
/// - every = 2 hours
/// - duration = 20 minutes
///
/// which would produce:
///
/// - `[2026-03-20 00:15, 2026-03-20 00:35)`
/// - `[2026-03-20 02:15, 2026-03-20 02:35)`
/// - `[2026-03-20 04:15, 2026-03-20 04:35)`
///
/// and so on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntervalPattern<M> {
    pub start: Duration,
    pub every: Duration,
    pub duration: Duration,
    pub meta: M,
}

impl<M> IntervalPattern<M> {
    /// Creates a new recurring pattern entry.
    pub fn new(start: Duration, every: Duration, duration: Duration, meta: M) -> Self {
        Self {
            start,
            every,
            duration,
            meta,
        }
    }
}

/// A window source composed of one or more independently recurring interval
/// patterns sharing a common anchor.
///
/// For each [`IntervalPattern`] and each integer occurrence index `k >= 0`,
/// a concrete window occurrence is:
///
/// - `start = anchor + pattern.start + k * pattern.every`
/// - `end = start + pattern.duration`
///
/// # Overlap
///
/// Pattern entries may overlap each other. A single pattern entry may also
/// overlap with its own subsequent occurrences if `duration > every`.
///
/// # Canonical pattern rules
///
/// This source requires:
///
/// - `pattern` is not empty
/// - for each pattern entry:
///   - `start >= 0`
///   - `every > 0`
///   - `duration > 0`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntervalSource<M> {
    anchor: DateTime<Utc>,
    pattern: Vec<IntervalPattern<M>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntervalSourceError {
    EmptyPattern,
    NegativeStart,
    StartOutOfRange,
    NonPositiveEvery,
    EveryOutOfRange,
    NonPositiveDuration,
    DurationOutOfRange,
}

impl<M> IntervalSource<M> {
    pub fn new(
        anchor: DateTime<Utc>,
        mut pattern: Vec<IntervalPattern<M>>,
    ) -> Result<Self, IntervalSourceError> {
        if pattern.is_empty() {
            return Err(IntervalSourceError::EmptyPattern);
        }

        for entry in &pattern {
            if entry.start < Duration::zero() {
                return Err(IntervalSourceError::NegativeStart);
            }

            entry
                .start
                .num_nanoseconds()
                .ok_or(IntervalSourceError::StartOutOfRange)?;

            if entry.every <= Duration::zero() {
                return Err(IntervalSourceError::NonPositiveEvery);
            }

            entry
                .every
                .num_nanoseconds()
                .ok_or(IntervalSourceError::EveryOutOfRange)?;

            if entry.duration <= Duration::zero() {
                return Err(IntervalSourceError::NonPositiveDuration);
            }

            entry
                .duration
                .num_nanoseconds()
                .ok_or(IntervalSourceError::DurationOutOfRange)?;
        }

        pattern.sort_by(|a, b| {
            a.start
                .cmp(&b.start)
                .then_with(|| a.every.cmp(&b.every))
                .then_with(|| a.duration.cmp(&b.duration))
        });

        Ok(Self { anchor, pattern })
    }

    /// Convenience constructor for a source with a single recurring interval.
    pub fn single(
        anchor: DateTime<Utc>,
        start: Duration,
        every: Duration,
        duration: Duration,
        meta: M,
    ) -> Result<Self, IntervalSourceError> {
        Self::new(
            anchor,
            vec![IntervalPattern {
                start,
                every,
                duration,
                meta,
            }],
        )
    }

    #[inline]
    pub fn anchor(&self) -> DateTime<Utc> {
        self.anchor
    }

    #[inline]
    pub fn pattern(&self) -> &[IntervalPattern<M>] {
        &self.pattern
    }

    fn pattern_start_ns(entry: &IntervalPattern<M>) -> i64 {
        entry
            .start
            .num_nanoseconds()
            .expect("validated in constructor: start fits in nanoseconds")
    }

    fn pattern_every_ns(entry: &IntervalPattern<M>) -> i64 {
        entry
            .every
            .num_nanoseconds()
            .expect("validated in constructor: every fits in nanoseconds")
    }

    fn pattern_duration_ns(entry: &IntervalPattern<M>) -> i64 {
        entry
            .duration
            .num_nanoseconds()
            .expect("validated in constructor: duration fits in nanoseconds")
    }

    fn occurrence_start_at(
        &self,
        entry: &IntervalPattern<M>,
        occurrence_index: i64,
    ) -> Option<DateTime<Utc>> {
        if occurrence_index < 0 {
            return None;
        }

        let every_ns = Self::pattern_every_ns(entry);
        let start_ns = Self::pattern_start_ns(entry);

        let repeated_ns = every_ns.checked_mul(occurrence_index)?;
        let total_ns = start_ns.checked_add(repeated_ns)?;

        self.anchor
            .checked_add_signed(Duration::nanoseconds(total_ns))
    }

    fn window_at(&self, pattern_index: usize, occurrence_index: i64) -> Option<Window<M>>
    where
        M: Clone,
    {
        let entry = self.pattern.get(pattern_index)?;
        let start = self.occurrence_start_at(entry, occurrence_index)?;
        let end = start.checked_add_signed(entry.duration)?;

        Window::new(start, end, entry.meta.clone())
    }

    fn occurrence_index_floor_for_time(
        &self,
        entry: &IntervalPattern<M>,
        time: DateTime<Utc>,
    ) -> i64 {
        let first_start = match self.anchor.checked_add_signed(entry.start) {
            Some(dt) => dt,
            None => {
                return -1;
            }
        };

        let delta = time - first_start;
        let delta_ns = delta.num_nanoseconds().unwrap_or_else(|| {
            if time < first_start {
                i64::MIN
            } else {
                i64::MAX
            }
        });

        delta_ns.div_euclid(Self::pattern_every_ns(entry))
    }

    fn max_active_occurrences_back(entry: &IntervalPattern<M>) -> i64 {
        let every_ns = Self::pattern_every_ns(entry);
        let duration_ns = Self::pattern_duration_ns(entry);

        (duration_ns - 1).div_euclid(every_ns)
    }
}

impl<M> WindowSource for IntervalSource<M>
where
    M: Clone,
{
    type Meta = M;

    fn active_windows(&self, now: DateTime<Utc>) -> Vec<Window<Self::Meta>> {
        let mut windows = Vec::new();

        for (pattern_index, entry) in self.pattern.iter().enumerate() {
            let current_occurrence = self.occurrence_index_floor_for_time(entry, now);
            let max_back = Self::max_active_occurrences_back(entry);

            let start_occurrence = current_occurrence.saturating_sub(max_back).max(0);

            for occurrence_index in start_occurrence..=current_occurrence.max(-1) {
                if occurrence_index < 0 {
                    continue;
                }

                if let Some(window) = self.window_at(pattern_index, occurrence_index) {
                    if window.is_active(now) {
                        windows.push(window);
                    }
                }
            }
        }

        windows.sort_by(|a, b| a.start.cmp(&b.start).then_with(|| a.end.cmp(&b.end)));
        windows
    }

    fn next_window(&self, after: DateTime<Utc>) -> Option<Window<Self::Meta>> {
        let mut best: Option<Window<Self::Meta>> = None;

        for (pattern_index, entry) in self.pattern.iter().enumerate() {
            let floor = self.occurrence_index_floor_for_time(entry, after);

            let candidate_index = floor.saturating_add(1).max(0);

            if let Some(window) = self.window_at(pattern_index, candidate_index) {
                if window.start > after {
                    match &best {
                        Some(current)
                            if current.start < window.start
                                || (current.start == window.start && current.end <= window.end) => {
                        }
                        _ => best = Some(window),
                    }
                }
            }
        }

        best
    }
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, TimeZone, Utc};

    use crate::WindowSource;

    use super::{IntervalPattern, IntervalSource, IntervalSourceError};

    fn dt(y: i32, m: u32, d: u32, hh: u32, mm: u32, ss: u32) -> chrono::DateTime<Utc> {
        Utc.with_ymd_and_hms(y, m, d, hh, mm, ss).unwrap()
    }

    #[test]
    fn rejects_empty_pattern() {
        let anchor = dt(2026, 3, 20, 0, 0, 0);
        let err = IntervalSource::<()>::new(anchor, vec![]).unwrap_err();

        assert_eq!(err, IntervalSourceError::EmptyPattern);
    }

    #[test]
    fn rejects_negative_start() {
        let anchor = dt(2026, 3, 20, 0, 0, 0);
        let err = IntervalSource::new(
            anchor,
            vec![IntervalPattern::new(
                Duration::hours(-1),
                Duration::days(1),
                Duration::hours(1),
                (),
            )],
        )
        .unwrap_err();

        assert_eq!(err, IntervalSourceError::NegativeStart);
    }

    #[test]
    fn rejects_non_positive_every() {
        let anchor = dt(2026, 3, 20, 0, 0, 0);
        let err = IntervalSource::new(
            anchor,
            vec![IntervalPattern::new(
                Duration::hours(1),
                Duration::zero(),
                Duration::hours(1),
                (),
            )],
        )
        .unwrap_err();

        assert_eq!(err, IntervalSourceError::NonPositiveEvery);
    }

    #[test]
    fn rejects_non_positive_duration() {
        let anchor = dt(2026, 3, 20, 0, 0, 0);
        let err = IntervalSource::new(
            anchor,
            vec![IntervalPattern::new(
                Duration::hours(1),
                Duration::days(1),
                Duration::zero(),
                (),
            )],
        )
        .unwrap_err();

        assert_eq!(err, IntervalSourceError::NonPositiveDuration);
    }

    #[test]
    fn next_window_chooses_earliest_across_patterns() {
        let anchor = dt(2026, 3, 20, 0, 0, 0);
        let src = IntervalSource::new(
            anchor,
            vec![
                IntervalPattern::new(
                    Duration::hours(9),
                    Duration::days(1),
                    Duration::hours(3),
                    "morning",
                ),
                IntervalPattern::new(
                    Duration::hours(13),
                    Duration::days(1),
                    Duration::hours(4),
                    "afternoon",
                ),
            ],
        )
        .unwrap();

        let after = dt(2026, 3, 20, 10, 0, 0);
        let next = src.next_window(after).unwrap();

        assert_eq!(next.start, dt(2026, 3, 20, 13, 0, 0));
        assert_eq!(next.end, dt(2026, 3, 20, 17, 0, 0));
        assert_eq!(next.meta, "afternoon");
    }

    #[test]
    fn next_window_moves_to_later_occurrence() {
        let anchor = dt(2026, 3, 20, 0, 0, 0);
        let src = IntervalSource::new(
            anchor,
            vec![IntervalPattern::new(
                Duration::hours(9),
                Duration::days(1),
                Duration::hours(3),
                "morning",
            )],
        )
        .unwrap();

        let after = dt(2026, 3, 20, 23, 0, 0);
        let next = src.next_window(after).unwrap();

        assert_eq!(next.start, dt(2026, 3, 21, 9, 0, 0));
        assert_eq!(next.meta, "morning");
    }

    #[test]
    fn active_windows_finds_multiple_entries() {
        let anchor = dt(2026, 3, 20, 0, 0, 0);
        let src = IntervalSource::new(
            anchor,
            vec![
                IntervalPattern::new(
                    Duration::hours(9),
                    Duration::days(1),
                    Duration::hours(4),
                    "a",
                ),
                IntervalPattern::new(
                    Duration::hours(11),
                    Duration::days(1),
                    Duration::hours(4),
                    "b",
                ),
            ],
        )
        .unwrap();

        let now = dt(2026, 3, 20, 11, 30, 0);
        let active = src.active_windows(now);

        assert_eq!(active.len(), 2);
        assert_eq!(active[0].meta, "a");
        assert_eq!(active[1].meta, "b");
    }

    #[test]
    fn active_windows_finds_overlap_from_same_pattern() {
        let anchor = dt(2026, 3, 20, 0, 0, 0);
        let src = IntervalSource::new(
            anchor,
            vec![IntervalPattern::new(
                Duration::hours(23),
                Duration::days(1),
                Duration::hours(3),
                "late",
            )],
        )
        .unwrap();

        let now = dt(2026, 3, 21, 1, 0, 0);
        let active = src.active_windows(now);

        assert_eq!(active.len(), 1);
        assert_eq!(active[0].start, dt(2026, 3, 20, 23, 0, 0));
        assert_eq!(active[0].end, dt(2026, 3, 21, 2, 0, 0));
        assert_eq!(active[0].meta, "late");
    }

    #[test]
    fn active_windows_finds_multiple_overlapping_occurrences_of_same_pattern() {
        let anchor = dt(2026, 3, 20, 0, 0, 0);
        let src = IntervalSource::new(
            anchor,
            vec![IntervalPattern::new(
                Duration::minutes(0),
                Duration::hours(1),
                Duration::hours(3),
                "x",
            )],
        )
        .unwrap();

        let now = dt(2026, 3, 20, 2, 30, 0);
        let active = src.active_windows(now);

        assert_eq!(active.len(), 3);
        assert_eq!(active[0].start, dt(2026, 3, 20, 0, 0, 0));
        assert_eq!(active[1].start, dt(2026, 3, 20, 1, 0, 0));
        assert_eq!(active[2].start, dt(2026, 3, 20, 2, 0, 0));
    }

    #[test]
    fn single_constructor_works() {
        let anchor = dt(2026, 3, 20, 0, 0, 0);
        let src = IntervalSource::single(
            anchor,
            Duration::minutes(10),
            Duration::hours(1),
            Duration::minutes(20),
            "x",
        )
        .unwrap();

        let next = src.next_window(dt(2026, 3, 20, 0, 15, 0)).unwrap();
        assert_eq!(next.start, dt(2026, 3, 20, 1, 10, 0));
        assert_eq!(next.end, dt(2026, 3, 20, 1, 30, 0));
        assert_eq!(next.meta, "x");
    }

    #[test]
    fn supports_different_cadences_in_same_source() {
        let anchor = dt(2026, 3, 20, 0, 0, 0);
        let src = IntervalSource::new(
            anchor,
            vec![
                IntervalPattern::new(
                    Duration::minutes(15),
                    Duration::hours(2),
                    Duration::minutes(20),
                    "fast",
                ),
                IntervalPattern::new(
                    Duration::hours(9),
                    Duration::days(1),
                    Duration::hours(1),
                    "daily",
                ),
            ],
        )
        .unwrap();

        let next = src.next_window(dt(2026, 3, 20, 8, 0, 0)).unwrap();
        assert_eq!(next.start, dt(2026, 3, 20, 8, 15, 0));
        assert_eq!(next.meta, "fast");
    }
}
