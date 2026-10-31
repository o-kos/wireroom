//! Run protocol 1.0: individual messages and bounded NDJSON transport.
//!
//! Callers own lifecycle/order/FIFO validation, monotonic frontiers, current-frame
//! equality, finalization and draining, process exit handling and EOF-as-stop.
//! Paths are checked lexically only. Callers must enforce containment without
//! check/open races, regular files, size, publication, immutability and lifetime.
//! No API here executes commands, accesses files or synthesizes runtime events.
//!
//! JSON containers have an explicit implementation budget of [`MAX_DEPTH`] levels;
//! integers remain exact; noninteger producer numbers use finite binary64.
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

/// Invalid individual-message data; JSON parser details are retained as text.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ValidationError {
    #[error("invalid JSON: {0}")]
    Json(String),
    #[error("JSON container depth exceeds implementation budget")]
    DepthLimit,
    #[error("invalid JSON number token")]
    InvalidNumber,
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
