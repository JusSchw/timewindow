use chrono::{
    DateTime, Datelike, Duration, LocalResult, NaiveDate, NaiveDateTime, NaiveTime, TimeZone,
    Timelike, Utc, Weekday,
};
use chrono_tz::Tz;

use crate::{Window, WindowSource};

/// A single timezone-aware recurring schedule entry.
///
/// A `Schedule` describes one recurring rule in a specific timezone:
///
/// - `timezone` determines how local wall-clock times are interpreted
/// - `rule` determines which local dates or times recur
/// - `at` is the local wall-clock time for each occurrence
/// - `duration` is the window length
/// - `meta` is cloned into each produced [`Window`]
///
/// # Timezone semantics
///
/// Schedule rules are evaluated in the schedule's local timezone, then
/// converted to UTC when windows are produced.
///
/// This means a rule such as "daily at 12:00 in New York" remains anchored to
/// local noon even as the UTC offset changes across daylight saving
/// transitions.
///
/// # DST handling
///
/// When localizing a wall-clock timestamp:
///
/// - if the local time is unambiguous, that instant is used
/// - if the local time is ambiguous, the earlier instant is chosen
/// - if the local time does not exist, that occurrence is skipped
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Schedule<M> {
    /// Timezone in which the schedule is interpreted.
    pub timezone: Tz,
    /// Recurrence rule for this schedule.
    pub rule: ScheduleRule,
    /// Local wall-clock time at which each occurrence starts.
    pub at: NaiveTime,
    /// Duration of each produced window.
    pub duration: Duration,
    /// Metadata cloned into each produced window.
    pub meta: M,
}

impl<M> Schedule<M> {
    /// Creates a new schedule entry.
    pub fn new(
        timezone: Tz,
        rule: ScheduleRule,
        at: NaiveTime,
        duration: Duration,
        meta: M,
    ) -> Self {
        Self {
            timezone,
            rule,
            at,
            duration,
            meta,
        }
    }
}

/// A recurrence rule evaluated in a schedule's local timezone.
///
/// # Rule families
///
/// The rule variants fall into two broad groups:
///
/// - clock-based rules:
///   - [`ScheduleRule::Minutely`]
///   - [`ScheduleRule::Hourly`]
/// - calendar-based rules:
///   - [`ScheduleRule::Daily`]
///   - [`ScheduleRule::Weekly`]
///   - [`ScheduleRule::Monthly`]
///   - [`ScheduleRule::Yearly`]
///
/// # Anchoring semantics
///
/// Recurrence rules must have stable phase semantics so that querying at
/// different times does not redefine the schedule.
///
/// This implementation uses the following anchors:
///
/// - `Minutely { every }`:
///   occurrences happen at local times whose minute satisfies
///   `minute % every == 0`, with seconds and subseconds taken from `at`.
/// - `Hourly { every }`:
///   occurrences happen at local times whose hour satisfies
///   `hour % every == 0`, with minute/second/subsecond taken from `at`.
/// - `Daily { every }`:
///   anchored to the fixed local date `1970-01-01`.
/// - `Weekly { every, ... }`:
///   anchored to the Monday of the week containing `1970-01-01`.
/// - `Monthly { every, day }`:
///   anchored to January 1970.
/// - `Yearly { every, month, day }`:
///   anchored to the calendar year 1970.
///
/// These anchors provide stable recurrence phase across calls to
/// [`WindowSource::next_window`] and [`WindowSource::active_windows`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScheduleRule {
    /// Recur every `every` minutes within each hour.
    ///
    /// For example, `Minutely { every: 15 }` with `at = 00:00:30` produces
    /// local times such as:
    ///
    /// - `00:00:30`
    /// - `00:15:30`
    /// - `00:30:30`
    /// - `00:45:30`
    /// - `01:00:30`
    ///
    /// and so on.
    Minutely { every: u32 },

    /// Recur every `every` hours within each day.
    ///
    /// For example, `Hourly { every: 6 }` with `at = 00:10:00` produces local
    /// times such as:
    ///
    /// - `00:10:00`
    /// - `06:10:00`
    /// - `12:10:00`
    /// - `18:10:00`
    /// - next day `00:10:00`
    ///
    /// and so on.
    Hourly { every: u32 },

    /// Recur every `every` days, anchored to `1970-01-01`.
    Daily { every: u32 },

    /// Recur every `every` weeks on the specified weekdays, anchored to the
    /// Monday of the week containing `1970-01-01`.
    Weekly { every: u32, weekdays: Vec<Weekday> },

    /// Recur every `every` months on a specific day-of-month.
    ///
    /// Months that do not contain `day` are skipped.
    Monthly { every: u32, day: u32 },

    /// Recur every `every` years on a specific month/day.
    ///
    /// Impossible dates such as February 30 are rejected during construction.
    Yearly { every: u32, month: u32, day: u32 },
}

/// A [`WindowSource`] composed of one or more independent schedules.
///
/// Each contained [`Schedule`] is evaluated independently. Their produced
/// windows may overlap freely.
///
/// # Overlap
///
/// Multiple schedules may be active simultaneously. A single schedule may also
/// overlap with its own subsequent occurrences if its `duration` exceeds its
/// recurrence spacing.
///
/// # Ordering
///
/// Returned windows are sorted by `(start, end)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduleSource<M> {
    schedules: Vec<Schedule<M>>,
}

/// Errors returned when constructing a [`ScheduleSource`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScheduleSourceError {
    /// The source was constructed with no schedules.
    EmptySchedules,
    /// A schedule duration was zero or negative.
    NonPositiveDuration,
    /// A schedule duration could not be represented in nanoseconds.
    DurationOutOfRange,
    /// A rule had an invalid recurrence parameter such as `every == 0`.
    InvalidRule,
    /// A weekly rule had no weekdays.
    EmptyWeekdays,
    /// A monthly rule used an invalid day-of-month.
    InvalidDayOfMonth,
    /// A yearly rule used an invalid month.
    InvalidMonth,
    /// A yearly rule used an invalid day or impossible month/day combination.
    InvalidDayOfYear,
}

impl<M> ScheduleSource<M> {
    /// Creates a new source from a non-empty list of schedules.
    ///
    /// # Validation
    ///
    /// This constructor validates:
    ///
    /// - `schedules` is not empty
    /// - each duration is positive
    /// - each duration fits in nanoseconds
    /// - each `every` parameter is non-zero
    /// - weekly schedules contain at least one weekday
    /// - monthly day values are in `1..=31`
    /// - yearly month/day values form a possible calendar date
    pub fn new(mut schedules: Vec<Schedule<M>>) -> Result<Self, ScheduleSourceError> {
        if schedules.is_empty() {
            return Err(ScheduleSourceError::EmptySchedules);
        }

        for schedule in &schedules {
            if schedule.duration <= Duration::zero() {
                return Err(ScheduleSourceError::NonPositiveDuration);
            }

            schedule
                .duration
                .num_nanoseconds()
                .ok_or(ScheduleSourceError::DurationOutOfRange)?;

            match &schedule.rule {
                ScheduleRule::Minutely { every }
                | ScheduleRule::Hourly { every }
                | ScheduleRule::Daily { every }
                | ScheduleRule::Monthly { every, .. }
                | ScheduleRule::Yearly { every, .. } => {
                    if *every == 0 {
                        return Err(ScheduleSourceError::InvalidRule);
                    }
                }
                ScheduleRule::Weekly { every, weekdays } => {
                    if *every == 0 {
                        return Err(ScheduleSourceError::InvalidRule);
                    }
                    if weekdays.is_empty() {
                        return Err(ScheduleSourceError::EmptyWeekdays);
                    }
                }
            }

            match schedule.rule {
                ScheduleRule::Monthly { day, .. } => {
                    if !(1..=31).contains(&day) {
                        return Err(ScheduleSourceError::InvalidDayOfMonth);
                    }
                }
                ScheduleRule::Yearly { month, day, .. } => {
                    if !(1..=12).contains(&month) {
                        return Err(ScheduleSourceError::InvalidMonth);
                    }
                    if !(1..=31).contains(&day) {
                        return Err(ScheduleSourceError::InvalidDayOfYear);
                    }

                    if NaiveDate::from_ymd_opt(2000, month, day).is_none() {
                        return Err(ScheduleSourceError::InvalidDayOfYear);
                    }
                }
                _ => {}
            }
        }

        schedules.sort_by(|a, b| {
            a.timezone
                .name()
                .cmp(b.timezone.name())
                .then_with(|| rule_sort_key(&a.rule).cmp(&rule_sort_key(&b.rule)))
                .then_with(|| a.at.cmp(&b.at))
                .then_with(|| a.duration.cmp(&b.duration))
        });

        Ok(Self { schedules })
    }

    /// Convenience constructor for a source containing a single schedule.
    pub fn single(schedule: Schedule<M>) -> Result<Self, ScheduleSourceError> {
        Self::new(vec![schedule])
    }

    /// Returns the schedules contained in this source.
    #[inline]
    pub fn schedules(&self) -> &[Schedule<M>] {
        &self.schedules
    }

    /// Converts a local naive datetime in `tz` into UTC.
    ///
    /// # DST behavior
    ///
    /// - unambiguous local times are converted directly
    /// - ambiguous local times choose the earlier instant
    /// - nonexistent local times return `None`
    fn localize(tz: Tz, naive: NaiveDateTime) -> Option<DateTime<Utc>> {
        match tz.from_local_datetime(&naive) {
            LocalResult::Single(dt) => Some(dt.with_timezone(&Utc)),
            LocalResult::Ambiguous(a, _) => Some(a.with_timezone(&Utc)),
            LocalResult::None => None,
        }
    }

    /// Builds a window from a local naive start datetime.
    fn window_for_local_start(schedule: &Schedule<M>, local: NaiveDateTime) -> Option<Window<M>>
    where
        M: Clone,
    {
        let start = Self::localize(schedule.timezone, local)?;
        let end = start.checked_add_signed(schedule.duration)?;
        Window::new(start, end, schedule.meta.clone())
    }

    /// Builds a window for a date-based schedule occurrence on `local_date`.
    fn window_for_occurrence(schedule: &Schedule<M>, local_date: NaiveDate) -> Option<Window<M>>
    where
        M: Clone,
    {
        Self::window_for_local_start(schedule, local_date.and_time(schedule.at))
    }

    /// Fixed anchor date used for daily recurrence phase.
    ///
    /// This gives `Daily { every }` stable behavior across queries.
    fn fixed_daily_anchor() -> NaiveDate {
        NaiveDate::from_ymd_opt(1970, 1, 1).expect("valid fixed anchor date")
    }

    /// Fixed anchor week used for weekly recurrence phase.
    ///
    /// This is the Monday of the week containing `1970-01-01`.
    fn fixed_weekly_anchor_monday() -> NaiveDate {
        let epoch = Self::fixed_daily_anchor();
        Self::weekly_anchor_monday(epoch)
    }

    /// Returns whether `date` matches a daily cadence anchored at `anchor`.
    fn matches_daily_anchor(date: NaiveDate, every: u32, anchor: NaiveDate) -> bool {
        let delta_days = (date - anchor).num_days();
        delta_days >= 0 && delta_days % i64::from(every) == 0
    }

    /// Returns the Monday of the week containing `date`.
    fn weekly_anchor_monday(date: NaiveDate) -> NaiveDate {
        let days_from_monday = i64::from(date.weekday().num_days_from_monday());
        date - Duration::days(days_from_monday)
    }

    /// Returns whether `date` matches a weekly cadence anchored at
    /// `anchor_monday`, and also falls on one of the permitted `weekdays`.
    fn matches_weekly_anchor(
        date: NaiveDate,
        every: u32,
        anchor_monday: NaiveDate,
        weekdays: &[Weekday],
    ) -> bool {
        if !weekdays.contains(&date.weekday()) {
            return false;
        }

        let this_monday = Self::weekly_anchor_monday(date);
        let delta_weeks = (this_monday - anchor_monday).num_days() / 7;
        delta_weeks >= 0 && delta_weeks % i64::from(every) == 0
    }

    /// Returns a linear month index for `(year, month)`.
    ///
    /// This is used to compute stable monthly recurrence phase.
    fn month_index(year: i32, month: u32) -> i64 {
        i64::from(year) * 12 + i64::from(month) - 1
    }

    /// Returns whether `date` matches a monthly cadence on `day`.
    ///
    /// Monthly cadence is anchored to January 1970. Months that do not contain
    /// `day` simply do not produce an occurrence.
    fn matches_monthly_anchor(date: NaiveDate, every: u32, day: u32) -> bool {
        if date.day() != day {
            return false;
        }

        let anchor = 1970_i64 * 12;
        let current = Self::month_index(date.year(), date.month());
        let delta = current - anchor;
        delta >= 0 && delta % i64::from(every) == 0
    }

    /// Returns whether `date` matches a yearly cadence on `month/day`.
    ///
    /// Yearly cadence is anchored to calendar year 1970.
    fn matches_yearly_anchor(date: NaiveDate, every: u32, month: u32, day: u32) -> bool {
        if date.month() != month || date.day() != day {
            return false;
        }

        let delta_years = i64::from(date.year() - 1970);
        delta_years >= 0 && delta_years % i64::from(every) == 0
    }

    /// Returns whether a date-based rule matches on `date`.
    ///
    /// This helper applies only to:
    ///
    /// - daily
    /// - weekly
    /// - monthly
    /// - yearly
    ///
    /// For minutely and hourly rules, this returns `false`.
    fn rule_matches_on_date(schedule: &Schedule<M>, date: NaiveDate) -> bool {
        match &schedule.rule {
            ScheduleRule::Daily { every } => {
                Self::matches_daily_anchor(date, *every, Self::fixed_daily_anchor())
            }
            ScheduleRule::Weekly { every, weekdays } => Self::matches_weekly_anchor(
                date,
                *every,
                Self::fixed_weekly_anchor_monday(),
                weekdays,
            ),
            ScheduleRule::Monthly { every, day } => {
                Self::matches_monthly_anchor(date, *every, *day)
            }
            ScheduleRule::Yearly { every, month, day } => {
                Self::matches_yearly_anchor(date, *every, *month, *day)
            }
            ScheduleRule::Minutely { .. } | ScheduleRule::Hourly { .. } => false,
        }
    }

    /// Returns the next window for a minutely schedule strictly after `after`.
    ///
    /// The recurrence is anchored to minute-of-hour modulo `every`.
    ///
    /// For example, with `every = 15`, valid local minutes are:
    ///
    /// - `00`
    /// - `15`
    /// - `30`
    /// - `45`
    ///
    /// The seconds and subseconds are taken from `schedule.at`.
    fn minutely_next_after(schedule: &Schedule<M>, after: DateTime<Utc>) -> Option<Window<M>>
    where
        M: Clone,
    {
        let every = i64::from(match schedule.rule {
            ScheduleRule::Minutely { every } => every,
            _ => return None,
        });

        let local_after = after.with_timezone(&schedule.timezone).naive_local();

        let second = schedule.at.second();
        let nanos = schedule.at.nanosecond();

        let current_date = local_after.date();
        let current_hour = i64::from(local_after.hour());
        let current_minute = i64::from(local_after.minute());

        let base = current_date.and_hms_nano_opt(
            current_hour as u32,
            current_minute as u32,
            second,
            nanos,
        )?;

        let candidate = if base > local_after {
            base
        } else {
            let next_minute_of_hour = ((current_minute / every) + 1) * every;
            let total_minutes = current_hour
                .checked_mul(60)?
                .checked_add(next_minute_of_hour)?;

            let day_carry = total_minutes.div_euclid(24 * 60);
            let minute_of_day = total_minutes.rem_euclid(24 * 60);

            let hour = (minute_of_day / 60) as u32;
            let minute = (minute_of_day % 60) as u32;

            let date = current_date.checked_add_signed(Duration::days(day_carry))?;
            date.and_hms_nano_opt(hour, minute, second, nanos)?
        };

        let start = Self::localize(schedule.timezone, candidate)?;
        if start <= after {
            return None;
        }

        let end = start.checked_add_signed(schedule.duration)?;
        Window::new(start, end, schedule.meta.clone())
    }

    /// Returns the next window for an hourly schedule strictly after `after`.
    ///
    /// The recurrence is anchored to hour-of-day modulo `every`.
    ///
    /// The minute, second, and subsecond fields are taken from `schedule.at`.
    fn hourly_next_after(schedule: &Schedule<M>, after: DateTime<Utc>) -> Option<Window<M>>
    where
        M: Clone,
    {
        let every = i64::from(match schedule.rule {
            ScheduleRule::Hourly { every } => every,
            _ => return None,
        });

        let local_after = after.with_timezone(&schedule.timezone).naive_local();

        let minute = schedule.at.minute();
        let second = schedule.at.second();
        let nanos = schedule.at.nanosecond();

        let current_date = local_after.date();
        let current_hour = i64::from(local_after.hour());

        let base = current_date.and_hms_nano_opt(current_hour as u32, minute, second, nanos)?;

        let candidate = if base > local_after {
            base
        } else {
            let next_hour = ((current_hour / every) + 1) * every;

            let day_carry = next_hour.div_euclid(24);
            let hour_of_day = next_hour.rem_euclid(24) as u32;

            let date = current_date.checked_add_signed(Duration::days(day_carry))?;
            date.and_hms_nano_opt(hour_of_day, minute, second, nanos)?
        };

        let start = Self::localize(schedule.timezone, candidate)?;
        if start <= after {
            return None;
        }

        let end = start.checked_add_signed(schedule.duration)?;
        Window::new(start, end, schedule.meta.clone())
    }

    /// Returns the next window for a date-based rule strictly after `after`.
    ///
    /// This applies to:
    ///
    /// - daily
    /// - weekly
    /// - monthly
    /// - yearly
    ///
    /// The method scans forward over local dates until it finds the first
    /// matching occurrence whose localized UTC start is strictly greater than
    /// `after`.
    fn daily_like_next_after(schedule: &Schedule<M>, after: DateTime<Utc>) -> Option<Window<M>>
    where
        M: Clone,
    {
        let local_after = after.with_timezone(&schedule.timezone).naive_local();
        let start_date = local_after.date();

        for day_offset in 0..=3660 {
            let date = start_date.checked_add_signed(Duration::days(day_offset))?;
            if !Self::rule_matches_on_date(schedule, date) {
                continue;
            }

            let start = match Self::localize(schedule.timezone, date.and_time(schedule.at)) {
                Some(start) => start,
                None => continue,
            };

            if start <= after {
                continue;
            }

            let end = start.checked_add_signed(schedule.duration)?;
            return Window::new(start, end, schedule.meta.clone());
        }

        None
    }

    /// Returns all active windows for a single schedule at `now`.
    ///
    /// Because windows may overlap recurrence boundaries, this method scans
    /// backward over a bounded local-date range derived from the schedule
    /// duration.
    ///
    /// # Lookback strategy
    ///
    /// The scan range is:
    ///
    /// - at least one day
    /// - plus the whole-number day portion of the duration
    /// - plus extra slack to cover sparse calendar recurrences
    ///
    /// This keeps the implementation simple and correct for long-running
    /// windows while remaining bounded.
    fn active_windows_for_schedule(schedule: &Schedule<M>, now: DateTime<Utc>) -> Vec<Window<M>>
    where
        M: Clone,
    {
        let local_now = now.with_timezone(&schedule.timezone).naive_local();

        let lookback_days = schedule.duration.num_days().max(1).saturating_add(370);

        let mut windows = Vec::new();

        match &schedule.rule {
            ScheduleRule::Minutely { every } => {
                let every = i64::from(*every);
                let second = schedule.at.second();
                let nanos = schedule.at.nanosecond();

                for day_offset in -lookback_days..=0 {
                    let date = match local_now
                        .date()
                        .checked_add_signed(Duration::days(day_offset))
                    {
                        Some(date) => date,
                        None => continue,
                    };

                    for hour in 0..24_u32 {
                        for minute in 0..60_u32 {
                            if i64::from(minute) % every != 0 {
                                continue;
                            }

                            let local = match date.and_hms_nano_opt(hour, minute, second, nanos) {
                                Some(local) => local,
                                None => continue,
                            };

                            let start = match Self::localize(schedule.timezone, local) {
                                Some(start) => start,
                                None => continue,
                            };
                            let end = match start.checked_add_signed(schedule.duration) {
                                Some(end) => end,
                                None => continue,
                            };

                            if let Some(window) = Window::new(start, end, schedule.meta.clone()) {
                                if window.is_active(now) {
                                    windows.push(window);
                                }
                            }
                        }
                    }
                }
            }
            ScheduleRule::Hourly { every } => {
                let every = i64::from(*every);
                let minute = schedule.at.minute();
                let second = schedule.at.second();
                let nanos = schedule.at.nanosecond();

                for day_offset in -lookback_days..=0 {
                    let date = match local_now
                        .date()
                        .checked_add_signed(Duration::days(day_offset))
                    {
                        Some(date) => date,
                        None => continue,
                    };

                    for hour in 0..24_u32 {
                        if i64::from(hour) % every != 0 {
                            continue;
                        }

                        let local = match date.and_hms_nano_opt(hour, minute, second, nanos) {
                            Some(local) => local,
                            None => continue,
                        };

                        let start = match Self::localize(schedule.timezone, local) {
                            Some(start) => start,
                            None => continue,
                        };
                        let end = match start.checked_add_signed(schedule.duration) {
                            Some(end) => end,
                            None => continue,
                        };

                        if let Some(window) = Window::new(start, end, schedule.meta.clone()) {
                            if window.is_active(now) {
                                windows.push(window);
                            }
                        }
                    }
                }
            }
            ScheduleRule::Daily { .. }
            | ScheduleRule::Weekly { .. }
            | ScheduleRule::Monthly { .. }
            | ScheduleRule::Yearly { .. } => {
                for day_offset in -lookback_days..=0 {
                    let date = match local_now
                        .date()
                        .checked_add_signed(Duration::days(day_offset))
                    {
                        Some(date) => date,
                        None => continue,
                    };

                    if !Self::rule_matches_on_date(schedule, date) {
                        continue;
                    }

                    if let Some(window) = Self::window_for_occurrence(schedule, date) {
                        if window.is_active(now) {
                            windows.push(window);
                        }
                    }
                }
            }
        }

        windows
    }
}

/// Returns a stable sort key for schedule rules.
///
/// This is used only for constructor-time normalization of schedule ordering.
fn rule_sort_key(rule: &ScheduleRule) -> (u8, u32, u32, u32) {
    match rule {
        ScheduleRule::Minutely { every } => (0, *every, 0, 0),
        ScheduleRule::Hourly { every } => (1, *every, 0, 0),
        ScheduleRule::Daily { every } => (2, *every, 0, 0),
        ScheduleRule::Weekly { every, weekdays } => (3, *every, weekdays.len() as u32, 0),
        ScheduleRule::Monthly { every, day } => (4, *every, *day, 0),
        ScheduleRule::Yearly { every, month, day } => (5, *every, *month, *day),
    }
}

impl<M> WindowSource for ScheduleSource<M>
where
    M: Clone,
{
    type Meta = M;

    fn active_windows(&self, now: DateTime<Utc>) -> Vec<Window<Self::Meta>> {
        let mut windows = Vec::new();

        for schedule in &self.schedules {
            windows.extend(Self::active_windows_for_schedule(schedule, now));
        }

        windows.sort_by(|a, b| a.start.cmp(&b.start).then_with(|| a.end.cmp(&b.end)));
        windows
    }

    fn next_window(&self, after: DateTime<Utc>) -> Option<Window<Self::Meta>> {
        let mut best: Option<Window<Self::Meta>> = None;

        for schedule in &self.schedules {
            let candidate = match schedule.rule {
                ScheduleRule::Minutely { .. } => Self::minutely_next_after(schedule, after),
                ScheduleRule::Hourly { .. } => Self::hourly_next_after(schedule, after),
                ScheduleRule::Daily { .. }
                | ScheduleRule::Weekly { .. }
                | ScheduleRule::Monthly { .. }
                | ScheduleRule::Yearly { .. } => Self::daily_like_next_after(schedule, after),
            };

            if let Some(window) = candidate {
                match &best {
                    Some(current)
                        if current.start < window.start
                            || (current.start == window.start && current.end <= window.end) => {}
                    _ => best = Some(window),
                }
            }
        }

        best
    }
}

#[cfg(test)]
mod tests {
    use chrono::{Datelike, Duration, NaiveTime, TimeZone, Utc, Weekday};
    use chrono_tz::America::New_York;
    use chrono_tz::Europe::Berlin;
    use chrono_tz::UTC;

    use crate::WindowSource;

    use super::{Schedule, ScheduleRule, ScheduleSource, ScheduleSourceError};

    fn dt(y: i32, m: u32, d: u32, hh: u32, mm: u32, ss: u32) -> chrono::DateTime<Utc> {
        Utc.with_ymd_and_hms(y, m, d, hh, mm, ss).unwrap()
    }

    fn time(h: u32, m: u32, s: u32) -> NaiveTime {
        NaiveTime::from_hms_opt(h, m, s).unwrap()
    }

    #[test]
    fn rejects_empty_schedules() {
        let err = ScheduleSource::<()>::new(vec![]).unwrap_err();
        assert_eq!(err, ScheduleSourceError::EmptySchedules);
    }

    #[test]
    fn rejects_zero_duration() {
        let err = ScheduleSource::single(Schedule::new(
            New_York,
            ScheduleRule::Daily { every: 1 },
            time(12, 0, 0),
            Duration::zero(),
            (),
        ))
        .unwrap_err();

        assert_eq!(err, ScheduleSourceError::NonPositiveDuration);
    }

    #[test]
    fn rejects_empty_weekdays() {
        let err = ScheduleSource::single(Schedule::new(
            New_York,
            ScheduleRule::Weekly {
                every: 1,
                weekdays: vec![],
            },
            time(12, 0, 0),
            Duration::hours(1),
            (),
        ))
        .unwrap_err();

        assert_eq!(err, ScheduleSourceError::EmptyWeekdays);
    }

    #[test]
    fn rejects_impossible_yearly_date() {
        let err = ScheduleSource::single(Schedule::new(
            UTC,
            ScheduleRule::Yearly {
                every: 1,
                month: 2,
                day: 30,
            },
            time(12, 0, 0),
            Duration::hours(1),
            (),
        ))
        .unwrap_err();

        assert_eq!(err, ScheduleSourceError::InvalidDayOfYear);
    }

    #[test]
    fn weekly_next_window_works_in_timezone() {
        let source = ScheduleSource::single(Schedule::new(
            New_York,
            ScheduleRule::Weekly {
                every: 1,
                weekdays: vec![Weekday::Tue, Weekday::Fri],
            },
            time(12, 0, 0),
            Duration::hours(2),
            "lunch",
        ))
        .unwrap();

        let after = dt(2026, 3, 23, 0, 0, 0);
        let next = source.next_window(after).unwrap();

        assert_eq!(next.meta, "lunch");
        assert_eq!(next.start, dt(2026, 3, 24, 16, 0, 0));
        assert_eq!(next.end, dt(2026, 3, 24, 18, 0, 0));
    }

    #[test]
    fn daily_next_window_respects_dst_offset() {
        let source = ScheduleSource::single(Schedule::new(
            New_York,
            ScheduleRule::Daily { every: 1 },
            time(12, 0, 0),
            Duration::hours(1),
            "noon",
        ))
        .unwrap();

        let before_dst = source.next_window(dt(2026, 3, 7, 18, 0, 0)).unwrap();
        let after_dst = source.next_window(dt(2026, 3, 8, 18, 0, 0)).unwrap();

        assert_eq!(before_dst.start, dt(2026, 3, 8, 16, 0, 0));
        assert_eq!(after_dst.start, dt(2026, 3, 9, 16, 0, 0));
    }

    #[test]
    fn active_windows_can_overlap_across_schedules() {
        let source = ScheduleSource::new(vec![
            Schedule::new(
                Berlin,
                ScheduleRule::Daily { every: 1 },
                time(10, 0, 0),
                Duration::hours(3),
                "a",
            ),
            Schedule::new(
                Berlin,
                ScheduleRule::Daily { every: 1 },
                time(11, 0, 0),
                Duration::hours(3),
                "b",
            ),
        ])
        .unwrap();

        let now = Berlin
            .with_ymd_and_hms(2026, 3, 20, 11, 30, 0)
            .unwrap()
            .with_timezone(&Utc);

        let active = source.active_windows(now);
        assert_eq!(active.len(), 2);
        assert_eq!(active[0].meta, "a");
        assert_eq!(active[1].meta, "b");
    }

    #[test]
    fn monthly_skips_missing_days() {
        let source = ScheduleSource::single(Schedule::new(
            UTC,
            ScheduleRule::Monthly { every: 1, day: 31 },
            time(12, 0, 0),
            Duration::hours(1),
            "month-end-ish",
        ))
        .unwrap();

        let next = source.next_window(dt(2026, 4, 1, 0, 0, 0)).unwrap();
        assert_eq!(next.start, dt(2026, 5, 31, 12, 0, 0));
    }

    #[test]
    fn next_window_chooses_earliest_across_multiple_schedules() {
        let source = ScheduleSource::new(vec![
            Schedule::new(
                New_York,
                ScheduleRule::Weekly {
                    every: 1,
                    weekdays: vec![Weekday::Fri],
                },
                time(12, 0, 0),
                Duration::hours(1),
                "fri",
            ),
            Schedule::new(
                Berlin,
                ScheduleRule::Daily { every: 1 },
                time(9, 0, 0),
                Duration::hours(1),
                "daily",
            ),
        ])
        .unwrap();

        let next = source.next_window(dt(2026, 3, 19, 12, 0, 0)).unwrap();
        assert_eq!(next.meta, "daily");
    }

    #[test]
    fn daily_every_two_has_stable_phase() {
        let source = ScheduleSource::single(Schedule::new(
            UTC,
            ScheduleRule::Daily { every: 2 },
            time(12, 0, 0),
            Duration::hours(1),
            "x",
        ))
        .unwrap();

        let next1 = source.next_window(dt(2026, 3, 20, 11, 0, 0)).unwrap();
        let next2 = source.next_window(dt(2026, 3, 21, 11, 0, 0)).unwrap();

        assert_eq!(next1.start, dt(2026, 3, 20, 12, 0, 0));
        assert_eq!(next2.start, dt(2026, 3, 22, 12, 0, 0));
    }

    #[test]
    fn weekly_every_two_has_stable_phase() {
        let source = ScheduleSource::single(Schedule::new(
            UTC,
            ScheduleRule::Weekly {
                every: 2,
                weekdays: vec![Weekday::Mon],
            },
            time(12, 0, 0),
            Duration::hours(1),
            "x",
        ))
        .unwrap();

        let next1 = source.next_window(dt(2026, 3, 16, 0, 0, 0)).unwrap();
        let next2 = source.next_window(dt(2026, 3, 17, 0, 0, 0)).unwrap();

        assert!(next2.start >= next1.start);
        assert_eq!(next1.start.weekday(), Weekday::Mon);
        assert_eq!(next2.start.weekday(), Weekday::Mon);
    }

    #[test]
    fn minutely_rolls_over_midnight_correctly() {
        let source = ScheduleSource::single(Schedule::new(
            UTC,
            ScheduleRule::Minutely { every: 30 },
            time(0, 0, 5),
            Duration::minutes(1),
            "tick",
        ))
        .unwrap();

        let next = source.next_window(dt(2026, 3, 20, 23, 59, 10)).unwrap();
        assert_eq!(next.start, dt(2026, 3, 21, 0, 0, 5));
    }
}
