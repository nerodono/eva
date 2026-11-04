use std::{cmp::Ordering, fmt};

use proc_macro2::{Span, TokenStream};
use quote::{ToTokens, quote};
use syn::parse::{Parse, ParseStream};

/// The integer type behind an `#[int]` enum.
#[derive(Clone, Copy)]
pub enum Repr {
    U8,
    U16,
    U32,
    U64,
    U128,
    I8,
    I16,
    I32,
    I64,
    I128,
}

impl Repr {
    const ALL: [Self; 10] = [
        Self::U8,
        Self::U16,
        Self::U32,
        Self::U64,
        Self::U128,
        Self::I8,
        Self::I16,
        Self::I32,
        Self::I64,
        Self::I128,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::U8 => "u8",
            Self::U16 => "u16",
            Self::U32 => "u32",
            Self::U64 => "u64",
            Self::U128 => "u128",
            Self::I8 => "i8",
            Self::I16 => "i16",
            Self::I32 => "i32",
            Self::I64 => "i64",
            Self::I128 => "i128",
        }
    }

    /// Inclusive bounds; `u128` stops at `i128::MAX` because values are handled as `i128`.
    pub fn bounds(self) -> (i128, i128) {
        match self {
            Self::U8 => (0, u8::MAX.into()),
            Self::U16 => (0, u16::MAX.into()),
            Self::U32 => (0, u32::MAX.into()),
            Self::U64 => (0, u64::MAX.into()),
            Self::U128 => (0, i128::MAX),
            Self::I8 => (i8::MIN.into(), i8::MAX.into()),
            Self::I16 => (i16::MIN.into(), i16::MAX.into()),
            Self::I32 => (i32::MIN.into(), i32::MAX.into()),
            Self::I64 => (i64::MIN.into(), i64::MAX.into()),
            Self::I128 => (i128::MIN, i128::MAX),
        }
    }

    /// For `#[repr(...)]`, which takes the bare name.
    pub fn ident(self) -> syn::Ident {
        syn::Ident::new(self.name(), Span::call_site())
    }

    pub fn out_of_range(self, value: impl fmt::Display) -> String {
        let (min, max) = self.bounds();
        let range = match self {
            Self::U128 => "0..=i128::MAX".to_owned(),
            _ => format!("{min}..={max}"),
        };
        format!("{value} is out of range for {self} ({range})")
    }

    /// `value` as a JSON schema bound typed as the repr, or as an `f64` rounded away from the
    /// range when it is outside `i64` and `u64`, which serde_json rejects at runtime.
    pub fn schema_bound(self, value: i128, bound: Bound) -> TokenStream {
        if i64::try_from(value).is_ok() || u64::try_from(value).is_ok() {
            return syn::LitInt::new(&format!("{value}{self}"), Span::call_site())
                .into_token_stream();
        }
        let float = value as f64;
        // `as i128` saturates, but the only float here above `i128::MAX` is `i128::MAX as f64`.
        let rounded = if float >= i128::MAX as f64 {
            Ordering::Greater
        } else {
            (float as i128).cmp(&value)
        };
        let float = match (bound, rounded) {
            (Bound::Minimum, Ordering::Greater) => float.next_down(),
            (Bound::Maximum, Ordering::Less) => float.next_up(),
            _ => float,
        };
        syn::LitFloat::new(&format!("{float:?}f64"), Span::call_site()).into_token_stream()
    }
}

#[derive(Clone, Copy)]
pub enum Bound {
    Minimum,
    Maximum,
}

impl fmt::Display for Repr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// `::core::primitive::<name>`, so user types named like primitives cannot interfere.
impl ToTokens for Repr {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let ident = self.ident();
        tokens.extend(quote!(::core::primitive::#ident));
    }
}

impl Parse for Repr {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let names = Self::ALL.map(Self::name).join(", ");
        let ident: syn::Ident = input.parse().map_err(|e| {
            syn::Error::new(
                e.span(),
                format!("expected `newtype` or the repr, one of: {names}"),
            )
        })?;
        Self::ALL
            .into_iter()
            .find(|repr| ident == repr.name())
            .ok_or_else(|| {
                let msg =
                    format!("unsupported repr `{ident}`; expected `newtype` or one of: {names}");
                syn::Error::new(ident.span(), msg)
            })
    }
}
