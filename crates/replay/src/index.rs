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
// This is an implementation work budget, separate from the portable per-index limit.
const INDEX_WORK_BYTES: u64 = profile::DIRECTORY;
struct IndexBudget {
    remaining: u64,
}
impl IndexBudget {
    fn charge(&mut self, bytes: u64) -> Result<(), Error> {
        self.remaining = self
            .remaining
            .checked_sub(bytes)
            .ok_or(Error::Resource("archive index work"))?;
        Ok(())
    }
}
pub(crate) fn scan<R: Read + Seek + ?Sized>(r: &mut R) -> Result<Index, Error> {
    scan_with_budget(r, INDEX_WORK_BYTES)
}
fn scan_with_budget<R: Read + Seek + ?Sized>(r: &mut R, work_bytes: u64) -> Result<Index, Error> {
    let length = r.seek(SeekFrom::End(0))?;
    profile::limit(length, profile::ARCHIVE, "archive bytes")?;
    if length < 22 {
        return Err(Error::Archive("end directory"));
    }
    let mut budget = IndexBudget {
        remaining: work_bytes,
    };
    let mut failure = Error::Archive("end directory");
    let fixed = bytes(r, length - 22, 22)?;
    let fixed_candidate = u32_at(&fixed, 0) == 0x06054b50 && u16_at(&fixed, 20) == 0;
    if fixed_candidate {
        match candidate(r, &fixed, length - 22, &mut budget)? {
            Attempt::Accepted(index) => return Ok(index),
            Attempt::Rejected(error) => failure = error,
        }
    }
    let tail_len = length.min(65557) as usize;
    let tail = bytes(r, length - tail_len as u64, tail_len)?;
    for end in (0..tail.len() - 21).rev() {
        if (fixed_candidate && end + 22 == tail.len())
            || u32_at(&tail, end) != 0x06054b50
            || end + 22 + usize::from(u16_at(&tail, end + 20)) != tail.len()
        {
            continue;
        }
        match candidate(
            r,
            &tail[end..],
            length - tail_len as u64 + end as u64,
            &mut budget,
        )? {
            Attempt::Accepted(index) => return Ok(index),
            Attempt::Rejected(error) => failure = error,
        }
    }
    Err(failure)
}
enum Attempt {
    Accepted(Index),
    Rejected(Error),
}
fn candidate<R: Read + Seek + ?Sized>(
    r: &mut R,
    end: &[u8],
    at: u64,
    budget: &mut IndexBudget,
) -> Result<Attempt, Error> {
    let layout = match directory_layout(r, end, at) {
        Ok(layout) => layout,
        Err(error) => return Ok(Attempt::Rejected(error)),
    };
    // Exhausted work cannot be mistaken for a false signature in opaque comments.
    budget.charge(layout.size)?;
    Ok(match read_directory(r, layout) {
        Ok(index) => Attempt::Accepted(index),
        Err(error) => Attempt::Rejected(error),
    })
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
    struct Counted<'a> {
        inner: Cursor<&'a [u8]>,
        index_reads: usize,
        index_bytes: u64,
    }
    impl Read for Counted<'_> {
        fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
            let index = self.inner.position() == 0;
            let n = self.inner.read(output)?;
            if index {
                self.index_reads += 1;
                self.index_bytes += n as u64;
            }
            Ok(n)
        }
    }
    impl Seek for Counted<'_> {
        fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
            self.inner.seek(position)
        }
    }
    fn observed(data: &[u8]) -> Counted<'_> {
        Counted {
            inner: Cursor::new(data),
            index_reads: 0,
            index_bytes: 0,
        }
    }
    fn fake_candidates(count: usize) -> (Vec<u8>, u64, u64) {
        let mut entries = required();
        for n in 0..6000 {
            entries.push((format!("extensions/{n}"), 0));
        }
        let mut data = indexed(&entries);
        let base = data.len() as u64 - 98;
        let original_end = data.len() - 22;
        data[original_end + 20..].copy_from_slice(&((count * 22) as u16).to_le_bytes());
        let mut work = base;
        for n in 0..count {
            let at = data.len() as u64;
            work += at;
            data.extend(0x06054b50u32.to_le_bytes());
            data.extend([0; 4]);
            data.extend((entries.len() as u16).to_le_bytes());
            data.extend((entries.len() as u16).to_le_bytes());
            data.extend((at as u32).to_le_bytes());
            data.extend(0u32.to_le_bytes());
            data.extend((((count - n - 1) * 22) as u16).to_le_bytes());
        }
        (data, base, work)
    }
    #[test]
    fn aggregate_candidate_budget_stops_index_reads_without_timing_assumptions() {
        let (data, base, _) = fake_candidates(400);
        let mut source = observed(&data);
        assert!(matches!(
            scan(&mut source),
            Err(Error::Resource("archive index work"))
        ));
        assert!(source.index_reads > 1);
        assert!(source.index_reads < 400);
        assert!(source.index_bytes <= INDEX_WORK_BYTES);
        assert!(source.index_reads as u64 <= INDEX_WORK_BYTES / base);
    }
    #[test]
    fn aggregate_budget_equality_succeeds_and_excess_stops_before_next_read() {
        let (data, base, work) = fake_candidates(3);
        let mut source = observed(&data);
        assert!(scan_with_budget(&mut source, work).is_ok());
        assert_eq!(source.index_reads, 4);
        assert_eq!(source.index_bytes, work);
        let mut source = observed(&data);
        assert!(matches!(
            scan_with_budget(&mut source, work - 1),
            Err(Error::Resource("archive index work"))
        ));
        assert_eq!(source.index_reads, 3);
        assert_eq!(source.index_bytes, work - base);
    }
    #[test]
    fn first_candidate_can_use_the_entire_portable_directory_limit() {
        let mut entries = required();
        for n in 0..1100 {
            entries.push((format!("extensions/{n}"), 0));
        }
        let mut headers: Vec<_> = entries
            .iter()
            .map(|(name, size)| central(name, *size))
            .collect();
        let mut padding = profile::DIRECTORY as usize - headers.iter().map(Vec::len).sum::<usize>();
        let mut data = Vec::with_capacity(profile::DIRECTORY as usize + 22);
        for header in &mut headers {
            let length = padding.min(65535);
            padding -= length;
            header[32..34].copy_from_slice(&(length as u16).to_le_bytes());
            data.extend_from_slice(header);
            data.resize(data.len() + length, 0);
        }
        assert_eq!(padding, 0);
        assert_eq!(data.len() as u64, profile::DIRECTORY);
        data.extend(0x06054b50u32.to_le_bytes());
        data.extend([0; 4]);
        data.extend((entries.len() as u16).to_le_bytes());
        data.extend((entries.len() as u16).to_le_bytes());
        data.extend((profile::DIRECTORY as u32).to_le_bytes());
        data.extend([0; 6]);
        let mut source = observed(&data);
        assert!(scan(&mut source).is_ok());
        assert_eq!(source.index_reads, 1);
        assert_eq!(source.index_bytes, INDEX_WORK_BYTES);
        let mut source = observed(&data);
        assert!(matches!(
            scan_with_budget(&mut source, INDEX_WORK_BYTES - 1),
            Err(Error::Resource("archive index work"))
        ));
        assert_eq!(source.index_reads, 0);
    }
    #[test]
    fn false_comment_candidate_profile_limit_is_not_aggregate_exhaustion() {
        let (mut data, _, _) = fake_candidates(1);
        let end = data.len() - 22;
        data[end + 12..end + 16].copy_from_slice(&((profile::DIRECTORY + 1) as u32).to_le_bytes());
        let mut source = observed(&data);
        assert!(scan(&mut source).is_ok());
        assert_eq!(source.index_reads, 1);
    }
}
