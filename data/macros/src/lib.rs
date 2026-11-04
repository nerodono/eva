use eva_macros_common::{docs, opts, utils};
use proc_macro::TokenStream;

fn run(
    args: TokenStream,
    item: TokenStream,
    expand: eva_macros_common::Expand,
    fallback: fn(proc_macro2::TokenStream) -> proc_macro2::TokenStream,
) -> TokenStream {
    eva_macros_common::run(args.into(), item.into(), expand, fallback).into()
}
mod data;
mod int;
mod str;

/// A plain data type: a struct or enum with the usual derives, plus optional `Display`, `Error`
/// and `From` impls.
///
/// ```text
/// #[data(option, ...)]
/// ```
///
/// By default the item derives `Debug`, `Clone`, `PartialEq`, serde's `Serialize` and
/// `Deserialize`, and schemars' `JsonSchema`, all reached through `eva-data` or a facade, so the crate needs no
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
/// | `crate = path` | Override the runtime path; otherwise discover `eva-data` or a supported facade through Cargo, respecting dependency renames. |
///
/// Discovery tries `eva-data`, `eva`, `eva-core`, `eva-handlers`, then `eva-rt`.
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
///   `pub const fn as_static_str(&self, convert_case: Option<eva_data::Case>) -> &'static str`,
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

/// Registers the `#[data]` helper attributes; expands to nothing.
#[proc_macro_derive(RastGawno, attributes(display, from, source))]
pub fn rast_gawno(_item: TokenStream) -> TokenStream {
    TokenStream::new()
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
/// | `crate = path` | Override the runtime path; otherwise discover `eva-data` or a supported facade, respecting Cargo dependency renames. `newtype` with `crate = $crate` also needs `not(schemars)`: the `JsonSchema` derive takes the path as a string, which `$crate` can't be. |
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
/// | `crate = path` | Override the runtime path; otherwise discover `eva-data` or a supported facade, respecting Cargo dependency renames. |
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
/// - With `eva-data/rand`: `rand`'s `Distribution<Name> for StandardUniform`, uniform over
///   the values. `eva-core` enables this feature automatically;
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

fn runtime() -> syn::Path {
    eva_macros_common::resolve_crates(&["eva-data", "eva", "eva-core", "eva-handlers", "eva-rt"])
        .next()
        .map(|(_, path)| syn::parse_quote!(#path::_priv::data))
        .unwrap_or_else(|| syn::parse_quote!(::eva_data))
}
