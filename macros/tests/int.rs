use eva::_priv::schemars::{self, JsonSchema};
use serde_json::json;

#[eva::int(u8, 1..=3 | 7)]
enum Small {}

#[eva::int(u8, b'z' | 5..8 | b'a'..=b'c' | 7 | b'b')]
enum Scrambled {}

#[eva::int(u8, 0..=255)]
enum Byte {}

#[eva::int(i8, -128..=127)]
enum FullI8 {}

#[eva::int(i16, 200..=300 | -300..=-200)]
enum Split {}

#[eva::int(
    i128,
    170141183460469231731687303715884105727 | -1..=1 | -170141183460469231731687303715884105728
)]
enum WideI128 {}

#[eva::int(u64, 18446744073709551613..=18446744073709551615 | 0u64)]
enum WideU64 {}

#[eva::int(u32, 3000000000..=3000000002)]
enum BigU32 {}

#[eva::int(u128, 170141183460469231731687303715884105727)]
enum MaxU128 {}

#[test]
fn values_are_sorted_merged_and_deduped() {
    assert_eq!(Scrambled::RANGES, [5..=7, 97..=99, 122..=122]);
    assert_eq!(Scrambled::VALUES, [5, 6, 7, 97, 98, 99, 122]);
    assert_eq!(
        Scrambled::VARIANTS,
        [
            Scrambled::POS5,
            Scrambled::POS6,
            Scrambled::POS7,
            Scrambled::POS97,
            Scrambled::POS98,
            Scrambled::POS99,
            Scrambled::POS122,
        ]
    );
    for (i, variant) in Scrambled::VARIANTS.into_iter().enumerate() {
        assert_eq!(variant.nth(), i);
        assert_eq!(variant.into_inner(), Scrambled::VALUES[i]);
    }
}

fn assert_nth<T: Copy + PartialEq + std::fmt::Debug>(variants: &[T], nth: fn(T) -> usize) {
    for (i, &variant) in variants.iter().enumerate() {
        assert_eq!(nth(variant), i, "{variant:?}");
    }
}

#[test]
fn nth_of_signed_and_wide_ranges() {
    assert_nth(&FullI8::VARIANTS, FullI8::nth);
    assert_eq!(FullI8::VARIANTS.len(), 256);
    assert_eq!(FullI8::NEG128.nth(), 0);
    assert_eq!(FullI8::POS127.nth(), 255);

    assert_nth(&Split::VARIANTS, Split::nth);
    assert_eq!(Split::POS200.nth(), 101);

    assert_nth(&WideI128::VARIANTS, WideI128::nth);
    assert_eq!(WideI128::VALUES, [i128::MIN, -1, 0, 1, i128::MAX]);
    assert_eq!(
        WideI128::NEG170141183460469231731687303715884105728.into_inner(),
        i128::MIN
    );

    assert_nth(&WideU64::VARIANTS, WideU64::nth);
    assert_eq!(WideU64::VALUES, [0, u64::MAX - 2, u64::MAX - 1, u64::MAX]);

    assert_nth(&Byte::VARIANTS, Byte::nth);
    assert_eq!(MaxU128::VALUES, [i128::MAX as u128]);
}

#[test]
fn constructors_and_conversions() {
    assert_eq!(Small::new(7), Some(Small::POS7));
    assert_eq!(Small::new(4), None);
    assert_eq!(unsafe { Small::new_unchecked(2) }, Small::POS2);
    assert_eq!(Small::POS3.into_inner(), 3);
    assert_eq!(u8::from(Small::POS3), 3);
    assert_eq!(Small::POS3, 3);
    assert!(Small::POS1 < Small::POS7);
    assert_eq!(Split::NEG300.max(Split::POS200), Split::POS200);

    let three = const { Small::new(3).unwrap() };
    assert_eq!(three, Small::POS3);
    assert!(matches!(three, Small::POS3));
}

#[test]
fn repr_slices() {
    let mut values = [Small::POS1, Small::POS7];
    assert_eq!(Small::as_repr_slice(&values), [1, 7]);
    unsafe { Small::as_repr_slice_mut(&mut values)[0] = 2 };
    assert_eq!(values, [Small::POS2, Small::POS7]);
}

#[test]
fn formats_as_the_number() {
    assert_eq!(Split::NEG250.to_string(), "-250");
    assert_eq!(format!("{:?}", Scrambled::POS97), "97");
}

#[eva::int(u8, 0..=1, not(Debug, Display))]
enum Bit {}

impl std::fmt::Debug for Bit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "bit {}", self.into_inner())
    }
}

impl std::fmt::Display for Bit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(if *self == 1 { "on" } else { "off" })
    }
}

#[test]
fn not_debug_display() {
    assert_eq!(format!("{:?}", Bit::POS1), "bit 1");
    assert_eq!(Bit::POS0.to_string(), "off");
}

#[test]
fn serde_as_number() {
    assert_eq!(serde_json::to_string(&Split::NEG200).unwrap(), "-200");
    assert_eq!(serde_json::from_str::<Split>("250").unwrap(), Split::POS250);
}

fn deserialize_error<T: eva::_priv::serde::de::DeserializeOwned + std::fmt::Debug>(
    value: serde_json::Value,
) -> String {
    serde_json::from_value::<T>(value).unwrap_err().to_string()
}

#[eva::int(i128, -5..=5)]
enum SmallI128 {}

#[test]
fn deserialize_error_names_value_and_ranges() {
    assert_eq!(
        deserialize_error::<Small>(json!(4)),
        "invalid value: integer `4`, expected an integer in 1..=3 | 7"
    );
    assert_eq!(
        deserialize_error::<Split>(json!(-301)),
        "invalid value: integer `-301`, expected an integer in -300..=-200 | 200..=300"
    );
    assert_eq!(
        deserialize_error::<SmallI128>(json!(6)),
        "invalid value: integer `6`, expected an integer in -5..=5"
    );
    assert_eq!(
        deserialize_error::<Small>(json!(256)),
        "invalid value: integer `256`, expected u8"
    );
}

fn any_of<T: JsonSchema>() -> serde_json::Value {
    let schema = serde_json::to_value(schemars::schema_for!(T)).unwrap();
    schema["anyOf"].clone()
}

/// Percentage
/// of something.
#[eva::int(u8, 0..=100 | 200)]
enum Percent {}

#[test]
fn schema() {
    let schema = serde_json::to_value(schemars::schema_for!(Percent)).unwrap();
    let description = schema["description"].as_str().unwrap();
    let words = description.split_whitespace().collect::<Vec<_>>();
    assert_eq!(words.join(" "), "Percentage of something.");
    assert_eq!(
        schema["anyOf"],
        json!([
            {"type": "integer", "minimum": 0, "maximum": 100},
            {"type": "integer", "minimum": 200, "maximum": 200},
        ])
    );
}

#[test]
fn schema_of_wide_types() {
    assert_eq!(
        any_of::<BigU32>(),
        json!([{"type": "integer", "minimum": 3000000000u32, "maximum": 3000000002u32}])
    );
    assert_eq!(
        any_of::<WideU64>(),
        json!([
            {"type": "integer", "minimum": 0, "maximum": 0},
            {"type": "integer", "minimum": u64::MAX - 2, "maximum": u64::MAX},
        ])
    );
    // `i128::MAX as f64` is 2^127, above the value.
    let max = i128::MAX as f64;
    assert_eq!(
        any_of::<WideI128>(),
        json!([
            {"type": "integer", "minimum": i128::MIN as f64, "maximum": i128::MIN as f64},
            {"type": "integer", "minimum": -1, "maximum": 1},
            {"type": "integer", "minimum": max.next_down(), "maximum": max},
        ])
    );
}

/// -2^63 - 1, which rounds up to -2^63, and 2^64 + 1, which rounds down to 2^64.
#[eva::int(i128, -9223372036854775809 | 18446744073709551617)]
enum PastI64 {}

#[test]
fn inexact_schema_bounds_round_outward() {
    let (low, high) = (-(2_f64.powi(63)), 2_f64.powi(64));
    assert_eq!(
        any_of::<PastI64>(),
        json!([
            {"type": "integer", "minimum": low.next_down(), "maximum": low},
            {"type": "integer", "minimum": high, "maximum": high.next_up()},
        ])
    );
}

#[test]
fn random_values_are_allowed() {
    use eva::rand::{Rng as _, SeedableRng as _, xoshiro::Xoshiro256PlusPlus};

    let mut rng = Xoshiro256PlusPlus::seed_from_u64(0);
    let mut seen = [false; 4];
    for _ in 0..200 {
        seen[rng.random::<Small>().nth()] = true;
    }
    assert_eq!(seen, [true; 4]);
}

/// Docs are read only for the schema, so other doc forms work with `not(schemars)`.
#[doc = concat!("Not", " a literal.")]
#[doc(hidden)]
#[eva::int(u8, 0..=1, not(schemars))]
enum ComputedDocs {}

/// Hidden, with a schema.
#[doc(hidden)]
#[eva::int(u8, 0..=1)]
enum HiddenDocs {}

#[test]
fn docs() {
    assert_eq!(ComputedDocs::VALUES, HiddenDocs::VALUES);
    let schema = serde_json::to_value(schemars::schema_for!(HiddenDocs)).unwrap();
    assert_eq!(schema["description"], "Hidden, with a schema.");
}

/// User items that shadow the names the generated code uses.
#[allow(dead_code, non_camel_case_types, non_upper_case_globals, unused_macros)]
mod shadowed {
    struct u8;
    struct i16;
    struct i64;
    struct u64;
    struct i128;
    struct u128;
    struct usize;
    struct bool;
    struct str;
    struct Some;
    struct None;
    struct Ok;
    struct Err;
    struct Cow;
    struct Ordering;
    struct Formatter;
    trait Clone {}
    trait Copy {}
    trait From {}
    trait Sized {}
    trait PartialEq {}
    trait Eq {}
    trait PartialOrd {}
    trait Ord {}
    trait Debug {}
    trait Display {}
    mod core {}

    const other: u8 = u8;
    const f: u8 = u8;
    const serializer: u8 = u8;
    const deserializer: u8 = u8;
    const this: u8 = u8;
    const index: u8 = u8;
    const rng: u8 = u8;

    macro_rules! unreachable {
        () => {
            compile_error!("shadowed")
        };
    }
    macro_rules! concat {
        ($($_:tt)*) => {
            compile_error!("shadowed")
        };
    }
    macro_rules! format_args {
        ($($_:tt)*) => {
            compile_error!("shadowed")
        };
    }
    macro_rules! module_path {
        () => {
            compile_error!("shadowed")
        };
    }

    #[eva::int(i16, -3..=3)]
    pub enum Option {}

    #[eva::int(u8, 0..=1)]
    pub enum Result {}

    #[eva::int(u8, 0..=1)]
    pub enum R {}

    #[eva::int(u8, 0..=1)]
    pub enum S {}

    #[eva::int(u128, 0..=1)]
    pub enum D {}

    #[eva::int(u8, 0..=1)]
    pub enum __R {}

    const ok: u8 = u8;
    const schema: u8 = u8;
    const generator: u8 = u8;
    struct Integer;
    struct Hash;

    #[eva::int(newtype(n < 10))]
    pub struct Nz(::core::num::NonZeroU16);

    #[eva::int(newtype)]
    pub struct Named {
        value: ::core::primitive::i32,
    }
}

#[test]
fn hygiene() {
    assert_eq!(shadowed::Option::new(-3).unwrap().nth(), 0);
    assert_eq!(shadowed::Option::NEG1.to_string(), "-1");
    assert_eq!(format!("{:?}", shadowed::Option::POS2), "2");
    assert!(shadowed::Option::NEG1 < shadowed::Option::POS1);
    assert_eq!(shadowed::Option::POS1, 1);
    assert_eq!(i16::from(shadowed::Option::POS3), 3);
    assert_eq!(serde_json::to_string(&shadowed::S::POS1).unwrap(), "1");
    assert_eq!(
        serde_json::from_str::<shadowed::D>("1").unwrap(),
        shadowed::D::POS1
    );
    assert!(serde_json::from_str::<shadowed::D>("2").is_err());
    assert!(serde_json::from_str::<shadowed::Result>("2").is_err());
    assert_eq!(
        schemars::schema_for!(shadowed::S).get("anyOf"),
        schemars::schema_for!(HiddenDocs).get("anyOf")
    );
    let _: shadowed::R = eva::rand::random();
    let _: shadowed::__R = eva::rand::random();
}

// == Newtypes ==

use std::num::{NonZeroI64, NonZeroU16, NonZeroU128};

use eva::int::{InfallibleInteger, Integer};

/// A TCP port.
#[eva::int(newtype)]
struct Port(NonZeroU16);

/// Above the well-known range.
#[eva::int(newtype(n >= 1024))]
struct HighPort(NonZeroU16);

#[eva::int(newtype(n % 2 == 0))]
struct Even(u8);

#[eva::int(newtype)]
struct Plain(u8);

#[eva::int(newtype)]
struct TelegramId {
    id: NonZeroI64,
}

#[eva::int(newtype)]
struct Wide(NonZeroU128);

/// Below half.
#[eva::int(newtype(n < 50))]
struct LowPercent(Percent);

#[eva::int(newtype)]
struct Nested(HighPort);

fn infallible<T: InfallibleInteger>() {}

#[test]
fn newtype_packs_the_primitive() {
    let port = Port::try_new(8080).unwrap();
    assert_eq!(port.prim(), 8080);
    assert_eq!(port.inner(), NonZeroU16::new(8080).unwrap());
    assert_eq!(port, 8080);
    assert_eq!(NonZeroU16::from(port), port.inner());
    assert_eq!(Port::try_new(0), None);
    assert_eq!(unsafe { Port::new_unchecked(1) }.prim(), 1);

    assert_eq!(HighPort::try_new(1024).map(|p| p.prim()), Some(1024));
    assert_eq!(HighPort::try_new(1023), None);
    assert_eq!(HighPort::try_new(0), None);

    assert_eq!(Even::try_new(4).map(|e| e.prim()), Some(4));
    assert_eq!(Even::try_new(3), None);
    assert_eq!(Even::try_new(4).unwrap().inner(), 4);

    let id = TelegramId::try_new(-5).unwrap();
    assert_eq!(id.prim(), -5);
    assert_eq!(id.inner(), NonZeroI64::new(-5).unwrap());
    assert_eq!(TelegramId::try_new(0), None);

    assert_eq!(LowPercent::try_new(49).map(|p| p.prim()), Some(49));
    assert_eq!(LowPercent::try_new(50), None);
    assert_eq!(LowPercent::try_new(49).unwrap().inner(), Percent::POS49);

    assert_eq!(Nested::try_new(2048).map(|n| n.prim()), Some(2048));
    assert_eq!(Nested::try_new(80), None);
}

#[test]
fn newtype_without_condition_is_infallible() {
    const LOCALHOST: Port = Port::new(NonZeroU16::new(8080).unwrap());
    assert_eq!(LOCALHOST.prim(), 8080);
    assert_eq!(Port::from(NonZeroU16::MIN), Port::new(NonZeroU16::MIN));
    assert_eq!(Plain::new(7), 7);
    assert_eq!(<Plain as InfallibleInteger>::new(7), Plain::new(7));
    assert_eq!(Nested::new(HighPort::try_new(2048).unwrap()).prim(), 2048);

    infallible::<Port>();
    infallible::<Plain>();
    infallible::<Nested>();
    infallible::<Percent>();
    infallible::<NonZeroU16>();
    assert_eq!(<Percent as Integer>::try_new(50), Some(Percent::POS50));
    assert_eq!(<Percent as Integer>::prim(&Percent::POS50), 50);
}

#[test]
fn newtype_compares_and_hashes_by_value() {
    let a = Port::new(NonZeroU16::new(1).unwrap());
    let b = Port::new(NonZeroU16::new(2).unwrap());
    assert!(a < b);
    assert_eq!(a.max(b), b);
    let set: std::collections::HashSet<Port> = [a, b, a].into_iter().collect();
    assert_eq!(set.len(), 2);
    let set: std::collections::HashSet<Percent> = Percent::VARIANTS.into_iter().collect();
    assert_eq!(set.len(), Percent::VARIANTS.len());

    const ONE: Port = Port(NonZeroU16::MIN);
    assert!(matches!(a, ONE));
    assert_eq!(size_of::<Option<Port>>(), size_of::<Port>());
}

#[test]
fn newtype_formats_as_the_number() {
    assert_eq!(Port::try_new(443).unwrap().to_string(), "443");
    assert_eq!(format!("{:?}", TelegramId::try_new(-1).unwrap()), "-1");
    assert_eq!(format!("{:>5}", Even::try_new(2).unwrap()), "    2");
}

#[test]
fn newtype_serde() {
    let port = HighPort::try_new(8080).unwrap();
    assert_eq!(serde_json::to_string(&port).unwrap(), "8080");
    assert_eq!(serde_json::from_str::<HighPort>("8080").unwrap(), port);
    assert_eq!(serde_json::from_str::<TelegramId>("-7").unwrap().prim(), -7);
    let max = serde_json::from_str::<Wide>(&u128::MAX.to_string()).unwrap();
    assert_eq!(max.prim(), u128::MAX);

    assert_eq!(
        deserialize_error::<Port>(json!(0)),
        "invalid value: integer `0`, expected a non-zero u16"
    );
    assert_eq!(
        deserialize_error::<HighPort>(json!(80)),
        "invalid value: integer `80`, expected a non-zero u16 where n >= 1024"
    );
    assert_eq!(
        deserialize_error::<Even>(json!(3)),
        "invalid value: integer `3`, expected u8 where n % 2 == 0"
    );
    assert_eq!(
        deserialize_error::<TelegramId>(json!(0)),
        "invalid value: integer `0`, expected a non-zero i64"
    );
    assert_eq!(
        deserialize_error::<Wide>(json!(0)),
        "invalid value: integer `0`, expected a non-zero u128"
    );
    assert_eq!(
        deserialize_error::<LowPercent>(json!(75)),
        "invalid value: integer `75`, expected an integer in 0..=100 | 200 where n < 50"
    );
    assert_eq!(
        deserialize_error::<Nested>(json!(80)),
        "invalid value: integer `80`, expected a non-zero u16 where n >= 1024"
    );
    assert_eq!(
        deserialize_error::<Port>(json!(65536)),
        "invalid value: integer `65536`, expected u16"
    );
}

#[test]
fn newtype_schema() {
    let schema = serde_json::to_value(schemars::schema_for!(Port)).unwrap();
    assert_eq!(schema["title"], "Port");
    assert_eq!(schema["description"], "A TCP port.");
    assert_eq!(schema["type"], "integer");
    assert_eq!(schema["format"], "uint16");
    assert_eq!(schema["minimum"], 1);

    let schema = serde_json::to_value(schemars::schema_for!(LowPercent)).unwrap();
    assert_eq!(schema["description"], "Below half.");
    assert_eq!(schema["anyOf"], any_of::<Percent>());

    let schema = serde_json::to_value(schemars::schema_for!(Even)).unwrap();
    assert!(schema.get("description").is_none(), "{schema}");
}

#[eva::int(newtype, not(Debug, Display, schemars), crate = ::eva)]
struct Secret(u64);

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("***")
    }
}

#[test]
fn newtype_not_debug_display_schemars() {
    let secret = Secret::new(42);
    assert_eq!(format!("{secret:?}"), "***");
    assert_eq!(serde_json::to_string(&secret).unwrap(), "42");
}

#[test]
fn newtype_hygiene() {
    assert_eq!(shadowed::Nz::try_new(3).unwrap().prim(), 3);
    assert_eq!(shadowed::Nz::try_new(0), None);
    assert_eq!(shadowed::Nz::try_new(2).unwrap().to_string(), "2");
    assert_eq!(format!("{:?}", shadowed::Nz::try_new(2).unwrap()), "2");
    assert_eq!(
        serde_json::to_string(&shadowed::Nz::try_new(1).unwrap()).unwrap(),
        "1"
    );
    assert_eq!(
        deserialize_error::<shadowed::Nz>(json!(0)),
        "invalid value: integer `0`, expected a non-zero u16 where n < 10"
    );
    assert_eq!(schemars::schema_for!(shadowed::Nz).get("description"), None);
    assert_eq!(shadowed::Named::new(1).inner(), 1);
    assert_eq!(shadowed::Named::try_new(1), Some(shadowed::Named::new(1)));
}

/// The condition runs after the field's check, so it may rely on the field's invariant.
#[eva::int(newtype(1024 % n == 0))]
struct Divisor(NonZeroU16);

#[eva::int(newtype(n < 2048))]
struct Stacked(HighPort);

macro_rules! bounded {
    ($name:ident: $inner:ty, $cond:expr) => {
        #[eva::int(newtype($cond))]
        struct $name($inner);
    };
}

macro_rules! kinded {
    ($kind:expr, $name:ident) => {
        #[eva::int($kind)]
        struct $name(u8);
    };
}

bounded!(Big: u8, n > 10);
kinded!(newtype, Forwarded);

#[test]
fn condition_after_the_field_check() {
    assert_eq!(Divisor::try_new(0), None);
    assert_eq!(Divisor::try_new(256).map(|d| d.prim()), Some(256));
    assert_eq!(Divisor::try_new(3), None);
}

#[test]
fn stacked_conditions_join_with_and() {
    assert_eq!(Stacked::try_new(1500).map(|s| s.prim()), Some(1500));
    assert_eq!(Stacked::try_new(2048), None);
    assert_eq!(
        deserialize_error::<Stacked>(json!(80)),
        "invalid value: integer `80`, expected a non-zero u16 where n >= 1024 and n < 2048"
    );
}

#[test]
fn condition_and_kind_through_macro_rules() {
    assert_eq!(Big::try_new(11).map(|b| b.prim()), Some(11));
    assert_eq!(Big::try_new(10), None);
    assert_eq!(Forwarded::new(3).prim(), 3);
}

/// Percent's docs do not leak into the newtype's schema.
#[test]
fn schema_description_is_the_newtypes_own() {
    #[eva::int(newtype)]
    struct Undocumented(Percent);

    let schema = serde_json::to_value(schemars::schema_for!(Undocumented)).unwrap();
    assert!(schema.get("description").is_none(), "{schema}");
    assert_eq!(schema["anyOf"], any_of::<Percent>());
}
