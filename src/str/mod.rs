use std::{fmt, marker::PhantomData, mem::MaybeUninit, slice, str::FromStr};

pub use compact_str::{
    CompactString, CompactStringExt, ToCompactString, ToCompactStringError, format_compact,
};
use perfect_derive::perfect_derive;

use crate::data;

#[macro_export]
macro_rules! single_ascii_char {
    ($ty:ty) => {
        const _: () = {
            use ::core::{result::Result, str::FromStr};
            use ::std::string::String;

            use $crate::str::{HasPattern, ParseError};

            impl HasPattern for $ty {
                #[inline]
                fn pat_into(buf: &mut String) {
                    $crate::push_ascii_pat!(Self, buf);
                }
            }

            impl FromStr for $ty {
                type Err = ParseError;

                #[inline]
                fn from_str(s: &str) -> Result<Self, Self::Err> {
                    let [c]: [u8; 1] = s.as_bytes().try_into().map_err(|_| ParseError::Length)?;
                    Self::new(c).ok_or(ParseError::Char)
                }
            }
        };
    };
}

#[perfect_derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[derive(serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct PhantomDataStr<C>(pub PhantomData<C>);

impl<C> HasPattern for PhantomDataStr<C> {
    #[inline]
    fn pat_into(buf: &mut String) {
        _ = buf;
    }
}

unsafe impl<C> FixedUtf8 for PhantomDataStr<C> {}
impl<C> FromStr for PhantomDataStr<C> {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() {
            Ok(Self(PhantomData))
        } else {
            Err(ParseError::Length)
        }
    }
}

/// Simple parse error.
#[data(copy, error, display(doc), crate = crate)]
pub enum ParseError {
    /// Unexpected char.
    Char,
    /// Invalid length of string.
    Length,
    /// Value is out of the representable range.
    Range,
}

/// Error of parsing a [`FixedUtf8`] string.
pub trait FixedParseError {
    /// The input is not `expected` bytes long.
    fn length(expected: usize) -> Self;

    /// A multibyte char crosses the boundary between two components.
    fn char() -> Self;
}

impl FixedParseError for ParseError {
    #[inline]
    fn length(expected: usize) -> Self {
        _ = expected;
        Self::Length
    }

    #[inline]
    fn char() -> Self {
        Self::Char
    }
}

/// Parses the `T` at the front of `rest` and advances past it; the caller checks the total
/// length first, so a failed split is a char boundary.
#[doc(hidden)]
pub fn parse_fixed_part<T, E>(rest: &mut &str) -> Result<T, E>
where
    T: FixedUtf8,
    E: FixedParseError + From<T::Err>,
{
    let Some((part, tail)) = rest.split_at_checked(size_of::<T>()) else {
        return Err(E::char());
    };
    *rest = tail;
    Ok(part.parse()?)
}

/// `align_of::<T>()`, bounded by `FixedUtf8` so a field that isn't one fails type checking,
/// at the field, before a generated layout assertion evaluates.
#[doc(hidden)]
pub const fn fixed_align<T: FixedUtf8>() -> usize {
    align_of::<T>()
}

pub trait HasPattern {
    fn pat_into(buf: &mut String);
    #[inline]
    fn regex_pat() -> String {
        let mut s = String::new();
        Self::pat_into(&mut s);
        s
    }
    #[inline]
    fn regex_pat_fullmatch() -> String {
        let mut s = String::with_capacity(8);
        s.push('^');
        Self::pat_into(&mut s);
        s.push('$');
        s
    }
}

impl HasPattern for String {
    #[inline]
    fn pat_into(buf: &mut String) {
        buf.push_str(".*");
    }
}

impl HasPattern for CompactString {
    #[inline]
    fn pat_into(buf: &mut String) {
        buf.push_str(".*");
    }
}

/// A string stored inline in exactly `size_of::<Self>()` bytes.
///
/// # Safety
///
/// `Self` has alignment 1, and all bytes of every value are initialized and form valid UTF-8,
/// so [`reinterpret`] can view the value as `&str`.
pub unsafe trait FixedUtf8
where
    Self: Sized + Copy + FromStr<Err: FixedParseError + fmt::Display> + HasPattern,
{
}

/// Reinterpret fixed size string as a standard library string slice.
///
/// A `T` that breaks the alignment rule of [`FixedUtf8`] fails to build:
///
/// ```compile_fail,E0080
/// # use std::str::FromStr;
/// #
/// # use eva::str::{FixedUtf8, HasPattern, ParseError};
/// #
/// #[derive(Clone, Copy)]
/// #[repr(C, align(2))]
/// pub struct Wide([u8; 2]);
///
/// unsafe impl FixedUtf8 for Wide {}
/// #
/// # impl HasPattern for Wide {
/// #     fn pat_into(buf: &mut String) {
/// #         buf.push_str("..");
/// #     }
/// # }
/// #
/// # impl FromStr for Wide {
/// #     type Err = ParseError;
/// #
/// #     fn from_str(s: &str) -> Result<Self, Self::Err> {
/// #         s.as_bytes().try_into().map(Self).map_err(|_| ParseError::Length)
/// #     }
/// # }
///
/// eva::str::reinterpret(&Wide(*b"ab"));
/// ```
pub const fn reinterpret<'a, T: FixedUtf8>(val: &'a T) -> &'a str {
    const {
        assert!(
            align_of::<T>() == 1,
            "`FixedUtf8` types must have alignment 1"
        )
    };
    let ts = size_of::<T>();
    let slice: &'a [u8] = unsafe { slice::from_raw_parts(val as *const T as *const u8, ts) };
    unsafe { std::str::from_utf8_unchecked(slice) }
}

#[derive(Clone, Copy)]
#[repr(C)]
union EitherUnion<L: FixedUtf8, R: FixedUtf8> {
    lhs: L,
    rhs: R,
}

#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct Either<L: FixedUtf8, R: FixedUtf8>(EitherUnion<L, R>);

impl<L: FixedUtf8, R: FixedUtf8> FromStr for Either<L, R> {
    type Err = R::Err;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Ok(res) = L::from_str(s) {
            Ok(Self::left(res))
        } else {
            R::from_str(s).map(Self::right)
        }
    }
}

unsafe impl<L: FixedUtf8, R: FixedUtf8> FixedUtf8 for Either<L, R> {}

impl<L: FixedUtf8, R: FixedUtf8> HasPattern for Either<L, R> {
    fn pat_into(buf: &mut String) {
        buf.push('(');
        L::pat_into(buf);
        buf.push('|');
        R::pat_into(buf);
        buf.push(')');
    }
}

impl<L: FixedUtf8, R: FixedUtf8> Either<L, R> {
    const fn new(un: EitherUnion<L, R>) -> Self {
        const {
            if size_of::<L>() != size_of::<R>() {
                panic!("Could not make string `Either` of differently sized strings");
            }
        }

        Self(un)
    }
    pub const fn left(lhs: L) -> Self {
        Self::new(EitherUnion { lhs })
    }

    pub const fn right(rhs: R) -> Self {
        Self::new(EitherUnion { rhs })
    }

    pub const fn as_str(&self) -> &str {
        reinterpret(self)
    }
}

/// Sequence of fixed size string.
#[crate::str(custom, copy, crate = crate)]
#[repr(transparent)]
pub struct Seq<const N: usize, T: FixedUtf8>(pub [T; N]);

impl<const N: usize, T: FixedUtf8> Seq<N, T> {
    pub const fn as_str(&self) -> &str {
        reinterpret(self)
    }
}

impl<const N: usize, T: FixedUtf8> HasPattern for Seq<N, T> {
    fn pat_into(buf: &mut String) {
        buf.push_str("(?:");
        T::pat_into(buf);
        buf.push_str("){");
        buf.push_str(&N.to_string());
        buf.push('}');
    }
}

unsafe impl<const N: usize, T: FixedUtf8> FixedUtf8 for Seq<N, T> {}

impl<const N: usize, T> FromStr for Seq<N, T>
where
    T: FixedUtf8,
{
    type Err = T::Err;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let expected_size = size_of::<T>() * N;
        if s.len() != expected_size {
            return Err(T::Err::length(expected_size));
        }

        let mut rest = s;
        let mut items = [const { MaybeUninit::<T>::uninit() }; N];
        for item in &mut items {
            item.write(parse_fixed_part::<T, T::Err>(&mut rest)?);
        }

        // SAFETY: the loop initialized every item, and `MaybeUninit<T>` has the layout of `T`.
        Ok(Self(unsafe { (&raw const items).cast::<[T; N]>().read() }))
    }
}

#[cfg(test)]
mod tests;

pub mod ascii;
