// Copyright (c) 2026 Oleg Kosyankov
// SPDX-License-Identifier: MIT
// Adapted from Argand shared_file.rs; see README.md and LICENSE.
//! One opened file read by several parsers, each at a position of its own.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::sync::Arc;

/// A cursor over a shared handle that reads at a position of its own, never the handle's.
#[derive(Debug)]
pub struct SourceCursor {
    file: Arc<File>,
    pos: u64,
    len: u64,
}

impl SourceCursor {
    /// Inspect the retained identity, including modification metadata for diagnostics.
    pub fn file_metadata(&self) -> std::io::Result<std::fs::Metadata> {
        self.file.metadata()
    }
    pub(crate) fn new(file: Arc<File>) -> std::io::Result<Self> {
        let len = file.metadata()?.len();
        Ok(Self { file, pos: 0, len })
    }
}

impl Read for SourceCursor {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        let read = read_at(&self.file, buf, self.pos)?;
        self.pos = self.pos.checked_add(read as u64).ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "cursor overflow")
        })?;
        Ok(read)
    }
}

impl Seek for SourceCursor {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        let target = match pos {
            SeekFrom::Start(offset) => Some(offset),
            SeekFrom::End(delta) => self.len.checked_add_signed(delta),
            SeekFrom::Current(delta) => self.pos.checked_add_signed(delta),
        };
        let target = target.ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "seek target is outside the u64 position range",
            )
        })?;
        self.pos = target;
        Ok(target)
    }
}

#[cfg(unix)]
fn read_at(file: &File, buf: &mut [u8], offset: u64) -> std::io::Result<usize> {
    std::os::unix::fs::FileExt::read_at(file, buf, offset)
}

/// `seek_read` moves the handle's own position, which no reader here relies on.
#[cfg(windows)]
fn read_at(file: &File, buf: &mut [u8], offset: u64) -> std::io::Result<usize> {
    std::os::windows::fs::FileExt::seek_read(file, buf, offset)
}
