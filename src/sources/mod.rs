#[cfg(feature = "interval")]
mod interval;

#[cfg(feature = "schedule")]
mod schedule;

#[cfg(feature = "interval")]
pub use interval::{IntervalPattern, IntervalSource, IntervalSourceError};

#[cfg(feature = "schedule")]
pub use schedule::{Schedule, ScheduleRule, ScheduleSource, ScheduleSourceError};
