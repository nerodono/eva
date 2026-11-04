//! Integer types: the traits behind `#[int]`, which pack a primitive into a checked value.

use core::{fmt, hash::Hash, marker::PhantomData, num::NonZero};

use schemars::JsonSchema;
use serde::{
    Serialize,
    de::{self, DeserializeOwned},
};
use trait_set::trait_set;

mod sealed {
    pub trait Sealed {}
}

trait_set! {
    pub trait IntegerProps = Copy + Eq + Ord + Hash;
}

/// A primitive integer: what `#[int]` values are created from and unpack to.
pub trait Primitive:
    sealed::Sealed
    + IntegerProps
    + fmt::Debug
    + fmt::Display
    + Serialize
    + DeserializeOwned
    + JsonSchema
    + Send
    + Sync
    + 'static
{
    /// serde's `invalid_value` for `self`, which `Unexpected` holds only up to 64 bits.
    #[doc(hidden)]
    fn invalid_value<E: de::Error>(self, expected: &dyn de::Expected) -> E;
}

/// An integer that packs a [`Primitive`]: the primitives themselves, `NonZero<_>` and `#[int]`
/// types. `try_new(x.prim()) == Some(x)` for every `x`.
pub trait Integer: IntegerProps {
    /// The field of a newtype; the type itself otherwise.
    type Inner: Integer<Prim = Self::Prim>;
    type Prim: Primitive;

    /// Packs `prim`, `None` if it is not an allowed value.
    fn try_new(prim: Self::Prim) -> Option<Self>;

    /// Packs `prim` without checking it.
    ///
    /// # Safety
    ///
    /// `prim` must be an allowed value: `try_new(prim)` must be `Some`.
    unsafe fn new_unchecked(prim: Self::Prim) -> Self;

    fn inner(&self) -> Self::Inner;

    fn prim(&self) -> Self::Prim;

    /// Describes the allowed values as [`de::Expected`] does: `a non-zero u16`.
    fn expecting(f: &mut fmt::Formatter<'_>) -> fmt::Result;
}

/// An [`Integer`] that accepts every value of its [`Inner`](Integer::Inner): everything but a
/// newtype with a condition.
pub trait InfallibleInteger: Integer {
    fn new(inner: Self::Inner) -> Self;
}

/// Deserializes `T` through its primitive.
pub fn deserialize<'de, T: Integer, D: de::Deserializer<'de>>(
    deserializer: D,
) -> Result<T, D::Error> {
    let prim = <T::Prim as serde::Deserialize<'de>>::deserialize(deserializer)?;
    T::try_new(prim).ok_or_else(|| invalid_value::<T, D::Error>(prim))
}

/// The error for a `prim` that `T::try_new` rejects: ``invalid value: integer `0`, expected a
/// non-zero u16``.
pub fn invalid_value<T: Integer, E: de::Error>(prim: T::Prim) -> E {
    prim.invalid_value(&Expecting::<T>(PhantomData))
}

/// `Integer::expecting` of a newtype with a condition: the field's text, then the condition after
/// `where`, or after `and` when the field already has one.
#[doc(hidden)]
pub fn expecting_where(
    f: &mut fmt::Formatter<'_>,
    inner: fn(&mut fmt::Formatter<'_>) -> fmt::Result,
    condition: &str,
) -> fmt::Result {
    struct Inner(fn(&mut fmt::Formatter<'_>) -> fmt::Result);

    impl fmt::Display for Inner {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            (self.0)(f)
        }
    }

    let inner = Inner(inner).to_string();
    let joiner = if inner.contains(" where ") {
        " and "
    } else {
        " where "
    };
    write!(f, "{inner}{joiner}{condition}")
}

struct Expecting<T>(PhantomData<T>);

impl<T: Integer> de::Expected for Expecting<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        T::expecting(f)
    }
}

/// `Integer` and `InfallibleInteger` for the primitive itself and for its `NonZero`.
macro_rules! integers {
    ($t:ident) => {
        impl Integer for $t {
            type Inner = Self;
            type Prim = Self;

            #[inline]
            fn try_new(prim: $t) -> Option<Self> {
                Some(prim)
            }

            #[inline]
            unsafe fn new_unchecked(prim: $t) -> Self {
                prim
            }

            #[inline]
            fn inner(&self) -> Self {
                *self
            }

            #[inline]
            fn prim(&self) -> Self {
                *self
            }

            fn expecting(f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(stringify!($t))
            }
        }

        impl InfallibleInteger for $t {
            #[inline]
            fn new(inner: Self) -> Self {
                inner
            }
        }

        impl Integer for NonZero<$t> {
            type Inner = Self;
            type Prim = $t;

            #[inline]
            fn try_new(prim: $t) -> Option<Self> {
                NonZero::new(prim)
            }

            #[inline]
            unsafe fn new_unchecked(prim: $t) -> Self {
                unsafe { NonZero::new_unchecked(prim) }
            }

            #[inline]
            fn inner(&self) -> Self {
                *self
            }

            #[inline]
            fn prim(&self) -> $t {
                NonZero::get(*self)
            }

            fn expecting(f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(concat!("a non-zero ", stringify!($t)))
            }
        }

        impl InfallibleInteger for NonZero<$t> {
            #[inline]
            fn new(inner: Self) -> Self {
                inner
            }
        }
    };
}

macro_rules! primitives {
    ($($t:ident => $unexpected:ident as $wide:ty),* $(,)?) => {$(
        impl sealed::Sealed for $t {}

        impl Primitive for $t {
            fn invalid_value<E: de::Error>(self, expected: &dyn de::Expected) -> E {
                E::invalid_value(de::Unexpected::$unexpected(self as $wide), expected)
            }
        }

        integers!($t);
    )*};
}

macro_rules! wide_primitives {
    ($($t:ident),* $(,)?) => {$(
        impl sealed::Sealed for $t {}

        impl Primitive for $t {
            fn invalid_value<E: de::Error>(self, expected: &dyn de::Expected) -> E {
                E::custom(format_args!("invalid value: integer `{self}`, expected {expected}"))
            }
        }

        integers!($t);
    )*};
}

primitives! {
    u8 => Unsigned as u64,
    u16 => Unsigned as u64,
    u32 => Unsigned as u64,
    u64 => Unsigned as u64,
    usize => Unsigned as u64,
    i8 => Signed as i64,
    i16 => Signed as i64,
    i32 => Signed as i64,
    i64 => Signed as i64,
    isize => Signed as i64,
}

wide_primitives!(u128, i128);

#[cfg(test)]
mod tests {
    use core::num::{NonZeroI8, NonZeroU16, NonZeroU128};

    use serde_json::{Value, json};

    use super::*;

    fn from_json<T: Integer>(value: Value) -> Result<T, serde_json::Error> {
        deserialize(value)
    }

    #[test]
    fn primitives_pass_through() {
        assert_eq!(<u8 as Integer>::try_new(0), Some(0));
        assert_eq!(<u8 as InfallibleInteger>::new(7), 7);
        assert_eq!(<i128 as Integer>::prim(&-1), -1);
        assert_eq!(from_json::<u8>(json!(7)).unwrap(), 7);
    }

    #[test]
    fn non_zero_packs_and_unpacks() {
        assert_eq!(<NonZeroU16 as Integer>::try_new(0), None);
        assert_eq!(
            <NonZeroU16 as Integer>::try_new(8).map(|x| x.prim()),
            Some(8)
        );
        assert_eq!(
            unsafe { <NonZeroU16 as Integer>::new_unchecked(3) }.get(),
            3
        );
        assert_eq!(from_json::<NonZeroI8>(json!(-3)).unwrap().get(), -3);
    }

    #[test]
    fn rejected_values_name_the_expectation() {
        assert_eq!(
            from_json::<NonZeroU16>(json!(0)).unwrap_err().to_string(),
            "invalid value: integer `0`, expected a non-zero u16"
        );
        assert_eq!(
            from_json::<NonZeroI8>(json!(0)).unwrap_err().to_string(),
            "invalid value: integer `0`, expected a non-zero i8"
        );
        assert_eq!(
            from_json::<NonZeroU128>(json!(0)).unwrap_err().to_string(),
            "invalid value: integer `0`, expected a non-zero u128"
        );
        assert_eq!(
            from_json::<u8>(json!(256)).unwrap_err().to_string(),
            "invalid value: integer `256`, expected u8"
        );
    }
}
