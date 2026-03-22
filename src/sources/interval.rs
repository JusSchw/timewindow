use std::fmt;

use chrono::{DateTime, Duration, Utc};

use crate::{Window, WindowSource};

/// A single recurring interval definition relative to a shared source anchor.
///
/// Each pattern entry defines its own recurrence schedule:
///
/// - `at` is the offset from the source anchor to the recurrence point of the
///   first occurrence
/// - `offset` shifts the actual window start relative to that recurrence point
/// - `every` is the recurrence period for subsequent occurrences
/// - `duration` is the window length
/// - `meta` is cloned into each generated occurrence
///
/// For occurrence index `k >= 0`, the recurrence point is:
///
/// - `anchor + at + k * every`
///
/// and the concrete window is:
///
/// - `start = anchor + at + k * every + offset`
/// - `end = start + duration`
///
/// # Example
///
/// If a source has:
///
/// - anchor = `2026-03-20T00:00:00Z`
///
/// and a pattern entry with:
///
/// - at = 0 minutes
/// - offset = -1 minute
/// - every = 5 minutes
/// - duration = 7 minutes
///
/// then it produces windows like:
///
/// - `[2026-03-19 23:59, 2026-03-20 00:06)`
/// - `[2026-03-20 00:04, 2026-03-20 00:11)`
/// - `[2026-03-20 00:09, 2026-03-20 00:16)`
///
/// and so on.
///
/// Another entry in the same source could have a completely different cadence,
/// for example:
///
/// - at = 9 hours
/// - offset = 0 hours
/// - every = 1 day
/// - duration = 3 hours
///
/// which would produce:
///
/// - `[2026-03-20 09:00, 2026-03-20 12:00)`
/// - `[2026-03-21 09:00, 2026-03-21 12:00)`
/// - `[2026-03-22 09:00, 2026-03-22 12:00)`
///
/// and so on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntervalPattern<M> {
    pub at: Duration,
    pub offset: Duration,
    pub every: Duration,
    pub duration: Duration,
    pub meta: M,
}

impl<M> IntervalPattern<M> {
    /// Creates a new recurring pattern entry.
    pub fn new(
        at: Duration,
        offset: Duration,
        every: Duration,
        duration: Duration,
        meta: M,
    ) -> Self {
        Self {
            at,
            offset,
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
/// define the recurrence point:
///
/// - `point = anchor + pattern.at + k * pattern.every`
///
/// and the concrete window:
///
/// - `start = point + pattern.offset`
/// - `end = start + pattern.duration`
///
/// # Overlap
///
/// Pattern entries may overlap each other. A single pattern entry may also
/// overlap with its own subsequent occurrences if `duration > every`, or if
/// `offset` causes windows to straddle neighboring recurrence points.
///
/// # Canonical pattern rules
///
/// This source requires:
///
/// - `pattern` is not empty
/// - for each pattern entry:
///   - `at >= 0`
///   - `every > 0`
///   - `duration > 0`
///
/// `offset` may be negative, zero, or positive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntervalSource<M> {
    anchor: DateTime<Utc>,
    pattern: Vec<IntervalPattern<M>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntervalSourceError {
    EmptyPattern,
    NegativeAt,
    AtOutOfRange,
    OffsetOutOfRange,
    NonPositiveEvery,
    EveryOutOfRange,
    NonPositiveDuration,
    DurationOutOfRange,
}

impl fmt::Display for IntervalSourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IntervalSourceError::EmptyPattern => write!(f, "pattern must not be empty"),
            IntervalSourceError::NegativeAt => {
                write!(f, "pattern recurrence point must be non-negative")
            }
            IntervalSourceError::AtOutOfRange => {
                write!(
                    f,
                    "pattern recurrence point is out of range for nanosecond precision"
                )
            }
            IntervalSourceError::OffsetOutOfRange => {
                write!(f, "pattern offset is out of range for nanosecond precision")
            }
            IntervalSourceError::NonPositiveEvery => {
                write!(f, "pattern recurrence must be positive")
            }
            IntervalSourceError::EveryOutOfRange => {
                write!(
                    f,
                    "pattern recurrence is out of range for nanosecond precision"
                )
            }
            IntervalSourceError::NonPositiveDuration => {
                write!(f, "pattern duration must be positive")
            }
            IntervalSourceError::DurationOutOfRange => {
                write!(
                    f,
                    "pattern duration is out of range for nanosecond precision"
                )
            }
        }
    }
}

impl std::error::Error for IntervalSourceError {}

impl<M> IntervalSource<M> {
    pub fn new(
        anchor: DateTime<Utc>,
        mut pattern: Vec<IntervalPattern<M>>,
    ) -> Result<Self, IntervalSourceError> {
        if pattern.is_empty() {
            return Err(IntervalSourceError::EmptyPattern);
        }

        for entry in &pattern {
            if entry.at < Duration::zero() {
                return Err(IntervalSourceError::NegativeAt);
            }

            entry
                .at
                .num_nanoseconds()
                .ok_or(IntervalSourceError::AtOutOfRange)?;

            entry
                .offset
                .num_nanoseconds()
                .ok_or(IntervalSourceError::OffsetOutOfRange)?;

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
            a.at.cmp(&b.at)
                .then_with(|| a.offset.cmp(&b.offset))
                .then_with(|| a.every.cmp(&b.every))
                .then_with(|| a.duration.cmp(&b.duration))
        });

        Ok(Self { anchor, pattern })
    }

    /// Convenience constructor for a source with a single recurring interval.
    pub fn single(
        anchor: DateTime<Utc>,
        at: Duration,
        offset: Duration,
        every: Duration,
        duration: Duration,
        meta: M,
    ) -> Result<Self, IntervalSourceError> {
        Self::new(
            anchor,
            vec![IntervalPattern {
                at,
                offset,
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

    fn pattern_at_ns(entry: &IntervalPattern<M>) -> i64 {
        entry
            .at
            .num_nanoseconds()
            .expect("validated in constructor: at fits in nanoseconds")
    }

    fn pattern_every_ns(entry: &IntervalPattern<M>) -> i64 {
        entry
            .every
            .num_nanoseconds()
            .expect("validated in constructor: every fits in nanoseconds")
    }

    fn recurrence_point_at(
        &self,
        entry: &IntervalPattern<M>,
        occurrence_index: i64,
    ) -> Option<DateTime<Utc>> {
        if occurrence_index < 0 {
            return None;
        }

        let at_ns = Self::pattern_at_ns(entry);
        let every_ns = Self::pattern_every_ns(entry);

        let repeated_ns = every_ns.checked_mul(occurrence_index)?;
        let total_ns = at_ns.checked_add(repeated_ns)?;

        self.anchor
            .checked_add_signed(Duration::nanoseconds(total_ns))
    }

    fn window_at(&self, pattern_index: usize, occurrence_index: i64) -> Option<Window<M>>
    where
        M: Clone,
    {
        let entry = self.pattern.get(pattern_index)?;
        let point = self.recurrence_point_at(entry, occurrence_index)?;
        let start = point.checked_add_signed(entry.offset)?;
        let end = start.checked_add_signed(entry.duration)?;

        Window::new(start, end, entry.meta.clone())
    }

    fn occurrence_index_floor_for_recurrence_point(
        &self,
        entry: &IntervalPattern<M>,
        time: DateTime<Utc>,
    ) -> i64 {
        let first_point = match self.anchor.checked_add_signed(entry.at) {
            Some(dt) => dt,
            None => {
                return -1;
            }
        };

        let delta = time - first_point;
        let delta_ns = delta.num_nanoseconds().unwrap_or_else(|| {
            if time < first_point {
                i64::MIN
            } else {
                i64::MAX
            }
        });

        delta_ns.div_euclid(Self::pattern_every_ns(entry))
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
            let shifted_now = match now.checked_sub_signed(entry.offset) {
                Some(t) => t,
                None => continue,
            };

            let range_start = match shifted_now.checked_sub_signed(entry.duration) {
                Some(t) => t,
                None => continue,
            };

            let last_occurrence =
                self.occurrence_index_floor_for_recurrence_point(entry, shifted_now);

            let first_occurrence = self
                .occurrence_index_floor_for_recurrence_point(entry, range_start)
                .saturating_add(1);

            let start_idx = first_occurrence.max(0);
            let end_idx = last_occurrence.max(-1);

            if start_idx > end_idx {
                continue;
            }

            for occurrence_index in start_idx..=end_idx {
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

        for entry in &self.pattern {
            let shifted_after = match after.checked_sub_signed(entry.offset) {
                Some(t) => t,
                None => continue,
            };

            let floor = self.occurrence_index_floor_for_recurrence_point(entry, shifted_after);
            let candidate_index = floor.saturating_add(1).max(0);

            let point = match self.recurrence_point_at(entry, candidate_index) {
                Some(point) => point,
                None => continue,
            };

            let start = match point.checked_add_signed(entry.offset) {
                Some(start) => start,
                None => continue,
            };

            let end = match start.checked_add_signed(entry.duration) {
                Some(end) => end,
                None => continue,
            };

            let window = match Window::new(start, end, entry.meta.clone()) {
                Some(window) => window,
                None => continue,
            };

            if window.start <= after {
                continue;
            }

            match &best {
                Some(current)
                    if current.start < window.start
                        || (current.start == window.start && current.end <= window.end) => {}
                _ => best = Some(window),
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
    fn rejects_negative_at() {
        let anchor = dt(2026, 3, 20, 0, 0, 0);
        let err = IntervalSource::new(
            anchor,
            vec![IntervalPattern::new(
                Duration::hours(-1),
                Duration::zero(),
                Duration::days(1),
                Duration::hours(1),
                (),
            )],
        )
        .unwrap_err();

        assert_eq!(err, IntervalSourceError::NegativeAt);
    }

    #[test]
    fn rejects_non_positive_every() {
        let anchor = dt(2026, 3, 20, 0, 0, 0);
        let err = IntervalSource::new(
            anchor,
            vec![IntervalPattern::new(
                Duration::hours(1),
                Duration::zero(),
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
                Duration::zero(),
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
                    Duration::zero(),
                    Duration::days(1),
                    Duration::hours(3),
                    "morning",
                ),
                IntervalPattern::new(
                    Duration::hours(13),
                    Duration::zero(),
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
                Duration::zero(),
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
                    Duration::zero(),
                    Duration::days(1),
                    Duration::hours(4),
                    "a",
                ),
                IntervalPattern::new(
                    Duration::hours(11),
                    Duration::zero(),
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
                Duration::zero(),
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
                Duration::zero(),
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
            Duration::zero(),
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
                    Duration::zero(),
                    Duration::hours(2),
                    Duration::minutes(20),
                    "fast",
                ),
                IntervalPattern::new(
                    Duration::hours(9),
                    Duration::zero(),
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

    #[test]
    fn negative_offset_can_start_window_before_anchor() {
        let anchor = dt(2026, 3, 20, 0, 0, 0);
        let src = IntervalSource::single(
            anchor,
            Duration::zero(),
            Duration::minutes(-1),
            Duration::minutes(5),
            Duration::minutes(7),
            "x",
        )
        .unwrap();

        let next = src.next_window(dt(2026, 3, 19, 23, 58, 0)).unwrap();
        assert_eq!(next.start, dt(2026, 3, 19, 23, 59, 0));
        assert_eq!(next.end, dt(2026, 3, 20, 0, 6, 0));
        assert_eq!(next.meta, "x");
    }

    #[test]
    fn negative_offset_overlaps_front_and_back() {
        let anchor = dt(2026, 3, 20, 0, 0, 0);
        let src = IntervalSource::single(
            anchor,
            Duration::zero(),
            Duration::minutes(-1),
            Duration::minutes(5),
            Duration::minutes(7),
            "x",
        )
        .unwrap();

        let active = src.active_windows(dt(2026, 3, 20, 0, 5, 30));
        assert_eq!(active.len(), 2);

        assert_eq!(active[0].start, dt(2026, 3, 19, 23, 59, 0));
        assert_eq!(active[0].end, dt(2026, 3, 20, 0, 6, 0));

        assert_eq!(active[1].start, dt(2026, 3, 20, 0, 4, 0));
        assert_eq!(active[1].end, dt(2026, 3, 20, 0, 11, 0));
    }

    #[test]
    fn next_window_respects_negative_offset_phase() {
        let anchor = dt(2026, 3, 20, 0, 0, 0);
        let src = IntervalSource::single(
            anchor,
            Duration::zero(),
            Duration::minutes(-1),
            Duration::minutes(5),
            Duration::minutes(7),
            "x",
        )
        .unwrap();

        let next = src.next_window(dt(2026, 3, 20, 0, 6, 0)).unwrap();
        assert_eq!(next.start, dt(2026, 3, 20, 0, 9, 0));
        assert_eq!(next.end, dt(2026, 3, 20, 0, 16, 0));
    }
}
