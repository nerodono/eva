use std::{borrow::Cow, ops::Deref, time::Duration};

use compact_str::{CompactString, ToCompactString};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::data;

#[repr(transparent)]
#[data(crate = crate, copy, not(serde, schemars))]
pub struct HumanDuration(pub Duration);

impl Deref for HumanDuration {
    type Target = Duration;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl JsonSchema for HumanDuration {
    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("HumanDuration")
    }

    fn schema_id() -> Cow<'static, str> {
        Cow::Borrowed(concat!(module_path!(), "::HumanDuration"))
    }

    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        String::json_schema(generator)
    }
}

impl<'de> Deserialize<'de> for HumanDuration {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = CompactString::deserialize(deserializer)?;
        humantime::parse_duration(&s)
            .map_err(serde::de::Error::custom)
            .map(Self)
    }
}

impl Serialize for HumanDuration {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        humantime::format_duration(self.0)
            .to_compact_string()
            .serialize(serializer)
    }
}
