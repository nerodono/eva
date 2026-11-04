use eva_data::str::{HasPattern, ParseError, Seq, ascii};

#[eva_data::int(i8, -2..=0 | 4)]
enum Small {}

#[eva_data::int(newtype(n != 0))]
struct NonZero(Small);

#[eva_data::str(fixed(error = ParseError))]
struct Pin(Seq<4, ascii::Digit>);

#[eva_data::str(newtype)]
struct Name(String);

#[test]
fn standalone_data_defines_checked_integer_and_string_types() {
    assert_eq!(Small::VALUES, [-2, -1, 0, 4]);
    assert_eq!(Small::new(3), None);
    assert_eq!(NonZero::try_new(0), None);
    assert_eq!(NonZero::try_new(4).unwrap().prim(), 4);
    assert_eq!(serde_json::from_str::<Small>("-1").unwrap(), Small::NEG1);
    assert!(serde_json::from_str::<Small>("3").is_err());

    let pin: Pin = "0042".parse().unwrap();
    assert_eq!(pin.as_str(), "0042");
    assert_eq!(serde_json::to_string(&pin).unwrap(), r#""0042""#);
    assert!("12x4".parse::<Pin>().is_err());
    assert_eq!(Pin::regex_pat(), "(?:[0-9]){4}");
    let name: Name = "Eva".parse().unwrap();
    assert_eq!(name, "Eva");
}

#[cfg(feature = "rand")]
#[test]
fn random_sampling_covers_custom_and_builtin_types() {
    use rand::{Rng as _, SeedableRng as _};
    let mut rng = rand::rngs::StdRng::seed_from_u64(42);
    for _ in 0..256 {
        let value: Small = rng.random();
        assert!(Small::VALUES.contains(&value.into_inner()));
        let digit: ascii::Digit = rng.random();
        assert!(digit.parse() < 10);
    }
}
