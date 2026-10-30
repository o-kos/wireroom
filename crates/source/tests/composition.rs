use dmd_protocol::{Rate, Stream, StreamFormat};
use dmd_replay::{Record, Recording};
use std::io::Cursor;

// Reuse the stored ZIP fixture builder from replay conformance tests. Do not
// include that test module: its unrelated tests would run in this crate too.
fn archive(meta: &str, journal: &[u8], payload: Option<&[u8]>) -> Vec<u8> {
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
        zip.extend(0x04034b50u32.to_le_bytes());
        zip.extend(20u16.to_le_bytes());
        zip.extend(0x800u16.to_le_bytes());
        zip.extend([0; 6]);
        zip.extend(crc.to_le_bytes());
        zip.extend(n.to_le_bytes());
        zip.extend(n.to_le_bytes());
        zip.extend((name.len() as u16).to_le_bytes());
        zip.extend([0; 2]);
        zip.extend(name.bytes());
        zip.extend(data);
        central.extend(0x02014b50u32.to_le_bytes());
        central.extend(20u16.to_le_bytes());
        central.extend(20u16.to_le_bytes());
        central.extend(0x800u16.to_le_bytes());
        central.extend([0; 6]);
        central.extend(crc.to_le_bytes());
        central.extend(n.to_le_bytes());
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

#[test]
fn exact_document_source_composes_with_replay_in_lf_and_crlf_checkouts() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap()
        .join("docs/replay.md");
    let document = std::fs::read_to_string(path).unwrap().replace("\r\n", "\n");
    for text in [document.clone(), document.replace('\n', "\r\n")] {
        assert_document(&text);
    }
}

fn assert_document(document: &str) {
    let document = document.replace("\r\n", "\n");
    let source_section = document.split("The signal `example.wav`").nth(1).unwrap();
    let hex = source_section
        .split("```text\n")
        .nth(1)
        .unwrap()
        .split("```")
        .next()
        .unwrap();
    let bytes: Vec<u8> = hex
        .split_whitespace()
        .map(|byte| u8::from_str_radix(byte, 16).unwrap())
        .collect();
    assert_eq!(bytes.len(), 60);
    let source = dmd_source::inspect(&mut Cursor::new(bytes)).unwrap();
    assert_eq!(source.byte_size, 60);
    assert_eq!(source.frames, 8);
    assert_eq!(
        source.stream,
        Stream {
            format: StreamFormat::Real,
            channels: 1,
            rate: Rate {
                numerator: 4,
                denominator: 1
            },
        }
    );
    let replay = source_section.split("`example.wav.replay`").nth(1).unwrap();
    let metadata = replay
        .split("```json\n")
        .nth(1)
        .unwrap()
        .split("\n```")
        .next()
        .unwrap();
    let journal = replay
        .split("```json\n")
        .nth(2)
        .unwrap()
        .split("```")
        .next()
        .unwrap();
    let recording = Recording::from_reader(
        Cursor::new(archive(metadata, journal.as_bytes(), Some(b"TEST"))),
        source.frames,
    )
    .unwrap();
    let mut journal = recording.journal().unwrap();
    let mut boundaries = Vec::new();
    let mut finalizations = 0;
    while let Some(record) = journal.next_record().unwrap() {
        match record {
            Record::Boundary { from, to } => boundaries.push((from, to)),
            Record::Finalization => finalizations += 1,
            _ => {}
        }
    }
    assert_eq!(boundaries, [(0, 4), (4, 4), (4, 8)]);
    assert_eq!(finalizations, 1);
}
