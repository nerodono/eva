#![deny(missing_docs)]
//! The struct survives a rejected signature with its docs and derives.

/// Documented.
#[eva::endpoint]
pub fn Blocking(state: (), args: ()) {}

fn main() {
    let copy = Blocking;
    let _ = (Blocking, copy, format!("{copy:?}"));
}
