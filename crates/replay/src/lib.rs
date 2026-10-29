//! Read-only replay recording data. Source decoding, digest verification, playback
//! and safe output publication belong to callers. A digest is not authentication.
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/README.md"))]
mod archive;
mod index;
mod journal;
mod metadata;
mod profile;

pub use archive::{Asset, Recording};
pub use journal::{Journal, Record};
pub use metadata::Metadata;
use std::path::{Path, PathBuf};

/// Append only the literal companion suffix, preserving native path bytes.
pub fn companion_path(signal: impl AsRef<Path>) -> PathBuf {
    let mut path = signal.as_ref().as_os_str().to_os_string();
    path.push(".replay");
    PathBuf::from(path)
}

/// Inspectable failures; no error requires parsing diagnostic text.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid archive: {0}")]
    Archive(&'static str),
    #[error("invalid metadata field: {0}")]
    Metadata(&'static str),
    #[error("unsupported version pair")]
    Version,
    #[error("invalid JSON/message: {0}")]
    Json(#[from] dmd_protocol::ValidationError),
    #[error("journal framing: {0}")]
    Framing(&'static str),
    #[error("invalid journal sequence: {0}")]
    Sequence(&'static str),
    #[error("missing asset: {0}")]
    MissingAsset(String),
    #[error("conflicting asset reference: {0}")]
    ConflictingAsset(String),
    #[error("entry integrity: {0}")]
    Integrity(&'static str),
    #[error("resource limit: {0}")]
    Resource(&'static str),
    #[error("initial settings differ: {0}")]
    Settings(&'static str),
}
