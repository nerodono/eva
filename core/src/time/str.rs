//! # Strings that represent time

use crate::{
    int, str,
    str::{ParseError, Seq, ascii},
};

use super::{
    Day, Hours, LooseDate, Mins, Month, PreciseTime, Secs, SecsTime, Time, Timestamp, Year,
    time::SubsecNanos, timestamp::LooseTimestamp,
};

// == TimestampStr ==

/// Precise string timestamp representation.
///
/// format: `dd.mm.YYYY HH:MM:SS.NNNNNNNNN`.
#[str(fixed(error = ParseError), crate = crate)]
pub struct TimestampStr(DateStr, ascii::Space, PreciseTimeStr);

impl TimestampStr {
    pub const fn new(date: DateStr, time: PreciseTimeStr) -> Self {
        Self(date, ascii::Space::POS32, time)
    }

    pub const fn parse_compact(self) -> Option<Timestamp> {
        let Some(loose) = self.parse() else {
            return None;
        };
        loose.compact_loose().compact()
    }

    pub const fn parse(self) -> Option<LooseTimestamp<LooseDate>> {
        let Some(date) = self.0.parse() else {
            return None;
        };
        let Some(time) = self.2.parse() else {
            return None;
        };

        Some(LooseTimestamp::<_>::new(date, time))
    }
}

// == PreciseTimeStr ==

/// [`TimeStr`] with seconds an nanoseconds.
///
/// format: `HH:MM:SS.NNNNNNNNN`, where N is nanosecond digit.
#[str(fixed(error = ParseError), crate = crate)]
pub struct PreciseTimeStr(SecsTimeStr, ascii::Dot, SubsecNanosStr);

impl PreciseTimeStr {
    pub const fn new(secs_time: SecsTimeStr, subsec_nanos: SubsecNanosStr) -> Self {
        Self(secs_time, ascii::Dot::POS46, subsec_nanos)
    }

    pub const fn parse(self) -> Option<PreciseTime> {
        let Some(time) = self.0.parse() else {
            return None;
        };

        let nanos = self.2.parse();

        Some(PreciseTime::new(time, nanos))
    }
}

// == SecsTimeStr ==

/// [`TimeStr`] with seconds part.
///
/// format: `HH:MM:SS`.
#[str(fixed(error = ParseError), crate = crate)]
pub struct SecsTimeStr(TimeStr, ascii::Colon, SecsStr);

impl SecsTimeStr {
    pub const fn new(time: TimeStr, secs: SecsStr) -> Self {
        Self(time, ascii::Colon::POS58, secs)
    }

    pub const fn parse(self) -> Option<SecsTime> {
        let Some(time) = self.0.parse() else {
            return None;
        };
        let secs = self.2.parse();

        Some(SecsTime::new(time, secs))
    }
}

// == DateStr ==

#[str(fixed(error = ParseError), crate = crate)]
pub struct DateStr(DayStr, ascii::Dot, MonthStr, ascii::Dot, YearStr);

impl DateStr {
    pub const fn new(day: DayStr, month: MonthStr, year: YearStr) -> Self {
        Self(day, ascii::Dot::POS46, month, ascii::Dot::POS46, year)
    }

    pub const fn parse(self) -> Option<LooseDate> {
        let Some(day) = self.0.parse() else {
            return None;
        };
        let Some(month) = self.2.parse() else {
            return None;
        };
        let Some(year) = self.4.parse() else {
            return None;
        };

        LooseDate::from_dmy(day, month, year)
    }
}

// == TimeStr ==

/// Partly valid time string. Contains hours and minutes.
///
/// Format: `HH:MM`.
#[str(fixed(error = ParseError), crate = crate)]
pub struct TimeStr(HoursStr, ascii::Colon, MinsStr);

impl TimeStr {
    pub const fn new(hours: HoursStr, minutes: MinsStr) -> Self {
        Self(hours, ascii::Colon::POS58, minutes)
    }

    pub const fn parse(self) -> Option<Time> {
        let Some(hours) = self.0.parse() else {
            return None;
        };
        let mins = self.2.parse();

        Some(Time::new(hours, mins))
    }
}

// == YearStr ==

#[str(fixed(error = ParseError), crate = crate)]
pub struct YearStr(FirstYearChar, Seq<3, ascii::Digit>);

impl YearStr {
    pub const fn parse(self) -> Option<Year> {
        let mut num = digit(self.0 as u8) as u16 * 1000;
        let i = self.1.0;
        num += digit(i[0] as u8) as u16 * 100;
        num += digit(i[1] as u8) as u16 * 10;
        num += digit(i[2] as u8) as u16;

        let Some(relative) = num.checked_sub(1970) else {
            return None;
        };

        Year::new(relative as u8)
    }
}

#[int(u8, b'1' | b'2', crate = crate)]
enum FirstYearChar {}
ascii::valid!(FirstYearChar);

// == MonthStr ==

#[str(fixed(error = ParseError), crate = crate)]
pub struct MonthStr(FirstMonthChar, ascii::Digit);

impl MonthStr {
    pub const fn parse(self) -> Option<Month> {
        let num = digit(self.0 as u8) * 10 + digit(self.1 as u8);
        let Some(index) = num.checked_sub(1) else {
            return None;
        };

        Month::from_repr(index)
    }
}

#[int(u8, b'0'..=b'1', crate = crate)]
enum FirstMonthChar {}
ascii::valid!(FirstMonthChar);

// == DayStr ==

#[str(fixed(error = ParseError), crate = crate)]
pub struct DayStr(FirstDayChar, ascii::Digit);

impl DayStr {
    pub const fn parse(self) -> Option<Day> {
        let num = digit(self.0 as u8) * 10 + digit(self.1 as u8);
        Day::new(num)
    }
}

#[int(u8, b'0'..=b'3', crate = crate)]
enum FirstDayChar {}
ascii::valid!(FirstDayChar);

// == SubsecNanosStr ==

#[str(fixed(error = ParseError), crate = crate)]
pub struct SubsecNanosStr(Seq<9, ascii::Digit>);

impl SubsecNanosStr {
    pub const fn parse(self) -> SubsecNanos {
        let i = self.0.0;
        let mut n = 0;

        n += digit(i[8] as u8) as u32 * 100_000_000;
        n += digit(i[7] as u8) as u32 * 10_000_000;
        n += digit(i[6] as u8) as u32 * 1_000_000;
        n += digit(i[5] as u8) as u32 * 100_000;
        n += digit(i[4] as u8) as u32 * 10_000;
        n += digit(i[3] as u8) as u32 * 1_000;
        n += digit(i[2] as u8) as u32 * 1_00;
        n += digit(i[1] as u8) as u32 * 10;
        n += digit(i[0] as u8) as u32;

        unsafe { SubsecNanos::new_unchecked(n) }
    }
}

// == SecsStr ==

#[str(fixed(error = ParseError), crate = crate)]
pub struct SecsStr(FirstSecsChar, ascii::Digit);

impl SecsStr {
    pub const fn parse(self) -> Secs {
        let num = digit(self.0 as u8) * 10 + digit(self.1 as u8);
        unsafe { Secs::new_unchecked(num) }
    }
}

#[int(u8, b'0'..=b'5', crate = crate)]
enum FirstSecsChar {}
ascii::valid!(FirstSecsChar);

// == HoursStr ==

/// Number of hours, two ascii digits.
#[str(fixed(error = ParseError), crate = crate)]
pub struct HoursStr(FirstHoursChar, ascii::Digit);

impl HoursStr {
    pub const fn parse(self) -> Option<Hours> {
        let num = digit(self.0 as u8) * 10 + digit(self.1 as u8);
        Hours::new(num)
    }
}

#[int(u8, b'0' | b'1' | b'2', crate = crate)]
enum FirstHoursChar {}
ascii::valid!(FirstHoursChar);

// == MinsStr ==

/// Number of minutes, two ascii digits.
#[str(fixed(error = ParseError), crate = crate)]
pub struct MinsStr(FirstMinsChar, ascii::Digit);

impl MinsStr {
    pub const fn parse(self) -> Mins {
        unsafe { Mins::new_unchecked(digit(self.0 as u8) * 10 + digit(self.1 as u8)) }
    }
}

#[int(u8, b'0'..=b'5', crate = crate)]
enum FirstMinsChar {}
ascii::valid!(FirstMinsChar);

// == Utils ==

const fn digit(n: u8) -> u8 {
    n - b'0'
}
