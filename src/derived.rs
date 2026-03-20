use chrono::{DateTime, Utc};

use crate::{Window, WindowSource};

/// A [`WindowSource`] derived from another source by mapping its produced
/// windows.
///
/// This trait is useful for building lightweight adapters around an existing
/// source. A derived source:
///
/// - exposes an underlying [`WindowSource`] via [`Self::source`]
/// - transforms each produced [`Window`] via [`Self::map_window`]
/// - automatically implements [`WindowSource`] through a blanket impl
///
/// The default implementations of [`Self::map_active_windows`] and
/// [`Self::map_next_window`] delegate to the wrapped source and then apply
/// [`Self::map_window`] to each result.
///
/// # Semantics
///
/// A derived source typically preserves the recurrence and overlap behavior of
/// its wrapped source, but this is not required. Because [`Self::map_window`]
/// can transform the full window, including `start` and `end`, a derived
/// source may alter timing semantics as well as metadata.
///
/// # Blanket implementation
///
/// Any type implementing `DerivedWindowSource` automatically implements
/// [`WindowSource`] with `Meta = Self::Meta`.
pub trait DerivedWindowSource {
    /// The underlying source type being adapted.
    type Source: WindowSource;

    /// The metadata type exposed by the derived source.
    type Meta;

    /// Returns the underlying source.
    fn source(&self) -> &Self::Source;

    /// Maps a window from the underlying source into a window exposed by this
    /// derived source.
    fn map_window(
        &self,
        window: Window<<Self::Source as WindowSource>::Meta>,
    ) -> Window<Self::Meta>;

    /// Returns all active windows from the underlying source, mapped through
    /// [`Self::map_window`].
    fn map_active_windows(&self, now: DateTime<Utc>) -> Vec<Window<Self::Meta>> {
        self.source()
            .active_windows(now)
            .into_iter()
            .map(|w| self.map_window(w))
            .collect()
    }

    /// Returns the next window from the underlying source, mapped through
    /// [`Self::map_window`].
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
