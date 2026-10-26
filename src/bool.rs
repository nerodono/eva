//! Boolean values whose type fixes their only possible value.

use std::borrow::Cow;

use crate::schemars::{self, JsonSchema};
use serde::{Deserialize, Serialize, de};

macro_rules! constant_bool {
    ($name:ident, $error:ident, $value:literal, $display:literal, $message:literal) => {
        #[crate::data(copy, ord, error, display($message), crate = crate)]
        pub struct $error;

        #[crate::data(copy, ord, not(serde, schemars), display($display), crate = crate)]
        #[derive(Default)]
        pub enum $name {
            #[default]
            Value,
        }

        impl $name {
            pub const fn new() -> Self {
                Self::Value
            }
        }

        impl From<$name> for bool {
            fn from(_: $name) -> Self {
                $value
            }
        }

        impl TryFrom<bool> for $name {
            type Error = $error;

            fn try_from(value: bool) -> Result<Self, Self::Error> {
                if value == $value { Ok(Self::Value) } else { Err($error) }
            }
        }

        impl Serialize for $name {
            fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_bool($value)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                Self::try_from(bool::deserialize(deserializer)?).map_err(de::Error::custom)
            }
        }

        impl JsonSchema for $name {
            fn schema_id() -> Cow<'static, str> {
                concat!(module_path!(), "::", stringify!($name)).into()
            }

            fn schema_name() -> Cow<'static, str> {
                stringify!($name).into()
            }

            fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
                schemars::json_schema!({"type": "boolean", "const": $value})
            }
        }
    };
}

constant_bool!(True, MustBeTrue, true, "true", "must be `true`");
constant_bool!(False, MustBeFalse, false, "false", "must be `false`");
