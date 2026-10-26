use super::{
    Date, Day, Hours, Leapness, LooseDate, Mins, Month, PreciseTime, Secs, SecsTime, SubsecNanos,
    Time, Timestamp, Year, str,
};

#[track_caller]
fn each_date(mut f: impl FnMut(LooseDate)) {
    for year in Year::VARIANTS {
        for month in Month::VARIANTS {
            let last_day = month.last_day(year.leapness());
            for day in Day::VARIANTS.into_iter().take(last_day as usize) {
                f(LooseDate::from_dmy(day, month, year).unwrap());
            }
        }
    }
}

#[track_caller]
#[allow(dead_code)]
fn each_time(mut f: impl FnMut(Time)) {
    for hours in Hours::VARIANTS {
        for mins in Mins::VARIANTS {
            let time = Time::new(hours, mins);
            f(time);
        }
    }
}

mod timestamp {
    use super::*;

    #[cfg(feature = "get_time_test")]
    #[test]
    fn get_time() {
        each_date(|date| {
            let date = date.compact();
            each_time(|time| {
                let ts = date.to_secs() + time.to_secs();
                if ts == 0 {
                    return;
                }
                let ts = Timestamp::from_secs_checked(ts).unwrap();

                let actual_date = ts.date();
                let actual_time = ts.time().secs_time().time();

                assert_eq!(actual_date, Some(date));
                assert_eq!(time, actual_time);
            });
        });
    }

    #[test]
    fn from_date_and_back() {
        each_date(|date| {
            let date = date.compact();
            let secs = date.to_secs();
            let ts = Timestamp::from_secs(secs);

            assert_eq!(ts.date(), Some(date));
        });
    }
}

mod time {
    use super::*;

    #[track_caller]
    fn time(hours: u8, mins: u8) -> Time {
        Time::new(Hours::new(hours).unwrap(), Mins::new(mins).unwrap())
    }

    #[test]
    fn format_precise() {
        #[track_caller]
        fn case(res: &str, t: Time, subsecs: u8, nanos: u32) {
            let expected = PreciseTime::new(
                SecsTime::new(t, Secs::new(subsecs).unwrap()),
                SubsecNanos::new(nanos).unwrap(),
            );
            assert_eq!(expected.to_str().as_str(), res);
        }

        case("15:00:00.000000000", time(15, 00), 0, 0);
        case("15:00:00.133713370", time(15, 00), 0, 133713370);
    }

    #[test]
    fn format_secs() {
        #[track_caller]
        fn case(res: &str, t: Time, subsecs: u8) {
            let expected = SecsTime::new(t, Secs::new(subsecs).unwrap());
            assert_eq!(expected.to_str().as_str(), res);
        }

        case("15:00:00", time(15, 00), 00);
        case("15:15:30", time(15, 15), 30);
    }

    #[test]
    fn format_basic() {
        #[track_caller]
        fn case(res: &str, t: Time) {
            let actual = t.to_str();
            let expected: Time = res.parse().unwrap();

            assert_eq!(expected.to_str(), actual);
        }

        case("15:00", time(15, 00));
        case("23:59", time(23, 59));
        case("00:00", time(00, 00));
    }
}

mod date {
    use super::*;

    #[track_caller]
    fn year(abs: u16) -> Year {
        Year::from_abs(abs).unwrap()
    }

    #[test]
    fn year_parse() {
        let year: str::YearStr = "2025".parse().unwrap();
        let parsed = year.parse().unwrap();

        assert_eq!(parsed.abs(), 2025);
    }

    #[test]
    fn first_day_of_year() {
        for y in Year::VARIANTS {
            let date = y.first_day();

            assert_eq!(date.day_of_month(), 1);
            assert_eq!(date.month(), Month::Jan);
            assert_eq!(date.year(), y);
        }
    }

    #[test]
    fn leapness_works_well() {
        const fn is_leap(y: u16) -> bool {
            y.is_multiple_of(400) || (!y.is_multiple_of(100) && y.is_multiple_of(4))
        }

        for y in Year::VARIANTS {
            let expected = is_leap(y.abs());
            let actual = y.leapness().is_leap();

            assert_eq!(expected, actual, "{y} wrongly calculated leapness");
        }
    }

    #[test]
    fn zero_date() {
        let date = Date::from_days(0);
        let y = year(1970);

        assert_eq!(date.to_str().as_str(), "01.01.1970");
        assert_eq!(y.first_day(), date);
    }

    #[test]
    fn loose_and_compact_identity() {
        for y in Year::VARIANTS {
            let leapness = y.leapness();
            for month in Month::VARIANTS {
                let last_day = month.last_day(leapness);
                for day in Day::VARIANTS
                    .into_iter()
                    .take_while(|d| *d as u8 <= last_day as u8)
                {
                    eprintln!("{}.{}.{}", day, month, y.abs());
                    let dmy = Date::from_dmy(day, month, y).unwrap();
                    assert_eq!(dmy, dmy.loose().compact());
                }
            }
        }
    }

    #[test]
    fn days_from_year_start() {
        // TODO: extend tests.
        assert_eq!(Month::Jan.last_day(Leapness::Ordinary), 31);
        assert_eq!(Month::Feb.days_from_year_start(Leapness::Ordinary), 31);
    }

    // Some problematic cases.

    #[test]
    fn from_dmy_28feb_1970() {
        let y = year(1970);
        let m = Month::Feb;
        let d = Day::new(1).unwrap();

        let date = Date::from_dmy(d, m, y).unwrap();

        assert_eq!(date.year(), y);
        assert_eq!(date.month(), m);
        assert_eq!(date.day_of_month(), d);
    }

    #[test]
    fn from_dmy_1jan_1970() {
        let y = year(1970);
        let m = Month::Jan;
        let d = Day::new(1).unwrap();

        let date = Date::from_dmy(d, m, y).unwrap();

        assert_eq!(date.year(), y);
        assert_eq!(date.month(), m);
        assert_eq!(date.day_of_month(), d);
    }

    #[test]
    fn from_dmy_31dec_1972() {
        let y = year(1972);
        let m = Month::Dec;
        let d = Day::new(31).unwrap();

        {
            let date = Date::from_dmy(Day::first(), Month::Jan, year(1972)).unwrap();
            assert_eq!(date.year().abs(), 1972);
        }

        let date = Date::from_dmy(d, m, y).unwrap();
        assert_eq!(date.year().abs(), y.abs());
        assert_eq!(date.month(), m);
        assert_eq!(date.day_of_month(), d);
    }

    #[test]
    fn from_dmy_31dec_2000() {
        let y = year(2000);
        let m = Month::Dec;
        let d = Day::new(31).unwrap();

        let date = Date::from_dmy(d, m, y).unwrap();

        eprintln!(
            "{}.{}.{} = {}.{}.{}",
            date.day_of_month(),
            date.month(),
            date.year().abs(),
            d,
            m,
            y.abs()
        );

        assert_eq!(date.year(), y);
        assert_eq!(date.month(), m);
        assert_eq!(date.day_of_month(), d);
    }

    #[test]
    fn from_dmy() {
        for y in Year::VARIANTS {
            for month in Month::VARIANTS {
                let last_day = month.last_day(y.leapness());
                for day in Day::VARIANTS
                    .into_iter()
                    .take_while(|d| *d as u8 <= last_day as u8)
                {
                    let date = Date::from_dmy(day, month, y).expect("got invalid date");

                    let real_dom = date.day_of_month();
                    let real_year = date.year();
                    let real_month = date.month();

                    eprintln!(
                        "{real_dom} {real_month} {} = {day}.{month}.{}",
                        real_year.abs(),
                        y.abs()
                    );

                    assert_eq!(real_dom, day);
                    assert_eq!(real_month, month);
                    assert_eq!(real_year.abs(), y.abs());
                }
            }
        }
    }

    // Human-readable formats carry a date string, binary formats carry the
    // raw day count — both directions must agree per format.

    #[test]
    fn serde_human_readable_roundtrip() {
        each_date(|loose| {
            let date = loose.compact();
            let json = serde_json::to_string(&date).unwrap();

            assert_eq!(json, format!("\"{}\"", date.to_str().as_str()));
            let back: Date = serde_json::from_str(&json).unwrap();
            assert_eq!(back, date);
        });
    }

    #[test]
    fn serde_owned_string() {
        each_date(|loose| {
            let date = loose.compact();
            let value = serde_json::to_value(date).unwrap();

            let back: Date = serde_json::from_value(value).unwrap();
            assert_eq!(back, date);
        });

        let escaped: Date = serde_json::from_str(r#""\u0030\u0034.10.2024""#).unwrap();
        assert_eq!(escaped, "04.10.2024".parse().unwrap());
    }

    #[test]
    fn serde_binary_roundtrip() {
        each_date(|loose| {
            let date = loose.compact();
            let bytes = postcard::to_allocvec(&date).unwrap();

            let back: Date = postcard::from_bytes(&bytes).unwrap();
            assert_eq!(back, date);

            let days: u16 = postcard::from_bytes(&bytes).unwrap();
            assert_eq!(days, date.days());
        });
    }

    #[test]
    fn format() {
        use Month::*;

        #[track_caller]
        fn case(fmt: &str, dmy: (u8, Month, u16)) {
            let (d, m, y) = dmy;
            let d = Day::new(d).unwrap();
            let y = year(y);
            let date = Date::from_dmy(d, m, y).unwrap();

            assert_eq!(date.to_str().as_str(), fmt);
        }

        case("01.01.1970", (1, Jan, 1970));
        case("01.01.2024", (1, Jan, 2024));
        case("29.09.2005", (29, Sep, 2005));
    }
}
