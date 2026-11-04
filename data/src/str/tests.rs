use std::str::FromStr;

use super::{FixedUtf8, HasPattern, ParseError, Seq, ascii};

type Digits = Seq<3, ascii::Digit>;
type Printable = Seq<4, ascii::Printable>;

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

#[test]
fn seq_round_trip() {
    let seq: Printable = "ab-c".parse().unwrap();
    assert_eq!(seq.as_str(), "ab-c");
}

#[test]
fn seq_rejects_multibyte_char_of_right_byte_length() {
    assert_eq!("abé".len(), 4);
    assert_eq!("abé".parse::<Printable>(), Err(ParseError::Char));
    assert_eq!("é".parse::<Seq<2, ascii::Char>>(), Err(ParseError::Char));
    assert_eq!("é".parse::<Seq<2, AnyByte>>(), Err(ParseError::Char));
    assert_eq!("aé".parse::<Seq<3, AnyByte>>(), Err(ParseError::Char));
    let seq: Seq<2, AnyByte> = "ab".parse().unwrap();
    assert_eq!(seq.0.map(|byte| byte.0), *b"ab");
}

#[test]
fn seq_rejects_wrong_length() {
    assert_eq!("abc".parse::<Printable>(), Err(ParseError::Length));
    assert_eq!("abcde".parse::<Printable>(), Err(ParseError::Length));
}

#[test]
fn seq_pattern_groups_the_element() {
    assert_eq!(Digits::regex_pat(), "(?:[0-9]){3}");
    assert_eq!(Seq::<2, Digits>::regex_pat(), "(?:(?:[0-9]){3}){2}");
}

#[test]
fn ascii_pattern_escapes_backslash() {
    let mut pat = String::new();
    ascii::push_ascii_pat(b'\\', &mut pat);
    assert_eq!(pat, r"\\");
}
