use crate::hash::blake3;

#[test]
fn blake3_from_str_and_back_identical() {
    const STR: &str = "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff";

    let h: blake3::Hash = STR.parse().unwrap();
    let hex = h.hex();

    assert_eq!(hex.as_str(), STR);
}

#[test]
fn blake3_hex_round_trip_all_digits() {
    let hash = blake3::hash(b"eva");
    let hex = hash.hex();

    let parsed: blake3::Hash = hex.as_str().parse().unwrap();
    assert_eq!(parsed, hash);
}

#[test]
fn blake3_parse_case_insensitive() {
    const LOWER: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    let lower: blake3::Hash = LOWER.parse().unwrap();
    let upper: blake3::Hash = LOWER.to_uppercase().parse().unwrap();

    assert_eq!(lower, upper);
    assert_eq!(lower.hex().as_str(), LOWER);
}

#[test]
fn blake3_rejects_non_hex_letters() {
    const STR: &str = "gggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggggg";

    assert!(STR.parse::<blake3::Hash>().is_err());
}
