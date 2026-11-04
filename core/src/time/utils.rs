pub const fn is_leap_year(year: u16) -> bool {
    let ndiv_by4 = (year & 3) != 0;
    let cycle = ((year & 15) != 0) && year.is_multiple_of(25);

    !(ndiv_by4 || cycle)
}

pub const fn divmod(lhs: u64, rhs: u64) -> (u64, u64) {
    let div = lhs / rhs;
    let mod_ = lhs - (div * rhs);

    (div, mod_)
}

/// Returns number of additional days in `year`.
pub const fn leap_days_after(year: u16) -> u16 {
    let first_criteria = year / 400;

    let every4 = year >> 2;
    let every100 = year / 100;

    let second_criteria = every4 - every100;

    first_criteria + second_criteria
}
