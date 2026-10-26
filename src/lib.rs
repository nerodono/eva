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

pub use bytes;
pub use bytesize;
pub use schemars;
pub use url;

pub use eva_macros::{auto_impl, data, endpoint, export, int, service, str};
pub use seq_macro::seq;

pub use perfect_derive::perfect_derive;
pub use trait_set::trait_set;

#[cfg(feature = "cli")]
pub mod cli;

pub mod logging;

pub mod array;
pub mod error;
pub mod utils;

pub mod sync;

pub mod fut;
#[cfg(feature = "tokio")]
pub mod supervisor;
pub mod time;

pub mod bool;
pub mod generic;
pub mod int;
pub use bool::{False, True};
pub mod str;

pub mod rand;

pub mod collections;
pub mod handling;
pub mod hash;

pub mod component_configs;
pub mod signal;

pub use either::{self, Either, Left, Right};
pub use paste::paste;

#[doc(hidden)]
pub mod _priv;
