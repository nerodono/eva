use std::{borrow::Cow, mem, str::FromStr};

use schemars::JsonSchema;
use seq_macro::seq;
use serde::{Deserialize, Serialize, de};

use crate::{
    data, int,
    str::{HasPattern, ParseError},
};

use super::str;

#[data(ord, copy, display(name), crate = crate)]
pub enum Leapness {
    Leap = 1,
    Ordinary = 0,
}

impl Leapness {
    pub const fn is_leap(self) -> bool {
        matches!(self, Self::Leap)
    }

    pub const fn is_ordinary(self) -> bool {
        matches!(self, Self::Ordinary)
    }
}

#[data(copy, ord, not(serde, schemars), crate = crate)]
pub struct LooseDate {
    day: Day,
    month: Month,
    year: Year,
}

impl JsonSchema for LooseDate {
    fn schema_id() -> Cow<'static, str> {
        Cow::Borrowed(concat!(module_path!(), "::LooseDate"))
    }

    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("LooseDate")
    }

    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "string",
            "pattern": str::DateStr::regex_pat_fullmatch(),
            "description": "Day, month and year"
        })
    }
}

impl<'de> Deserialize<'de> for LooseDate {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        struct Visitor;

        impl de::Visitor<'_> for Visitor {
            type Value = LooseDate;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a date string")
            }

            // Any string, not only borrowed ones: tree and reader-based deserializers own theirs.
            fn visit_str<E: de::Error>(self, s: &str) -> Result<LooseDate, E> {
                let date: str::DateStr = s.parse().map_err(E::custom)?;

                date.parse().ok_or(E::custom(zst_error!(
                    "invalid day, month and year combination"
                )))
            }
        }

        deserializer.deserialize_str(Visitor)
    }
}

impl Serialize for LooseDate {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.to_str().serialize(serializer)
    }
}

impl LooseDate {
    pub const fn from_dmy(day: Day, month: Month, year: Year) -> Option<Self> {
        let last_day = month.last_day(year.leapness());
        if day as u8 > last_day as u8 {
            return None;
        }

        Some(Self { day, month, year })
    }

    /// Get day of month.
    pub const fn day_of_month(self) -> Day {
        self.day
    }

    /// Get current month.
    pub const fn month(self) -> Month {
        self.month
    }

    /// Get year.
    pub const fn year(self) -> Year {
        self.year
    }

    /// Convert date to more compact representation.
    pub const fn compact(self) -> Date {
        let year = self.year();
        let leapness = year.leapness();

        let first_day_of_year = year.first_day().days();
        let year_offset = self.month().days_from_year_start(leapness);
        let month_offset = self.day_of_month() as u16 - 1;

        Date::from_days(first_day_of_year + year_offset + month_offset)
    }

    /// Convert date to string.
    pub const fn to_str(self) -> str::DateStr {
        str::DateStr::new(
            self.day_of_month().to_str(),
            self.month().to_str(),
            self.year().to_str(),
        )
    }
}

#[data(copy, ord, not(serde, schemars), crate = crate)]
#[derive(Hash)]
pub struct Date(u16);

impl JsonSchema for Date {
    fn schema_id() -> Cow<'static, str> {
        Cow::Borrowed(concat!(module_path!(), "::Date"))
    }

    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("Date")
    }

    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "anyOf": [
                {
                    "type": "integer",
                    "minimum": 0,
                    "maximum": 65535,
                    "description": "number of days since 1970, only in binary formats"
                },
                LooseDate::json_schema(generator)
            ]
        })
    }
}

impl FromStr for Date {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        str::DateStr::from_str(s)?
            .parse()
            .map(|d| d.compact())
            .ok_or(ParseError::Char)
    }
}

impl<'de> Deserialize<'de> for Date {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        if deserializer.is_human_readable() {
            let loose = LooseDate::deserialize(deserializer)?;
            Ok(loose.compact())
        } else {
            Ok(Date::from_days(u16::deserialize(deserializer)?))
        }
    }
}

impl Serialize for Date {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if serializer.is_human_readable() {
            self.to_str().serialize(serializer)
        } else {
            self.days().serialize(serializer)
        }
    }
}

impl Date {
    pub const MIN: Self = Self(0);
    pub const MAX: Self = Self(u16::MAX);

    pub const fn from_dmy(day: Day, month: Month, year: Year) -> Option<Self> {
        let Some(loosen) = LooseDate::from_dmy(day, month, year) else {
            return None;
        };

        Some(loosen.compact())
    }

    pub const fn to_secs(self) -> u64 {
        let hours = (self.0 as u64) * 24;
        let mins = hours * 60;

        mins * 60
    }

    /// Create date from number of days.
    pub const fn from_days(days: u16) -> Self {
        Self(days)
    }

    /// Get number of days.
    pub const fn days(self) -> u16 {
        self.0
    }

    /// Loosen up layout to less compact one.
    pub const fn loose(self) -> LooseDate {
        let year = self.year();
        let total_days = self.days();
        let leapness = year.leapness();
        let days_since_year = total_days - year.first_day().days();

        let month = unsafe {
            Month::from_days_since_year_start(days_since_year, leapness).unwrap_unchecked()
        };
        let day_of_month = days_since_year - month.days_from_year_start(leapness);

        LooseDate {
            // SAFETY: safe, since condition for exiting loop is falling
            // into days range.
            day: unsafe { Day::new_unchecked(day_of_month as u8 + 1) },
            month,
            year,
        }
    }

    /// Get current year.
    pub const fn year(self) -> Year {
        let days = self.days();
        if days == 0 {
            return Year::MIN;
        }
        let naive_year = days / 365;
        let leap_days = super::utils::leap_days_after((naive_year + Year::ORIGIN) - 1)
            - Year::LEAP_DAYS_BEFORE1970;
        // naive_year includes leap days from the previous years! Skewing
        // calculations!
        let fixed = days - leap_days;

        debug_assert!((fixed / 365) < 179);
        unsafe { Year::new_unchecked((fixed / 365) as u8) }
    }

    /// Get number of days since the current year. I.e. if it's
    /// January 1st -> result would be 0.
    pub const fn days_since_year(self) -> u16 {
        self.0 - self.year().first_day().days()
    }

    /// Get current month.
    pub const fn month(self) -> Month {
        self.loose().month()
    }

    /// Get day of month.
    pub const fn day_of_month(self) -> Day {
        self.loose().day_of_month()
    }

    /// Convert date to string.
    pub const fn to_str(self) -> str::DateStr {
        self.loose().to_str()
    }
}

#[int(u8, 0..179, crate = crate)]
pub enum Year {}

impl Year {
    pub const MIN: Self = Self::new(0).unwrap();
    pub const MAX: Self = Self::new(178).unwrap();

    const LEAP_DAYS_BEFORE1970: u16 = super::utils::leap_days_after(1970 - 1);
    pub const ORIGIN: u16 = 1970;

    pub const fn from_abs(abs: u16) -> Option<Self> {
        if matches!(abs, 0..Self::ORIGIN) {
            return None;
        }

        let shifted = abs - Self::ORIGIN;
        if shifted > Self::MAX as u16 {
            None
        } else {
            Some(unsafe { Year::new_unchecked(shifted as u8) })
        }
    }

    pub const fn leapness(self) -> Leapness {
        if super::utils::is_leap_year(self.abs()) {
            Leapness::Leap
        } else {
            Leapness::Ordinary
        }
    }

    /// Get date of the first day in a year.
    pub const fn first_day(self) -> Date {
        let leap_days = super::utils::leap_days_after(self.abs() - 1) - Self::LEAP_DAYS_BEFORE1970;
        let naive_days = (self as u16) * 365;

        Date::from_days(naive_days + leap_days)
    }

    /// Convert relative year to absolute year.
    pub const fn abs(self) -> u16 {
        self as u16 + Self::ORIGIN
    }

    /// Convert year to fixed size string.
    pub const fn to_str(self) -> str::YearStr {
        let v = self.abs();
        let buf = [
            (v / 1000) as u8 + b'0',
            (v / 100 % 10) as u8 + b'0',
            (v / 10 % 10) as u8 + b'0',
            (v % 10) as u8 + b'0',
        ];

        unsafe { mem::transmute::<[u8; 4], str::YearStr>(buf) }
    }
}

#[int(u8, 1..=32, crate = crate)]
pub enum Day {}

impl Day {
    pub const fn first() -> Self {
        Self::VARIANTS[0]
    }

    /// Get index inside [`Day::VARIANTS`].
    pub const fn index(self) -> usize {
        self as usize - 1
    }

    /// Convert day to fixed size string.
    pub const fn to_str(self) -> str::DayStr {
        let d = self as u8;
        unsafe { mem::transmute::<[u8; 2], str::DayStr>([d / 10 + b'0', d % 10 + b'0']) }
    }
}

#[data(copy, ord, crate = crate, display(name))]
pub enum Month {
    Jan = 0,
    Feb = 1,
    Mar = 2,
    Apr = 3,
    May = 4,
    Jun = 5,
    Jul = 6,
    Aug = 7,
    Sep = 8,
    Oct = 9,
    Nov = 10,
    Dec = 11,
}

impl Month {
    pub const MIN: Self = Self::Jan;
    pub const MAX: Self = Self::Dec;
    pub const VARIANTS: [Self; 12] = {
        use Month::*;

        [Jan, Feb, Mar, Apr, May, Jun, Jul, Aug, Sep, Oct, Nov, Dec]
    };

    pub const fn prev(self) -> Self {
        let Some(prev) = self.prev_checked() else {
            return Self::MIN;
        };

        prev
    }

    pub const fn prev_checked(self) -> Option<Self> {
        let Some(prev) = (self as u8).checked_sub(1) else {
            return None;
        };

        Some(unsafe { Month::from_repr_unchecked(prev) })
    }

    pub const fn next(self) -> Self {
        let Some(next) = self.next_checked() else {
            return Self::MAX;
        };

        next
    }

    pub const fn next_checked(self) -> Option<Self> {
        let repr = self as u8 + 1;
        Self::from_repr(repr)
    }

    const fn from_days_since_year_start_impl(mut days: u16, leapness: Leapness) -> Self {
        let mut month = Month::Jan;
        loop {
            let last_day = month.last_day(leapness);
            if days < last_day as u16 {
                break;
            }

            days -= last_day as u16;
            month = month.next();
        }

        month
    }

    pub const fn from_days_since_year_start(days: u16, leapness: Leapness) -> Option<Self> {
        use Leapness::*;
        const ORDINARY: [Month; 365] = seq!(N in 0..365 {
            [#(Month::from_days_since_year_start_impl(N, Ordinary),)*]
        });
        const LEAP: [Month; 366] = seq!(N in 0..366 {
            [#(Month::from_days_since_year_start_impl(N, Leap),)*]
        });

        match leapness {
            Leap => {
                if days >= 366 {
                    None
                } else {
                    Some(LEAP[days as usize])
                }
            }
            Ordinary => {
                if days >= 365 {
                    None
                } else {
                    Some(ORDINARY[days as usize])
                }
            }
        }
    }

    pub const fn days_from_year_start(self, leapness: Leapness) -> u16 {
        const fn l(m: Month) -> u16 {
            m.calc_days_from_year_start(Leapness::Leap)
        }

        const fn o(m: Month) -> u16 {
            m.calc_days_from_year_start(Leapness::Ordinary)
        }

        const LEAP: [u16; 12] = seq!(N in 0..12 {
            [#(l(Month::VARIANTS[N]),)*]
        });
        const ORDINARY: [u16; 12] = seq!(N in 0..12 {
            [#(o(Month::VARIANTS[N]),)*]
        });

        match leapness {
            Leapness::Leap => LEAP[self as usize],
            Leapness::Ordinary => ORDINARY[self as usize],
        }
    }

    const fn calc_days_from_year_start(self, leapness: Leapness) -> u16 {
        let mut cur = Self::Jan;
        let mut days = 0_u16;

        // Sigh, recursion would be better.
        while cur as u8 != self as u8 {
            let last_day = cur.last_day(leapness);
            days += last_day as u16;
            cur = cur.next();
        }

        days
    }

    pub const fn last_day_naive(self) -> Day {
        self.last_day(Leapness::Ordinary)
    }

    /// Get last day of month with respect to the leapness of year.
    pub const fn last_day(self, leapness: Leapness) -> Day {
        use Day::*;
        const LAST_DAY: [Day; 12] = [
            POS31, POS28, POS31, POS30, POS31, POS30, POS31, POS31, POS30, POS31, POS30, POS31,
        ];
        const LAST_LEAP_DAY: [Day; 12] = {
            let mut src = LAST_DAY;
            src[1] = POS29;
            src
        };
        const TBL: [[Day; 12]; 2] = [LAST_DAY, LAST_LEAP_DAY];

        TBL[leapness.is_leap() as usize][self as usize]
    }

    /// # Safety
    /// The `repr` must be a valid `Month` discriminant (0..=11).
    pub const unsafe fn from_repr_unchecked(repr: u8) -> Self {
        unsafe { mem::transmute::<u8, Self>(repr) }
    }

    pub const fn from_repr(repr: u8) -> Option<Self> {
        match repr {
            0..12 => Some(unsafe { Self::from_repr_unchecked(repr) }),
            _ => None,
        }
    }

    /// Convert month to fixed size string.
    pub const fn to_str(self) -> str::MonthStr {
        let v = self as u8 + 1;
        unsafe { mem::transmute::<[u8; 2], str::MonthStr>([v / 10 + b'0', v % 10 + b'0']) }
    }
}
