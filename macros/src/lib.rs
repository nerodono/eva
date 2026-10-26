use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;

mod docs;
mod opts;
mod utils;

mod auto_impl;
mod data;
mod endpoint;
mod export;
mod int;
mod service;
mod str;

type Expand = fn(TokenStream2, TokenStream2) -> syn::Result<TokenStream2>;

/// On error, emits `fallback(item)` next to the diagnostics so uses of the annotated name
/// don't add unrelated errors.
fn run(
    args: TokenStream,
    item: TokenStream,
    expand: Expand,
    fallback: fn(TokenStream2) -> TokenStream2,
) -> TokenStream {
    let item = TokenStream2::from(item);
    match expand(args.into(), item.clone()) {
        Ok(expanded) => expanded.into(),
        Err(error) => {
            let mut out = fallback(item);
            out.extend(error.to_compile_error());
            out.into()
        }
    }
}

/// A plain data type: a struct or enum with the usual derives, plus optional `Display`, `Error`
/// and `From` impls.
///
/// ```text
/// #[data(option, ...)]
/// ```
///
/// By default the item derives `Debug`, `Clone`, `PartialEq`, serde's `Serialize` and
/// `Deserialize`, and schemars' `JsonSchema`, all reached through `eva`, so the crate needs no
/// direct serde or schemars dependency. serde names fields and variants in snake_case
/// (`#[serde(rename_all = "snake_case")]`) unless the item has its own container
/// `#[serde(rename_all ...)]`. Attributes on the item pass through unchanged.
///
/// # Options
///
/// | Option | Effect |
/// | --- | --- |
/// | `copy` | Also derive `Copy`. |
/// | `eq` | Also derive `Eq`. |
/// | `ord` | Also derive `Eq`, `PartialOrd` and `Ord`. |
/// | `error` | Implement `Error`, see [Errors](#errors). |
/// | `display(...)` | Implement `Display`, see [Display](#display). |
/// | `not(...)` | Skip parts of the default: `serde` (both serde derives), `Serialize`, `Deserialize`, `schemars` or `JsonSchema`, `Debug`, `Clone`, `PartialEq`, and `as_static_str` (enums only). |
/// | `crate = path` | Path to `eva`, `::eva` by default. |
///
/// Options may come in any order, each at most once. `not(...)` wins over an option that
/// implies the same trait, so `copy, not(Clone)` derives `Copy` and leaves `Clone` to a
/// hand-written impl. serde and schemars take the crate path as a string, which `$crate` cannot
/// be written as, so `crate = $crate` inside `macro_rules!` needs `not(serde, schemars)`.
///
/// # Display
///
/// `display(...)` on the type and `#[display(...)]` on an enum variant take one of:
///
/// - `"format", args...`, as in `write!`. Named fields are bound by reference under their names
///   and tuple fields as `_0`, `_1`, ...; `self`, constants and const generics are in scope too.
/// - `name`: the variant name in snake_case.
/// - `doc`: the variant's doc comment as one line. Doc lines are trimmed, empty ones dropped and
///   the rest joined with spaces. One trailing `.` is removed (an ellipsis stays), and the first
///   letter is lowercased unless the first word has more capitals: `Invalid input.` prints
///   `invalid input`, while `MessagePack` and `HTTP` stay. The docs must be literal `///` text
///   (not `#[doc = include_str!(..)]`), and a variant without docs is an error.
///
/// `display(name)` and `display("name")` differ: a string literal is always a format string, so
/// `display("name")` prints `name` for every variant.
///
/// On a struct, `display(...)` takes a format string, which sees the struct's fields.
///
/// On an enum, each variant uses its own `#[display(...)]` if it has one, else the type-level
/// `display(...)`. A type-level format string sees only `self`, not the variant's fields;
/// type-level `name` and `doc` apply to each variant. Without a type-level `display(...)`,
/// either every variant has `#[display(...)]`, or none does and no `Display` is generated,
/// except for an `error` enum without variants: it has no values to display, and `Error` needs
/// the impl.
///
/// # Errors
///
/// `error` implements `Error`. It needs `Debug` and `Display`, generated or hand-written.
/// `source()` returns the field marked `#[source]`, and `None` for variants without one.
///
/// # Helper attributes
///
/// | Attribute | Where | Effect |
/// | --- | --- | --- |
/// | `#[display(...)]` | An enum variant, once. | The variant's `Display`, see above. |
/// | `#[from]` | The only field of a struct or variant. | `impl From<FieldType> for Type`, generics preserved. |
/// | `#[source]` | A field, at most one per struct or variant; needs `error`. | `Error::source` returns it. The field is an `Error + 'static` or a `Box<dyn Error [+ Send [+ Sync]]>`. |
///
/// `#[from]` only converts: add `#[source]` to also make the field the error source.
///
/// # Generated items
///
/// - The derives above, and the `Display`, `Error` and `From` impls. The impls reuse the item's
///   generics and where-clause without adding bounds, so put bounds such as `T: Display` on the
///   type. A variant's `#[cfg]` also applies to its match arms and `From` impl.
/// - On enums, unless `not(as_static_str)`:
///   `pub const fn as_static_str(&self, convert_case: Option<eva::generic::Case>) -> &'static str`,
///   the variant name as written, or converted to `convert_case`.
/// - A no-op derive that registers the helper attributes.
///
/// # Examples
///
/// A struct, with fields bound in the format string:
///
/// ```rust
/// use eva::data;
///
/// #[data(copy, error, display("{got} is over the limit of {LIMIT}"))]
/// pub struct TooBig<const LIMIT: usize> {
///     got: usize,
/// }
///
/// assert_eq!(TooBig::<3> { got: 5 }.to_string(), "5 is over the limit of 3");
/// ```
///
/// An enum with a type-level default and per-variant overrides:
///
/// ```rust
/// use eva::data;
///
/// #[data(error, display("lookup failed"))]
/// pub enum LookupError {
///     Timeout,
///     #[display("no user {_0}")]
///     NotFound(u32),
///     #[display("bad query {query:?} at {}", pos + 1)]
///     BadQuery { query: String, pos: usize },
///     #[display(doc)]
///     /// Rate limit exceeded.
///     Throttled,
/// }
///
/// assert_eq!(LookupError::Timeout.to_string(), "lookup failed");
/// assert_eq!(LookupError::NotFound(7).to_string(), "no user 7");
/// let bad = LookupError::BadQuery { query: "x".into(), pos: 0 };
/// assert_eq!(bad.to_string(), r#"bad query "x" at 1"#);
/// assert_eq!(LookupError::Throttled.to_string(), "rate limit exceeded");
/// ```
///
/// Variant names and the wire format:
///
/// ```rust
/// use eva::{data, generic::Case};
///
/// #[data(copy, ord, display(name))]
/// pub enum Role {
///     SuperAdmin,
///     User,
/// }
///
/// assert_eq!(Role::SuperAdmin.to_string(), "super_admin");
/// assert_eq!(serde_json::to_string(&Role::SuperAdmin).unwrap(), r#""super_admin""#);
/// assert_eq!(Role::SuperAdmin.as_static_str(None), "SuperAdmin");
/// assert_eq!(Role::SuperAdmin.as_static_str(Some(Case::Kebab)), "super-admin");
/// assert!(Role::SuperAdmin < Role::User);
///
/// #[data(eq)]
/// #[serde(rename_all = "camelCase")]
/// pub struct Page {
///     next_cursor: u32,
/// }
///
/// assert_eq!(
///     serde_json::to_string(&Page { next_cursor: 1 }).unwrap(),
///     r#"{"nextCursor":1}"#,
/// );
/// ```
///
/// Conversions and error sources:
///
/// ```rust
/// use std::error::Error;
///
/// use eva::data;
///
/// #[data(copy, error, display("the input is empty"))]
/// pub struct Empty;
///
/// #[data(error, not(serde, schemars, Clone, PartialEq))]
/// pub enum LoadError {
///     #[display("io: {_0}")]
///     Io(#[from] #[source] std::io::Error),
///     #[display("parse: {_0}")]
///     Parse(#[from] Empty),
///     #[display("plugin failed")]
///     Plugin(#[source] Box<dyn Error + Send + Sync>),
/// }
///
/// #[data(copy, display("user #{_0}"))]
/// pub struct UserId(#[from] u64);
///
/// let err = LoadError::from(Empty);
/// assert_eq!(err.to_string(), "parse: the input is empty");
/// assert!(err.source().is_none());
///
/// let err = LoadError::Plugin("crashed".into());
/// assert_eq!(err.source().unwrap().to_string(), "crashed");
/// assert_eq!(UserId::from(3).to_string(), "user #3");
/// ```
///
/// Opting out, and an error enum without variants:
///
/// ```rust
/// use std::{error::Error, fmt};
///
/// use eva::data;
///
/// #[data(copy, not(Debug, schemars), crate = ::eva)]
/// pub struct Secret(u64);
///
/// impl fmt::Debug for Secret {
///     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
///         f.write_str("<secret>")
///     }
/// }
///
/// #[data(error)]
/// pub enum Never {}
///
/// fn is_error<E: Error>() {}
///
/// assert_eq!(format!("{:?}", Secret(1)), "<secret>");
/// is_error::<Never>();
/// ```
#[proc_macro_attribute]
pub fn data(args: TokenStream, item: TokenStream) -> TokenStream {
    run(args, item, data::expand, data::fallback)
}

/// A string type: a struct (or, for `custom`, also an enum or union) that is viewed as `&str`
/// through `as_str` and parsed through `FromStr`.
///
/// `#[str(<kind> [, options...])]`, where the kind comes first:
///
/// | Kind | Item | `as_str` and `FromStr` |
/// | --- | --- | --- |
/// | `newtype` | A struct with exactly one field (tuple or named). | Generated, delegating to the field: `pub fn as_str` (not `const`) calls `<Field>::as_str`, and `FromStr::Err` is the field's. |
/// | `fixed(error = Type)` | A struct whose fields all implement `eva::str::FixedUtf8`. | Generated: `pub const fn as_str` views the fields' bytes back to back. `FromStr::Err` is `Type`, which implements `FixedParseError`, `Display` and `From` of every field's error. |
/// | `custom` | A struct, enum or union; a union also needs `copy`, since `Clone` is derived. | Written by you: an inherent `fn as_str(&self) -> &str` (it may be `const`) and, unless `not(Deserialize)`, `FromStr` with an `Err: Display`. |
///
/// | Option | Effect |
/// | --- | --- |
/// | `copy` | Also derive `Copy` (`fixed` always does). |
/// | `not(...)` | Skip implementations: `serde`, `Serialize`, `Deserialize`, `schemars` / `JsonSchema`, `Debug`, `Display`, and `pat` (`HasPattern`, `newtype` only). `custom` rejects `schemars` / `JsonSchema`: it never implements `JsonSchema`. |
/// | `crate = path` | Path to `eva`, `::eva` by default. `newtype` with `crate = $crate` also needs `not(schemars)`: the `JsonSchema` derive takes the path as a string, which `$crate` can't be. |
///
/// Every kind gets:
/// - `Clone` (derived);
/// - `Hash`, `PartialEq`, `Eq`, `PartialOrd` and `Ord` through `as_str`, so values compare and
///   sort as their strings, consistently with `Borrow<str>`;
/// - `PartialEq` with `str`, `&str` and `String`, in both directions;
/// - `Borrow<str>`, `AsRef<str>` and `Deref<Target = str>`;
/// - `Display` formatted like `str` (width, fill and alignment apply; precision truncates), and
///   `Debug` as a quoted string, which like `str` ignores width and precision;
/// - serde: serialized as a string, deserialized through `FromStr` from a borrowed or owned
///   string or UTF-8 bytes.
///
/// Generated code calls `Self::as_str(self)`, never method syntax, so a `custom` type without an
/// inherent `as_str` is a compile error rather than a call through its own `Deref`.
///
/// `newtype` also forwards `HasPattern` to the field, and derives `JsonSchema`, which describes the
/// field's schema (a named field gets `#[schemars(transparent)]`).
///
/// `fixed`:
/// - adds `#[repr(C)]` and rejects a `#[repr(...)]` of your own;
/// - requires every field to have alignment 1, so the value has no padding; this is checked when
///   the type is defined or, for a generic type, when an instantiation's `as_str` or `FromStr` is
///   compiled: `cargo build` reports it, `cargo check` doesn't (see the last example);
/// - implements `FixedUtf8` and `HasPattern` (the fields' patterns in order);
/// - parses by checking that the input is exactly `size_of::<Self>()` bytes
///   (`FixedParseError::length` otherwise), then cutting each field's bytes off the front; a
///   multibyte char across a field boundary is `FixedParseError::char`;
/// - describes itself in `JsonSchema` as a string with `minLength = maxLength = size_of::<Self>()`,
///   the anchored `HasPattern` as `pattern`, and the doc comment as `description`; each
///   instantiation of a generic type gets its own schema.
///
/// `custom` gets nothing beyond the list above: implement `HasPattern` and `JsonSchema` yourself
/// if you need them.
///
/// ```rust
/// use std::collections::BTreeSet;
///
/// use eva::str::{ParseError, Seq, ascii};
///
/// /// A four-digit code.
/// #[eva::str(fixed(error = ParseError))]
/// pub struct Pin(Seq<4, ascii::Digit>);
///
/// #[eva::str(newtype)]
/// pub struct Title(String);
///
/// #[eva::str(newtype, copy)]
/// pub struct Login {
///     pin: Pin,
/// }
///
/// let pin: Pin = "0420".parse()?;
/// assert_eq!(pin, "0420");
/// assert_eq!("0420", pin);
/// assert_eq!(pin.as_str().len(), size_of::<Pin>());
/// assert_eq!("042".parse::<Pin>(), Err(ParseError::Length));
/// assert_eq!("04é".parse::<Pin>(), Err(ParseError::Char));
///
/// let login: Login = "1234".parse()?;
/// let titles: BTreeSet<Title> = ["b", "a"].map(|s| s.parse().unwrap()).into();
/// assert!(titles.contains("a"));
/// assert_eq!(serde_json::to_string(&login).unwrap(), r#""1234""#);
/// # Ok::<(), ParseError>(())
/// ```
///
/// A `custom` string, and opting out of traits to write them by hand:
///
/// ```rust
/// use std::{fmt, str::FromStr};
///
/// use eva::str::{ParseError, Seq, ascii};
///
/// #[eva::str(custom, copy)]
/// pub enum Color {
///     Red,
///     Green,
/// }
///
/// impl Color {
///     pub const fn as_str(&self) -> &'static str {
///         match self {
///             Self::Red => "red",
///             Self::Green => "green",
///         }
///     }
/// }
///
/// impl FromStr for Color {
///     type Err = ParseError;
///
///     fn from_str(s: &str) -> Result<Self, Self::Err> {
///         match s {
///             "red" => Ok(Self::Red),
///             "green" => Ok(Self::Green),
///             _ => Err(ParseError::Char),
///         }
///     }
/// }
///
/// #[eva::str(newtype, not(Debug, Display, schemars, pat), crate = ::eva)]
/// pub struct Password(String);
///
/// impl fmt::Debug for Password {
///     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
///         f.write_str("***")
///     }
/// }
///
/// // `serde` is `Serialize` and `Deserialize`; `JsonSchema` is `schemars`.
/// #[eva::str(fixed(error = ParseError), not(serde, JsonSchema))]
/// pub struct Internal(Seq<2, ascii::Digit>);
///
/// #[eva::str(fixed(error = ParseError), not(Serialize))]
/// pub struct Input(Seq<2, ascii::Digit>);
///
/// #[eva::str(fixed(error = ParseError), not(Deserialize))]
/// pub struct Output(Seq<2, ascii::Digit>);
///
/// let mut colors = [Color::Red, Color::Green];
/// colors.sort();
/// assert_eq!(colors, ["green", "red"]);
///
/// let password: Password = "hunter2".parse().unwrap();
/// assert_eq!(format!("{password:?}"), "***");
///
/// let output: Output = "42".parse().unwrap();
/// assert_eq!(serde_json::to_string(&output).unwrap(), r#""42""#);
/// ```
///
/// A generic `fixed` type checks its layout per instantiation, when that is built:
///
/// ```rust,compile_fail,E0080
/// # use std::str::FromStr;
/// #
/// # use eva::str::{FixedUtf8, HasPattern, ParseError};
/// #
/// # #[derive(Clone, Copy)]
/// # #[repr(C, align(2))]
/// # pub struct Wide([u8; 2]);
/// #
/// # unsafe impl FixedUtf8 for Wide {}
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
/// #[eva::str(fixed(error = ParseError))]
/// pub struct Generic<T: FixedUtf8>(T);
///
/// // `Wide` breaks the `FixedUtf8` contract with alignment 2.
/// let _ = "ab".parse::<Generic<Wide>>();
/// ```
#[proc_macro_attribute]
pub fn str(args: TokenStream, item: TokenStream) -> TokenStream {
    run(args, item, str::expand, str::fallback)
}

/// An integer type: an enum restricted to the given values, with one variant per value, or a
/// [newtype](#newtypes) over an integer field.
///
/// ```text
/// #[int(<repr>, <ranges> [, options...])]
/// <vis> enum Name {}
/// ```
///
/// - `repr`: `u8`, `u16`, `u32`, `u64`, `u128`, `i8`, `i16`, `i32`, `i64` or `i128`.
/// - `ranges`: `v`, `a..b` or `a..=b`, joined by `|`. A value is an integer literal (unsuffixed or
///   suffixed with the repr, negative only for a signed repr), a byte literal or a char literal.
///   Every value must fit the repr (`u128` values stop at `i128::MAX`), and there are at most 4096
///   values in total.
/// - The enum must be empty and must not be generic or have a where clause: the macro writes the
///   variants.
///
/// | Option | Effect |
/// | --- | --- |
/// | `not(...)` | Skip trait impls: `serde` (both), `Serialize`, `Deserialize`, `schemars` / `JsonSchema`, `Debug`, `Display`. |
/// | `crate = path` | Path to `eva`, `::eva` by default. |
///
/// ## Variants and order
///
/// The enum is `#[repr(<repr>)]` and each variant's discriminant is its value, so `as` casts and
/// `transmute` to the repr work. A variant is named `POS<n>` for a value `n >= 0` and `NEG<n>` for
/// `-n`: `POS0`, `NEG5`, and `POS58` for `b':'`. To name a value by its literal, write
/// `const { Name::new(v).unwrap() }`, which fails to compile for a value outside the ranges.
///
/// Overlapping and adjacent ranges are merged, and everything is in ascending order of value,
/// whatever order the ranges are written in. This order is part of the contract: `RANGES`,
/// `VALUES`, `VARIANTS` and `nth` follow it.
///
/// ## Generated items
///
/// All inherent items have the enum's visibility.
///
/// | Item | |
/// | --- | --- |
/// | `const RANGES: [RangeInclusive<repr>; _]` | The merged ranges. |
/// | `const VALUES: [repr; _]` | Every value. |
/// | `const VARIANTS: [Self; _]` | Every variant; `VARIANTS[i].nth() == i`. |
/// | `const fn new(value: repr) -> Option<Self>` | The variant for `value`. |
/// | `const unsafe fn new_unchecked(value: repr) -> Self` | The same, for a `value` known to be allowed. |
/// | `const fn into_inner(self) -> repr` | The value. |
/// | `const fn nth(self) -> usize` | The index in `VARIANTS`. |
/// | `const fn as_repr_slice(&[Self]) -> &[repr]` | |
/// | `const unsafe fn as_repr_slice_mut(&mut [Self]) -> &mut [repr]` | Only allowed values may be written through it. |
///
/// Traits:
/// - `Clone`, `Copy`; `PartialEq`, `Eq` (derived, so the variants work as patterns);
///   `PartialEq<repr>`, `PartialOrd`, `Ord`, `Hash` (by value), `From<Name> for repr`;
/// - `Debug` and `Display` as the number;
/// - serde's `Serialize` and `Deserialize` as the number. A value outside the ranges fails with
///   ``invalid value: integer `0`, expected an integer in 1..=64``;
/// - `JsonSchema`: `{"anyOf": [{"type": "integer", "minimum": a, "maximum": b}, ...]}`, one entry
///   per range, with the doc comment as `description`. Bounds outside `i64` and `u64` are written
///   as floats. Doc comments are read only for the schema;
/// - `rand`'s `Distribution<Name> for StandardUniform`, uniform over the values;
/// - `eva::int::Integer` and `InfallibleInteger`, so the enum can be the field of a
///   [newtype](#newtypes).
///
/// `Default` is not implemented: write it by hand, as
/// `const { Name::new(v).unwrap() }`. Neither is `TryFrom<repr>`: convert with `new`.
///
/// ## Newtypes
///
/// ```text
/// #[int(newtype[(<condition>)] [, options...])]
/// <vis> struct Name(<Inner>);
/// ```
///
/// - `Inner`, the only field (tuple or named), implements `eva::int::Integer`: a primitive (`u8` to
///   `u128`, `i8` to `i128`, `usize`, `isize`), a `NonZero<_>` or an `#[int]` type. Values are
///   created from and unpack to the field's primitive, `<Inner as Integer>::Prim`, so a `NonZero`
///   or `#[int]` field brings its own check.
/// - `condition`: an expression over `n`, the value as the primitive, checked after the field accepts
///   it; `try_new` returns `None` when it is false, and its source text ends the serde error.
/// - The struct must not be generic, have a where clause or a `#[repr]`: the macro adds
///   `#[repr(transparent)]`.
/// - `not(...)` and `crate = path` as for enums.
///
/// Generated items, with the struct's visibility:
///
/// | Item | |
/// | --- | --- |
/// | `const fn new(inner: Inner) -> Self` | Wraps the field; only without a condition. |
/// | `fn try_new(prim: Prim) -> Option<Self>` | Packs `prim`, if the field accepts it and the condition holds. |
/// | `unsafe fn new_unchecked(prim: Prim) -> Self` | The same, unchecked. |
/// | `const fn inner(&self) -> Inner` | The field. |
/// | `fn prim(&self) -> Prim` | The value as its primitive. |
///
/// Traits:
/// - `Clone`, `Copy`, `PartialEq`, `Eq`, `PartialOrd`, `Ord`, `Hash` (derived, so consts of the
///   type work as patterns); `PartialEq<Prim>`, as a generic impl over primitives, so no other
///   `PartialEq<Foreign>` can be added by hand; `From<Name> for Inner` and, without a condition,
///   `From<Inner> for Name`;
/// - `Integer`, so the type can be the field of another newtype (an enum answers `try_new` with its
///   `new`), and `InfallibleInteger` without a condition;
/// - `Debug` and `Display` as the number;
/// - serde's `Serialize` and `Deserialize` as the number. A rejected value fails with
///   ``invalid value: integer `80`, expected a non-zero u16 where n >= 1024``; stacked conditions
///   join with `and`;
/// - `JsonSchema`: the field's schema, with the doc comment as `description` in place of the field's.
///
/// ## Examples
///
/// ```rust
/// use eva::int;
///
/// /// Percentage.
/// #[int(u8, 0..=100)]
/// pub enum Percent {}
///
/// let half = Percent::new(50).unwrap();
/// assert_eq!(half, Percent::POS50);
/// assert_eq!(half.into_inner(), 50);
/// assert_eq!(half, 50);
/// assert_eq!(u8::from(half), 50);
/// assert_eq!(half.to_string(), "50");
/// assert!(Percent::new(101).is_none());
///
/// let full = const { Percent::new(100).unwrap() };
/// assert!(half < full);
/// ```
///
/// Order, merging and `nth`:
///
/// ```rust
/// use eva::int;
///
/// #[int(u8, b'z' | b'a'..=b'c' | b'b' | b'd')]
/// pub enum Letter {}
///
/// assert_eq!(Letter::RANGES, [b'a'..=b'd', b'z'..=b'z']);
/// assert_eq!(Letter::VALUES, [b'a', b'b', b'c', b'd', b'z']);
/// assert_eq!(Letter::VARIANTS[4], Letter::POS122);
/// assert_eq!(Letter::POS122.nth(), 4);
/// ```
///
/// Negative values and serde:
///
/// ```rust
/// use eva::int;
///
/// #[int(i16, -5..=-1 | 1..5)]
/// pub enum NonZero {}
///
/// assert_eq!(NonZero::VALUES, [-5, -4, -3, -2, -1, 1, 2, 3, 4]);
/// assert_eq!(serde_json::to_string(&NonZero::NEG3).unwrap(), "-3");
/// assert_eq!(serde_json::from_str::<NonZero>("4").unwrap(), NonZero::POS4);
///
/// let err = serde_json::from_value::<NonZero>(serde_json::json!(0)).unwrap_err();
/// assert_eq!(
///     err.to_string(),
///     "invalid value: integer `0`, expected an integer in -5..=-1 | 1..=4",
/// );
/// ```
///
/// `not(...)` and `crate = path`:
///
/// ```rust
/// use std::fmt;
///
/// #[eva::int(u8, b'0'..=b'9', not(Display, schemars), crate = ::eva)]
/// pub enum Digit {}
///
/// impl fmt::Display for Digit {
///     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
///         write!(f, "{}", self.into_inner() as char)
///     }
/// }
///
/// assert_eq!(Digit::POS55.to_string(), "7");
/// assert_eq!(format!("{:?}", Digit::POS55), "55");
/// ```
///
/// Newtypes over `NonZeroU16`, with and without a condition:
///
/// ```rust
/// use std::num::NonZeroU16;
///
/// use eva::int;
///
/// /// A listening port.
/// #[int(newtype(n >= 1024))]
/// pub struct Port(NonZeroU16);
///
/// #[int(newtype)]
/// pub struct Workers(NonZeroU16);
///
/// let port = Port::try_new(8080).unwrap();
/// assert_eq!(port.prim(), 8080);
/// assert_eq!(port, 8080);
/// assert_eq!(port.inner(), NonZeroU16::new(8080).unwrap());
/// assert_eq!(Port::try_new(0), None);
/// assert_eq!(Port::try_new(80), None);
///
/// const ONE: Workers = Workers::new(NonZeroU16::MIN);
/// assert_eq!(ONE, 1);
/// assert_eq!(Workers::try_new(0), None);
///
/// assert_eq!(serde_json::to_string(&port).unwrap(), "8080");
/// let err = serde_json::from_str::<Port>("80").unwrap_err();
/// assert_eq!(
///     err.to_string(),
///     "invalid value: integer `80`, expected a non-zero u16 where n >= 1024",
/// );
/// ```
#[proc_macro_attribute]
pub fn int(args: TokenStream, item: TokenStream) -> TokenStream {
    run(args, item, int::expand, int::fallback)
}

/// Turns an async fn into a unit struct that implements `eva::handling::Endpoint`.
///
/// ```text
/// #[endpoint]
/// [attrs] vis async fn Name<generics>(state: S, args: A) -> O where ... { body }
/// ```
///
/// expands to
///
/// ```text
/// [struct attrs]
/// #[derive(Clone, Copy, Debug, Default)]
/// vis struct Name;
///
/// [impl attrs]
/// impl<generics> Endpoint<A, S> for Name where ... {
///     type Output = O;
///
///     [call attrs]
///     async fn call(&self, state: S, args: A) -> Self::Output { body }
/// }
/// ```
///
/// `Name` is a value: pass it wherever an `Endpoint` is expected and run it with
/// `Name.call(state, args).await`. There is no fn left to call as `Name(state, args)`. The struct
/// takes the fn's name, so name endpoints in CamelCase.
///
/// ## Options
///
/// | Option         | Meaning                                  |
/// |----------------|------------------------------------------|
/// | `crate = path` | The path to `eva`; `::eva` by default.   |
///
/// ## Signature rules
///
/// Violations are compile errors.
///
/// - The fn is `async`, and not `const`, `unsafe` (`Endpoint::call` is safe), `extern` or variadic.
/// - It takes exactly two typed parameters, state first, then args; any patterns, no `self`.
/// - No `impl Trait` in parameter types (use a generic parameter) or in the return type (use a
///   concrete type).
/// - Every generic parameter (type, lifetime or const) appears in `S` or `A`, otherwise the impl
///   cannot constrain it. A projection such as `T::Assoc` or `<T as Trait>::Assoc` does not count.
/// - Elided lifetimes in `S` and `A` work (`state: &mut State<C>` implements the endpoint for every
///   borrow). A lifetime in `O` must be a named one taken from `S` or `A`.
///
/// ## Attributes
///
/// | On the fn                                              | Goes to                 |
/// |--------------------------------------------------------|-------------------------|
/// | doc comments, `#[doc(...)]`, `#[deprecated]`           | the struct              |
/// | `#[allow]`, `#[warn]`, `#[deny]`, `#[forbid]`          | the struct and the impl |
/// | `#[expect]`                                            | the impl                |
/// | unsafe attributes, `#[must_use]`                       | rejected                |
/// | anything else (`#[fastrace::trace]`, `#[inline]`, ...) | `call`                  |
///
/// Unsafe attributes (`#[unsafe(...)]` and the bare `#[no_mangle]`, `#[export_name]`,
/// `#[link_section]`) and `#[must_use]` are rejected: on `call` they would not do what they
/// promise for the fn.
///
/// `#[expect]` is not copied to the struct: that copy would be unfulfilled whenever the lint fires
/// only in the body. Lints of the struct itself, such as `non_camel_case_types`, take `#[allow]`.
/// Parameter attributes stay on their parameters. `#[cfg]` and `#[cfg_attr]` are evaluated before the
/// macro runs: `#[cfg]` gates the whole expansion and a `cfg_attr`'s result is routed like any other
/// attribute. The impl of a deprecated endpoint does not silence deprecation lints in the body. A body
/// whose future is not `Send` is reported at the return type (at the name when there is none).
///
/// ## Examples
///
/// ```rust
/// use eva::{endpoint, handling::Endpoint};
///
/// /// Adds the state to the args.
/// #[endpoint]
/// pub async fn Add(state: &u32, args: u32) -> u32 {
///     *state + args
/// }
///
/// # fn block_on<F: Future>(fut: F) -> F::Output {
/// #     let mut fut = std::pin::pin!(fut);
/// #     let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
/// #     loop {
/// #         if let std::task::Poll::Ready(out) = fut.as_mut().poll(&mut cx) {
/// #             return out;
/// #         }
/// #     }
/// # }
/// assert_eq!(block_on(Add.call(&2, 3)), 5);
/// ```
///
/// A generic endpoint over a mutably borrowed state, with `crate = path` and routed attributes:
///
/// ```rust
/// use eva::handling::Endpoint;
///
/// pub struct State<C> {
///     pub hits: u32,
///     pub config: C,
/// }
///
/// /// Counts hits.
/// #[eva::endpoint(crate = ::eva)]
/// #[deprecated = "count hits elsewhere"]
/// #[inline]
/// pub async fn Hit<C: Send + Sync>(state: &mut State<C>, args: u32) -> u32 {
///     state.hits += args;
///     state.hits
/// }
///
/// fn assert_endpoint<C>(_: impl for<'a> Endpoint<u32, &'a mut State<C>, Output = u32>) {}
/// #[expect(deprecated)]
/// let hit = Hit;
/// assert_endpoint::<()>(hit);
/// ```
///
/// A generic parameter that only the return type mentions is rejected:
///
/// ```rust,compile_fail
/// #[eva::endpoint]
/// pub async fn Make<T: Default>(state: (), args: ()) -> T {
///     T::default()
/// }
/// ```
#[proc_macro_attribute]
pub fn endpoint(args: TokenStream, item: TokenStream) -> TokenStream {
    run(args, item, endpoint::expand, endpoint::fallback)
}

/// Registers the `#[data]` helper attributes; expands to nothing.
#[proc_macro_derive(RastGawno, attributes(display, from, source))]
pub fn rast_gawno(_item: TokenStream) -> TokenStream {
    TokenStream::new()
}

/// Defines a service and its endpoint metadata from a trait declaration.
///
/// ```rust
/// use eva::{export, service};
///
/// #[eva::data(error, display("user not found"))]
/// pub struct NotFound;
///
/// #[service]
/// pub trait Users {
///     #[errors(NoSuchUser(NotFound))]
///     fn get(id: u64) -> String;
///
///     #[flags(write)]
///     fn create(name: String) -> ok! { id: u64 };
/// }
///
/// #[export]
/// pub struct Session {
///     users: Users,
/// }
/// # fn main() {}
/// ```
///
/// The name is preserved exactly: `Users` becomes a module containing:
///
/// - `Trait`, with `fn get(&mut self) -> impl CallStep<get::Args, Output = get::Output>`.
///   The trait is also implemented for `&mut S` and `Box<S>`.
/// - `Client<S>`, which owns any `S: Trait`. Its methods accept the declared arguments,
///   construct `Args`, and return a `Send` future. `Client::new(backend)`,
///   `Client::new(&mut backend)` and `Client::new(Box::new(backend))` are supported.
///   `inner_mut()` exposes the underlying service trait for callers that already have `Args`.
/// - One module per method, containing `Args`, `Ok`, `Err` and `Output`.
/// - `for_each_endpoint!`, the service metadata callback used by `#[export]`.
///
/// `#[service(export)]` also re-exports every method module beside the service.
/// `#[service(export(get, create))]` re-exports only the selected methods. The
/// re-exports have the same visibility as the service; `Trait` and `Client` stay
/// in the service module.
///
/// `-> T` creates `pub struct Ok(pub T)` with `From<T>` and transparent
/// serialization. To customize that wrapper, use `ok!(...)` with outer attributes
/// before the type: `-> ok!(#[data(copy)] user::Id)`. `-> ok!(T)` is equivalent to `-> T`.
/// `-> ok! { field: T }` creates a named-field success structure. `ok!` is consumed
/// by the attribute; it needs no import. Omitted return types mean `()`.
/// Inner attributes at the beginning of `ok! { ... }` apply to the generated structure:
///
/// ```rust
/// use eva::service;
///
/// #[service]
/// pub trait Health {
///     fn status() -> ok! {
///         #![data(copy)]
///         #![derive(Default)]
///
///         ready: bool,
///     };
/// }
/// # fn main() {}
/// ```
///
/// Arguments always become named fields, including a single argument. Empty argument
/// lists produce `Args {}`. Lifetimes in argument types are carried into `Args` and
/// client methods; elided reference lifetimes and `'_` are inferred.
///
/// `#[errors(path::Error, OtherError(other::Error))]` creates a per-method enum with
/// payloads, `From` implementations and delegated error display. Repeated variant
/// names require an explicit `Variant(path::Error)` name. No errors attribute means an empty enum.
/// `#[service(error = path::Error)]` selects one shared error type for all methods;
/// `#[error(path::Error)]` selects an existing error type for one method.
///
/// `Output` defaults to `Result<Ok, Err>`. `#[service(result = path::Response)]` uses
/// a different two-parameter result constructor, such as `Result<O, Generic<E>>`.
///
/// Generated data uses `#[eva::data]`. `#[args(...)]` and `#[err(...)]` forward
/// attributes to the corresponding type; attributes inside `ok!` customize `Ok`.
/// A supplied `data(...)` replaces its default configuration. A stream argument can use
/// `#[args(data(not(Debug, serde, schemars, Clone, PartialEq)))]`. Argument and
/// `ok!` field attributes are preserved, including serde defaults and documentation.
///
/// `embed! { ... }` places ordinary Rust items in the preceding method's module.
/// It must follow a method; whitespace does not affect this association.
/// In method declarations, `this![...]` prefixes its contents with that method's
/// path. It accepts tokens, including generic arguments and nested paths, rather
/// than just an identifier. All three helpers (`ok!`, `this!`, `embed!`) are
/// consumed by `service` and need no imports.
///
/// ```rust
/// use eva::service;
///
/// #[service(export)]
/// pub trait Values {
///     fn get(value: this![Value<i32>]) -> ok! { value: this![Value<i32>] };
///     embed! {
///         #[eva::data]
///         pub struct Value<T>(pub T);
///     }
/// }
/// # fn main() {}
/// ```
///
/// Middleware is assembled with the concrete implementation. An endpoint expression
/// such as `LogCall("users.get").apply(handlers::Get)` can be bound to state wherever
/// a plain endpoint can. Service declarations and client calls need no changes.
#[proc_macro_attribute]
pub fn service(args: TokenStream, item: TokenStream) -> TokenStream {
    run(args, item, service::expand, |_| TokenStream2::new())
}

/// Composes service modules into a session and a single X-macro.
///
/// ```rust
/// use eva::{export, service};
///
/// #[service]
/// pub trait Health {
///     fn ping() -> eva::True;
/// }
///
/// #[export]
/// pub struct Session {
///     health: Health,
/// }
/// # fn main() {}
/// ```
///
/// The fields declare accessors and service module paths. They are not stored fields:
/// the generated `Session<S>(pub S)` stores one implementation. `Session::new`,
/// `into_inner`, `inner_mut`, and `mut_` construct, extract or borrow it. Each accessor
/// returns the service's `Client<&mut S>`. `SessionFeatures` is the combined service
/// bound and has a blanket implementation.
///
/// The macro `for_each_endpoint!` calls its callback once with all services. Use
/// `#[export(name = for_each_op)]` to choose a different name. If the session is
/// declared in a nested module, re-export the macro at the crate root under the
/// same name. Service paths resolve where the session is declared, so ordinary
/// private imports and aliases work.
/// The macro has a hidden companion module of the same name; the same `pub use`
/// re-exports both, keeping metadata paths valid across crates and dependency renames.
///
/// ```ignore
/// macro_rules! project {
///     ($(
///         $segment:ident {$(
///             $accessor:ident: $Name:ident {
///                 trait: $Trait:path,
///                 client: $Client:path,
///                 $(
///                     $(#[$flag:ident])*
///                     $method:ident = $endpoint:ident $(<$lt:lifetime>)? {
///                         args: $Args:ty,
///                         ok: $Ok:ty,
///                         err: $Err:ty,
///                         output: $Output:ty,
///                         docs: [$($doc:literal),*],
///                     }
///                 )*
///             }
///         )*}
///     )*) => { /* implementations, transport routes, tools, etc. */ };
/// }
/// for_each_endpoint!(project);
/// ```
///
/// Segments use the explicitly declared accessor names. Trait and client paths are
/// provided directly; callbacks do not have to reconstruct them. Method flags and
/// documentation come from `#[service]`, so no second method registry is needed.
///
/// A backend can select middleware in its callback using a separate expression:
///
/// ```ignore
/// macro_rules! decorate {
///     (users, get; $handler:expr) => {
///         LogCall("users.get").apply($handler)
///     };
///     ($service:ident, $method:ident; $handler:expr) => { $handler };
/// }
/// ```
#[proc_macro_attribute]
pub fn export(args: TokenStream, item: TokenStream) -> TokenStream {
    run(args, item, export::expand, |_| TokenStream2::new())
}

/// Implements the trait for `&T`, `&mut T`, `Box<T>`, `Rc<T>` and `Arc<T>` by forwarding to `T`.
///
/// ```rust
/// #[eva::auto_impl(&, &mut, Box)]
/// trait Counter {
///     const STEP: u32;
///     type Out;
///     fn get(&self) -> Self::Out;
/// }
///
/// struct Fixed;
/// impl Counter for Fixed {
///     const STEP: u32 = 1;
///     type Out = u32;
///     fn get(&self) -> u32 { 7 }
/// }
///
/// fn read(c: impl Counter<Out = u32>) -> u32 { c.get() }
/// assert_eq!(read(&Fixed) + read(Box::new(Fixed)), 14);
/// ```
///
/// Allowed receivers per proxy: `&` and `Rc`/`Arc` take `&self` only, `&mut` adds `&mut self`,
/// `Box` adds `self`; methods without a receiver forward everywhere. `T: ?Sized` unless a
/// method takes or returns `Self` by value or requires `Self: Sized`; method-level
/// `where Self: Trait` bounds become bounds on `T`. Arguments must be plain names.
#[proc_macro_attribute]
pub fn auto_impl(args: TokenStream, item: TokenStream) -> TokenStream {
    run(args, item, auto_impl::expand, |item| item)
}
