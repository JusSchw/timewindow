use chrono::{DateTime, Utc};

use crate::{BidirectionalWindowSource, Window, WindowSource};

/// Iterator over successive future windows produced by a [`WindowSource`].
///
/// This iterator repeatedly calls [`WindowSource::next_window`] starting from an
/// initial cursor.
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
        self.cursor = window.start;
        Some(window)
    }
}

/// Iterator over successive previous windows produced by a
/// [`BidirectionalWindowSource`].
///
/// This iterator repeatedly calls [`BidirectionalWindowSource::prev_window`]
/// starting from an initial cursor.
pub struct PrevWindows<'a, S>
where
    S: BidirectionalWindowSource,
{
    source: &'a S,
    cursor: DateTime<Utc>,
}

impl<'a, S> PrevWindows<'a, S>
where
    S: BidirectionalWindowSource,
{
    pub fn new(source: &'a S, from: DateTime<Utc>) -> Self {
        Self {
            source,
            cursor: from,
        }
    }
}

impl<'a, S> Iterator for PrevWindows<'a, S>
where
    S: BidirectionalWindowSource,
{
    type Item = Window<S::Meta>;

    fn next(&mut self) -> Option<Self::Item> {
        let window = self.source.prev_window(self.cursor)?;
        self.cursor = window.start;
        Some(window)
    }
}

pub trait WindowSourceExt: WindowSource {
    /// Returns an iterator over windows produced after `from`.
    ///
    /// This is a convenience wrapper around [`NextWindows::new`].
    fn next_windows_from(&self, from: DateTime<Utc>) -> NextWindows<'_, Self>
    where
        Self: Sized,
    {
        NextWindows::new(self, from)
    }
}

impl<T: WindowSource> WindowSourceExt for T {}

pub trait BidirectionalWindowSourceExt: BidirectionalWindowSource {
    /// Returns an iterator over windows produced before `from`.
    ///
    /// This is a convenience wrapper around [`PrevWindows::new`].
    fn prev_windows_from(&self, from: DateTime<Utc>) -> PrevWindows<'_, Self>
    where
        Self: Sized,
    {
        PrevWindows::new(self, from)
    }
}

impl<T: BidirectionalWindowSource> BidirectionalWindowSourceExt for T {}
