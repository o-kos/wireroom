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
    pub zip64_sizes: bool,
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
fn u64_at(b: &[u8], i: usize) -> u64 {
    u64::from_le_bytes([
        b[i],
        b[i + 1],
        b[i + 2],
        b[i + 3],
        b[i + 4],
        b[i + 5],
        b[i + 6],
        b[i + 7],
    ])
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
pub(crate) fn extras(b: &[u8]) -> Result<BTreeMap<u16, &[u8]>, Error> {
    let mut fields = BTreeMap::new();
    let mut i = 0;
    while i < b.len() {
        if b.len() - i < 4 {
            return Err(Error::Archive("extra field framing"));
        }
        let id = u16_at(b, i);
        let n = usize::from(u16_at(b, i + 2));
        i += 4;
        let value = b
            .get(i..i + n)
            .ok_or(Error::Archive("extra field length"))?;
        if fields.insert(id, value).is_some() && id == 1 {
            return Err(Error::Archive("duplicate extra field"));
        }
        i += n;
    }
    Ok(fields)
}
pub(crate) fn scan<R: Read + Seek + ?Sized>(r: &mut R) -> Result<Index, Error> {
    let length = r.seek(SeekFrom::End(0))?;
    profile::limit(length, profile::ARCHIVE, "archive bytes")?;
    if length < 22 {
        return Err(Error::Archive("end directory"));
    }
    let fixed = bytes(r, length - 22, 22)?;
    if u32_at(&fixed, 0) == 0x06054b50
        && u16_at(&fixed, 20) == 0
        && let Ok(index) =
            directory_layout(r, &fixed, length - 22).and_then(|layout| read_directory(r, layout))
    {
        return Ok(index);
    }
    let tail_len = length.min(65557) as usize;
    let tail = bytes(r, length - tail_len as u64, tail_len)?;
    let mut failure = Error::Archive("end directory");
    for end in (0..tail.len() - 21).rev() {
        if u32_at(&tail, end) != 0x06054b50
            || end + 22 + usize::from(u16_at(&tail, end + 20)) != tail.len()
        {
            continue;
        }
        match directory_layout(r, &tail[end..], length - tail_len as u64 + end as u64)
            .and_then(|layout| read_directory(r, layout))
        {
            Ok(index) => return Ok(index),
            Err(error) => failure = error,
        }
    }
    Err(failure)
}
struct Directory {
    count: u64,
    size: u64,
    offset: u64,
}
fn read_directory<R: Read + Seek + ?Sized>(r: &mut R, layout: Directory) -> Result<Index, Error> {
    let data = bytes(r, layout.offset, layout.size as usize)?;
    parse_directory(&data, layout.count, layout.offset)
}
fn directory_layout<R: Read + Seek + ?Sized>(
    r: &mut R,
    e: &[u8],
    end_at: u64,
) -> Result<Directory, Error> {
    if u16_at(e, 4) != 0 || u16_at(e, 6) != 0 || u16_at(e, 8) != u16_at(e, 10) {
        return Err(Error::Archive("multiple disks"));
    }
    let mut count = u64::from(u16_at(e, 10));
    let mut size = u64::from(u32_at(e, 12));
    let mut offset = u64::from(u32_at(e, 16));
    let locator_present = if offset.checked_add(size) != Some(end_at) && end_at >= 20 {
        u32_at(&bytes(r, end_at - 20, 20)?, 0) == 0x07064b50
    } else {
        false
    };
    let directory_end = if count == 65535
        || size == u64::from(u32::MAX)
        || offset == u64::from(u32::MAX)
        || locator_present
    {
        let locator = bytes(
            r,
            end_at
                .checked_sub(20)
                .ok_or(Error::Archive("ZIP64 locator"))?,
            20,
        )?;
        if u32_at(&locator, 0) != 0x07064b50
            || u32_at(&locator, 4) != 0
            || u32_at(&locator, 16) != 1
        {
            return Err(Error::Archive("ZIP64 locator"));
        }
        let pos = u64_at(&locator, 8);
        let z = bytes(r, pos, 56)?;
        if u32_at(&z, 0) != 0x06064b50
            || u64_at(&z, 4) < 44
            || pos
                .checked_add(12)
                .and_then(|p| p.checked_add(u64_at(&z, 4)))
                != end_at.checked_sub(20)
            || u32_at(&z, 16) != 0
            || u32_at(&z, 20) != 0
            || u64_at(&z, 24) != u64_at(&z, 32)
        {
            return Err(Error::Archive("ZIP64 end"));
        }
        consistent(count, 65535, u64_at(&z, 32))?;
        consistent(size, u64::from(u32::MAX), u64_at(&z, 40))?;
        consistent(offset, u64::from(u32::MAX), u64_at(&z, 48))?;
        count = u64_at(&z, 32);
        size = u64_at(&z, 40);
        offset = u64_at(&z, 48);
        pos
    } else {
        end_at
    };
    profile::limit(count, 65536, "entry count")?;
    profile::limit(size, profile::DIRECTORY, "central directory")?;
    if offset.checked_add(size) != Some(directory_end) {
        return Err(Error::Archive("central range"));
    }
    Ok(Directory {
        count,
        size,
        offset,
    })
}
fn consistent(legacy: u64, sentinel: u64, wide: u64) -> Result<(), Error> {
    if legacy != sentinel && legacy != wide {
        return Err(Error::Archive("ZIP64 legacy mismatch"));
    }
    Ok(())
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
        let comment = usize::from(u16_at(h, 32));
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
        cursor = cursor
            .checked_add(comment)
            .filter(|c| *c <= data.len())
            .ok_or(Error::Archive("central comment"))?;
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
    if flags & 0x800 == 0 || flags & !0x080e != 0 || !matches!(method, 0 | 8) {
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
    let fields = extras(extra)?;
    let z = fields.get(&1).copied().unwrap_or_default();
    let mut p = 0;
    let size = extended(u64::from(u32_at(h, 24)), u64::from(u32::MAX), z, &mut p, 8)?;
    let compressed = extended(u64::from(u32_at(h, 20)), u64::from(u32::MAX), z, &mut p, 8)?;
    let offset = extended(u64::from(u32_at(h, 42)), u64::from(u32::MAX), z, &mut p, 8)?;
    let disk = extended(u64::from(u16_at(h, 34)), 65535, z, &mut p, 4)?;
    if disk != 0 {
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
        zip64_sizes: fields.contains_key(&1),
    })
}
pub(crate) fn extended(
    value: u64,
    sentinel: u64,
    z: &[u8],
    p: &mut usize,
    width: usize,
) -> Result<u64, Error> {
    if value != sentinel {
        return Ok(value);
    }
    let b = z.get(*p..*p + width).ok_or(Error::Archive("ZIP64 extra"))?;
    *p += width;
    Ok(if width == 8 {
        u64_at(b, 0)
    } else {
        u64::from(u32_at(b, 0))
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    fn central(name: &str, size: u64) -> Vec<u8> {
        let mut b = Vec::new();
        b.extend(0x02014b50u32.to_le_bytes());
        for n in [45u16, 45, 0x800, 0, 0, 0] {
            b.extend(n.to_le_bytes());
        }
        b.extend(0u32.to_le_bytes());
        b.extend(0u32.to_le_bytes());
        b.extend(u32::MAX.to_le_bytes());
        b.extend((name.len() as u16).to_le_bytes());
        b.extend(12u16.to_le_bytes());
        b.extend([0; 10]);
        b.extend(0u32.to_le_bytes());
        b.extend(name.bytes());
        b.extend(1u16.to_le_bytes());
        b.extend(8u16.to_le_bytes());
        b.extend(size.to_le_bytes());
        b
    }
    fn indexed(entries: &[(String, u64)]) -> Vec<u8> {
        let mut b = Vec::new();
        for (name, size) in entries {
            b.extend(central(name, *size));
        }
        let size = b.len() as u64;
        let zip64 = b.len() as u64;
        b.extend(0x06064b50u32.to_le_bytes());
        b.extend(44u64.to_le_bytes());
        b.extend([45, 0, 45, 0]);
        b.extend([0; 8]);
        b.extend((entries.len() as u64).to_le_bytes());
        b.extend((entries.len() as u64).to_le_bytes());
        b.extend(size.to_le_bytes());
        b.extend(0u64.to_le_bytes());
        b.extend(0x07064b50u32.to_le_bytes());
        b.extend(0u32.to_le_bytes());
        b.extend(zip64.to_le_bytes());
        b.extend(1u32.to_le_bytes());
        b.extend(0x06054b50u32.to_le_bytes());
        b.extend([0; 4]);
        b.extend([255; 4]);
        b.extend(u32::MAX.to_le_bytes());
        b.extend(u32::MAX.to_le_bytes());
        b.extend([0; 2]);
        b
    }
    fn required() -> Vec<(String, u64)> {
        vec![("metadata.json".into(), 0), ("journal.ndjson".into(), 0)]
    }
    #[test]
    fn scanner_checks_zip64_entry_and_total_declared_sizes() {
        let mut entries = required();
        entries.push(("extensions/one".into(), profile::ENTRY));
        assert!(scan(&mut Cursor::new(indexed(&entries))).is_ok());
        entries[2].1 += 1;
        assert!(matches!(
            scan(&mut Cursor::new(indexed(&entries))),
            Err(Error::Resource("entry decoded bytes"))
        ));
        let mut entries = required();
        for n in 0..16 {
            entries.push((format!("extensions/{n}"), profile::ENTRY));
        }
        assert!(scan(&mut Cursor::new(indexed(&entries))).is_ok());
        entries.push(("extensions/excess".into(), 1));
        assert!(matches!(
            scan(&mut Cursor::new(indexed(&entries))),
            Err(Error::Resource("total decoded"))
        ));
    }
    #[test]
    fn scanner_checks_metadata_declared_limit() {
        let mut entries = required();
        entries[0].1 = profile::JSON_BYTES as u64;
        assert!(scan(&mut Cursor::new(indexed(&entries))).is_ok());
        entries[0].1 += 1;
        assert!(matches!(
            scan(&mut Cursor::new(indexed(&entries))),
            Err(Error::Resource("metadata bytes"))
        ));
    }
    #[test]
    fn scanner_checks_entry_count_at_the_portable_boundary() {
        let mut entries = required();
        for n in 2..65536 {
            entries.push((format!("extensions/{n}"), 0));
        }
        assert!(scan(&mut Cursor::new(indexed(&entries))).is_ok());
        entries.push(("extensions/excess".into(), 0));
        assert!(matches!(
            scan(&mut Cursor::new(indexed(&entries))),
            Err(Error::Resource("entry count"))
        ));
    }
}
