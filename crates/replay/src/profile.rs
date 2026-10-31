use crate::Error;
use dmd_protocol::Json;

pub(crate) const ARCHIVE: u64 = u32::MAX as u64 - 1;
pub(crate) const DIRECTORY: u64 = 8 << 20;
pub(crate) const ENTRY: u64 = 1 << 30;
pub(crate) const ENTRIES: u64 = 4096;
pub(crate) const JSON_BYTES: usize = 1 << 24;

pub(crate) fn limit(value: u64, maximum: u64, name: &'static str) -> Result<(), Error> {
    if value > maximum {
        return Err(Error::Resource(name));
    }
    Ok(())
}
pub(crate) fn path(name: &str) -> Result<(), Error> {
    dmd_protocol::validate_result_path(name)?;
    limit(name.len() as u64, 4096, "path bytes")?;
    limit(name.split('/').count() as u64, 64, "path depth")?;
    platform_path(name)
}
#[cfg(not(windows))]
fn platform_path(_: &str) -> Result<(), Error> {
    Ok(())
}
#[cfg(windows)]
fn platform_path(name: &str) -> Result<(), Error> {
    for part in name.split('/') {
        let stem = part
            .split('.')
            .next()
            .unwrap_or_default()
            .to_ascii_uppercase();
        if part.ends_with(['.', ' '])
            || part.contains(['<', '>', '"', '|', '?', '*'])
            || part.chars().any(|c| c < ' ')
            || matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
            || reserved_port(&stem)
        {
            return Err(Error::Archive("unrepresentable path"));
        }
    }
    Ok(())
}
#[cfg(windows)]
fn reserved_port(stem: &str) -> bool {
    let suffix = stem
        .strip_prefix("COM")
        .or_else(|| stem.strip_prefix("LPT"));
    suffix.is_some_and(|suffix| {
        matches!(
            suffix,
            "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³"
        )
    })
}
pub(crate) fn json(text: &str) -> Result<Json, Error> {
    Ok(Json::parse(text)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn path_bytes_and_components_are_bounded_independently() {
        assert!(path(&"x".repeat(4096)).is_ok());
        assert!(matches!(path(&"x".repeat(4097)), Err(Error::Resource(_))));
        assert!(path(&vec!["x"; 64].join("/")).is_ok());
        assert!(matches!(
            path(&vec!["x"; 65].join("/")),
            Err(Error::Resource(_))
        ));
        assert!(path(&"é".repeat(2048)).is_ok());
        assert!(path(&"é".repeat(2049)).is_err());
    }
}
