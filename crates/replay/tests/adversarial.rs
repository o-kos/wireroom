use std::{
    fs,
    io::{Cursor, Write},
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

use dmd_protocol::{Descriptor, Json};
use dmd_replay::Recording;
use flate2::{Compression, write::DeflateEncoder};

const METADATA: &[u8] = br#"{"format":"dmd-replay","version":{"major":1,"minor":0},"protocol":{"major":1,"minor":0},"signal_sha256":"896e28eebf9cd2df518466bc293bc311b6e3e6fd8dd3c44fd37453eb7fe0e813","module":"demo","info":null,"mask":{},"defaults":{},"freq":0,"params":{}}"#;
const JOURNAL: &[u8] = b"{\"event\":\"progress\",\"frame\":8}\n{\"record\":\"finalization\"}\n";

struct Member {
    name: Vec<u8>,
    decoded: Vec<u8>,
    compressed: Vec<u8>,
    method: u16,
    flags: u16,
    attributes: u32,
    zip64: bool,
    descriptor_signature: Option<bool>,
}

impl Member {
    fn stored(name: &str, data: &[u8]) -> Self {
        Self {
            name: name.as_bytes().to_vec(),
            decoded: data.to_vec(),
            compressed: data.to_vec(),
            method: 0,
            flags: 0x0800,
            attributes: 0,
            zip64: false,
            descriptor_signature: None,
        }
    }

    fn deflated(name: &str, data: &[u8]) -> Self {
        let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(data).unwrap();
        Self {
            compressed: encoder.finish().unwrap(),
            method: 8,
            ..Self::stored(name, data)
        }
    }

    fn header(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend(0x04034b50_u32.to_le_bytes());
        for value in [20, self.flags, self.method, 0, 0] {
            bytes.extend(value.to_le_bytes());
        }
        let crc = if self.descriptor_signature.is_some() {
            0
        } else {
            crc32fast::hash(&self.decoded)
        };
        bytes.extend(crc.to_le_bytes());
        let compressed = if self.zip64 {
            u32::MAX
        } else if self.descriptor_signature.is_some() {
            0
        } else {
            self.compressed.len() as u32
        };
        let decoded = if self.zip64 {
            u32::MAX
        } else if self.descriptor_signature.is_some() {
            0
        } else {
            self.decoded.len() as u32
        };
        bytes.extend(compressed.to_le_bytes());
        bytes.extend(decoded.to_le_bytes());
        bytes.extend((self.name.len() as u16).to_le_bytes());
        bytes.extend((if self.zip64 { 20_u16 } else { 0 }).to_le_bytes());
        bytes.extend(&self.name);
        if self.zip64 {
            bytes.extend(1_u16.to_le_bytes());
            bytes.extend(16_u16.to_le_bytes());
            bytes.extend((self.decoded.len() as u64).to_le_bytes());
            bytes.extend((self.compressed.len() as u64).to_le_bytes());
        }
        bytes
    }
    fn write_local(&self, bytes: &mut Vec<u8>) {
        bytes.extend(self.header());
        bytes.extend(&self.compressed);
        if let Some(signed) = self.descriptor_signature {
            if signed {
                bytes.extend(0x08074b50_u32.to_le_bytes());
            }
            bytes.extend(crc32fast::hash(&self.decoded).to_le_bytes());
            if self.zip64 {
                bytes.extend((self.compressed.len() as u64).to_le_bytes());
                bytes.extend((self.decoded.len() as u64).to_le_bytes());
            } else {
                bytes.extend((self.compressed.len() as u32).to_le_bytes());
                bytes.extend((self.decoded.len() as u32).to_le_bytes());
            }
        }
    }
}

struct Zip {
    bytes: Vec<u8>,
    local: Vec<usize>,
    central: Vec<usize>,
}

impl Zip {
    fn build(members: &[Member]) -> Self {
        let mut bytes = Vec::new();
        let mut local = Vec::new();
        let mut central = Vec::new();
        for member in members {
            local.push(bytes.len());
            member.write_local(&mut bytes);
        }
        let directory = bytes.len();
        for (member, offset) in members.iter().zip(&local) {
            central.push(bytes.len());
            bytes.extend(0x02014b50_u32.to_le_bytes());
            for value in [20, 20, member.flags, member.method, 0, 0] {
                bytes.extend(value.to_le_bytes());
            }
            bytes.extend(crc32fast::hash(&member.decoded).to_le_bytes());
            bytes.extend(
                (if member.zip64 {
                    u32::MAX
                } else {
                    member.compressed.len() as u32
                })
                .to_le_bytes(),
            );
            bytes.extend(
                (if member.zip64 {
                    u32::MAX
                } else {
                    member.decoded.len() as u32
                })
                .to_le_bytes(),
            );
            for value in [
                member.name.len() as u16,
                if member.zip64 { 28 } else { 0 },
                0,
                0,
                0,
            ] {
                bytes.extend(value.to_le_bytes());
            }
            bytes.extend(member.attributes.to_le_bytes());
            bytes.extend(
                (if member.zip64 {
                    u32::MAX
                } else {
                    *offset as u32
                })
                .to_le_bytes(),
            );
            bytes.extend(&member.name);
            if member.zip64 {
                bytes.extend(1_u16.to_le_bytes());
                bytes.extend(24_u16.to_le_bytes());
                bytes.extend((member.decoded.len() as u64).to_le_bytes());
                bytes.extend((member.compressed.len() as u64).to_le_bytes());
                bytes.extend((*offset as u64).to_le_bytes());
            }
        }
        let directory_size = bytes.len() - directory;
        bytes.extend(0x06054b50_u32.to_le_bytes());
        for value in [0, 0, members.len() as u16, members.len() as u16] {
            bytes.extend(value.to_le_bytes());
        }
        bytes.extend((directory_size as u32).to_le_bytes());
        bytes.extend((directory as u32).to_le_bytes());
        bytes.extend(0_u16.to_le_bytes());
        Self {
            bytes,
            local,
            central,
        }
    }

    fn standard(extra: Vec<Member>) -> Self {
        let mut members = vec![
            Member::stored("metadata.json", METADATA),
            Member::stored("journal.ndjson", JOURNAL),
        ];
        members.extend(extra);
        Self::build(&members)
    }

    fn zip64_footer(&mut self) {
        let end = self.bytes.len() - 22;
        let mut old = self.bytes.split_off(end);
        let count = u16::from_le_bytes(old[10..12].try_into().unwrap()) as u64;
        let size = u32::from_le_bytes(old[12..16].try_into().unwrap()) as u64;
        let offset = u32::from_le_bytes(old[16..20].try_into().unwrap()) as u64;
        self.bytes.extend(0x06064b50_u32.to_le_bytes());
        self.bytes.extend(44_u64.to_le_bytes());
        self.bytes.extend(45_u16.to_le_bytes());
        self.bytes.extend(45_u16.to_le_bytes());
        self.bytes.extend(0_u32.to_le_bytes());
        self.bytes.extend(0_u32.to_le_bytes());
        self.bytes.extend(count.to_le_bytes());
        self.bytes.extend(count.to_le_bytes());
        self.bytes.extend(size.to_le_bytes());
        self.bytes.extend(offset.to_le_bytes());
        self.bytes.extend(0x07064b50_u32.to_le_bytes());
        self.bytes.extend(0_u32.to_le_bytes());
        self.bytes.extend((end as u64).to_le_bytes());
        self.bytes.extend(1_u32.to_le_bytes());
        old[8..12].copy_from_slice(&[0xff; 4]);
        old[12..20].copy_from_slice(&[0xff; 8]);
        self.bytes.extend(old);
    }

    fn u16(&mut self, offset: usize, value: u16) {
        self.bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
    }

    fn u32(&mut self, offset: usize, value: u32) {
        self.bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
}

struct Fixture(PathBuf);

impl Fixture {
    fn new(bytes: &[u8]) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .unwrap()
            .join("target/replay-adversarial");
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join(format!(
            "{}-{}.replay",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::write(&path, bytes).unwrap();
        Self(path)
    }

    fn open(&self) -> Result<Recording, dmd_replay::Error> {
        Recording::open(&self.0, 8)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_file(&self.0).unwrap();
    }
}

fn descriptor(path: &str, size: u64) -> Descriptor {
    Descriptor::File {
        id: "opaque".into(),
        kind: "vendor-type".into(),
        body: Json::Null,
        path: path.into(),
        size,
    }
}

fn asset_fails(zip: &Zip, path: &str, size: u64) {
    let fixture = Fixture::new(&zip.bytes);
    let recording = fixture.open().unwrap();
    let Ok(mut asset) = recording.asset(&descriptor(path, size)) else {
        return;
    };
    let mut buffer = [0; 7];
    loop {
        match asset.read_chunk(&mut buffer) {
            Err(_) => break,
            Ok(0) => panic!("invalid asset was accepted"),
            Ok(_) => assert!(!asset.verified()),
        }
    }
}

#[test]
fn index_rejects_nonadjacent_prefix_conflicts_and_unused_duplicate_names() {
    for names in [
        vec!["payloads/a", "payloads/a-", "payloads/a/b"],
        vec!["extensions/x", "extensions/x"],
        vec!["metadata.json"],
    ] {
        let zip = Zip::standard(
            names
                .iter()
                .map(|name| Member::stored(name, b"x"))
                .collect(),
        );
        assert!(Fixture::new(&zip.bytes).open().is_err(), "{names:?}");
    }
}

#[test]
fn index_checks_unused_flags_kinds_and_raw_utf8() {
    let mut cases = Vec::new();
    for flags in [0, 0x0801, 0x0840] {
        let mut member = Member::stored("extensions/unused", b"ignored");
        member.flags = flags;
        cases.push(member);
    }
    let mut directory = Member::stored("extensions/unused", b"");
    directory.attributes = 0x10;
    cases.push(directory);
    let mut invalid = Member::stored("extensions/unused", b"");
    *invalid.name.last_mut().unwrap() = 0xff;
    cases.push(invalid);
    for member in cases {
        let zip = Zip::standard(vec![member]);
        assert!(Fixture::new(&zip.bytes).open().is_err());
    }
}

#[test]
fn local_header_validation_is_lazy_for_unused_assets() {
    let mut zip = Zip::standard(vec![Member::stored("payloads/a", b"TEST")]);
    zip.u16(zip.local[2] + 8, 8);
    let fixture = Fixture::new(&zip.bytes);
    let recording = fixture.open().unwrap();
    assert!(recording.asset(&descriptor("a", 4)).is_err());
    let mut journal = recording.journal().unwrap();
    while journal.next_record().unwrap().is_some() {}
}

#[test]
fn metadata_and_journal_local_headers_are_setup_obligations() {
    for member in [0, 1] {
        let mut zip = Zip::standard(Vec::new());
        zip.u16(zip.local[member] + 6, 0);
        assert!(Fixture::new(&zip.bytes).open().is_err());
    }
}

#[test]
fn bad_stored_crc_and_actual_size_are_never_certified() {
    let mut bad_crc = Zip::standard(vec![Member::stored("payloads/a", b"TEST")]);
    bad_crc.u32(bad_crc.local[2] + 14, 0);
    bad_crc.u32(bad_crc.central[2] + 16, 0);
    asset_fails(&bad_crc, "a", 4);
    let mut bad_size = Zip::standard(vec![Member::stored("payloads/a", b"TEST")]);
    bad_size.u32(bad_size.local[2] + 22, 5);
    bad_size.u32(bad_size.central[2] + 24, 5);
    asset_fails(&bad_size, "a", 5);
}

#[test]
fn deflate_requires_stream_end_and_exact_compressed_consumption() {
    for trailing in [false, true] {
        let mut member = Member::deflated("payloads/a", b"TEST");
        if trailing {
            member.compressed.extend(b"unused compressed suffix");
        } else {
            member.compressed.pop();
        }
        asset_fails(&Zip::standard(vec![member]), "a", 4);
    }
}

#[test]
fn partial_reads_and_empty_buffers_do_not_claim_integrity() {
    let fixture = Fixture::new(&Zip::standard(vec![Member::stored("payloads/a", b"TEST")]).bytes);
    let recording = fixture.open().unwrap();
    let mut asset = recording.asset(&descriptor("a", 4)).unwrap();
    assert_eq!(asset.read_chunk(&mut []).unwrap(), 0);
    assert!(!asset.verified());
    let mut byte = [0];
    assert_eq!(asset.read_chunk(&mut byte).unwrap(), 1);
    assert_eq!(byte, [b'T']);
    assert!(!asset.verified());
}

#[test]
fn consumed_ranges_include_embedded_local_headers() {
    let embedded = Member::stored("payloads/b", b"B");
    let mut content = embedded.header();
    content.extend(&embedded.compressed);
    let mut zip = Zip::standard(vec![Member::stored("payloads/a", &content), embedded]);
    let embedded_offset = zip.local[2] + 30 + "payloads/a".len();
    zip.u32(zip.central[3] + 42, embedded_offset as u32);
    for first_a in [true, false] {
        let fixture = Fixture::new(&zip.bytes);
        let recording = fixture.open().unwrap();
        let a = descriptor("a", content.len() as u64);
        let b = descriptor("b", 1);
        let (first, second) = if first_a { (&a, &b) } else { (&b, &a) };
        let mut asset = recording.asset(first).unwrap();
        let mut buffer = [0; 64];
        while asset.read_chunk(&mut buffer).unwrap() != 0 {}
        assert!(asset.verified());
        assert!(recording.asset(second).is_err());
    }
}

#[test]
fn clean_stored_and_deflated_assets_preserve_bytes() {
    for member in [
        Member::stored("payloads/a", b"TEST"),
        Member::deflated("payloads/a", b"TEST"),
    ] {
        let fixture = Fixture::new(&Zip::standard(vec![member]).bytes);
        let recording = fixture.open().unwrap();
        let mut asset = recording.asset(&descriptor("a", 4)).unwrap();
        let mut actual = Vec::<u8>::new();
        let mut buffer = [0; 2];
        loop {
            let count = asset.read_chunk(&mut buffer).unwrap();
            if count == 0 {
                break;
            }
            actual.extend(&buffer[..count]);
        }
        assert_eq!(actual, b"TEST");
        assert!(asset.verified());
    }
}

#[test]
fn fake_eocd_inside_a_valid_comment_is_not_the_directory() {
    let mut zip = Zip::standard(Vec::new());
    let comment = b"prefix PK\x05\x06 this is not an end record suffix";
    let length_offset = zip.bytes.len() - 2;
    zip.u16(length_offset, comment.len() as u16);
    zip.bytes.extend(comment);
    let fixture = Fixture::new(&zip.bytes);
    let recording = fixture.open().unwrap();
    let mut journal = recording.journal().unwrap();
    while journal.next_record().unwrap().is_some() {}
}

#[test]
fn stored_archive_bytes_can_be_supplied_without_a_path() {
    let bytes = Zip::standard(Vec::new()).bytes;
    let recording = Recording::from_reader(Cursor::new(bytes), 8).unwrap();
    let mut journal = recording.journal().unwrap();
    while journal.next_record().unwrap().is_some() {}
}

#[test]
fn zip64_entries_and_both_descriptor_widths_with_or_without_signatures() {
    for zip64 in [false, true] {
        for signature in [None, Some(false), Some(true)] {
            let mut member = Member::deflated("payloads/a", b"TEST");
            member.zip64 = zip64;
            member.descriptor_signature = signature;
            if signature.is_some() {
                member.flags |= 8;
            }
            let mut zip = Zip::standard(vec![member]);
            if zip64 {
                zip.zip64_footer();
            }
            let recording = Recording::from_reader(Cursor::new(zip.bytes), 8).unwrap();
            let mut asset = recording.asset(&descriptor("a", 4)).unwrap();
            let mut output = [0; 16];
            assert_eq!(asset.read_chunk(&mut output).unwrap(), 4);
            assert_eq!(&output[..4], b"TEST");
            assert_eq!(asset.read_chunk(&mut output).unwrap(), 0);
            assert!(asset.verified());
        }
    }
}

#[test]
fn invalid_descriptors_and_zip64_locator_are_rejected() {
    for zip64 in [false, true] {
        let mut member = Member::stored("payloads/a", b"TEST");
        member.zip64 = zip64;
        member.descriptor_signature = Some(true);
        member.flags |= 8;
        let data_end = member.header().len() + member.compressed.len();
        let mut zip = Zip::standard(vec![member]);
        zip.u32(zip.local[2] + data_end + 4, 0);
        asset_fails(&zip, "a", 4);
    }
    let mut zip = Zip::standard(Vec::new());
    zip.zip64_footer();
    let locator = zip.bytes.len() - 42;
    zip.u32(locator + 16, 2);
    assert!(Fixture::new(&zip.bytes).open().is_err());
}
