use core::{fmt, marker::PhantomData, str::FromStr};

use serde::de;

/// Deserializes `T` from a string through [`FromStr`] without allocating.
pub fn deserialize_from_str<'de, T, D>(deserializer: D) -> Result<T, D::Error>
where
    T: FromStr<Err: fmt::Display>,
    D: de::Deserializer<'de>,
{
    deserializer.deserialize_str(FromStrVisitor(PhantomData))
}

struct FromStrVisitor<T>(PhantomData<T>);

impl<T: FromStr<Err: fmt::Display>> de::Visitor<'_> for FromStrVisitor<T> {
    type Value = T;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a string")
    }

    fn visit_str<E: de::Error>(self, v: &str) -> Result<T, E> {
        v.parse().map_err(E::custom)
    }

    fn visit_bytes<E: de::Error>(self, v: &[u8]) -> Result<T, E> {
        match core::str::from_utf8(v) {
            Ok(v) => self.visit_str(v),
            Err(_) => Err(E::invalid_value(de::Unexpected::Bytes(v), &self)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn from_json(json: &str) -> Result<u8, serde_json::Error> {
        deserialize_from_str(&mut serde_json::Deserializer::from_str(json))
    }

    #[test]
    fn parses_through_from_str() {
        assert_eq!(from_json(r#""42""#).unwrap(), 42);
        assert!(from_json(r#""x""#).is_err());
        assert!(from_json("42").is_err());
    }
}
