mod common;
use common::{chunk, fmt, standard, wave};
use dmd_source::{Error, MAX_CHUNKS, Malformed, Resource, inspect};
use std::io::{self, Read, Seek, SeekFrom};

/// Sparse virtual bytes. Every read must be wholly inside a declared header;
/// a speculative/buffered payload read fails, even when the bytes would be zero.
struct Reader {
    segments: Vec<(u64, Vec<u8>)>,
    len: u64,
    pos: u64,
    reads: usize,
    seeks: usize,
    bytes: usize,
    short: usize,
    interrupt: bool,
    fail_read: Option<usize>,
    fail_seek: Option<usize>,
    eof: Option<usize>,
}
impl Reader {
    fn new(segments: Vec<(u64, Vec<u8>)>, len: u64) -> Self {
        Self {
            segments,
            len,
            pos: 0,
            reads: 0,
            seeks: 0,
            bytes: 0,
            short: usize::MAX,
            interrupt: false,
            fail_read: None,
            fail_seek: None,
            eof: None,
        }
    }
    fn small() -> Self {
        let bytes = standard();
        Self::new(vec![(0, bytes[..44].to_vec())], 60)
    }
}
impl Read for Reader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.reads += 1;
        if self.fail_read == Some(self.reads) {
            return Err(io::Error::other("injected read failure"));
        }
        if self.eof == Some(self.reads) {
            return Ok(0);
        }
        if self.interrupt && self.reads % 2 == 1 {
            return Err(io::ErrorKind::Interrupted.into());
        }
        let count = buf.len().min(self.short);
        let (start, segment) = self
            .segments
            .iter()
            .find(|(start, bytes)| {
                self.pos >= *start && self.pos + count as u64 <= start + bytes.len() as u64
            })
            .expect("attempted payload read");
        let offset = (self.pos - start) as usize;
        buf[..count].copy_from_slice(&segment[offset..offset + count]);
        self.pos += count as u64;
        self.bytes += count;
        Ok(count)
    }
}
impl Seek for Reader {
    fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
        self.seeks += 1;
        if self.fail_seek == Some(self.seeks) {
            return Err(io::Error::other("injected seek failure"));
        }
        self.pos = match from {
            SeekFrom::Start(pos) => pos,
            SeekFrom::End(delta) => self.len.checked_add_signed(delta).unwrap(),
            SeekFrom::Current(delta) => self.pos.checked_add_signed(delta).unwrap(),
        };
        Ok(self.pos)
    }
}

#[test]
fn headers_only_even_for_near_four_gib_payload_and_u64_lengths() {
    let data_size = u32::MAX - 47;
    let len = u64::from(u32::MAX) + 7;
    let mut headers = standard()[..44].to_vec();
    headers[4..8].copy_from_slice(&(u32::MAX - 1).to_le_bytes());
    headers[40..44].copy_from_slice(&data_size.to_le_bytes());
    // One odd ancillary body and its padding follow the huge sample extent.
    let tail_at = 44 + u64::from(data_size);
    let mut tail = b"tail".to_vec();
    tail.extend(1u32.to_le_bytes());
    let mut reader = Reader::new(vec![(0, headers), (tail_at, tail)], len);
    let meta = inspect(&mut reader).unwrap();
    assert_eq!(meta.byte_size, 4_294_967_302);
    assert_eq!(meta.frames, u64::from(data_size) / 2);
    assert_eq!(reader.bytes, 52);
    assert_eq!(reader.reads, 5);
    assert_eq!(reader.seeks, 5);
    let mut impossible = Reader::small();
    impossible.len = u64::MAX;
    assert!(matches!(
        inspect(&mut impossible),
        Err(Error::Malformed(Malformed::ContainerLength))
    ));
}

#[test]
fn huge_ancillary_payload_is_skipped() {
    let size = u32::MAX - 100;
    let size = size - (size & 1);
    let mut first = b"RIFF\0\0\0\0WAVEJUNK".to_vec();
    first.extend(size.to_le_bytes());
    let next = 20 + u64::from(size);
    let rest = wave(&[chunk(b"fmt ", &fmt(1, 8, 1)), chunk(b"data", &[])])[12..].to_vec();
    let len = next + rest.len() as u64;
    first[4..8].copy_from_slice(&((len - 8) as u32).to_le_bytes());
    let mut reader = Reader::new(vec![(0, first), (next, rest)], len);
    assert_eq!(inspect(&mut reader).unwrap().frames, 0);
    assert_eq!(reader.bytes, 52);
}

#[test]
fn budget_equality_and_excess_measure_headers_without_scanning_remainder() {
    for extra in [0, 1, 100] {
        let mut chunks = vec![chunk(b"fmt ", &fmt(1, 8, 1)), chunk(b"data", &[])];
        chunks.extend((2..MAX_CHUNKS + extra).map(|_| chunk(b"JUNK", &[])));
        let bytes = wave(&chunks);
        let len = bytes.len() as u64;
        let mut reader = Reader::new(vec![(0, bytes)], len);
        let result = inspect(&mut reader);
        if extra == 0 {
            assert_eq!(result.unwrap().frames, 0);
            assert_eq!(reader.pos, len);
        } else {
            assert!(matches!(
                result,
                Err(Error::Resource(Resource::ChunkBudget))
            ));
            assert_eq!(reader.pos, len - u64::from(extra) * 8);
        }
        assert_eq!(reader.reads, MAX_CHUNKS as usize + 2);
        assert_eq!(reader.bytes, 12 + MAX_CHUNKS as usize * 8 + 16);
        assert_eq!(reader.seeks, MAX_CHUNKS as usize + 2);
    }
}

#[test]
fn every_required_read_and_seek_failure_is_observable() {
    let mut baseline = Reader::small();
    inspect(&mut baseline).unwrap();
    for call in 1..=baseline.reads {
        let mut reader = Reader::small();
        reader.fail_read = Some(call);
        assert!(
            matches!(inspect(&mut reader), Err(Error::Io(_))),
            "read {call}"
        );
        let mut reader = Reader::small();
        reader.eof = Some(call);
        assert!(matches!(
            inspect(&mut reader),
            Err(Error::Malformed(Malformed::Truncated))
        ));
    }
    for call in 1..=baseline.seeks {
        let mut reader = Reader::small();
        reader.fail_seek = Some(call);
        assert!(
            matches!(inspect(&mut reader), Err(Error::Io(_))),
            "seek {call}"
        );
    }
}

#[test]
fn arbitrary_short_and_interrupted_reads_preserve_exact_results() {
    for short in 1..=18 {
        let mut reader = Reader::small();
        reader.short = short;
        reader.interrupt = true;
        let meta = inspect(&mut reader).unwrap();
        assert_eq!(
            (meta.byte_size, meta.frames, meta.stream.rate.numerator),
            (60, 8, 4)
        );
        assert_eq!(reader.bytes, 44);
    }
}
