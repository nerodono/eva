//! # Timezones
//!
//! Timezone utilities.

use crate::{data, time::Hours};

#[data(ord, copy, crate = crate)]
pub struct FixedOffset(Hours);

impl FixedOffset {
    /// UTC time offset. Basically no offset.
    pub const UTC: Self = Self(Hours::new(0).unwrap());

    /// Make offset from hours.
    pub const fn from_hours(hours: Hours) -> Self {
        Self(hours)
    }

    /// Get offset in hours.
    pub const fn hours(self) -> Hours {
        self.0
    }
}
