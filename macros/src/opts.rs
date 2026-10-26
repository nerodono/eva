//! Options are collected first and resolved afterwards, so the result never depends on the
//! order the user wrote them in.

use std::any::Any;

use proc_macro2::{Group, Spacing, TokenTree};
use quote::ToTokens as _;
use syn::{
    Token,
    ext::IdentExt as _,
    parse::{Parse, ParseStream, Parser as _},
    token,
};

use crate::utils::Errors;

/// Options accepted by one macro; `not(...)` is accepted when [`Table::not`] is non-empty.
pub struct Table {
    pub keys: &'static [Key],
    pub not: &'static [&'static [&'static str]],
}

impl Table {
    fn shape(&self, key: &syn::Ident) -> Option<Shape> {
        match self.keys.iter().find(|k| key == k.name) {
            Some(k) => Some(k.shape),
            None => (key == "not" && !self.not.is_empty()).then_some(Shape::Not),
        }
    }

    fn names(&self) -> impl Iterator<Item = &'static str> {
        let not = (!self.not.is_empty()).then_some("not");
        self.keys.iter().map(|k| k.name).chain(not)
    }

    fn not_names(&self) -> impl Iterator<Item = &'static str> + Clone {
        self.not.iter().flat_map(|names| names.iter().copied())
    }
}

/// `not(...)` keys of every macro that implements data traits, resolved by [`Opts::common`].
pub const COMMON_NOT: &[&str] = &[
    "serde",
    "Serialize",
    "Deserialize",
    "schemars",
    "JsonSchema",
    "Debug",
];

pub struct Key {
    name: &'static str,
    shape: Shape,
}

#[derive(Clone, Copy)]
enum Shape {
    Flag,
    Value(fn(ParseStream) -> syn::Result<Box<dyn Any>>),
    List,
    Not,
}

impl Key {
    pub const fn flag(name: &'static str) -> Self {
        Self {
            name,
            shape: Shape::Flag,
        }
    }

    /// `name = T`; resolve it with [`Opts::value::<T>`].
    pub const fn value<T: Parse + 'static>(name: &'static str) -> Self {
        Self {
            name,
            shape: Shape::Value(parse_boxed::<T>),
        }
    }

    pub const fn list(name: &'static str) -> Self {
        Self {
            name,
            shape: Shape::List,
        }
    }
}

pub const CRATE: Key = Key::value::<syn::Path>("crate");

fn parse_boxed<T: Parse + 'static>(input: ParseStream) -> syn::Result<Box<dyn Any>> {
    Ok(Box::new(input.parse::<T>()?))
}

enum Arg {
    Flag,
    Value(Box<dyn Any>),
    List(Group),
    Not,
}

/// The [`COMMON_NOT`] traits that are not opted out of.
pub struct CommonTraits {
    pub serialize: bool,
    pub deserialize: bool,
    pub json_schema: bool,
    pub debug: bool,
}

pub struct Opts {
    table: &'static Table,
    found: Vec<(syn::Ident, Arg)>,
    not: Vec<syn::Ident>,
}

impl Opts {
    /// Parses comma-separated options up to the end of `input`.
    pub fn parse(input: ParseStream, table: &'static Table) -> syn::Result<Self> {
        let mut opts = Self {
            table,
            found: Vec::new(),
            not: Vec::new(),
        };
        let mut errors = Errors::default();
        parse_separated(input, &mut errors, |input, errors| {
            opts.parse_option(input, errors)
        })?;

        errors.finish()?;
        Ok(opts)
    }

    /// Parses the options that follow a positional prefix and its comma.
    pub fn parse_after_positional(input: ParseStream, table: &'static Table) -> syn::Result<Self> {
        if !input.is_empty() {
            input.parse::<Token![,]>()?;
        }
        Self::parse(input, table)
    }

    fn parse_option(&mut self, input: ParseStream, errors: &mut Errors) -> syn::Result<()> {
        let key = option_name(input)?;
        let Some(shape) = self.table.shape(&key) else {
            return Err(unknown(&key, self.table.names()));
        };
        if self.found.iter().any(|(found, _)| *found == key) {
            return Err(duplicate(&key));
        }

        let arg = match shape {
            Shape::Flag => {
                if !input.is_empty() && !input.peek(Token![,]) {
                    return Err(input.error(format!("option `{key}` takes no arguments")));
                }
                Arg::Flag
            }
            Shape::Value(parse) => {
                if !input.peek(Token![=]) {
                    let msg = format!("option `{key}` expects a value: `{key} = ...`");
                    return Err(syn::Error::new(key.span(), msg));
                }
                input.parse::<Token![=]>()?;
                Arg::Value(parse(input)?)
            }
            Shape::List | Shape::Not if !input.peek(token::Paren) => {
                let msg = format!("option `{key}` expects arguments: `{key}(...)`");
                return Err(syn::Error::new(key.span(), msg));
            }
            Shape::List => Arg::List(input.parse()?),
            Shape::Not => {
                let content;
                syn::parenthesized!(content in input);
                self.parse_not(&content, errors)?;
                Arg::Not
            }
        };

        self.found.push((key, arg));
        Ok(())
    }

    fn parse_not(&mut self, input: ParseStream, errors: &mut Errors) -> syn::Result<()> {
        parse_separated(input, errors, |input, _| {
            let key = option_name(input)?;
            if !self.table.not_names().any(|name| key == name) {
                return Err(unknown(&key, self.table.not_names()));
            }
            if self.not.contains(&key) {
                return Err(duplicate(&key));
            }
            self.not.push(key);
            Ok(())
        })
    }

    fn get(&self, name: &str, shape: fn(&Shape) -> bool) -> Option<(&syn::Ident, &Arg)> {
        debug_assert!(
            self.table
                .keys
                .iter()
                .any(|k| k.name == name && shape(&k.shape)),
            "option `{name}` is not in the table with this shape",
        );
        self.found
            .iter()
            .find(|(key, _)| key == name)
            .map(|(key, arg)| (key, arg))
    }

    /// The key, if the flag was given.
    pub fn flag(&self, name: &str) -> Option<&syn::Ident> {
        self.get(name, |s| matches!(s, Shape::Flag))
            .map(|(key, _)| key)
    }

    /// The value of `name = ...`; `T` must be the type its [`Key::value`] parses.
    pub fn value<T: Clone + 'static>(&self, name: &str) -> Option<T> {
        let (_, Arg::Value(value)) = self.get(name, |s| matches!(s, Shape::Value(_)))? else {
            unreachable!("option `{name}` is a value");
        };
        let value = value
            .downcast_ref::<T>()
            .unwrap_or_else(|| panic!("option `{name}` is not a `{}`", std::any::type_name::<T>()));
        Some(value.clone())
    }

    /// Parses the arguments of `name(...)`; `parse` must consume all of them.
    pub fn list<T>(
        &self,
        name: &str,
        parse: impl FnOnce(ParseStream) -> syn::Result<T>,
    ) -> syn::Result<Option<T>> {
        let Some((_, Arg::List(group))) = self.get(name, |s| matches!(s, Shape::List)) else {
            return Ok(None);
        };
        let parser = |input: ParseStream| {
            let content;
            syn::parenthesized!(content in input);
            let value = parse(&content)?;
            if !content.is_empty() {
                return Err(content.error("unexpected token"));
            }
            Ok(value)
        };
        parser.parse2(group.to_token_stream()).map(Some)
    }

    /// The key, if it was given inside `not(...)`.
    pub fn not(&self, name: &str) -> Option<&syn::Ident> {
        debug_assert!(
            self.table.not_names().any(|n| n == name),
            "`{name}` is not a `not(...)` key of the table",
        );
        self.not.iter().find(|key| *key == name)
    }

    pub fn common(&self) -> CommonTraits {
        let serde = self.not("serde").is_none();
        CommonTraits {
            serialize: serde && self.not("Serialize").is_none(),
            deserialize: serde && self.not("Deserialize").is_none(),
            json_schema: self.not("schemars").is_none() && self.not("JsonSchema").is_none(),
            debug: self.not("Debug").is_none(),
        }
    }

    /// The `crate = path` option, `::eva` by default.
    pub fn krate(&self) -> syn::Path {
        self.value(CRATE.name)
            .unwrap_or_else(|| syn::parse_quote!(::eva))
    }
}

/// Parses `entry, entry, ...` to the end of `input`; a failed entry is recorded and skipped.
fn parse_separated(
    input: ParseStream,
    errors: &mut Errors,
    mut entry: impl FnMut(ParseStream, &mut Errors) -> syn::Result<()>,
) -> syn::Result<()> {
    while !input.is_empty() {
        let parsed = entry(input, errors).and_then(|()| {
            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
            Ok(())
        });
        if let Err(e) = parsed {
            errors.push(e);
            skip_past_comma(input)?;
        }
    }
    Ok(())
}

fn option_name(input: ParseStream) -> syn::Result<syn::Ident> {
    if !input.peek(syn::Ident::peek_any) {
        return Err(input.error("expected an option name"));
    }
    input.call(syn::Ident::parse_any)
}

fn unknown(key: &syn::Ident, expected: impl Iterator<Item = &'static str>) -> syn::Error {
    let expected = expected.collect::<Vec<_>>().join(", ");
    let msg = format!("unknown option `{key}`; expected one of: {expected}");
    syn::Error::new(key.span(), msg)
}

fn duplicate(key: &syn::Ident) -> syn::Error {
    syn::Error::new(key.span(), format!("duplicate option `{key}`"))
}

/// Skips past the next comma outside `<...>`, which syn does not group like brackets.
fn skip_past_comma(input: ParseStream) -> syn::Result<()> {
    input.step(|cursor| {
        let mut rest = *cursor;
        let mut depth = 0_usize;
        let mut joined_to = None;
        while let Some((tt, next)) = rest.token_tree() {
            rest = next;
            let TokenTree::Punct(punct) = tt else {
                joined_to = None;
                continue;
            };
            match punct.as_char() {
                ',' if depth == 0 => break,
                '<' => depth += 1,
                // `->` and `=>` close nothing.
                '>' if !matches!(joined_to, Some('-' | '=')) => depth = depth.saturating_sub(1),
                _ => {}
            }
            joined_to = (punct.spacing() == Spacing::Joint).then_some(punct.as_char());
        }
        Ok(((), rest))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;

    const TABLE: Table = Table {
        keys: &[Key::flag("copy"), Key::list("display"), CRATE],
        not: &[&["Clone"], &["Debug"]],
    };

    fn parse_with<T>(
        tokens: proc_macro2::TokenStream,
        resolve: impl FnOnce(&Opts) -> syn::Result<T>,
    ) -> syn::Result<T> {
        (|input: ParseStream| resolve(&Opts::parse(input, &TABLE)?)).parse2(tokens)
    }

    fn messages(tokens: proc_macro2::TokenStream) -> Vec<String> {
        let err = parse_with(tokens, |_| Ok(())).unwrap_err();
        err.into_iter().map(|e| e.to_string()).collect()
    }

    #[test]
    fn order_independent() {
        let resolve = |opts: &Opts| Ok((opts.flag("copy").is_some(), opts.not("Clone").is_some()));
        let a = parse_with(quote!(copy, not(Clone)), resolve).unwrap();
        let b = parse_with(quote!(not(Clone), copy), resolve).unwrap();
        assert_eq!(a, (true, true));
        assert_eq!(a, b);
    }

    #[test]
    fn values_and_lists() {
        let (krate, display) = parse_with(quote!(display("x"), crate = crate,), |opts| {
            let display = opts.list("display", |input| input.parse::<syn::LitStr>())?;
            Ok((opts.krate(), display.unwrap().value()))
        })
        .unwrap();
        assert_eq!(krate.to_token_stream().to_string(), "crate");
        assert_eq!(display, "x");
    }

    #[test]
    fn errors_accumulate() {
        assert_eq!(
            messages(quote!(bogus = 1, copy, copy, not(Hash, Debug, Debug))),
            [
                "unknown option `bogus`; expected one of: copy, display, crate, not",
                "duplicate option `copy`",
                "unknown option `Hash`; expected one of: Clone, Debug",
                "duplicate option `Debug`",
            ]
        );
    }

    #[test]
    fn recovers_after_malformed_options() {
        assert_eq!(
            messages(quote!(
                bogus = Foo<A, B>,
                not(Hash(x), not(Clone), Clone),
                1,
                crate = ::eva extra,
                display("x")
            )),
            [
                "unknown option `bogus`; expected one of: copy, display, crate, not",
                "unknown option `Hash`; expected one of: Clone, Debug",
                "unknown option `not`; expected one of: Clone, Debug",
                "expected an option name",
                "expected `,`",
            ]
        );
    }

    #[test]
    fn shape_mismatch() {
        assert_eq!(
            messages(quote!(copy = 1, display, crate)),
            [
                "option `copy` takes no arguments",
                "option `display` expects arguments: `display(...)`",
                "option `crate` expects a value: `crate = ...`",
            ]
        );
    }

    #[test]
    fn list_leftovers_rejected() {
        let err = parse_with(quote!(display("x" y)), |opts| {
            opts.list("display", |input| input.parse::<syn::LitStr>())
        })
        .unwrap_err();
        assert_eq!(err.to_string(), "unexpected token");
    }
}
