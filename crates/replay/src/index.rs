use crate::{Error, profile};
use std::{
    collections::BTreeMap,
    io::{Read, Seek, SeekFrom},
};

#[derive(Clone)]
pub(crate) struct Entry {
    pub name: String,
    pub flags: u16,
    pub method: u16,
    pub crc: u32,
    pub compressed: u64,
    pub size: u64,
    pub offset: u64,
}
pub(crate) struct Index {
    pub entries: BTreeMap<String, Entry>,
    pub central: u64,
}
pub(crate) fn u16_at(b: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([b[i], b[i + 1]])
}
pub(crate) fn u32_at(b: &[u8], i: usize) -> u32 {
    u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]])
}
pub(crate) fn bytes<R: Read + Seek + ?Sized>(
    r: &mut R,
    at: u64,
    len: usize,
) -> Result<Vec<u8>, Error> {
    r.seek(SeekFrom::Start(at))?;
    let mut b = vec![0; len];
    r.read_exact(&mut b)?;
    Ok(b)
}
pub(crate) fn extras(b: &[u8]) -> Result<(), Error> {
    let mut cursor = 0;
    while cursor < b.len() {
        let header = b
            .get(cursor..cursor + 4)
            .ok_or(Error::Archive("extra field framing"))?;
        if u16_at(header, 0) == 1 {
            return Err(Error::Archive("ZIP64 unsupported"));
        }
        cursor += 4 + usize::from(u16_at(header, 2));
        if cursor > b.len() {
            return Err(Error::Archive("extra field length"));
        }
    }
    Ok(())
}

pub(crate) fn scan<R: Read + Seek + ?Sized>(r: &mut R) -> Result<Index, Error> {
    let length = r.seek(SeekFrom::End(0))?;
    profile::limit(length, profile::ARCHIVE, "archive bytes")?;
    let at = length
        .checked_sub(22)
        .ok_or(Error::Archive("end directory"))?;
    let end = bytes(r, at, 22)?;
    if u32_at(&end, 0) != 0x06054b50 || u16_at(&end, 20) != 0 {
        return Err(Error::Archive(
            "classic ZIP without archive comment required",
        ));
    }
    if u16_at(&end, 4) != 0 || u16_at(&end, 6) != 0 || u16_at(&end, 8) != u16_at(&end, 10) {
        return Err(Error::Archive("multiple disks"));
    }
    let count = u64::from(u16_at(&end, 10));
    let size = u64::from(u32_at(&end, 12));
    let offset = u64::from(u32_at(&end, 16));
    if count == 65535 || size == u64::from(u32::MAX) || offset == u64::from(u32::MAX) {
        return Err(Error::Archive("ZIP64 unsupported"));
    }
    profile::limit(count, profile::ENTRIES, "entry count")?;
    profile::limit(size, profile::DIRECTORY, "central directory")?;
    if offset.checked_add(size) != Some(at) {
        return Err(Error::Archive("central range"));
    }
    let data = bytes(r, offset, size as usize)?;
    parse_directory(&data, count, offset)
}
fn parse_directory(data: &[u8], count: u64, central: u64) -> Result<Index, Error> {
    let mut entries = BTreeMap::new();
    let mut cursor = 0usize;
    let mut total = 0u64;
    for _ in 0..count {
        let h = data
            .get(cursor..cursor + 46)
            .ok_or(Error::Archive("central header"))?;
        if u32_at(h, 0) != 0x02014b50 {
            return Err(Error::Archive("central signature"));
        }
        let name_len = usize::from(u16_at(h, 28));
        profile::limit(name_len as u64, 4096, "name bytes")?;
        let extra_len = usize::from(u16_at(h, 30));
        if u16_at(h, 32) != 0 {
            return Err(Error::Archive("entry comments unsupported"));
        }
        cursor += 46;
        let name_bytes = data
            .get(cursor..cursor + name_len)
            .ok_or(Error::Archive("central name"))?;
        let name = std::str::from_utf8(name_bytes)
            .map_err(|_| Error::Archive("name UTF-8"))?
            .to_string();
        cursor += name_len;
        let extra = data
            .get(cursor..cursor + extra_len)
            .ok_or(Error::Archive("central extras"))?;
        cursor += extra_len;
        let entry = parse_entry(h, name, extra)?;
        total = total
            .checked_add(entry.size)
            .ok_or(Error::Resource("total decoded"))?;
        profile::limit(total, profile::ARCHIVE, "total decoded")?;
        if entries.insert(entry.name.clone(), entry).is_some() {
            return Err(Error::Archive("duplicate entry"));
        }
    }
    if cursor != data.len() {
        return Err(Error::Archive("central trailing bytes"));
    }
    namespace(&entries)?;
    Ok(Index { entries, central })
}
fn parse_entry(h: &[u8], name: String, extra: &[u8]) -> Result<Entry, Error> {
    profile::path(&name)?;
    let flags = u16_at(h, 8);
    let method = u16_at(h, 10);
    if flags & 0x800 == 0 || flags & !0x0806 != 0 || !matches!(method, 0 | 8) {
        return Err(Error::Archive("flags or method"));
    }
    if method == 0 && flags & 6 != 0 {
        return Err(Error::Archive("stored flags"));
    }
    let attrs = u32_at(h, 38);
    let mode = attrs >> 16;
    if attrs & 0x418 != 0 || !matches!(mode & 0xf000, 0 | 0x8000) {
        return Err(Error::Archive("nonregular entry"));
    }
    extras(extra)?;
    let size = u64::from(u32_at(h, 24));
    let compressed = u64::from(u32_at(h, 20));
    let offset = u64::from(u32_at(h, 42));
    if [size, compressed, offset].contains(&u64::from(u32::MAX)) {
        return Err(Error::Archive("ZIP64 unsupported"));
    }
    if u16_at(h, 34) != 0 {
        return Err(Error::Archive("entry disk"));
    }
    profile::limit(size, profile::ENTRY, "entry decoded bytes")?;
    if name == "metadata.json" {
        profile::limit(size, profile::JSON_BYTES as u64, "metadata bytes")?;
    }
    Ok(Entry {
        name,
        flags,
        method,
        crc: u32_at(h, 16),
        compressed,
        size,
        offset,
    })
}
fn namespace(entries: &BTreeMap<String, Entry>) -> Result<(), Error> {
    for name in entries.keys() {
        if name != "metadata.json"
            && name != "journal.ndjson"
            && !name.starts_with("payloads/")
            && !name.starts_with("extensions/")
        {
            return Err(Error::Archive("namespace"));
        }
        for (i, _) in name.match_indices('/') {
            if entries.contains_key(&name[..i]) {
                return Err(Error::Archive("file prefix conflict"));
            }
        }
    }
    if !entries.contains_key("metadata.json") || !entries.contains_key("journal.ndjson") {
        return Err(Error::Archive("missing mandatory entry"));
    }
    #[cfg(windows)]
    windows_aliases(entries)?;
    Ok(())
}

#[cfg(windows)]
fn windows_aliases(entries: &BTreeMap<String, Entry>) -> Result<(), Error> {
    let mut aliases = BTreeMap::new();
    for name in entries.keys() {
        for end in name
            .match_indices('/')
            .map(|(at, _)| at)
            .chain(std::iter::once(name.len()))
        {
            let prefix = &name[..end];
            let key = prefix.to_uppercase();
            if aliases
                .insert(key, prefix)
                .is_some_and(|prior| prior != prefix)
            {
                return Err(Error::Archive("platform component alias"));
            }
        }
    }
    let files: std::collections::BTreeSet<_> =
        entries.keys().map(|name| name.to_uppercase()).collect();
    for name in &files {
        for (at, _) in name.match_indices('/') {
            if files.contains(&name[..at]) {
                return Err(Error::Archive("platform prefix conflict"));
            }
        }
    }
    Ok(())
}
