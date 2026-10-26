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

/// Text case.
#[data(copy, crate = crate, display(name))]
pub enum Case {
    Snake,
    Pascal,
    Kebab,
    Camel,
}
