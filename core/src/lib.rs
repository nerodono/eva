//! Eva foundational types and utilities.
extern crate self as eva_core;
pub use bytes;
pub use bytesize;
pub use schemars;
pub use url;

pub use eva_core_macros::auto_impl;
pub use eva_data::{data, int, push_ascii_pat, single_ascii_char, str, zst_error};
pub use seq_macro::seq;

pub use perfect_derive::perfect_derive;
pub use trait_set::trait_set;

pub mod array;
pub mod error;
pub mod utils;

pub mod sync;

pub mod fut;

pub mod time;

pub mod bool;
pub mod generic;
pub use bool::{False, True};

pub mod rand;

pub mod collections;

pub mod hash;

pub use either::{self, Either, Left, Right};
pub use paste::paste;

#[doc(hidden)]
pub mod _priv;
