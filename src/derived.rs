use chrono::{DateTime, Utc};

use crate::{BidirectionalWindowSource, Window, WindowSource};

pub trait DerivedWindowSource {
    type Source: WindowSource;
    type Meta;

    fn source(&self) -> &Self::Source;

    fn map_window(
        &self,
        window: Window<<Self::Source as WindowSource>::Meta>,
    ) -> Window<Self::Meta>;

    fn map_active_windows(&self, now: DateTime<Utc>) -> Vec<Window<Self::Meta>> {
        self.source()
            .active_windows(now)
            .into_iter()
            .map(|w| self.map_window(w))
            .collect()
    }

    fn map_next_window(&self, after: DateTime<Utc>) -> Option<Window<Self::Meta>> {
        self.source().next_window(after).map(|w| self.map_window(w))
    }
}

impl<T> WindowSource for T
where
    T: DerivedWindowSource,
{
    type Meta = T::Meta;

    fn active_windows(&self, now: DateTime<Utc>) -> Vec<Window<Self::Meta>> {
        self.map_active_windows(now)
    }

    fn next_window(&self, after: DateTime<Utc>) -> Option<Window<Self::Meta>> {
        self.map_next_window(after)
    }
}

pub trait DerivedBidirectionalWindowSource: DerivedWindowSource
where
    Self::Source: BidirectionalWindowSource,
{
    fn map_prev_window(&self, before: DateTime<Utc>) -> Option<Window<Self::Meta>> {
        self.source()
            .prev_window(before)
            .map(|w| self.map_window(w))
    }
}

impl<T> BidirectionalWindowSource for T
where
    T: DerivedBidirectionalWindowSource,
    <T as DerivedWindowSource>::Source: BidirectionalWindowSource,
{
    fn prev_window(&self, before: DateTime<Utc>) -> Option<Window<Self::Meta>> {
        self.map_prev_window(before)
    }
}
