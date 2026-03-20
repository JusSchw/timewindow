//! Generic time-window generation for recurring, overlapping, and metadata-rich intervals
//!
//! A window is a half-open UTC interval `[start, end)` paired with arbitrary
//! metadata.
//!
//! This crate is designed for systems that need to model:
//!
//! - recurring or rule-generated intervals
//! - overlapping events
//! - active/upcoming/previous windows
//! - arbitrary metadata attached to time periods
//!
//! # Core types
//!
//! - [`Window`] represents a single time interval with metadata.
//! - [`WindowSource`] produces active and upcoming windows.
//! - [`BidirectionalWindowSource`] also supports reverse lookup.
//! - [`WindowSourceExt`] and [`BidirectionalWindowSourceExt`] provide iterators.
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
//! # Metadata
//!
//! Each [`Window`] carries metadata of type `M`, allowing callers to associate
//! arbitrary context such as IDs, slugs, labels, or structured payloads.
//!
//! # Timezone
//!
//! All times are represented as `DateTime<Utc>`.

mod derived;
mod iter;
mod source;
mod window;

pub use derived::{DerivedBidirectionalWindowSource, DerivedWindowSource};
pub use iter::{BidirectionalWindowSourceExt, NextWindows, PrevWindows, WindowSourceExt};
pub use source::{BidirectionalWindowSource, WindowSource};
pub use window::Window;
