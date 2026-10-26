//! # Array utilities and types.

use std::{
    borrow::{Borrow, Cow},
    fmt,
    ops::Deref,
    sync::Arc,
};

use eva_macros::data;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize, de};

#[data(copy, error, display("too big, expected array no bigger than {LIMIT}"), crate = crate)]
pub struct TooBig<const LIMIT: usize>;

#[crate::perfect_derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ImmutableHeap<T, const MAX: usize>(Arc<[T]>);

impl<T, const MAX: usize> Default for ImmutableHeap<T, MAX> {
    fn default() -> Self {
        Self(Arc::new([]))
    }
}

impl<T, const MAX: usize> Borrow<[T]> for ImmutableHeap<T, MAX> {
    fn borrow(&self) -> &[T] {
        self.0.as_ref()
    }
}

impl<T, const MAX: usize> AsRef<[T]> for ImmutableHeap<T, MAX> {
    fn as_ref(&self) -> &[T] {
        self.0.as_ref()
    }
}

impl<T, const MAX: usize> Deref for ImmutableHeap<T, MAX> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        self.0.as_ref()
    }
}

impl<'a, T, const MAX: usize> IntoIterator for &'a ImmutableHeap<T, MAX> {
    type IntoIter = <&'a [T] as IntoIterator>::IntoIter;
    type Item = &'a T;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl<T, const MAX: usize> TryFrom<Vec<T>> for ImmutableHeap<T, MAX> {
    type Error = TooBig<MAX>;

    fn try_from(value: Vec<T>) -> Result<Self, Self::Error> {
        if value.len() > MAX {
            return Err(TooBig::<MAX>);
        }

        Ok(Self(value.into()))
    }
}

impl<T, const MAX: usize, const N: usize> TryFrom<[T; N]> for ImmutableHeap<T, MAX> {
    type Error = TooBig<MAX>;

    fn try_from(value: [T; N]) -> Result<Self, Self::Error> {
        if N > MAX {
            return Err(TooBig::<MAX>);
        }

        Ok(Self(Arc::new(value)))
    }
}

impl<'de, T: Deserialize<'de>, const MAX: usize> Deserialize<'de> for ImmutableHeap<T, MAX> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct Visitor<T, const MAX: usize> {
            v: Vec<T>,
        }

        impl<'d, T: Deserialize<'d>, const MAX: usize> de::Visitor<'d> for Visitor<T, MAX> {
            type Value = Vec<T>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                write!(formatter, "array no bigger than {MAX}")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: de::SeqAccess<'d>,
            {
                let mut dst = self.v;
                let mut fuel = MAX;

                loop {
                    let Some(element) = seq.next_element()? else {
                        break;
                    };

                    if fuel == 0 {
                        return Err(de::Error::custom(TooBig::<MAX>));
                    }

                    dst.push(element);
                    fuel -= 1;
                }

                Ok(dst)
            }
        }

        let v = deserializer.deserialize_seq(Visitor::<T, MAX> {
            v: Vec::with_capacity(MAX),
        })?;

        Ok(Self(v.into()))
    }
}

impl<T: Serialize, const MAX: usize> Serialize for ImmutableHeap<T, MAX> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl<T: JsonSchema, const MAX: usize> JsonSchema for ImmutableHeap<T, MAX> {
    fn schema_id() -> Cow<'static, str> {
        <Vec<T> as JsonSchema>::schema_id()
    }

    fn schema_name() -> Cow<'static, str> {
        <Vec<T> as JsonSchema>::schema_name()
    }

    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "array",
            "items": T::json_schema(generator),
            "maxItems": MAX,
        })
    }
}
