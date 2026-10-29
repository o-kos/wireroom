use crate::{Error, profile};
use dmd_protocol::{Json, Object, Parameters, Version};

/// Validated bindings and complete opaque producer metadata.
#[derive(Clone, Debug, PartialEq)]
pub struct Metadata {
    pub version: Version,
    pub protocol: Version,
    pub signal_sha256: String,
    pub module: String,
    pub info: Json,
    pub mask: Object,
    pub defaults: Parameters,
    pub freq: f64,
    pub params: Parameters,
    pub extra: Object,
}
impl Metadata {
    pub(crate) fn parse(text: &str) -> Result<Self, Error> {
        let mut map = object(profile::json(text)?, "metadata")?;
        if string(take(&mut map, "format")?, "format")? != "dmd-replay" {
            return Err(Error::Metadata("format"));
        }
        let version = version_value(take(&mut map, "version")?)?;
        let protocol = version_value(take(&mut map, "protocol")?)?;
        if version.major != 1 || protocol.major != 1 {
            return Err(Error::Version);
        }
        let signal_sha256 = string(take(&mut map, "signal_sha256")?, "signal_sha256")?;
        if signal_sha256.len() != 64
            || !signal_sha256
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(Error::Metadata("signal_sha256"));
        }
        let module = string(take(&mut map, "module")?, "module")?;
        if module.is_empty() || module.contains('\0') {
            return Err(Error::Metadata("module"));
        }
        let info = take(&mut map, "info")?;
        let mask = object(take(&mut map, "mask")?, "mask")?;
        let defaults = parameters(take(&mut map, "defaults")?)?;
        let params = parameters(take(&mut map, "params")?)?;
        if defaults.keys().any(|key| !params.contains_key(key)) {
            return Err(Error::Metadata("defaults"));
        }
        let freq = match take(&mut map, "freq")? {
            Json::Number(n) => n
                .as_str()
                .parse::<f64>()
                .map_err(|_| Error::Metadata("freq"))?,
            _ => return Err(Error::Metadata("freq")),
        };
        if !freq.is_finite() {
            return Err(Error::Metadata("freq"));
        }
        Ok(Self {
            version,
            protocol,
            signal_sha256,
            module,
            info,
            mask,
            defaults,
            freq,
            params,
            extra: map,
        })
    }
    /// Pure comparison of fixed settings; this executes no command.
    pub fn match_initial(
        &self,
        module: &str,
        freq: f64,
        overrides: &Parameters,
    ) -> Result<Parameters, Error> {
        if module != self.module {
            return Err(Error::Settings("module"));
        }
        if !freq.is_finite() || freq != self.freq {
            return Err(Error::Settings("frequency"));
        }
        let mut effective = self.defaults.clone();
        effective.extend(overrides.clone());
        if effective != self.params {
            return Err(Error::Settings("parameters"));
        }
        Ok(effective)
    }
}
fn take(map: &mut Object, key: &'static str) -> Result<Json, Error> {
    map.remove(key).ok_or(Error::Metadata(key))
}
fn object(value: Json, key: &'static str) -> Result<Object, Error> {
    match value {
        Json::Object(map) => Ok(map),
        _ => Err(Error::Metadata(key)),
    }
}
fn string(value: Json, key: &'static str) -> Result<String, Error> {
    match value {
        Json::String(text) => Ok(text),
        _ => Err(Error::Metadata(key)),
    }
}
fn parameters(value: Json) -> Result<Parameters, Error> {
    object(value, "parameters")?
        .into_iter()
        .map(|(k, v)| Ok((k, string(v, "parameters")?)))
        .collect()
}
fn version_value(value: Json) -> Result<Version, Error> {
    let mut map = object(value, "version")?;
    let major = integer(take(&mut map, "major")?)?;
    let minor = integer(take(&mut map, "minor")?)?;
    if !map.is_empty() {
        return Err(Error::Metadata("version"));
    }
    Ok(Version { major, minor })
}
fn integer(value: Json) -> Result<u32, Error> {
    match value {
        Json::Number(n) if n.as_str().bytes().all(|b| b.is_ascii_digit()) => {
            n.as_str().parse().map_err(|_| Error::Metadata("version"))
        }
        _ => Err(Error::Metadata("version")),
    }
}
