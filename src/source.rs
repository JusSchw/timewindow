use chrono::{DateTime, Utc};

use crate::Window;

/// A producer of time windows.
///
/// A `WindowSource` abstracts over any system that can answer two questions:
///
/// - which windows are active at a given moment?
/// - what is the next window after a given moment?
///
/// This trait is intentionally generic and can represent:
///
/// - static schedules
/// - recurring rules
/// - dynamic generators
/// - overlapping event systems
/// - session state machines
///
/// # Overlap
///
/// `active_windows` returns a `Vec` because multiple windows may be active at
/// the same time.
///
/// # Metadata
///
/// Each returned [`Window`] carries metadata of type [`Self::Meta`].
pub trait WindowSource {
    /// Metadata attached to each generated window.
    type Meta;

    /// Returns all windows active at `now`.
    ///
    /// Implementations may return zero, one, or many windows.
    ///
    /// This is especially useful for overlapping systems where multiple windows
    /// can be active simultaneously.
    fn active_windows(&self, now: DateTime<Utc>) -> Vec<Window<Self::Meta>>;

    /// Returns the next window after `after`.
    ///
    /// Implementers should document whether this is strictly after `after` or
    /// may include a window whose start is exactly equal to `after`.
    ///
    /// For reliable iterator behavior, implementations should ensure that
    /// repeated calls used for traversal make progress.
    fn next_window(&self, after: DateTime<Utc>) -> Option<Window<Self::Meta>>;
}

/// A [`WindowSource`] that also supports reverse lookup.
///
/// This trait is useful for:
///
/// - historical traversal
/// - reverse iteration
/// - finding the most recent previous window
pub trait BidirectionalWindowSource: WindowSource {
    /// Returns the previous window before `before`.
    ///
    /// Implementers should document whether this is strictly before `before` or
    /// may include a window whose start is exactly equal to `before`.
    ///
    /// For reliable reverse iteration, implementations should ensure that
    /// repeated calls used for traversal make progress.
    fn prev_window(&self, before: DateTime<Utc>) -> Option<Window<Self::Meta>>;
}
