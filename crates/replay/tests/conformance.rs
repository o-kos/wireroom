use dmd_protocol::{Descriptor, Event};
use dmd_replay::{Record, Recording};
use std::io::{Cursor, Write};

const META: &str = r#"{"format":"dmd-replay","version":{"major":1,"minor":0},"protocol":{"major":1,"minor":0},"signal_sha256":"896e28eebf9cd2df518466bc293bc311b6e3e6fd8dd3c44fd37453eb7fe0e813","module":"demo","info":null,"mask":{},"defaults":{"mode":"raw"},"freq":0,"params":{"mode":"fec"}}"#;
fn archive(meta: &str, journal: &[u8], payload: Option<&[u8]>) -> Vec<u8> {
    archive_method(meta, journal, payload, 0)
}
fn archive_method(meta: &str, journal: &[u8], payload: Option<&[u8]>, method: u16) -> Vec<u8> {
    let mut members = vec![
        ("metadata.json", meta.as_bytes()),
        ("journal.ndjson", journal),
    ];
    if let Some(bytes) = payload {
        members.push(("payloads/files/message.bin", bytes));
    }
    let mut zip = Vec::new();
    let mut central = Vec::new();
    for (name, data) in members {
        let offset = zip.len() as u32;
        let crc = crc32fast::hash(data);
        let n = data.len() as u32;
        let compressed = if method == 8 {
            let mut encoder =
                flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::fast());
            encoder.write_all(data).unwrap();
            encoder.finish().unwrap()
        } else {
            data.to_vec()
        };
        let packed = compressed.len() as u32;
        zip.extend(0x04034b50u32.to_le_bytes());
        zip.extend(20u16.to_le_bytes());
        zip.extend(0x800u16.to_le_bytes());
        zip.extend(method.to_le_bytes());
        zip.extend([0; 4]);
        zip.extend(crc.to_le_bytes());
        zip.extend(packed.to_le_bytes());
        zip.extend(n.to_le_bytes());
        zip.extend((name.len() as u16).to_le_bytes());
        zip.extend([0; 2]);
        zip.extend(name.bytes());
        zip.extend(compressed);
        central.extend(0x02014b50u32.to_le_bytes());
        central.extend(20u16.to_le_bytes());
        central.extend(20u16.to_le_bytes());
        central.extend(0x800u16.to_le_bytes());
        central.extend(method.to_le_bytes());
        central.extend([0; 4]);
        central.extend(crc.to_le_bytes());
        central.extend(packed.to_le_bytes());
        central.extend(n.to_le_bytes());
        central.extend((name.len() as u16).to_le_bytes());
        central.extend([0; 12]);
        central.extend(offset.to_le_bytes());
        central.extend(name.bytes());
    }
    let offset = zip.len() as u32;
    let size = central.len() as u32;
    zip.extend(central);
    zip.extend(0x06054b50u32.to_le_bytes());
    zip.extend([0; 4]);
    let count = if payload.is_some() { 3u16 } else { 2 };
    zip.extend(count.to_le_bytes());
    zip.extend(count.to_le_bytes());
    zip.extend(size.to_le_bytes());
    zip.extend(offset.to_le_bytes());
    zip.extend([0; 2]);
    zip
}

const JOURNAL: &[u8] = b"{\"event\":\"progress\",\"frame\":8}\n{\"record\":\"finalization\"}\n";
fn descriptor() -> Descriptor {
    Descriptor::File {
        id: "message".into(),
        kind: "file".into(),
        body: dmd_protocol::Json::Null,
        path: "files/message.bin".into(),
        size: 4,
    }
}
#[test]
fn stored_and_deflated_recordings_stream_exact_payload_and_terminal_boundary() {
    for method in [0, 8] {
        let recording = Recording::from_reader(
            Cursor::new(archive_method(META, JOURNAL, Some(b"TEST"), method)),
            8,
        )
        .unwrap();
        let mut journal = recording.journal().unwrap();
        assert_eq!(
            journal.next_record().unwrap(),
            Some(Record::Boundary { from: 0, to: 8 })
        );
        assert_eq!(journal.next_record().unwrap(), Some(Record::Finalization));
        assert_eq!(journal.next_record().unwrap(), None);
        let mut asset = recording.asset(&descriptor()).unwrap();
        let mut buffer = [0; 8];
        assert_eq!(asset.read_chunk(&mut buffer).unwrap(), 4);
        assert_eq!(&buffer[..4], b"TEST");
        assert!(!asset.verified());
        assert_eq!(asset.read_chunk(&mut buffer).unwrap(), 0);
        assert!(asset.verified());
    }
}
#[test]
fn metadata_digest_is_syntactic_and_settings_remain_fixed() {
    let recording = Recording::from_reader(Cursor::new(archive(META, JOURNAL, None)), 8).unwrap();
    let params = [("mode".into(), "fec".into())].into();
    assert!(
        recording
            .metadata()
            .match_initial("demo", 0.0, &params)
            .is_ok()
    );
    assert!(
        recording
            .metadata()
            .match_initial("other", 0.0, &params)
            .is_err()
    );
    assert!(
        recording
            .metadata()
            .match_initial("demo", 1.0, &params)
            .is_err()
    );
    assert!(
        recording
            .metadata()
            .match_initial("demo", 0.0, &Default::default())
            .is_err()
    );
    let bad = META.replace("896e28", "BADBAD");
    assert!(Recording::from_reader(Cursor::new(archive(&bad, JOURNAL, None)), 8).is_err());
}
#[test]
fn corrupt_journal_and_unused_payload_remain_lazy() {
    let mut bytes = archive(META, b"invalid journal\n", Some(b"TEST"));
    let at = bytes.windows(4).position(|b| b == b"TEST").unwrap();
    bytes[at] ^= 1;
    let recording = Recording::from_reader(Cursor::new(bytes), 8).unwrap();
    assert!(recording.journal().unwrap().next_record().is_err());
    let mut asset = recording.asset(&descriptor()).unwrap();
    let mut buffer = [0; 8];
    assert_eq!(asset.read_chunk(&mut buffer).unwrap(), 4);
    assert!(asset.read_chunk(&mut buffer).is_err());
}
#[test]
fn missing_asset_and_unsafe_index_names_fail_explicitly() {
    let recording = Recording::from_reader(Cursor::new(archive(META, JOURNAL, None)), 8).unwrap();
    assert!(recording.asset(&descriptor()).is_err());
    let bytes = archive(META, JOURNAL, Some(b"TEST"));
    let original = b"payloads/files/message.bin";
    let unsafe_name = b"payloads/../xx/message.bin";
    assert_eq!(original.len(), unsafe_name.len());
    let mut bytes = bytes;
    for at in 0..=bytes.len() - original.len() {
        if &bytes[at..at + original.len()] == original {
            bytes[at..at + original.len()].copy_from_slice(unsafe_name);
        }
    }
    assert!(Recording::from_reader(Cursor::new(bytes), 8).is_err());
}
#[test]
fn classic_profile_rejects_descriptors_zip64_encryption_and_comments_at_setup() {
    let original = archive(META, JOURNAL, None);
    let central = original
        .windows(4)
        .position(|b| b == b"PK\x01\x02")
        .unwrap();
    for flags in [0x808u16, 0x801] {
        let mut bytes = original.clone();
        bytes[central + 8..central + 10].copy_from_slice(&flags.to_le_bytes());
        assert!(Recording::from_reader(Cursor::new(bytes), 8).is_err());
    }
    let mut wide = original.clone();
    wide[central + 24..central + 28].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(Recording::from_reader(Cursor::new(wide), 8).is_err());
    let mut multipart = original.clone();
    let end = multipart.len() - 22;
    multipart[end + 4..end + 6].copy_from_slice(&1u16.to_le_bytes());
    assert!(Recording::from_reader(Cursor::new(multipart), 8).is_err());
    let mut entry_comment = original.clone();
    entry_comment[central + 32..central + 34].copy_from_slice(&1u16.to_le_bytes());
    assert!(matches!(
        Recording::from_reader(Cursor::new(entry_comment), 8),
        Err(dmd_replay::Error::Archive("entry comments unsupported"))
    ));
    let mut commented = original;
    let end = commented.len();
    commented[end - 2..].copy_from_slice(&1u16.to_le_bytes());
    commented.push(b'x');
    assert!(Recording::from_reader(Cursor::new(commented), 8).is_err());
}
#[test]
fn invalid_boundaries_and_missing_marker_are_rejected() {
    for journal in [
        b"{\"event\":\"progress\",\"frame\":9}\n".as_slice(),
        b"{\"record\":\"finalization\"}\n",
        b"{\"event\":\"progress\",\"frame\":8}\n",
    ] {
        let recording =
            Recording::from_reader(Cursor::new(archive(META, journal, None)), 8).unwrap();
        let mut journal = recording.journal().unwrap();
        while let Ok(Some(record)) = journal.next_record() {
            assert!(matches!(record, Record::Boundary { .. }));
        }
        assert!(journal.next_record().is_err());
    }
}
#[test]
fn producer_text_and_overlapping_source_spans_are_preserved() {
    let bytes = b"{\"event\":\"result\",\"samples\":[0,4],\"text\":\"CQ \"}\n{\"event\":\"progress\",\"frame\":8}\n{\"record\":\"finalization\"}\n{\"event\":\"result\",\"samples\":[2,8],\"text\":\"TEST\"}\n";
    let recording = Recording::from_reader(Cursor::new(archive(META, bytes, None)), 8).unwrap();
    let mut journal = recording.journal().unwrap();
    let mut text = Vec::new();
    while let Some(record) = journal.next_record().unwrap() {
        if let Record::Event(Event::Result(result)) = record {
            text.push((result.samples.start, result.samples.end, result.text));
        }
    }
    assert_eq!(
        text,
        [(0, 4, Some("CQ ".into())), (2, 8, Some("TEST".into()))]
    );
}
