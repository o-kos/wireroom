#![doc = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/README.md"))]
mod error;
mod riff;
mod shared_file;

use dmd_protocol::{Frame, Stream};
pub use error::{Error, Malformed, Resource, Unsupported};
pub use riff::{MAX_CHUNKS, inspect};
pub use shared_file::SourceCursor;
use std::{fs::File, path::Path, sync::Arc};

/// Exact metadata at inspection time; not a content stability guarantee.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Metadata {
    pub byte_size: u64,
    pub stream: Stream,
    pub frames: Frame,
}

/// Retains the opened identity. Clones and cursors never reopen its pathname.
#[derive(Clone, Debug)]
pub struct Source {
    file: Arc<File>,
    metadata: Metadata,
}
impl Source {
    /// Open read-only, then inspect the retained handle.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, Error> {
        Self::from_file(File::open(path)?)
    }

    /// Consume an already opened regular file, without using its shared position.
    /// The caller must provide read access and keep its contents stable.
    pub fn from_file(file: File) -> Result<Self, Error> {
        if !file.metadata()?.is_file() {
            return Err(Error::Unsupported(Unsupported::FileKind));
        }
        let file = Arc::new(file);
        let mut cursor = SourceCursor::new(file.clone())?;
        let metadata = inspect(&mut cursor)?;
        Ok(Self { file, metadata })
    }

    pub fn metadata(&self) -> Metadata {
        self.metadata
    }

    /// New unbuffered cursor, starting at byte zero, over the original identity.
    /// End-relative seeks use the length observed when this cursor is created.
    pub fn cursor(&self) -> Result<SourceCursor, Error> {
        Ok(SourceCursor::new(self.file.clone())?)
    }
}
