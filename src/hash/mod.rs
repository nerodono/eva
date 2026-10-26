use std::{borrow::Cow, fmt, marker::PhantomData, mem, str::FromStr};

use perfect_derive::perfect_derive;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    error::shit_happens,
    int, single_ascii_char, str,
    str::{HasPattern, ParseError, PhantomDataStr, Seq},
};

pub mod blake3;

pub type U64Hasher = ahash::AHasher;

#[int(u8, b'0'..=b'9' | b'A'..=b'F' | b'a'..=b'f', crate = crate)]
pub enum HexChar {}

single_ascii_char!(HexChar);
unsafe impl str::FixedUtf8 for HexChar {}

/// Hexadecimal representation of the hash.
#[str(fixed(error = ParseError), crate = crate)]
pub struct Hex<IH: IsHash, const SIZE: usize> {
    buf: Seq<SIZE, Seq<2, HexChar>>,
    _marker: PhantomDataStr<IH>,
}

impl<IH: IsHash, const SIZE: usize> Hex<IH, SIZE> {
    pub const fn parse(self) -> Hash<IH, SIZE> {
        const fn hexc(c: u8) -> u8 {
            let u = c.to_ascii_uppercase();
            match u {
                b'0'..=b'9' => u - b'0',
                b'A'..=b'F' => 10 + u - b'A',
                _ => shit_happens(),
            }
        }

        let mut out = [0_u8; SIZE];
        let mut offset = 0_usize;

        while offset < SIZE {
            let [upper, lower] = self.buf.0[offset].0;
            let upper = hexc(upper.into_inner());
            let lower = hexc(lower.into_inner());

            out[offset] = (upper << 4) | lower;
            offset += 1;
        }

        Hash::new(out)
    }
}

pub trait State {
    fn update(&mut self, buf: &[u8]);
}

#[perfect_derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Hash<IH: IsHash, const SIZE: usize> {
    buf: [u8; SIZE],
    _marker: PhantomData<[IH; 0]>,
}

impl<IH: IsHash, const SIZE: usize> fmt::Display for Hash<IH, SIZE> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.hex().as_str())
    }
}

impl<IH: IsHash, const SIZE: usize> JsonSchema for Hash<IH, SIZE> {
    fn schema_id() -> Cow<'static, str> {
        format!("{}::{}", module_path!(), Self::schema_name()).into()
    }

    fn schema_name() -> Cow<'static, str> {
        format!("Hash_{}__{}", IH::schema_name(), SIZE).into()
    }

    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        _ = generator;

        schemars::json_schema!({"anyOf": [
            {
                "type": "string",
                "description": format!("Hexadecimal representation of {} hash, only in human-readable formats", IH::schema_name()),
                "minLength": SIZE * 2,
                "maxLength": SIZE * 2,
                "pattern": <Hex<IH, SIZE>>::regex_pat_fullmatch(),
            },
            {"type": "array", "minItems": SIZE, "maxItems": SIZE, "items": {
                "type": "integer",
                "minimum": 0,
                "maximum": 255
            }}
        ]})
    }
}

impl<'de, IH: IsHash, const SIZE: usize> Deserialize<'de> for Hash<IH, SIZE>
where
    [u8; SIZE]: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        if deserializer.is_human_readable() {
            let hex = Hex::deserialize(deserializer)?;
            Ok(hex.parse())
        } else {
            let buf = <[u8; SIZE] as Deserialize<'de>>::deserialize(deserializer)?;
            Ok(Self {
                buf,
                _marker: PhantomData,
            })
        }
    }
}

impl<IH: IsHash, const SIZE: usize> Serialize for Hash<IH, SIZE> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if serializer.is_human_readable() {
            self.hex().serialize(serializer)
        } else {
            self.buf.serialize(serializer)
        }
    }
}

impl<IH: IsHash, const SIZE: usize> FromStr for Hash<IH, SIZE> {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Hex::from_str(s).map(|h| h.parse())
    }
}

impl<IH: IsHash, const SIZE: usize> Hash<IH, SIZE> {
    pub const fn new(buf: [u8; SIZE]) -> Self {
        Self {
            buf,
            _marker: PhantomData,
        }
    }

    pub const fn hex(&self) -> Hex<IH, SIZE> {
        const fn hexc(c: u8) -> u8 {
            match c {
                0..=9 => b'0' + c,
                10..=15 => b'a' + (c - 10),
                _ => shit_happens(),
            }
        }
        let mut out: [[u8; 2]; SIZE] = [[0, 0]; SIZE];
        let mut offset = 0_usize;

        while offset < SIZE {
            let num = self.buf[offset];
            let [upper, lower] = &mut out[offset];
            *upper = hexc(num >> 4);
            *lower = hexc(num & 0b1111);

            offset += 1;
        }

        Hex {
            buf: unsafe { mem::transmute_copy(&out) },
            _marker: PhantomDataStr(PhantomData),
        }
    }
}

pub trait IsHash: JsonSchema + Copy + 'static + Send + Sync + Ord {
    const FULL_SIZE: usize;
}

#[cfg(test)]
mod tests;
