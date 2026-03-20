use chrono::{DateTime, Utc};

use crate::{Window, WindowSource};

/// An iterator over successive upcoming windows from a [`WindowSource`].
///
/// This iterator repeatedly calls [`WindowSource::next_window`] and yields the
/// returned windows in ascending start-time order.
///
/// # Progress semantics
///
/// After yielding a window, the iterator advances its internal cursor to that
/// window's `start`, not its `end`. This is intentional: sources may produce
/// overlapping windows, and advancing by `end` could skip valid future windows
/// that start during an earlier yielded window.
///
/// Implementations of [`WindowSource::next_window`] are required to return a
/// window whose `start` is strictly greater than the supplied cursor. As a
/// defensive safeguard, this iterator terminates if a source returns a
/// non-progressing window.
///
/// # Termination
///
/// The iterator ends when the source returns `None`, or when the source
/// violates the strict-progress contract.
pub struct NextWindows<'a, S>
where
    S: WindowSource,
{
    source: &'a S,
    cursor: DateTime<Utc>,
}

impl<'a, S> NextWindows<'a, S>
where
    S: WindowSource,
{
    /// Creates an iterator over windows starting strictly after `from`.
    pub fn new(source: &'a S, from: DateTime<Utc>) -> Self {
        Self {
            source,
            cursor: from,
        }
    }
}

impl<'a, S> Iterator for NextWindows<'a, S>
where
    S: WindowSource,
{
    type Item = Window<S::Meta>;

    fn next(&mut self) -> Option<Self::Item> {
        let window = self.source.next_window(self.cursor)?;
        if window.start <= self.cursor {
            return None;
        }
        self.cursor = window.start;
        Some(window)
    }
}

/// Extension methods for [`WindowSource`].
///
/// This trait provides convenience iteration helpers for any window source.
pub trait WindowSourceExt: WindowSource {
    /// Returns an iterator over successive windows after `from`.
    ///
    /// Each yielded window is produced by repeated calls to
    /// [`WindowSource::next_window`]. The iterator preserves overlap-aware
    /// semantics by advancing from one yielded window's `start` to the next.
    ///
    /// For infinite recurring sources, this iterator may itself be infinite;
    /// callers can combine it with adapters like [`Iterator::take`].
    fn next_windows_from(&self, from: DateTime<Utc>) -> NextWindows<'_, Self>
    where
        Self: Sized,
    {
        NextWindows::new(self, from)
    }
}

impl<T: WindowSource> WindowSourceExt for T {}
