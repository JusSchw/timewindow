//! Generic time-window generation for recurring, overlapping, and metadata-rich
//! intervals.
//!
//! A window is a half-open UTC interval `[start, end)` paired with arbitrary
//! metadata.
//!
//! This crate is designed for systems that need to model:
//!
//! - recurring or rule-generated intervals
//! - overlapping events
//! - active and upcoming windows
//! - arbitrary metadata attached to time periods
//! - source composition and adaptation
//!
//! # Core types
//!
//! - [`Window`] represents a single time interval with metadata.
//! - [`WindowSource`] produces active and upcoming windows.
//! - [`WindowSourceExt`] provides iterator helpers for upcoming windows.
//! - [`DerivedWindowSource`] adapts one source into another by mapping windows.
//!
//! # Built-in sources
//!
//! When the `sources` feature is enabled, the crate also provides common source
//! implementations under [`sources`].
//!
//! # Interval semantics
//!
//! Windows are half-open intervals: a window is active when
//! `start <= now && now < end`.
//!
//! This means:
//!
//! - the start time is included
//! - the end time is excluded
//!
//! # Overlap
//!
//! Sources may produce overlapping windows. This crate does not assume
//! exclusivity or a calendar-like event model.
//!
//! As a result:
//!
//! - multiple windows may be active at the same instant
//! - upcoming windows are ordered by start time
//! - iterator helpers preserve overlapping schedules rather than skipping ahead
//!   to the previous window's end
//!
//! # Metadata
//!
//! Each [`Window`] carries metadata of type `M`, allowing callers to associate
//! arbitrary context such as IDs, slugs, labels, or structured payloads.
//!
//! # Extensibility
//!
//! You can extend the crate in two primary ways:
//!
//! - implement [`WindowSource`] for a custom generator
//! - implement [`DerivedWindowSource`] to adapt an existing source
//!
//! # Timezone
//!
//! All times are represented as `DateTime<Utc>`.
//!
//! # Example
//!
//! ```rust
//! use chrono::{Duration, TimeZone, Utc};
//! use timewindow::{WindowSource, WindowSourceExt};
//! use timewindow::sources::IntervalSource;
//!
//! let anchor = Utc.with_ymd_and_hms(2026, 3, 20, 0, 0, 0).unwrap();
//! let source = IntervalSource::single(
//!     anchor,
//!     Duration::hours(9),
//!     Duration::days(1),
//!     Duration::hours(2),
//!     "morning",
//! )
//! .unwrap();
//!
//! let windows: Vec<_> = source
//!     .next_windows_from(Utc.with_ymd_and_hms(2026, 3, 20, 8, 0, 0).unwrap())
//!     .take(2)
//!     .collect();
//!
//! assert_eq!(windows.len(), 2);
//! assert_eq!(windows[0].meta, "morning");
//! assert_eq!(windows[0].start, Utc.with_ymd_and_hms(2026, 3, 20, 9, 0, 0).unwrap());
//! assert_eq!(windows[1].start, Utc.with_ymd_and_hms(2026, 3, 21, 9, 0, 0).unwrap());
//! ```

mod derived;
mod iter;
mod source;
mod window;

#[cfg(feature = "sources")]
pub mod sources;

pub use derived::DerivedWindowSource;
pub use iter::{NextWindows, WindowSourceExt};
pub use source::WindowSource;
pub use window::Window;
