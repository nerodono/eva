use std::{mem, str::FromStr};

use crate::{data, int, str::ParseError, time::str};

// == PreciseTime ==

#[data(copy, ord, not(serde, schemars), crate = crate)]
pub struct PreciseTime {
    time: SecsTime,
    nanos: SubsecNanos,
}

impl FromStr for PreciseTime {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        str::PreciseTimeStr::from_str(s).and_then(|s| s.parse().ok_or(ParseError::Char))
    }
}

impl PreciseTime {
    pub const fn new(time: SecsTime, nanos: SubsecNanos) -> Self {
        Self { time, nanos }
    }

    pub const fn to_nanos(self) -> u64 {
        let secs = self.time.to_secs();
        self.nanos.into_inner() as u64 + secs * 1_000_000_000
    }

    pub const fn secs(self) -> Secs {
        self.secs_time().secs()
    }

    pub const fn mins(self) -> Mins {
        self.secs_time().mins()
    }

    pub const fn hours(self) -> Hours {
        self.secs_time().hours()
    }

    pub const fn subsecs_nanos(self) -> SubsecNanos {
        self.nanos
    }

    pub const fn secs_time(self) -> SecsTime {
        self.time
    }

    pub const fn to_str(self) -> str::PreciseTimeStr {
        str::PreciseTimeStr::new(self.time.to_str(), self.nanos.to_str())
    }
}

// == SecsTime ==

#[data(copy, ord, not(serde, schemars), crate = crate)]
#[derive(Hash)]
pub struct SecsTime {
    time: Time,
    secs: Secs,
}

impl FromStr for SecsTime {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        str::SecsTimeStr::from_str(s).and_then(|s| s.parse().ok_or(ParseError::Char))
    }
}

impl SecsTime {
    pub const fn to_secs(self) -> u64 {
        self.time.to_secs() + self.secs.into_inner() as u64
    }
}

impl SecsTime {
    pub const fn new(time: Time, secs: Secs) -> Self {
        Self { time, secs }
    }

    pub const fn hours(self) -> Hours {
        self.time().hours()
    }

    pub const fn mins(self) -> Mins {
        self.time().mins()
    }

    pub const fn secs(self) -> Secs {
        self.secs
    }

    pub const fn time(self) -> Time {
        self.time
    }

    pub const fn to_str(self) -> str::SecsTimeStr {
        str::SecsTimeStr::new(self.time.to_str(), self.secs.to_str())
    }
}

// == Time ==

#[data(copy, ord, not(serde, schemars), crate = crate)]
#[derive(Hash)]
pub struct Time {
    hours: Hours,
    mins: Mins,
}

impl FromStr for Time {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        str::TimeStr::from_str(s).and_then(|s| s.parse().ok_or(ParseError::Char))
    }
}

impl Time {
    pub const fn new(hours: Hours, mins: Mins) -> Self {
        Self { hours, mins }
    }

    pub const fn to_secs(self) -> u64 {
        (self.hours as u64 * 60 + self.mins as u64) * 60
    }

    pub const fn hours(self) -> Hours {
        self.hours
    }

    pub const fn mins(self) -> Mins {
        self.mins
    }

    pub const fn to_str(self) -> str::TimeStr {
        let hours = self.hours().to_str();
        let mins = self.mins().to_str();

        str::TimeStr::new(hours, mins)
    }
}

/// Hours.
#[int(u8, 0..24, crate = crate)]
pub enum Hours {}

impl Hours {
    pub const fn to_str(self) -> str::HoursStr {
        let last = (self as u8) % 10;
        let first = (self as u8) / 10;

        unsafe { mem::transmute::<[u8; 2], str::HoursStr>([b'0' + first, b'0' + last]) }
    }
}

/// Minutes.
#[int(u8, 0..60, crate = crate)]
pub enum Mins {}

impl Mins {
    pub const fn to_str(self) -> str::MinsStr {
        let first = (self as u8) / 10;
        let last = (self as u8) % 10;

        unsafe { mem::transmute::<[u8; 2], str::MinsStr>([b'0' + first, b'0' + last]) }
    }
}

/// Seconds.
#[int(u8, 0..60, crate = crate)]
pub enum Secs {}

impl Secs {
    pub const fn to_str(self) -> str::SecsStr {
        let first = (self as u8) / 10;
        let last = (self as u8) % 10;

        unsafe { mem::transmute::<[u8; 2], str::SecsStr>([b'0' + first, b'0' + last]) }
    }
}

#[data(copy, ord, not(serde), crate = crate)]
#[derive(Hash, Default)]
pub struct SubsecNanos(#[schemars(range(min = Self::MIN.0, max = Self::MAX.0))] u32);

impl SubsecNanos {
    pub const MIN: Self = Self(0);
    pub const MAX: Self = Self(999999999);

    pub const fn into_inner(self) -> u32 {
        self.0
    }

    pub const fn to_str(self) -> str::SubsecNanosStr {
        const fn digit(v: u32) -> u8 {
            (v % 10) as u8 + b'0'
        }
        let val = self.0;

        let arr = [
            digit(val / 100_000_000),
            digit(val / 10_000_000),
            digit(val / 1_000_000),
            digit(val / 100_000),
            digit(val / 10_000),
            digit(val / 1_000),
            digit(val / 100),
            digit(val / 10),
            digit(val),
        ];

        unsafe { mem::transmute::<[u8; 9], str::SubsecNanosStr>(arr) }
    }

    /// Create subsecs without checking the range.
    ///
    /// # Safety
    /// The `value` must be a valid subsecond nanosecond count (< 1_000_000_000).
    pub const unsafe fn new_unchecked(value: u32) -> Self {
        Self(value)
    }

    pub const fn new(value: u32) -> Option<Self> {
        if value > Self::MAX.0 {
            None
        } else {
            Some(Self(value))
        }
    }
}
