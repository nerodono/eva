use std::{
    collections::{BTreeSet, HashSet},
    fmt,
    str::FromStr,
};

use eva::{
    _priv::{
        schemars::{self, JsonSchema},
        serde::{
            Deserialize,
            de::value::{BytesDeserializer, Error as DeError, StringDeserializer},
        },
    },
    str::{CompactString, FixedUtf8, HasPattern, ParseError, Seq, ascii},
};

/// Four printable
/// ASCII chars.
#[eva::str(fixed(error = ParseError))]
struct Code(Seq<4, ascii::Printable>);

#[eva::str(fixed(error = ParseError))]
struct Pair(Seq<2, ascii::Char>, Seq<2, ascii::Char>);

#[eva::str(fixed(error = ParseError))]
struct Named {
    head: ascii::Digit,
    tail: Seq<2, ascii::Digit>,
}

#[eva::str(fixed(error = ParseError))]
struct Empty;

/// Takes any byte it is given, unlike eva's components, so only the boundary check rejects a
/// split char.
#[derive(Clone, Copy)]
struct AnyByte(u8);

// SAFETY: values only come from `from_str`, and a 1-byte `str` is ASCII.
unsafe impl FixedUtf8 for AnyByte {}

impl HasPattern for AnyByte {
    fn pat_into(buf: &mut String) {
        buf.push('.');
    }
}

impl FromStr for AnyByte {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match *s.as_bytes() {
            [byte] => Ok(Self(byte)),
            _ => Err(ParseError::Length),
        }
    }
}

#[eva::str(fixed(error = ParseError))]
struct Bytes(AnyByte, AnyByte);

#[eva::str(fixed(error = ParseError))]
struct Digits<const N: usize>(Seq<N, ascii::Digit>);

#[eva::str(fixed(error = ParseError))]
struct Two<T: FixedUtf8>(Seq<2, T>);

#[eva::str(newtype)]
struct Name(CompactString);

#[eva::str(newtype, copy, crate = ::eva)]
struct Wrapped {
    code: Code,
}

/// Stores the string padded with `z`, so field order differs from string order.
#[eva::str(custom, copy)]
struct Padded {
    buf: [u8; 3],
    len: u8,
}

impl Padded {
    fn as_str(&self) -> &str {
        std::str::from_utf8(&self.buf[..self.len.into()]).unwrap()
    }
}

impl FromStr for Padded {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.len() > 3 {
            return Err(ParseError::Length);
        }
        let mut buf = [b'z'; 3];
        buf[..s.len()].copy_from_slice(s.as_bytes());
        Ok(Self {
            buf,
            len: s.len() as u8,
        })
    }
}

#[eva::str(custom, copy)]
enum Color {
    Red,
    Green,
}

impl Color {
    const fn as_str(&self) -> &'static str {
        match self {
            Self::Red => "red",
            Self::Green => "green",
        }
    }
}

impl FromStr for Color {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "red" => Ok(Self::Red),
            "green" => Ok(Self::Green),
            _ => Err(ParseError::Char),
        }
    }
}

#[eva::str(newtype, not(Debug, Display, serde, schemars, pat))]
struct Secret(CompactString);

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("***")
    }
}

impl fmt::Display for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("***")
    }
}

#[eva::str(fixed(error = ParseError), not(JsonSchema, Serialize))]
struct Unschematic(ascii::Digit);

impl JsonSchema for Unschematic {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "Custom".into()
    }

    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({ "const": "custom" })
    }
}

#[derive(JsonSchema)]
#[schemars(crate = "eva::_priv::schemars")]
struct Instantiations {
    _short: Digits<2>,
    _long: Digits<4>,
}

fn schema<T: JsonSchema>() -> serde_json::Value {
    serde_json::to_value(schemars::schema_for!(T)).unwrap()
}

#[test]
fn fixed_round_trip() {
    let code: Code = "ab-c".parse().unwrap();
    assert_eq!(code.as_str(), "ab-c");
    assert_eq!(code.to_string(), "ab-c");
    assert_eq!(format!("{code:?}"), r#""ab-c""#);

    let named: Named = "123".parse().unwrap();
    assert_eq!(named.head.parse(), 1);
    assert_eq!(named, "123");

    assert_eq!("".parse::<Empty>().unwrap(), "");
    assert_eq!("x".parse::<Empty>(), Err(ParseError::Length));
}

#[test]
fn formats_like_str() {
    let code: Code = "ab-c".parse().unwrap();
    assert_eq!(
        format!("[{code:>6}] [{code:<6}] [{code:.2}]"),
        "[  ab-c] [ab-c  ] [ab]"
    );
    assert_eq!(format!("{code:>8?} {code:.2?}"), r#""ab-c" "ab-c""#);
}

#[test]
fn fixed_layout_is_the_fields() {
    assert_eq!(size_of::<Pair>(), 4);
    assert_eq!(align_of::<Pair>(), 1);
    assert_eq!(size_of::<Named>(), 3);
}

#[test]
fn fixed_rejects_multibyte_char_across_fields() {
    for (input, len) in [("aéb", 4), ("é1", 3)] {
        assert_eq!(input.len(), len);
    }
    assert_eq!("aéb".parse::<Pair>(), Err(ParseError::Char));
    assert_eq!("é1".parse::<Named>(), Err(ParseError::Char));
    assert_eq!("abé".parse::<Code>(), Err(ParseError::Char));
    assert_eq!("abc".parse::<Code>(), Err(ParseError::Length));
    assert_eq!("ab\u{7f}c".parse::<Code>(), Err(ParseError::Char));

    let bytes: Bytes = "ab".parse().unwrap();
    assert_eq!((bytes.0.0, bytes.1.0), (b'a', b'b'));
    assert_eq!("é".parse::<Bytes>(), Err(ParseError::Char));
    assert_eq!("é".parse::<Seq<2, AnyByte>>(), Err(ParseError::Char));
}

#[test]
fn fixed_generic() {
    let digits: Digits<3> = "123".parse().unwrap();
    assert_eq!(digits, "123");
    assert!("12".parse::<Digits<3>>().is_err());

    let two: Two<ascii::Digit> = "42".parse().unwrap();
    assert_eq!(format!("{two:?}"), r#""42""#);
    assert_eq!(serde_json::to_string(&two).unwrap(), r#""42""#);
}

#[test]
fn fixed_schema() {
    let code = schema::<Code>();
    assert_eq!(code["type"], "string");
    assert_eq!(code["minLength"], 4);
    assert_eq!(code["maxLength"], 4);
    assert_eq!(code["pattern"], "^(?:[ -~]){4}$");
    assert_eq!(code["description"], "Four printable\n ASCII chars.");
    assert_eq!(schema::<Unschematic>()["const"], "custom");
}

#[test]
fn fixed_schema_description_takes_any_doc_expr() {
    /// First line.
    ///
    #[doc = concat!(" Second", " paragraph.")]
    #[eva::str(fixed(error = ParseError))]
    struct Documented(ascii::Digit);

    let schema = schema::<Documented>();
    assert_eq!(schema["description"], "First line.\n\n Second paragraph.");
}

#[test]
fn generic_fixed_instantiations_get_their_own_schema() {
    let schema = schema::<Instantiations>();
    let defs = schema["$defs"].as_object().unwrap();
    let mut lens: Vec<_> = defs.values().map(|def| def["maxLength"].clone()).collect();
    lens.sort_by_key(|len| len.as_u64());
    assert_eq!(lens, [2, 4]);
}

#[test]
fn newtype_delegates() {
    let name: Name = "ab-c".parse().unwrap();
    assert_eq!(name.as_str(), "ab-c");
    let _: fn(&str) -> Result<Name, <CompactString as FromStr>::Err> = Name::from_str;

    let wrapped: Wrapped = "ab-c".parse().unwrap();
    assert_eq!(wrapped, "ab-c");
    assert_eq!("abc".parse::<Wrapped>(), Err(ParseError::Length));
    assert_eq!(Wrapped::regex_pat(), Code::regex_pat());
    assert_eq!(schema::<Wrapped>()["pattern"], "^(?:[ -~]){4}$");
}

#[test]
fn opted_out_traits_can_be_written_by_hand() {
    let secret: Secret = "hunter2".parse().unwrap();
    assert_eq!(format!("{secret} {secret:?}"), "*** ***");
    assert_eq!(secret, "hunter2");
}

#[test]
fn eq_with_strings_both_ways() {
    let name: Name = "x".parse().unwrap();
    let string = String::from("x");

    assert!(name == *"x");
    assert!(*"x" == name);
    assert!(name == "x");
    assert!("x" == name);
    assert!(name == string);
    assert!(string == name);
    assert!(&name == "x");
    assert_ne!(name, "y");
    assert_eq!(name, "x".parse::<Name>().unwrap());
}

#[test]
fn ordered_by_str() {
    let mut names: Vec<Name> = ["b", "ab", "a"].map(|s| s.parse().unwrap()).into();
    names.sort();
    assert_eq!(names, ["a", "ab", "b"]);

    let mut colors = [Color::Red, Color::Green];
    colors.sort();
    assert_eq!(colors, ["green", "red"]);
}

#[test]
fn lookup_by_str_matches_the_stored_order() {
    let items = ["ab", "a", "b", "abc"];
    let tree: BTreeSet<Padded> = items.iter().map(|s| s.parse().unwrap()).collect();
    let hash: HashSet<Padded> = items.iter().map(|s| s.parse().unwrap()).collect();

    for item in items {
        assert_eq!(tree.get(item).unwrap(), item);
        assert!(hash.contains(item));
    }
    assert!(!tree.contains("abz"));
    let order: Vec<&str> = tree.iter().map(|padded| padded.as_str()).collect();
    assert_eq!(order, ["a", "ab", "abc", "b"]);
}

#[test]
fn serde_round_trip() {
    let code: Code = "ab-c".parse().unwrap();
    let json = serde_json::to_string(&code).unwrap();
    assert_eq!(json, r#""ab-c""#);
    assert_eq!(serde_json::from_str::<Code>(&json).unwrap(), code);
    assert_eq!(
        serde_json::from_reader::<_, Code>(json.as_bytes()).unwrap(),
        code
    );

    let owned = StringDeserializer::<DeError>::new("ab-c".into());
    assert_eq!(Code::deserialize(owned).unwrap(), code);
    let bytes = BytesDeserializer::<DeError>::new(b"ab-c");
    assert_eq!(Code::deserialize(bytes).unwrap(), code);
    assert!(Code::deserialize(BytesDeserializer::<DeError>::new(b"ab\xffc")).is_err());

    let error = serde_json::from_str::<Code>(r#""abc""#).unwrap_err();
    assert!(error.to_string().starts_with("invalid length of string"));
    assert_eq!(
        serde_json::from_str::<Color>(r#""red""#).unwrap(),
        Color::Red
    );
}

/// User items that shadow the names the generated code uses.
#[allow(dead_code, non_camel_case_types, non_upper_case_globals, unused_macros)]
mod shadowed {
    use eva::str::{CompactString, FixedUtf8, ParseError, Seq, ascii};

    struct str;
    struct bool;
    struct usize;
    struct String;
    struct Cow;
    struct Some;
    struct None;
    struct Ok;
    struct Err;
    struct Ordering;
    struct Formatter;
    trait Clone {}
    trait Copy {}
    trait PartialEq {}
    trait Eq {}
    trait PartialOrd {}
    trait Ord {}
    trait Hash {}
    trait Debug {}
    trait Display {}
    trait FromStr {}
    mod core {}

    const rest: u8 = 0;
    const other: u8 = 0;
    const s: u8 = 0;
    const f: u8 = 0;
    const state: u8 = 0;
    const len: u8 = 0;
    const buf: u8 = 0;
    const expected: u8 = 0;
    const serializer: u8 = 0;
    const deserializer: u8 = 0;

    macro_rules! concat {
        ($($_:tt)*) => {
            compile_error!("shadowed")
        };
    }
    macro_rules! stringify {
        ($($_:tt)*) => {
            compile_error!("shadowed")
        };
    }
    macro_rules! module_path {
        () => {
            compile_error!("shadowed")
        };
    }

    /// Two digits.
    #[eva::str(fixed(error = ParseError))]
    pub struct Code(Seq<2, ascii::Digit>);

    #[eva::str(fixed(error = ParseError))]
    pub struct Generic<T: FixedUtf8>(T);

    #[eva::str(newtype, not(schemars))]
    pub struct Name {
        inner: CompactString,
    }

    #[eva::str(custom)]
    pub struct Custom;

    impl Custom {
        fn as_str(&self) -> &::core::primitive::str {
            "custom"
        }
    }

    impl ::core::str::FromStr for Custom {
        type Err = ParseError;

        fn from_str(input: &::core::primitive::str) -> Result<Self, Self::Err> {
            match input {
                "custom" => Result::Ok(Self),
                _ => Result::Err(ParseError::Char),
            }
        }
    }
}

#[test]
fn hygiene() {
    let code: shadowed::Code = "42".parse().unwrap();
    assert_eq!(code, "42");
    assert_eq!(format!("{code} {code:?}"), r#"42 "42""#);
    assert_eq!(
        serde_json::from_str::<shadowed::Code>(r#""42""#).unwrap(),
        code
    );
    assert_eq!(schema::<shadowed::Code>()["description"], "Two digits.");

    let generic: shadowed::Generic<ascii::Digit> = "7".parse().unwrap();
    assert_eq!(generic, "7");

    let name: shadowed::Name = "x".parse().unwrap();
    assert!(name < "y".parse().unwrap());
    assert_eq!(serde_json::to_string(&name).unwrap(), r#""x""#);

    let custom: shadowed::Custom = "custom".parse().unwrap();
    assert_eq!(String::from("custom"), custom);
}
