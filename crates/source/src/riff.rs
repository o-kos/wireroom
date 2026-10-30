// Copyright (c) 2026 Oleg Kosyankov
// SPDX-License-Identifier: MIT
// Adapted from Argand riff.rs; see README.md and LICENSE.
use crate::{Error, Malformed, Metadata, Resource, Unsupported};
use dmd_protocol::{Rate, Stream, StreamFormat};
use std::io::{Read, Seek, SeekFrom};

/// Maximum top-level chunks, including fmt and data. Checked before each header.
pub const MAX_CHUNKS: u32 = 65_536;

/// Inspect from byte zero, using seek-to-end for actual length. Leaves the cursor
/// at the container end on success; its position on failure is unspecified.
/// Requires ordinary Read/Seek semantics and stable contents for the whole call.
/// Reads only RIFF/chunk headers and the 16/18-byte fmt body, never payloads.
pub fn inspect<R: Read + Seek>(input: &mut R) -> Result<Metadata, Error> {
    let byte_size = input.seek(SeekFrom::End(0))?;
    input.seek(SeekFrom::Start(0))?;
    let mut header = [0; 12];
    read_header(input, &mut header)?;
    match &header[..4] {
        b"RIFX" | b"RF64" | b"BW64" => {
            return Err(Error::Unsupported(Unsupported::Container));
        }
        b"RIFF" => {}
        _ => return Err(Error::Malformed(Malformed::Identification)),
    }
    if &header[8..] != b"WAVE" {
        return Err(Error::Malformed(Malformed::Identification));
    }
    let end = add(
        u64::from(u32::from_le_bytes([
            header[4], header[5], header[6], header[7],
        ])),
        8,
    )?;
    if end != byte_size || end < 12 {
        return Err(Error::Malformed(Malformed::ContainerLength));
    }
    let mut state = Chunks::default();
    let mut pos = 12;
    let mut count = 0;
    while pos < end {
        if count == MAX_CHUNKS {
            return Err(Error::Resource(Resource::ChunkBudget));
        }
        count += 1;
        let body = add(pos, 8)?;
        if body > end {
            return Err(Error::Malformed(Malformed::Truncated));
        }
        let mut header = [0; 8];
        read_header(input, &mut header)?;
        let size = u64::from(u32::from_le_bytes([
            header[4], header[5], header[6], header[7],
        ]));
        let next = add(body, add(size, size & 1)?)?;
        if next > end {
            return Err(Error::Malformed(Malformed::ChunkExtent));
        }
        state.chunk(input, &header[..4], size)?;
        input.seek(SeekFrom::Start(next))?;
        pos = next;
    }
    state.finish(byte_size)
}

fn add(a: u64, b: u64) -> Result<u64, Error> {
    a.checked_add(b)
        .ok_or(Error::Malformed(Malformed::Arithmetic))
}

fn read_header(input: &mut impl Read, bytes: &mut [u8]) -> Result<(), Error> {
    input.read_exact(bytes).map_err(|error| {
        if error.kind() == std::io::ErrorKind::UnexpectedEof {
            Error::Malformed(Malformed::Truncated)
        } else {
            Error::Io(error)
        }
    })
}

#[derive(Default)]
struct Chunks {
    fmt: Option<FmtChunk>,
    data: Option<u64>,
}
impl Chunks {
    fn chunk(&mut self, input: &mut impl Read, id: &[u8], size: u64) -> Result<(), Error> {
        match id {
            b"fmt " => {
                if self.fmt.is_some() {
                    return Err(Error::Malformed(Malformed::DuplicateFmt));
                }
                if size < 16 {
                    return Err(Error::Malformed(Malformed::Truncated));
                }
                if !matches!(size, 16 | 18) {
                    return Err(Error::Unsupported(Unsupported::FmtSize(size)));
                }
                let mut body = [0; 18];
                let len = if size == 16 { 16 } else { 18 };
                read_header(input, &mut body[..len])?;
                self.fmt = Some(parse_fmt(&body[..len])?);
            }
            b"data" => {
                if self.fmt.is_none() {
                    return Err(Error::Malformed(Malformed::DataBeforeFmt));
                }
                if self.data.replace(size).is_some() {
                    return Err(Error::Malformed(Malformed::DuplicateData));
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn finish(self, byte_size: u64) -> Result<Metadata, Error> {
        let fmt = self.fmt.ok_or(Error::Malformed(Malformed::MissingFmt))?;
        let size = self.data.ok_or(Error::Malformed(Malformed::MissingData))?;
        if size % u64::from(fmt.block_align) != 0 {
            return Err(Error::Malformed(Malformed::PartialFrame));
        }
        Ok(Metadata {
            byte_size,
            stream: Stream {
                format: StreamFormat::Real,
                channels: u32::from(fmt.channels),
                rate: Rate {
                    numerator: u64::from(fmt.sample_rate),
                    denominator: 1,
                },
            },
            frames: size / u64::from(fmt.block_align),
        })
    }
}

#[derive(Debug, Clone, Copy)]
struct FmtChunk {
    format_tag: u16,
    channels: u16,
    sample_rate: u32,
    byte_rate: u32,
    block_align: u16,
    bits: u16,
}

fn parse_fmt(body: &[u8]) -> Result<FmtChunk, Error> {
    if body.len() < 16 {
        return Err(Error::Malformed(Malformed::Truncated));
    }
    let u16_at = |i: usize| u16::from_le_bytes([body[i], body[i + 1]]);
    let u32_at = |i: usize| u32::from_le_bytes([body[i], body[i + 1], body[i + 2], body[i + 3]]);
    let fmt = FmtChunk {
        format_tag: u16_at(0),
        channels: u16_at(2),
        sample_rate: u32_at(4),
        byte_rate: u32_at(8),
        block_align: u16_at(12),
        bits: u16_at(14),
    };
    fmt.validate()?;
    Ok(fmt)
}

impl FmtChunk {
    fn validate(self) -> Result<(), Error> {
        if self.format_tag != 1 {
            return Err(Error::Unsupported(Unsupported::FormatTag(self.format_tag)));
        }
        match self.channels {
            0 => return Err(Error::Malformed(Malformed::ZeroChannels)),
            1 | 2 => {}
            n => return Err(Error::Unsupported(Unsupported::Channels(n))),
        }
        if !matches!(self.bits, 8 | 16) {
            return Err(Error::Unsupported(Unsupported::Bits(self.bits)));
        }
        if self.sample_rate == 0 {
            return Err(Error::Malformed(Malformed::ZeroRate));
        }
        let align = self
            .channels
            .checked_mul(self.bits / 8)
            .ok_or(Error::Malformed(Malformed::Arithmetic))?;
        if self.block_align != align {
            return Err(Error::Malformed(Malformed::BlockAlign));
        }
        let rate = self
            .sample_rate
            .checked_mul(u32::from(align))
            .ok_or(Error::Malformed(Malformed::Arithmetic))?;
        if self.byte_rate != rate {
            return Err(Error::Malformed(Malformed::ByteRate));
        }
        Ok(())
    }
}
