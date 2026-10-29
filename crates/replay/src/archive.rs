use crate::{
    Error, Journal, Metadata,
    index::{self, Entry, Index},
    profile,
};
use dmd_protocol::Descriptor;
use flate2::{Decompress, FlushDecompress, Status};
use std::{
    collections::BTreeMap,
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
    sync::{Arc, Mutex},
};

trait Source: Read + Seek + Send {}
impl<T: Read + Seek + Send> Source for T {}
struct Shared {
    source: Box<dyn Source>,
    consumed: BTreeMap<u64, (u64, String)>,
    references: BTreeMap<String, u64>,
}
type Handle = Arc<Mutex<Shared>>;

/// An opened archive identity. Journal and assets share this handle, never a pathname.
pub struct Recording {
    handle: Handle,
    index: Index,
    metadata: Metadata,
    frames: u64,
}
impl Recording {
    pub fn open(path: impl AsRef<Path>, source_frames: u64) -> Result<Self, Error> {
        Self::from_reader(File::open(path)?, source_frames)
    }
    /// Retain a caller-owned seekable identity, useful for bounded remote or test sources.
    pub fn from_reader<R: Read + Seek + Send + 'static>(
        mut reader: R,
        source_frames: u64,
    ) -> Result<Self, Error> {
        let index = index::scan(&mut reader)?;
        let handle = Arc::new(Mutex::new(Shared {
            source: Box::new(reader),
            consumed: BTreeMap::new(),
            references: BTreeMap::new(),
        }));
        let metadata_entry = index
            .entries
            .get("metadata.json")
            .ok_or(Error::Archive("metadata"))?;
        let mut stream = member(&handle, metadata_entry, index.central)?;
        let mut data = Vec::new();
        let mut buffer = [0; 8192];
        loop {
            let n = stream.read_chunk(&mut buffer)?;
            if n == 0 {
                break;
            }
            profile::limit(
                (data.len() + n) as u64,
                profile::JSON_BYTES as u64,
                "metadata bytes",
            )?;
            data.extend_from_slice(&buffer[..n]);
        }
        let text = std::str::from_utf8(&data).map_err(|_| Error::Metadata("UTF-8"))?;
        let metadata = Metadata::parse(text)?;
        // Setup checks the journal's local structure, without consuming its content.
        let journal = index
            .entries
            .get("journal.ndjson")
            .ok_or(Error::Archive("journal"))?;
        member(&handle, journal, index.central)?;
        Ok(Self {
            handle,
            index,
            metadata,
            frames: source_frames,
        })
    }
    pub fn metadata(&self) -> &Metadata {
        &self.metadata
    }
    pub fn journal(&self) -> Result<Journal, Error> {
        let entry = self
            .index
            .entries
            .get("journal.ndjson")
            .ok_or(Error::Archive("journal"))?;
        Ok(Journal::new(
            member(&self.handle, entry, self.index.central)?,
            self.frames,
        ))
    }
    /// Open unchanged `payloads/<path>`. No extraction, output publication or implicit verification.
    pub fn asset(&self, descriptor: &Descriptor) -> Result<Asset, Error> {
        let Descriptor::File { path, size, .. } = descriptor else {
            return Err(Error::Archive("phase has no asset"));
        };
        profile::path(path)?;
        let name = format!("payloads/{path}");
        profile::path(&name)?;
        let entry = self
            .index
            .entries
            .get(&name)
            .ok_or_else(|| Error::MissingAsset(path.clone()))?;
        if entry.size != *size {
            return Err(Error::ConflictingAsset(path.clone()));
        }
        let mut shared = self
            .handle
            .lock()
            .map_err(|_| Error::Archive("archive handle poisoned"))?;
        if shared.references.get(path).is_some_and(|old| old != size) {
            return Err(Error::ConflictingAsset(path.clone()));
        }
        shared.references.insert(path.clone(), *size);
        drop(shared);
        member(&self.handle, entry, self.index.central)
    }
}

fn member(handle: &Handle, entry: &Entry, central: u64) -> Result<Asset, Error> {
    let mut shared = handle
        .lock()
        .map_err(|_| Error::Archive("archive handle poisoned"))?;
    let (start, end) = local(&mut *shared.source, entry, central)?;
    let range = (entry.offset, end);
    for (&old_start, (old_end, name)) in &shared.consumed {
        if name == &entry.name {
            continue;
        }
        if range.0 < *old_end && old_start < range.1 {
            return Err(Error::Archive("overlapping consumed entries"));
        }
    }
    shared
        .consumed
        .insert(range.0, (range.1, entry.name.clone()));
    Ok(Asset {
        range: Range {
            handle: handle.clone(),
            position: start,
            left: entry.compressed,
        },
        decoder: (entry.method == 8).then(|| Decompress::new(false)),
        input: [0; 8192],
        begin: 0,
        end: 0,
        size: entry.size,
        crc: entry.crc,
        hash: crc32fast::Hasher::new(),
        decoded: 0,
        verified: false,
        ended: false,
        failed: false,
    })
}
fn local(r: &mut dyn Source, entry: &Entry, central: u64) -> Result<(u64, u64), Error> {
    let h = index::bytes(r, entry.offset, 30)?;
    if index::u32_at(&h, 0) != 0x04034b50
        || index::u16_at(&h, 6) != entry.flags
        || index::u16_at(&h, 8) != entry.method
    {
        return Err(Error::Archive("local header mismatch"));
    }
    let names = usize::from(index::u16_at(&h, 26));
    let extra = usize::from(index::u16_at(&h, 28));
    profile::limit(names as u64, 4096, "local name bytes")?;
    let start = entry
        .offset
        .checked_add(30 + names as u64 + extra as u64)
        .ok_or(Error::Archive("local overflow"))?;
    let mut end = start
        .checked_add(entry.compressed)
        .filter(|end| *end <= central)
        .ok_or(Error::Archive("compressed range"))?;
    let variable = index::bytes(r, entry.offset + 30, names + extra)?;
    if variable[..names] != *entry.name.as_bytes() {
        return Err(Error::Archive("local name mismatch"));
    }
    let fields = index::extras(&variable[names..])?;
    local_sizes(&h, &fields, entry)?;
    if entry.flags & 8 != 0 {
        end = descriptor_end(r, end, entry, central)?;
    }
    Ok((start, end))
}
fn local_sizes(h: &[u8], fields: &BTreeMap<u16, &[u8]>, entry: &Entry) -> Result<(), Error> {
    let z = fields.get(&1).copied().unwrap_or_default();
    let mut p = 0;
    let size = index::extended(
        u64::from(index::u32_at(h, 22)),
        u64::from(u32::MAX),
        z,
        &mut p,
        8,
    )?;
    let compressed = index::extended(
        u64::from(index::u32_at(h, 18)),
        u64::from(u32::MAX),
        z,
        &mut p,
        8,
    )?;
    let crc = index::u32_at(h, 14);
    if entry.flags & 8 == 0 {
        if size != entry.size || compressed != entry.compressed || crc != entry.crc {
            return Err(Error::Archive("local sizes mismatch"));
        }
    } else if (size != 0 && size != entry.size)
        || (compressed != 0 && compressed != entry.compressed)
        || (crc != 0 && crc != entry.crc)
    {
        return Err(Error::Archive("descriptor local mismatch"));
    }
    Ok(())
}
fn descriptor_end(r: &mut dyn Source, at: u64, entry: &Entry, central: u64) -> Result<u64, Error> {
    // Both descriptor widths occur with ZIP64 local headers, even for small members.
    let available = (central - at).min(24) as usize;
    let b = index::bytes(r, at, available)?;
    let prefixes = if b.len() >= 4 && index::u32_at(&b, 0) == 0x08074b50 {
        vec![4, 0]
    } else {
        vec![0]
    };
    for prefix in prefixes {
        for wide in [false, true] {
            let width = if wide { 8 } else { 4 };
            let length = prefix + 4 + 2 * width;
            if b.len() < length || index::u32_at(&b, prefix) != entry.crc {
                continue;
            }
            let sizes = &b[prefix + 4..length];
            let mut p = 0;
            let compressed = index::extended(0, 0, sizes, &mut p, width)?;
            let size = index::extended(0, 0, sizes, &mut p, width)?;
            if compressed == entry.compressed && size == entry.size {
                return Ok(at + length as u64);
            }
        }
    }
    Err(Error::Archive("data descriptor mismatch"))
}
struct Range {
    handle: Handle,
    position: u64,
    left: u64,
}
impl Range {
    fn read(&mut self, output: &mut [u8]) -> Result<usize, Error> {
        let count = output.len().min(self.left.min(8192) as usize);
        if count == 0 {
            return Ok(0);
        }
        let mut shared = self
            .handle
            .lock()
            .map_err(|_| Error::Archive("archive handle poisoned"))?;
        shared.source.seek(SeekFrom::Start(self.position))?;
        let n = loop {
            match shared.source.read(&mut output[..count]) {
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                result => break result?,
            }
        };
        if n == 0 {
            return Err(Error::Integrity("compressed bytes truncated"));
        }
        self.position += n as u64;
        self.left -= n as u64;
        Ok(n)
    }
}
/// A streaming payload. Only a successful zero-length read with a nonempty buffer
/// certifies its decoded size, compressed stream and CRC. Dropping it does no scan.
pub struct Asset {
    range: Range,
    decoder: Option<Decompress>,
    input: [u8; 8192],
    begin: usize,
    end: usize,
    size: u64,
    crc: u32,
    hash: crc32fast::Hasher,
    decoded: u64,
    verified: bool,
    ended: bool,
    failed: bool,
}
impl Asset {
    pub fn verified(&self) -> bool {
        self.verified
    }
    pub fn read_chunk(&mut self, output: &mut [u8]) -> Result<usize, Error> {
        if self.failed {
            return Err(Error::Integrity("reader ended after failure"));
        }
        if output.is_empty() {
            return Ok(0);
        }
        let result = self.read_next(output);
        if result.is_err() {
            self.failed = true;
        }
        result
    }
    fn read_next(&mut self, output: &mut [u8]) -> Result<usize, Error> {
        if self.verified {
            return Ok(0);
        }
        let capacity = output.len().min(8192);
        let n = if self.decoder.is_none() {
            self.range.read(&mut output[..capacity])?
        } else {
            self.inflate(&mut output[..capacity])?
        };
        self.decoded = self
            .decoded
            .checked_add(n as u64)
            .ok_or(Error::Resource("actual decoded size"))?;
        profile::limit(self.decoded, profile::ENTRY, "actual entry bytes")?;
        if self.decoded > self.size {
            return Err(Error::Integrity("decoded size exceeds declaration"));
        }
        self.hash.update(&output[..n]);
        if n == 0 {
            if self.decoded != self.size || self.hash.clone().finalize() != self.crc {
                return Err(Error::Integrity("decoded size or CRC mismatch"));
            }
            self.verified = true;
        }
        Ok(n)
    }
    fn inflate(&mut self, output: &mut [u8]) -> Result<usize, Error> {
        if self.ended {
            return Ok(0);
        }
        loop {
            if self.begin == self.end {
                self.end = self.range.read(&mut self.input)?;
                self.begin = 0;
            }
            let decoder = self
                .decoder
                .as_mut()
                .ok_or(Error::Integrity("missing decompressor"))?;
            let before_in = decoder.total_in();
            let before_out = decoder.total_out();
            let status = decoder
                .decompress(
                    &self.input[self.begin..self.end],
                    output,
                    FlushDecompress::None,
                )
                .map_err(|_| Error::Integrity("invalid Deflate"))?;
            let used = (decoder.total_in() - before_in) as usize;
            let n = (decoder.total_out() - before_out) as usize;
            self.begin += used;
            if status == Status::StreamEnd {
                if self.begin != self.end || self.range.left != 0 {
                    return Err(Error::Integrity("trailing compressed bytes"));
                }
                self.ended = true;
            }
            if n > 0 || self.ended {
                return Ok(n);
            }
            if used == 0 {
                return Err(Error::Integrity("truncated Deflate"));
            }
        }
    }
}
