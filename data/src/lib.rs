//! Data definitions shared by Eva's independent subsystems.
extern crate self as eva_data;
pub use eva_data_macros::{data, int, str};
pub use schemars;
mod error;
mod from_str;
pub mod int;
mod rand;
pub mod str;
/// Text case.
#[data(copy, crate = crate, display(name))]
pub enum Case {
    Snake,
    Pascal,
    Kebab,
    Camel,
}

#[macro_export]
macro_rules! zst_error {
    ($($tt:tt)*) => {{
        #[$crate::data(
            copy,
            crate = $crate,
            not(serde, schemars),
            display($($tt)*),
        )]
        struct E;

        E
    }};
}

#[doc(hidden)]
pub mod _priv {
    pub use crate as data;
    pub use crate::__with_rand as with_rand;
    pub use crate::{Case, error::*, from_str::*};
    #[cfg(feature = "rand")]
    pub use ::rand;
    pub use eva_data_macros::RastGawno;
    pub use schemars;
    pub use serde;
}
