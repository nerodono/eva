use std::{borrow::Cow, num::NonZeroU64, ops::Add, str::FromStr, time::Duration};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize, de};

use crate::{
    data,
    str::{HasPattern, ParseError},
    time::{Hours, Mins, Secs, SecsTime, SubsecNanos, Time},
};

use super::{Date, LooseDate, PreciseTime, str, utils::divmod};

#[data(copy, ord, not(serde, schemars), crate = crate)]
pub struct LooseTimestamp<D = Date> {
    date: D,
    time: PreciseTime,
}

impl<D> JsonSchema for LooseTimestamp<D> {
    fn schema_id() -> Cow<'static, str> {
        Cow::Borrowed(concat!(module_path!(), "::LooseTimestamp"))
    }

    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("LooseTimestamp")
    }

    fn json_schema(_gen: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "string",
            "pattern": str::TimestampStr::regex_pat_fullmatch(),
            "description": "precise timestamp representation (UTC)"
        })
    }
}

impl Serialize for LooseTimestamp<LooseDate> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.to_str().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for LooseTimestamp<LooseDate> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = str::TimestampStr::deserialize(deserializer)?;
        let ts = s
            .parse()
            .ok_or(de::Error::custom(zst_error!("malformed timestamp")))?;
        Ok(ts)
    }
}

impl Serialize for LooseTimestamp {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.loose().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for LooseTimestamp {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let loose = LooseTimestamp::<LooseDate>::deserialize(deserializer)?;
        Ok(loose.compact_loose())
    }
}

impl LooseTimestamp<LooseDate> {
    pub const fn to_str(self) -> str::TimestampStr {
        let date = self.date.to_str();
        let time = self.time.to_str();

        str::TimestampStr::new(date, time)
    }

    pub const fn compact_loose(self) -> LooseTimestamp {
        LooseTimestamp {
            date: self.date.compact(),
            time: self.time,
        }
    }
}

impl LooseTimestamp {
    /// Returns [`None`] for the exact unix epoch, which [`Timestamp`]
    /// cannot represent.
    pub const fn compact(self) -> Option<Timestamp> {
        let secs = self.date().to_secs();
        let Some(nanos) = NonZeroU64::new(self.time().to_nanos() + secs * 1_000_000_000) else {
            return None;
        };

        Some(Timestamp::from_nanos(nanos))
    }

    pub const fn loose(self) -> LooseTimestamp<LooseDate> {
        LooseTimestamp {
            date: self.date.loose(),
            time: self.time,
        }
    }
}

impl<D: Copy> LooseTimestamp<D> {
    pub const fn new(date: D, time: PreciseTime) -> Self {
        Self { date, time }
    }

    pub const fn date(self) -> D {
        self.date
    }

    pub const fn time(self) -> PreciseTime {
        self.time
    }
}

/// Timestamp in nanoseconds, since UTC.
#[data(copy, ord, not(serde, schemars), display("{}", self.to_str()), crate = crate)]
#[derive(Hash)]
pub struct Timestamp(NonZeroU64);

impl FromStr for Timestamp {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        str::TimestampStr::from_str(s).and_then(|t| {
            let loose = t.parse().ok_or(ParseError::Char)?;
            loose.compact_loose().compact().ok_or(ParseError::Range)
        })
    }
}

impl JsonSchema for Timestamp {
    fn schema_id() -> Cow<'static, str> {
        Cow::Borrowed(concat!(module_path!(), "::Timestamp"))
    }

    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("Timestamp")
    }

    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "anyOf": [
                {
                    "type": "integer",
                    "minimum": 1,
                    "description": "Number of nanoseconds since UTC unix epoch, only in binary formats"
                },
                LooseTimestamp::<LooseDate>::json_schema(generator)
            ]
        })
    }
}

impl Serialize for Timestamp {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if serializer.is_human_readable() {
            self.loose().serialize(serializer)
        } else {
            self.0.serialize(serializer)
        }
    }
}

impl<'de> Deserialize<'de> for Timestamp {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        if deserializer.is_human_readable() {
            LooseTimestamp::deserialize(deserializer)?
                .compact()
                .ok_or_else(|| de::Error::custom(zst_error!("timestamp cannot be the unix epoch")))
        } else {
            let nanos = NonZeroU64::deserialize(deserializer)?;
            Ok(Self::from_nanos(nanos))
        }
    }
}

impl Add<Duration> for Timestamp {
    type Output = Timestamp;

    fn add(self, rhs: Duration) -> Self::Output {
        Self(
            self.0
                .checked_add(rhs.as_nanos() as u64)
                .expect("duration addition overflown"),
        )
    }
}

#[rustfmt::skip]
macro_rules! checked {
    ($lhs:expr) => {'ret: {
        let Some(value) = NonZeroU64::new($lhs) else {
            break 'ret None;
        };

        Some(Timestamp::from_nanos(value))
    }};
}

macro_rules! saturated {
    ($lhs:expr) => {
        if let Some(val) = checked!($lhs) {
            val
        } else {
            Timestamp::MIN
        }
    };
}

impl Timestamp {
    pub const fn from_nanos(nanos: NonZeroU64) -> Self {
        Self(nanos)
    }

    pub const fn from_secs(secs: u64) -> Self {
        saturated!(secs * 1_000_000_000)
    }

    pub const fn from_secs_checked(secs: u64) -> Option<Self> {
        checked!(secs * 1_000_000_000)
    }

    pub const fn from_millis(millis: u64) -> Self {
        saturated!(millis * 1_000_000)
    }

    pub const fn from_millis_checked(millis: u64) -> Option<Self> {
        checked!(millis * 1_000_000)
    }

    /// # Panics
    ///
    /// Panics if the timestamp is beyond the range representable
    /// by [`Date`] (~year 2149).
    pub fn loose(self) -> LooseTimestamp<LooseDate> {
        let date = self
            .date()
            .expect("timestamp is beyond the representable date range");
        let time = {
            let nanos = self.0.get();
            let (secs, subsec_nanos) = divmod(nanos, 1_000_000_000);
            let (mins, subsecs) = divmod(secs, 60);
            let (hours, submins) = divmod(mins, 60);
            let subhours = hours % 24;

            let time = Time::new(unsafe { Hours::new_unchecked(subhours as u8) }, unsafe {
                Mins::new_unchecked(submins as u8)
            });
            let secs_time = SecsTime::new(time, unsafe { Secs::new_unchecked(subsecs as u8) });

            let subsec_nanos = unsafe { SubsecNanos::new_unchecked(subsec_nanos as u32) };
            PreciseTime::new(secs_time, subsec_nanos)
        };

        LooseTimestamp { date, time }.loose()
    }

    /// Convert timestamp to string.
    pub fn to_str(self) -> str::TimestampStr {
        self.loose().to_str()
    }
}

impl Timestamp {
    /// Get date-precision time.
    ///
    /// Returns [`None`] if the timestamp is beyond the range
    /// representable by [`Date`] (~year 2149).
    pub const fn date(self) -> Option<Date> {
        let days = self.as_hours() / 24;
        if days > u16::MAX as u32 {
            return None;
        }

        Some(Date::from_days(days as u16))
    }

    /// Get time of current day.
    pub fn time(self) -> PreciseTime {
        self.loose().time()
    }

    /// Get time in hours.
    pub const fn as_hours(self) -> u32 {
        (self.as_mins() / 60) as u32
    }

    /// Get time in minutes.
    pub const fn as_mins(self) -> u64 {
        self.as_secs() / 60
    }

    /// Get time in seconds.
    pub const fn as_secs(self) -> u64 {
        self.as_millis() / 1_000
    }

    /// Get time in milliseconds.
    pub const fn as_millis(self) -> u64 {
        self.as_nanos().get() / 1_000_000
    }

    /// Get time in nanoseconds.
    pub const fn as_nanos(self) -> NonZeroU64 {
        self.0
    }
}

impl Timestamp {
    // 2025.01.01 00:00.00
    pub const TEST_ORIGIN: Self = Self(NonZeroU64::new(1735689600 * 1_000_000_000).unwrap());

    /// Minimum time that can be represented.
    pub const MIN: Self = Self(NonZeroU64::MIN);

    /// Maximum time that can be represented.
    pub const MAX: Self = Self(NonZeroU64::MAX);
}
