use crate::data;

pub trait Anything {}
impl<T: ?Sized> Anything for T {}

// TODO: include all possible ranges.
#[data(
    copy,
    error,
    display("integer out of range"),
    crate = crate
)]
pub struct OutOfRange;

pub use eva_data::Case;
