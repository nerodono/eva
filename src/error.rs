//! # Error utilities.

use std::fmt;

mod seal {
    pub trait Seal {}
}

#[doc(inline)]
pub use crate::_combined as combined;

#[macro_export]
#[doc(hidden)]
macro_rules! _combined {
    ($($(#[$outer_meta:meta])* $vis:vis enum $name:ident {
        $( $VarName:ident($ty:ty) ),* $(,)?
    })*) => {$(
        $(#[$outer_meta])*
        $vis enum $name {$(
            $VarName($ty)
        ),*}

        impl $name {
            pub fn transmogrify<T>(self) -> T
            where
                T: $crate::generic::Anything $(+ std::convert::From<$ty>)*
            {
                match self {$(
                    Self::$VarName(v) => v.into()
                ),*}
            }
        }

        $(
            impl std::convert::From<$ty> for $name {
                fn from(v: $ty) -> Self {
                    Self::$VarName(v)
                }
            }
        )*
    )*};
}

/// Indicate that error is highly unlikely.
#[track_caller]
pub const fn shit_happens() -> ! {
    panic!("shit happens")
}

pub trait ShitHappens<T>: seal::Seal {
    /// Same as [`shit_happens`], but for unwrapping errors.
    fn shit_happens(self) -> T;
}

impl<O> seal::Seal for Option<O> {}
impl<T> ShitHappens<T> for Option<T> {
    #[track_caller]
    fn shit_happens(self) -> T {
        self.unwrap_or_else(|| shit_happens())
    }
}

impl<O, E> seal::Seal for Result<O, E> {}
impl<O, E: fmt::Debug> ShitHappens<O> for Result<O, E> {
    #[track_caller]
    fn shit_happens(self) -> O {
        self.expect("shit happens")
    }
}
