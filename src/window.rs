use chrono::{DateTime, Duration, Utc};

/// A half-open time interval `[start, end)` with associated metadata.
///
/// A `Window<M>` represents a bounded span of time in UTC together with
/// arbitrary metadata of type `M`.
///
/// # Semantics
///
/// Windows are half-open intervals:
///
/// - `start` is included
/// - `end` is excluded
///
/// A window is active at time `now` when:
///
/// `start <= now && now < end`
///
/// This convention makes adjacent windows compose cleanly without overlapping
/// at boundaries.
///
/// # Type parameter
///
/// - `M`: arbitrary metadata associated with the window
///
/// # Examples
///
/// ```rust
/// use chrono::{TimeZone, Utc};
/// use timewindow::Window;
///
/// let window = Window::new(
///     Utc.with_ymd_and_hms(2026, 3, 20, 12, 0, 0).unwrap(),
///     Utc.with_ymd_and_hms(2026, 3, 20, 14, 0, 0).unwrap(),
///     "metadata",
/// )
/// .unwrap();
///
/// assert!(window.is_active(Utc.with_ymd_and_hms(2026, 3, 20, 12, 0, 0).unwrap()));
/// assert!(window.is_active(Utc.with_ymd_and_hms(2026, 3, 20, 13, 0, 0).unwrap()));
/// assert!(!window.is_active(Utc.with_ymd_and_hms(2026, 3, 20, 14, 0, 0).unwrap()));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Window<M> {
    /// Inclusive start of the window.
    pub start: DateTime<Utc>,
    /// Exclusive end of the window.
    pub end: DateTime<Utc>,
    // Arbitrary metadata associated with the window.
    pub meta: M,
}

impl<M> Window<M> {
    /// Creates a new window if `start < end`.
    ///
    /// Returns `None` if the interval is empty or invalid, including:
    ///
    /// - `start == end`
    /// - `start > end`
    ///
    /// # Examples
    ///
    /// ```rust
    /// use chrono::{TimeZone, Utc};
    /// use timewindow::Window;
    ///
    /// let ok = Window::new(
    ///     Utc.with_ymd_and_hms(2026, 3, 20, 12, 0, 0).unwrap(),
    ///     Utc.with_ymd_and_hms(2026, 3, 20, 13, 0, 0).unwrap(),
    ///     (),
    /// );
    ///
    /// assert!(ok.is_some());
    ///
    /// let invalid = Window::new(
    ///     Utc.with_ymd_and_hms(2026, 3, 20, 13, 0, 0).unwrap(),
    ///     Utc.with_ymd_and_hms(2026, 3, 20, 13, 0, 0).unwrap(),
    ///     (),
    /// );
    ///
    /// assert!(invalid.is_none());
    /// ```
    #[inline]
    pub fn new(start: DateTime<Utc>, end: DateTime<Utc>, meta: M) -> Option<Self> {
        (start < end).then_some(Self { start, end, meta })
    }

    /// Returns `true` if the window has not started yet at `now`.
    ///
    /// Equivalent to `now < self.start`.
    #[inline]
    pub fn is_upcoming(&self, now: DateTime<Utc>) -> bool {
        now < self.start
    }

    /// Returns `true` if the window is active at `now`.
    ///
    /// This uses half-open interval semantics:
    ///
    /// `self.start <= now && now < self.end`
    #[inline]
    pub fn is_active(&self, now: DateTime<Utc>) -> bool {
        self.start <= now && now < self.end
    }

    /// Returns `true` if the window has ended at or before `now`.
    ///
    /// Equivalent to `now >= self.end`.
    #[inline]
    pub fn is_expired(&self, now: DateTime<Utc>) -> bool {
        now >= self.end
    }

    /// Returns the total duration of the window.
    #[inline]
    pub fn duration(&self) -> Duration {
        self.end - self.start
    }

    /// Returns the time elapsed since the start of the window at `now`,
    /// if the window is currently active.
    ///
    /// Returns `None` if the window is not active at `now`.
    pub fn elapsed_at(&self, now: DateTime<Utc>) -> Option<Duration> {
        if self.is_active(now) {
            Some(now - self.start)
        } else {
            None
        }
    }

    /// Returns the time remaining until the end of the window at `now`,
    /// if the window is currently active.
    ///
    /// Returns `None` if the window is not active at `now`.
    pub fn remaining_at(&self, now: DateTime<Utc>) -> Option<Duration> {
        if self.is_active(now) {
            Some(self.end - now)
        } else {
            None
        }
    }
}
