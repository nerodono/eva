//! `v | a..b | a..=b`: parsed as written, then checked against the repr and merged.

use std::fmt;

use proc_macro2::{Span, TokenStream};
use quote::{ToTokens, quote};
use syn::{
    Token,
    parse::{Parse, ParseStream},
};

use super::repr::Repr;
use crate::utils::{self, Errors};

/// Every value becomes a variant, and type-checking the generated arrays grows superlinearly.
pub const MAX_VALUES: u128 = 4096;

/// A range as written: `start`, `start..end` or `start..=end`.
pub struct Written {
    start: Literal,
    end: Option<(syn::RangeLimits, Literal)>,
}

/// Parses `range | range | ...`.
pub fn parse(input: ParseStream) -> syn::Result<Vec<Written>> {
    let mut ranges = vec![input.parse()?];
    while let Some(bar) = input.parse::<Option<Token![|]>>()? {
        if input.is_empty() {
            return Err(syn::Error::new_spanned(bar, "expected a range after `|`"));
        }
        ranges.push(input.parse()?);
    }

    let literal_follows = input.peek2(syn::LitInt)
        || input.peek2(syn::LitByte)
        || input.peek2(syn::LitChar)
        || input.peek2(Token![-]);
    if input.peek(Token![,]) && literal_follows {
        return Err(syn::Error::new(input.span(), "separate ranges with `|`"));
    }
    Ok(ranges)
}

impl Parse for Written {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let start = input.parse()?;
        if !input.peek(Token![..]) {
            return Ok(Self { start, end: None });
        }

        let limits: syn::RangeLimits = input.parse()?;
        if input.is_empty() || input.peek(Token![|]) || input.peek(Token![,]) {
            return Err(syn::Error::new_spanned(
                limits,
                "expected the end of the range",
            ));
        }
        let end = input.parse()?;
        Ok(Self {
            start,
            end: Some((limits, end)),
        })
    }
}

impl ToTokens for Written {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.start.to_tokens(tokens);
        if let Some((limits, end)) = &self.end {
            limits.to_tokens(tokens);
            end.to_tokens(tokens);
        }
    }
}

impl Written {
    fn resolve(&self, repr: Repr) -> syn::Result<Range> {
        let Some((limits, end)) = &self.end else {
            let value = self.start.value(repr)?;
            return Ok(Range {
                start: value,
                end: value,
            });
        };

        let (start, end) = utils::both(self.start.value(repr), end.value(repr))?;
        let end = match limits {
            syn::RangeLimits::Closed(_) => Some(end),
            syn::RangeLimits::HalfOpen(_) => end.checked_sub(1),
        };
        match end {
            Some(end) if start <= end => Ok(Range { start, end }),
            _ => Err(syn::Error::new_spanned(self, "empty range")),
        }
    }
}

/// An integer, byte or char literal, with the sign apart because syn spans a negative literal at
/// its `-` alone.
struct Literal {
    minus: Option<Token![-]>,
    lit: syn::Lit,
}

impl Parse for Literal {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let expected = "expected an integer, byte or char literal";
        let minus: Option<Token![-]> = input.parse()?;
        match input.parse::<syn::Lit>() {
            Ok(lit @ syn::Lit::Int(_)) => Ok(Self { minus, lit }),
            Ok(lit @ (syn::Lit::Byte(_) | syn::Lit::Char(_))) => match minus {
                None => Ok(Self { minus, lit }),
                Some(minus) => Err(syn::Error::new_spanned(
                    quote!(#minus #lit),
                    "byte and char literals cannot be negative",
                )),
            },
            Ok(lit) => Err(syn::Error::new(lit.span(), expected)),
            Err(e) => Err(syn::Error::new(e.span(), expected)),
        }
    }
}

impl ToTokens for Literal {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.minus.to_tokens(tokens);
        self.lit.to_tokens(tokens);
    }
}

impl Literal {
    /// The value, which must fit `repr`.
    fn value(&self, repr: Repr) -> syn::Result<i128> {
        let error = |msg: String| syn::Error::new_spanned(self, msg);
        let (value, suffix) = match &self.lit {
            syn::Lit::Int(int) => {
                let suffix = int.suffix();
                if !suffix.is_empty() && suffix != repr.name() {
                    return Err(error(format!(
                        "suffix `{suffix}` does not match the repr `{repr}`"
                    )));
                }
                let sign = if self.minus.is_some() { "-" } else { "" };
                let text = format!("{sign}{}", int.base10_digits());
                let value = text
                    .parse::<i128>()
                    .map_err(|_| error(repr.out_of_range(&text)))?;
                (value, "")
            }
            syn::Lit::Byte(byte) => (byte.value().into(), byte.suffix()),
            syn::Lit::Char(ch) => (u32::from(ch.value()).into(), ch.suffix()),
            _ => unreachable!("`Literal` parses only integer, byte and char literals"),
        };

        if !suffix.is_empty() {
            return Err(error(format!(
                "unexpected suffix `{suffix}`; byte and char literals take none"
            )));
        }
        let (min, max) = repr.bounds();
        if !(min..=max).contains(&value) {
            let shown = match &self.lit {
                syn::Lit::Int(_) => value.to_string(),
                lit => format!("{} ({value})", lit.to_token_stream()),
            };
            return Err(error(repr.out_of_range(shown)));
        }
        Ok(value)
    }
}

/// An inclusive range of values.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct Range {
    pub start: i128,
    pub end: i128,
}

impl Range {
    /// `start` or `start..=end`, as a pattern or expression of any integer type.
    pub fn pattern(self) -> TokenStream {
        let start = literal(self.start);
        if self.start == self.end {
            quote!(#start)
        } else {
            let end = literal(self.end);
            quote!(#start..=#end)
        }
    }
}

impl fmt::Display for Range {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.start == self.end {
            write!(f, "{}", self.start)
        } else {
            write!(f, "{}..={}", self.start, self.end)
        }
    }
}

/// An unsuffixed literal, typed by its context.
pub fn literal(value: i128) -> syn::LitInt {
    syn::LitInt::new(&value.to_string(), Span::call_site())
}

/// Disjoint, non-adjacent ranges in ascending order.
#[derive(Debug, PartialEq)]
pub struct Ranges(Vec<Range>);

impl Ranges {
    /// Checks every written value against `repr`, then merges the ranges.
    pub fn resolve(written: &[Written], repr: Repr) -> syn::Result<Self> {
        let mut errors = Errors::default();
        let ranges = written
            .iter()
            .filter_map(|range| errors.ok(range.resolve(repr)))
            .collect();
        errors.finish()?;

        let merged = Self::merge(ranges);
        match merged.len() {
            Some(len) if len <= MAX_VALUES => Ok(merged),
            len => {
                let len = len.map_or_else(|| "2^128".to_owned(), |len| len.to_string());
                let msg = format!("{len} values; #[int] supports at most {MAX_VALUES}");
                let (first, last) = (written.first(), written.last());
                Err(syn::Error::new_spanned(quote!(#first #last), msg))
            }
        }
    }

    fn merge(mut ranges: Vec<Range>) -> Self {
        ranges.sort_unstable();
        let mut merged = Vec::<Range>::with_capacity(ranges.len());
        for range in ranges {
            match merged.last_mut() {
                Some(last) if range.start <= last.end.saturating_add(1) => {
                    last.end = last.end.max(range.end);
                }
                _ => merged.push(range),
            }
        }
        Self(merged)
    }

    /// The number of values, or `None` for all 2^128 of them.
    fn len(&self) -> Option<u128> {
        self.0.iter().try_fold(0_u128, |sum, range| {
            let len = range.end.abs_diff(range.start).checked_add(1)?;
            sum.checked_add(len)
        })
    }

    pub fn iter(&self) -> impl Iterator<Item = Range> + '_ {
        self.0.iter().copied()
    }

    pub fn values(&self) -> impl Iterator<Item = i128> + '_ {
        self.iter().flat_map(|range| range.start..=range.end)
    }
}

/// `1..=3 | 7`
impl fmt::Display for Ranges {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, range) in self.0.iter().enumerate() {
            if i > 0 {
                f.write_str(" | ")?;
            }
            range.fmt(f)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse::Parser as _;

    fn resolve(repr: Repr, tokens: TokenStream) -> syn::Result<Ranges> {
        let written = parse.parse2(tokens)?;
        Ranges::resolve(&written, repr)
    }

    fn ranges(pairs: &[(i128, i128)]) -> Ranges {
        Ranges(
            pairs
                .iter()
                .map(|&(start, end)| Range { start, end })
                .collect(),
        )
    }

    fn messages(repr: Repr, tokens: TokenStream) -> Vec<String> {
        let err = resolve(repr, tokens).unwrap_err();
        err.into_iter().map(|e| e.to_string()).collect()
    }

    #[test]
    fn merges_out_of_order_overlapping_and_adjacent() {
        let merged = resolve(Repr::U8, quote!(b'z' | 7..9 | 1..=3 | 2 | 4 | 10)).unwrap();
        assert_eq!(merged, ranges(&[(1, 4), (7, 8), (10, 10), (122, 122)]));
        assert_eq!(merged.to_string(), "1..=4 | 7..=8 | 10 | 122");
        assert_eq!(merged.len(), Some(8));
    }

    #[test]
    fn merges_at_the_i128_edges() {
        let max = i128::MAX;
        let merged = Ranges::merge(ranges(&[(max, max), (max - 1, max), (max, max)]).0);
        assert_eq!(merged, ranges(&[(max - 1, max)]));

        let full = Ranges::merge(ranges(&[(0, max), (i128::MIN, -1)]).0);
        assert_eq!(full, ranges(&[(i128::MIN, max)]));
        assert_eq!(full.len(), None);
    }

    #[test]
    fn negative_and_suffixed_values() {
        let merged = resolve(Repr::I8, quote!(-128..=-127 | 5i8 | 127)).unwrap();
        assert_eq!(merged, ranges(&[(-128, -127), (5, 5), (127, 127)]));
    }

    #[test]
    fn every_bad_literal_is_reported() {
        assert_eq!(
            messages(Repr::U8, quote!(300 | -1 | 'я' | 5u16 | b'a'x | 3..=1)),
            [
                "300 is out of range for u8 (0..=255)",
                "-1 is out of range for u8 (0..=255)",
                "'я' (1103) is out of range for u8 (0..=255)",
                "suffix `u16` does not match the repr `u8`",
                "unexpected suffix `x`; byte and char literals take none",
                "empty range",
            ]
        );
    }

    #[test]
    fn u128_stops_at_i128_max() {
        assert_eq!(
            messages(Repr::U128, quote!(170141183460469231731687303715884105728)),
            ["170141183460469231731687303715884105728 is out of range for u128 (0..=i128::MAX)"]
        );
    }

    #[test]
    fn exclusive_end_at_min_is_empty() {
        assert_eq!(messages(Repr::I8, quote!(-128..-128)), ["empty range"]);
    }

    #[test]
    fn too_many_values() {
        assert_eq!(
            messages(Repr::U16, quote!(0..=4000 | 4001..=4096)),
            ["4097 values; #[int] supports at most 4096"]
        );
        assert_eq!(
            messages(
                Repr::I128,
                quote!(
                    -170141183460469231731687303715884105728
                        ..=170141183460469231731687303715884105727
                )
            ),
            ["2^128 values; #[int] supports at most 4096"]
        );
    }

    #[test]
    fn comma_between_ranges() {
        let err = parse.parse2(quote!(0..=3, 5..=6)).map(|_| ()).unwrap_err();
        assert_eq!(err.to_string(), "separate ranges with `|`");
        let err = parse
            .parse2(quote!(0..=3, crate = crate))
            .map(|_| ())
            .unwrap_err();
        assert_eq!(err.to_string(), "unexpected token");
    }
}
