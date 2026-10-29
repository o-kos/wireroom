//! Exact JSON values, independent of binary64 and serde's private number markers.
use std::collections::BTreeMap;

use crate::ValidationError;

/// Implementation budget for nested JSON containers (not a replay-format limit).
pub const MAX_DEPTH: usize = 256;

/// An exact JSON number token. Construction validates the JSON number grammar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Number(String);

impl Number {
    pub fn new(token: impl Into<String>) -> Result<Self, ValidationError> {
        let token = token.into();
        let value = Json::parse(&token)?;
        match value {
            Json::Number(number) if number.0 == token => Ok(number),
            _ => Err(ValidationError::InvalidNumber),
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Producer JSON with exact number tokens and unique decoded object keys.
/// Object order and whitespace are not retained. No producer key is reserved here.
#[derive(Clone, Debug, PartialEq, Eq)]
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
        let mut parser = Parser { input, offset: 0 };
        let value = parser.value(0)?;
        parser.whitespace();
        if parser.offset != input.len() {
            return Err(ValidationError::Json {
                offset: parser.offset,
            });
        }
        Ok(value)
    }

    /// Check the explicit implementation depth budget, also for locally built values.
    pub fn validate(&self) -> Result<(), ValidationError> {
        self.validate_at_depth(0)
    }

    fn validate_at_depth(&self, depth: usize) -> Result<(), ValidationError> {
        match self {
            Self::Array(values) => {
                check_depth(depth)?;
                for value in values {
                    value.validate_at_depth(depth + 1)?;
                }
            }
            Self::Object(values) => {
                check_depth(depth)?;
                for value in values.values() {
                    value.validate_at_depth(depth + 1)?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Compact JSON, subject to the explicit container-depth budget.
    pub fn to_compact_string(&self) -> Result<String, ValidationError> {
        let mut output = String::new();
        self.append(&mut output, 0)?;
        Ok(output)
    }

    fn append(&self, output: &mut String, depth: usize) -> Result<(), ValidationError> {
        match self {
            Self::Null => output.push_str("null"),
            Self::Bool(value) => output.push_str(if *value { "true" } else { "false" }),
            Self::Number(value) => output.push_str(value.as_str()),
            Self::String(value) => append_string(output, value)?,
            Self::Array(values) => {
                check_depth(depth)?;
                output.push('[');
                for (index, value) in values.iter().enumerate() {
                    if index != 0 {
                        output.push(',');
                    }
                    value.append(output, depth + 1)?;
                }
                output.push(']');
            }
            Self::Object(values) => {
                check_depth(depth)?;
                output.push('{');
                for (index, (key, value)) in values.iter().enumerate() {
                    if index != 0 {
                        output.push(',');
                    }
                    append_string(output, key)?;
                    output.push(':');
                    value.append(output, depth + 1)?;
                }
                output.push('}');
            }
        }
        Ok(())
    }
}

fn append_string(output: &mut String, value: &str) -> Result<(), ValidationError> {
    let encoded = serde_json::to_string(value).map_err(|_| ValidationError::InvalidString)?;
    output.push_str(&encoded);
    Ok(())
}

fn check_depth(depth: usize) -> Result<(), ValidationError> {
    if depth >= MAX_DEPTH {
        return Err(ValidationError::DepthLimit);
    }
    Ok(())
}

struct Parser<'a> {
    input: &'a str,
    offset: usize,
}

impl Parser<'_> {
    fn error(&self) -> ValidationError {
        ValidationError::Json {
            offset: self.offset,
        }
    }
    fn byte(&self) -> Option<u8> {
        self.input.as_bytes().get(self.offset).copied()
    }
    fn whitespace(&mut self) {
        while matches!(self.byte(), Some(b' ' | b'\t' | b'\r' | b'\n')) {
            self.offset += 1;
        }
    }
    fn value(&mut self, depth: usize) -> Result<Json, ValidationError> {
        self.whitespace();
        match self.byte() {
            Some(b'{') => self.object(depth),
            Some(b'[') => self.array(depth),
            Some(b'"') => self.string().map(Json::String),
            Some(b't') => self.literal("true", Json::Bool(true)),
            Some(b'f') => self.literal("false", Json::Bool(false)),
            Some(b'n') => self.literal("null", Json::Null),
            Some(b'-' | b'0'..=b'9') => self.number(),
            _ => Err(self.error()),
        }
    }
    fn literal(&mut self, token: &str, value: Json) -> Result<Json, ValidationError> {
        if !self.input[self.offset..].starts_with(token) {
            return Err(self.error());
        }
        self.offset += token.len();
        Ok(value)
    }
    fn string(&mut self) -> Result<String, ValidationError> {
        let start = self.offset;
        self.offset += 1;
        while let Some(byte) = self.byte() {
            self.offset += 1;
            match byte {
                b'"' => {
                    return serde_json::from_str(&self.input[start..self.offset])
                        .map_err(|_| self.error());
                }
                b'\\' => {
                    if self.byte().is_none() {
                        return Err(self.error());
                    }
                    self.offset += 1;
                }
                _ => {}
            }
        }
        Err(self.error())
    }
    fn digits(&mut self) -> Result<(), ValidationError> {
        let start = self.offset;
        while matches!(self.byte(), Some(b'0'..=b'9')) {
            self.offset += 1;
        }
        if self.offset == start {
            return Err(self.error());
        }
        Ok(())
    }
    fn number(&mut self) -> Result<Json, ValidationError> {
        let start = self.offset;
        if self.byte() == Some(b'-') {
            self.offset += 1;
        }
        if self.byte() == Some(b'0') {
            self.offset += 1;
        } else {
            self.digits()?;
        }
        if self.byte() == Some(b'.') {
            self.offset += 1;
            self.digits()?;
        }
        if matches!(self.byte(), Some(b'e' | b'E')) {
            self.offset += 1;
            if matches!(self.byte(), Some(b'+' | b'-')) {
                self.offset += 1;
            }
            self.digits()?;
        }
        Ok(Json::Number(Number(
            self.input[start..self.offset].to_owned(),
        )))
    }
    fn separator(&mut self, end: u8) -> Result<bool, ValidationError> {
        self.whitespace();
        let byte = self.byte();
        self.offset += usize::from(byte.is_some());
        match byte {
            Some(value) if value == end => Ok(false),
            Some(b',') => Ok(true),
            _ => Err(self.error()),
        }
    }
    fn empty(&mut self, end: u8) -> bool {
        self.offset += 1;
        self.whitespace();
        if self.byte() == Some(end) {
            self.offset += 1;
            true
        } else {
            false
        }
    }
    fn array(&mut self, depth: usize) -> Result<Json, ValidationError> {
        check_depth(depth)?;
        let mut values = Vec::new();
        if self.empty(b']') {
            return Ok(Json::Array(values));
        }
        loop {
            values.push(self.value(depth + 1)?);
            if !self.separator(b']')? {
                break;
            }
        }
        Ok(Json::Array(values))
    }
    fn object(&mut self, depth: usize) -> Result<Json, ValidationError> {
        check_depth(depth)?;
        let mut values = BTreeMap::new();
        if self.empty(b'}') {
            return Ok(Json::Object(values));
        }
        loop {
            self.whitespace();
            if self.byte() != Some(b'"') {
                return Err(self.error());
            }
            let key = self.string()?;
            self.whitespace();
            if self.byte() != Some(b':') {
                return Err(self.error());
            }
            self.offset += 1;
            let value = self.value(depth + 1)?;
            if values.insert(key.clone(), value).is_some() {
                return Err(ValidationError::DuplicateKey(key));
            }
            if !self.separator(b'}')? {
                break;
            }
        }
        Ok(Json::Object(values))
    }
}
