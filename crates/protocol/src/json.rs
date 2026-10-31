//! Ordinary serde JSON values with unique decoded object keys.
use crate::ValidationError;
use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{self, MapAccess, SeqAccess, Visitor},
};
use std::{collections::BTreeMap, fmt};

/// Match serde_json's default parser container budget.
pub const MAX_DEPTH: usize = 127;

/// An ordinary i64/u64 or finite f64 JSON number, in serde's normalized spelling.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Number(String);
impl Number {
    pub fn new(token: impl Into<String>) -> Result<Self, ValidationError> {
        let number: serde_json::Number =
            serde_json::from_str(&token.into()).map_err(|_| ValidationError::InvalidNumber)?;
        Ok(Self(number.to_string()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl Serialize for Number {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let number: serde_json::Number =
            serde_json::from_str(&self.0).map_err(serde::ser::Error::custom)?;
        number.serialize(serializer)
    }
}

/// Object order and numeric spelling are not retained; u64 integers stay exact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub enum Json {
    Null,
    Bool(bool),
    Number(Number),
    String(String),
    Array(Vec<Json>),
    Object(BTreeMap<String, Json>),
}
impl Json {
    pub fn parse(input: &str) -> Result<Self, ValidationError> {
        serde_json::from_str(input).map_err(|error| ValidationError::Json(error.to_string()))
    }
    pub fn validate(&self) -> Result<(), ValidationError> {
        self.validate_at_depth(0)
    }
    fn validate_at_depth(&self, depth: usize) -> Result<(), ValidationError> {
        let children: Box<dyn Iterator<Item = &Json> + '_> = match self {
            Self::Array(values) => Box::new(values.iter()),
            Self::Object(values) => Box::new(values.values()),
            _ => return Ok(()),
        };
        if depth >= MAX_DEPTH {
            return Err(ValidationError::DepthLimit);
        }
        for value in children {
            value.validate_at_depth(depth + 1)?;
        }
        Ok(())
    }
    pub fn to_compact_string(&self) -> Result<String, ValidationError> {
        self.validate()?;
        serde_json::to_string(self).map_err(|_| ValidationError::InvalidNumber)
    }
}

impl<'de> Deserialize<'de> for Json {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(JsonVisitor)
    }
}
struct JsonVisitor;
impl<'de> Visitor<'de> for JsonVisitor {
    type Value = Json;
    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("ordinary JSON with unique object keys")
    }
    fn visit_unit<E: de::Error>(self) -> Result<Json, E> {
        Ok(Json::Null)
    }
    fn visit_bool<E: de::Error>(self, value: bool) -> Result<Json, E> {
        Ok(Json::Bool(value))
    }
    fn visit_i64<E: de::Error>(self, value: i64) -> Result<Json, E> {
        Ok(Json::Number(Number(value.to_string())))
    }
    fn visit_u64<E: de::Error>(self, value: u64) -> Result<Json, E> {
        Ok(Json::Number(Number(value.to_string())))
    }
    fn visit_f64<E: de::Error>(self, value: f64) -> Result<Json, E> {
        let number =
            serde_json::Number::from_f64(value).ok_or_else(|| E::custom("nonfinite number"))?;
        Ok(Json::Number(Number(number.to_string())))
    }
    fn visit_str<E: de::Error>(self, value: &str) -> Result<Json, E> {
        Ok(Json::String(value.into()))
    }
    fn visit_string<E: de::Error>(self, value: String) -> Result<Json, E> {
        Ok(Json::String(value))
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Json, A::Error> {
        let mut values = Vec::new();
        while let Some(value) = sequence.next_element()? {
            values.push(value);
        }
        Ok(Json::Array(values))
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Json, A::Error> {
        let mut values = BTreeMap::new();
        while let Some((key, value)) = map.next_entry::<String, Json>()? {
            if values.insert(key.clone(), value).is_some() {
                return Err(de::Error::custom(format!("duplicate key: {key}")));
            }
        }
        Ok(Json::Object(values))
    }
}
