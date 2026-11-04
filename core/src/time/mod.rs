//! # Date and time
//!
//! Contains heavily and precisely typed utilities for working
//! with types.
//!
//! # Glossary
//!
//! ### [`Clock`]
//!
//! Trait which is responsible for getting current time.
//!
//! - [`Mock`] - Mock [`Clock`], can be used for testing purposes
//! - [`RealTime`] - realtime clock
//!
//! ### Date and time types
//!
//! Main types:
//! - [`Date`] - day precise time, contains [`Day`], [`Month`] and [`Year`]
//! - [`Time`] - time during day, contains [`Hours`] and [`Mins`]
//! - [`SecsTime`] - same as [`Time`], but contains seconds
//! - [`PreciseTime`] - same as [`Time`], but contains seconds and nanoseconds
//! - [`Timestamp`] - maximum precision timestamp, nanoseconds precision
//!
//! Every timestamp here return timestamp in the UTC timezone.
//!
//! ### String representation
//!
//! Every type presented here have string representation, see [`self::str`] module.

pub use self::{
    clock::{Clock, Mock, RealTime},
    date::{Date, Day, Leapness, LooseDate, Month, Year},
    duration::HumanDuration,
    time::{Hours, Mins, PreciseTime, Secs, SecsTime, SubsecNanos, Time},
    timestamp::Timestamp,
};

pub mod ser;
pub mod str;
pub mod tz;

mod duration;

mod clock;
mod date;
#[allow(clippy::module_inception)]
mod time;
mod timestamp;

mod utils;

#[cfg(test)]
mod tests;
