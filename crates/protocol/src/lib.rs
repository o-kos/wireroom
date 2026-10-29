//! Run protocol 1.0: individual messages and bounded NDJSON transport.
//!
//! Callers own lifecycle/order/FIFO validation, monotonic frontiers, current-frame
//! equality, finalization and draining, process exit handling and EOF-as-stop.
//! Paths are checked lexically only. Callers must enforce containment without
//! check/open races, regular files, size, publication, immutability and lifetime.
//! No API here executes commands, accesses files or synthesizes runtime events.
//!
//! JSON containers have an explicit implementation budget of [`MAX_DEPTH`] levels;
//! this is independent of replay resource limits. Opaque numbers are never routed
//! through binary64. Known frequency and numeric speed fields intentionally are.
#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/README.md"))]

mod codec;
mod json;
mod types;
mod wire;

pub use codec::{DecodeError, EncodeError, MAX_LINE_BYTES, Reader, Writer};
pub use json::{Json, MAX_DEPTH, Number};
pub use types::*;

/// Protocol major, independent of crate, recording and ABI versions.
pub const PROTOCOL_VERSION: u32 = 1;
/// The version advertised by this implementation.
pub const CURRENT_VERSION: Version = Version { major: 1, minor: 0 };

/// Invalid individual-message data. All failures are inspectable without text parsing.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ValidationError {
    #[error("invalid JSON at byte {offset}")]
    Json { offset: usize },
    #[error("duplicate object key: {0}")]
    DuplicateKey(String),
    #[error("JSON container depth exceeds implementation budget")]
    DepthLimit,
    #[error("invalid JSON number token")]
    InvalidNumber,
    #[error("invalid JSON string")]
    InvalidString,
    #[error("expected {expected} in {field}")]
    Type {
        field: String,
        expected: &'static str,
    },
    #[error("missing field: {0}")]
    Missing(String),
    #[error("unknown field: {0}")]
    UnknownField(String),
    #[error("invalid value in {0}")]
    Value(String),
    #[error("unknown command: {0}")]
    UnknownCommand(String),
    #[error("unsupported protocol major: {0}")]
    UnsupportedMajor(u32),
}
