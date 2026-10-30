use dmd_protocol::{Descriptor, Event, Json, Parameters};
use dmd_replay::{Error, Record, Recording, companion_path};
use std::io::{Cursor, Read, Seek, SeekFrom};

const META: &str = r#"{"format":"dmd-replay","version":{"major":1,"minor":0},"protocol":{"major":1,"minor":0},"signal_sha256":"896e28eebf9cd2df518466bc293bc311b6e3e6fd8dd3c44fd37453eb7fe0e813","module":"demo","info":null,"mask":{},"defaults":{"mode":"raw"},"freq":0,"params":{"mode":"fec"}}"#;
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
fn open_recording(meta: &str, journal: &str, frames: u64) -> Result<Recording, Error> {
    Recording::from_reader(
        Cursor::new(archive(meta, journal.as_bytes(), Some(b"TEST"))),
        frames,
    )
}
fn collect(journal: &str, frames: u64) -> Result<Vec<Record>, Error> {
    let recording = open_recording(META, journal, frames)?;
    let mut reader = recording.journal()?;
    let mut records = Vec::new();
    while let Some(record) = reader.next_record()? {
        records.push(record);
    }
    Ok(records)
}
fn modify(meta: &str, key: &str, value: Option<Json>) -> String {
    let Json::Object(mut object) = Json::parse(meta).unwrap() else {
        panic!()
    };
    match value {
        Some(value) => {
            object.insert(key.into(), value);
        }
        None => {
            object.remove(key);
        }
    }
    Json::Object(object).to_compact_string().unwrap()
}
#[test]
fn literal_companion_paths() {
    assert_eq!(
        companion_path("capture.part.wav"),
        std::path::PathBuf::from("capture.part.wav.replay")
    );
    #[cfg(unix)]
    {
        use std::os::unix::ffi::{OsStrExt, OsStringExt};
        let signal = std::ffi::OsString::from_vec(vec![b's', 0xff, b'.', b'w']);
        assert_eq!(
            companion_path(signal).as_os_str().as_bytes(),
            &[
                b's', 0xff, b'.', b'w', b'.', b'r', b'e', b'p', b'l', b'a', b'y'
            ]
        );
    }
}
fn replay_specification() -> String {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap();
    std::fs::read_to_string(root.join("docs/replay.md")).unwrap()
}
#[test]
fn complete_specification_example_has_independent_expectations() {
    assert_specification_example(&replay_specification());
}
#[test]
fn complete_specification_example_accepts_crlf_markdown() {
    let spec = replay_specification()
        .replace("\r\n", "\n")
        .replace('\n', "\r\n");
    assert_specification_example(&spec);
}
fn assert_specification_example(spec: &str) {
    let spec = spec.replace("\r\n", "\n");
    let section = spec.split("`example.wav.replay`").nth(1).unwrap();
    let metadata = section
        .split("```json\n")
        .nth(1)
        .unwrap()
        .split("\n```")
        .next()
        .unwrap();
    let journal = section
        .split("```json\n")
        .nth(2)
        .unwrap()
        .split("```")
        .next()
        .unwrap();
    let recording = open_recording(metadata, journal, 8).unwrap();
    let metadata = recording.metadata();
    assert_eq!(metadata.module, "demo");
    assert_eq!(
        metadata.signal_sha256,
        "896e28eebf9cd2df518466bc293bc311b6e3e6fd8dd3c44fd37453eb7fe0e813"
    );
    assert_eq!(metadata.defaults["mode"], "raw");
    assert_eq!(metadata.params["mode"], "fec");
    let Json::Object(info) = &metadata.info else {
        panic!()
    };
    assert!(info.contains_key("vendorTag"));
    let mut journal = recording.journal().unwrap();
    let mut boundaries = Vec::new();
    let mut results = Vec::new();
    let mut marker = 0;
    while let Some(record) = journal.next_record().unwrap() {
        match record {
            Record::Boundary { from, to } => boundaries.push((from, to)),
            Record::Finalization => marker += 1,
            Record::Event(Event::Result(result)) => results.push(result),
            _ => {}
        }
    }
    assert_eq!(boundaries, [(0, 4), (4, 4), (4, 8)]);
    assert_eq!(marker, 1);
    assert_eq!(results.len(), 3);
    assert_example_results(&results);
    let mut asset = recording
        .asset(&results[2].binary.as_ref().unwrap()[0])
        .unwrap();
    let mut output = Vec::new();
    let mut byte = [0; 1];
    while asset.read_chunk(&mut byte).unwrap() != 0 {
        output.extend(byte);
    }
    assert_eq!(output, b"TEST");
    assert!(asset.verified());
}
fn assert_example_results(results: &[dmd_protocol::ResultEvent]) {
    assert_eq!(results[0].text.as_deref(), Some("CQ "));
    assert_eq!(
        results[0].extra["record"],
        Json::String("finalization".into())
    );
    assert_eq!(results[1].bits.as_deref(), Some("01"));
    assert_eq!(results[1].log.as_deref(), Some("producer note"));
    let Descriptor::Phase { points, .. } = &results[1].binary.as_ref().unwrap()[0] else {
        panic!()
    };
    assert_eq!((points[0].i, points[0].q), (1000, 0));
    assert_eq!((points[1].i, points[1].q), (-500, 250));
    assert_eq!(results[2].text.as_deref(), Some("TEST"));
    assert!(results[2].ale.is_some());
    assert!(results[2].params.is_some());
}
#[test]
fn metadata_required_types_and_null_info() {
    assert_eq!(
        open_recording(META, "", 0).unwrap().metadata().info,
        Json::Null
    );
    for key in [
        "format",
        "version",
        "protocol",
        "signal_sha256",
        "module",
        "info",
        "mask",
        "defaults",
        "freq",
        "params",
    ] {
        assert!(
            open_recording(&modify(META, key, None), "", 0).is_err(),
            "missing {key}"
        );
        if key != "info" {
            assert!(
                open_recording(&modify(META, key, Some(Json::Null)), "", 0).is_err(),
                "null {key}"
            );
        }
    }
    for (key, bad) in [
        ("format", Json::String("other".into())),
        ("module", Json::String(String::new())),
        ("module", Json::String("a\0b".into())),
        ("signal_sha256", Json::String("A".repeat(64))),
        ("freq", Json::parse("1e99999").unwrap()),
        ("defaults", Json::parse(r#"{"missing":"x"}"#).unwrap()),
        ("params", Json::parse(r#"{"mode":7}"#).unwrap()),
    ] {
        assert!(
            open_recording(&modify(META, key, Some(bad)), "", 0).is_err(),
            "{key}"
        );
    }
    for version in [
        r#"{"major":0,"minor":0}"#,
        r#"{"major":2,"minor":0}"#,
        r#"{"major":1.0,"minor":0}"#,
        r#"{"major":1,"minor":-1}"#,
        r#"{"major":1,"minor":0,"extra":null}"#,
    ] {
        for key in ["version", "protocol"] {
            assert!(
                open_recording(
                    &modify(META, key, Some(Json::parse(version).unwrap())),
                    "",
                    0
                )
                .is_err()
            );
        }
    }
    let higher = modify(
        META,
        "version",
        Some(Json::parse(r#"{"major":1,"minor":4294967295}"#).unwrap()),
    );
    assert!(open_recording(&higher, "", 0).is_ok());
    assert!(open_recording(&modify(META, "extension", Some(Json::Null)), "", 0).is_ok());
    for bad in [
        format!("\u{feff}{META}"),
        format!("{META} null"),
        META.replace(r#""info":null"#, r#""info":{"x":0,"\u0078":1}"#),
        META.replace("\"freq\":0", "\"freq\":01"),
    ] {
        assert!(open_recording(&bad, "", 0).is_err());
    }
}
#[test]
fn fixed_settings_are_complete_and_order_independent() {
    let recording = open_recording(META, "", 0).unwrap();
    let meta = recording.metadata();
    let overrides = Parameters::from([("mode".into(), "fec".into())]);
    assert_eq!(
        meta.match_initial("demo", -0.0, &overrides).unwrap(),
        meta.params
    );
    assert!(meta.match_initial("other", 0.0, &overrides).is_err());
    assert!(meta.match_initial("demo", 1.0, &overrides).is_err());
    assert!(meta.match_initial("demo", f64::NAN, &overrides).is_err());
    assert!(meta.match_initial("demo", 0.0, &Parameters::new()).is_err());
    let mut extra = overrides.clone();
    extra.insert("unknown".into(), "x".into());
    assert!(meta.match_initial("demo", 0.0, &extra).is_err());
    let meta = modify(
        META,
        "params",
        Some(Json::parse(r#"{"required":"yes","mode":"fec"}"#).unwrap()),
    );
    let recording = open_recording(&meta, "", 0).unwrap();
    assert!(
        recording
            .metadata()
            .match_initial("demo", 0.0, &overrides)
            .is_err()
    );
    let mut complete = overrides;
    complete.insert("required".into(), "yes".into());
    assert!(
        recording
            .metadata()
            .match_initial("demo", 0.0, &complete)
            .is_ok()
    );
}
#[test]
fn empty_sources_extensions_and_suffix() {
    assert_eq!(
        collect("{\"record\":\"finalization\"}\n", 0).unwrap(),
        [Record::Finalization]
    );
    let journal = concat!(
        "{\"event\":\"progress\",\"frame\":0}\n",
        "{\"event\":\"progress\",\"frame\":0}\n",
        "{\"record\":\"vendor-note\",\"value\":null}\n",
        "{\"record\":\"finalization\"}\n",
        "{\"event\":\"result\",\"samples\":[0,0]}\n",
        "{\"event\":\"progress\",\"frame\":0}\n"
    );
    let records = collect(journal, 0).unwrap();
    assert_eq!(records.len(), 5);
    assert!(matches!(
        records[4],
        Record::Event(Event::Progress { frame: 0 })
    ));
    let ignored = "{\"event\":\"vendor\",\"record\":false,\"binary\":[{\"path\":\"missing\"}]}\n{\"record\":\"finalization\"}\n";
    assert_eq!(collect(ignored, 0).unwrap(), [Record::Finalization]);
}
#[test]
fn ordering_and_framing_rejections() {
    for journal in [
        "",
        "\n",
        "{}\n",
        "[]\n",
        "{\"record\":\"finalization\"}",
        "{\"record\":\"finalization\"}\n{\"record\":\"finalization\"}\n",
        "{\"event\":null,\"record\":\"finalization\"}\n",
        "{\"event\":\"finished\"}\n",
        "{\"event\":\"result\",\"samples\":[0,0]}\n{\"record\":\"finalization\"}\n",
        "{\"event\":\"unknown\",\"x\":{\"a\":1,\"a\":2}}\n",
        "{\"record\":\"finalization\"} {}\n",
        "\u{feff}{\"record\":\"finalization\"}\n",
    ] {
        assert!(collect(journal, 0).is_err(), "{journal}");
    }
    for journal in [
        "{\"record\":\"finalization\"}\n",
        "{\"event\":\"progress\",\"frame\":4}\n{\"event\":\"progress\",\"frame\":3}\n",
        "{\"event\":\"log\",\"frame\":5,\"level\":\"info\",\"text\":\"\"}\n{\"event\":\"progress\",\"frame\":4}\n",
        "{\"event\":\"log\",\"frame\":5,\"level\":\"info\",\"text\":\"\"}\n{\"event\":\"log\",\"frame\":4,\"level\":\"info\",\"text\":\"\"}\n",
        "{\"event\":\"progress\",\"frame\":8}\n{\"record\":\"finalization\"}\n{\"event\":\"log\",\"frame\":4,\"level\":\"info\",\"text\":\"\"}\n",
        "{\"event\":\"progress\",\"frame\":8}\n{\"record\":\"finalization\"}\n{\"event\":\"progress\",\"frame\":7}\n",
        "{\"event\":\"result\",\"samples\":[0,9]}\n",
        "{\"event\":\"progress\",\"frame\":4.0}\n",
    ] {
        assert!(collect(journal, 8).is_err(), "{journal}");
    }
}
struct Tiny(Cursor<Vec<u8>>);
impl Read for Tiny {
    fn read(&mut self, b: &mut [u8]) -> std::io::Result<usize> {
        let n = b.len().min(1);
        self.0.read(&mut b[..n])
    }
}
impl Seek for Tiny {
    fn seek(&mut self, p: SeekFrom) -> std::io::Result<u64> {
        self.0.seek(p)
    }
}
#[test]
fn split_utf8_crlf_exact_producer_values_and_pending_validation() {
    let journal = concat!(
        "{\"event\":\"result\",\"samples\":[0,8],\"text\":\"Привет\",\"bits\":\"01\",\"log\":\"producer\",\"ale\":null,\"params\":null,\"record\":null,\"opaque\":[123456789012345678901234567890,0.12345678901234567890123456789]}\r\n",
        "{\"event\":\"progress\",\"frame\":8}\r\n",
        "{\"record\":\"finalization\"}\r\n"
    );
    let recording = Recording::from_reader(
        Tiny(Cursor::new(archive(META, journal.as_bytes(), None))),
        8,
    )
    .unwrap();
    let mut reader = recording.journal().unwrap();
    let Some(Record::Event(Event::Result(result))) = reader.next_record().unwrap() else {
        panic!()
    };
    assert_eq!(result.text.as_deref(), Some("Привет"));
    assert_eq!(result.bits.as_deref(), Some("01"));
    assert_eq!(result.ale, Some(Json::Null));
    assert_eq!(
        result.extra["opaque"].to_compact_string().unwrap(),
        "[123456789012345678901234567890,0.12345678901234567890123456789]"
    );
    assert!(matches!(
        reader.next_record().unwrap(),
        Some(Record::Boundary { from: 0, to: 8 })
    ));
    assert_eq!(reader.next_record().unwrap(), Some(Record::Finalization));
    assert_eq!(reader.next_record().unwrap(), None);
    let recording = open_recording(META, "{\"event\":\"result\",\"samples\":[0,0]}\n", 0).unwrap();
    let mut reader = recording.journal().unwrap();
    assert!(reader.next_record().unwrap().is_some());
    assert!(matches!(reader.next_record(), Err(Error::Sequence(_))));
}
#[test]
fn replay_depth_including_ignored_data_is_distinct() {
    for (depth, valid) in [(127, true), (128, false)] {
        let nested = format!("{}null{}", "[".repeat(depth), "]".repeat(depth));
        let journal = format!(
            "{{\"event\":\"unknown\",\"ignored\":{nested}}}\n{{\"record\":\"finalization\"}}\n"
        );
        assert_eq!(collect(&journal, 0).is_ok(), valid);
        let meta = modify(META, "info", Some(Json::parse(&nested).unwrap()));
        assert_eq!(open_recording(&meta, "", 0).is_ok(), valid);
    }
}
#[test]
fn malformed_descriptors_and_conflicting_references() {
    for binary in [
        r#"[{"id":"p","type":"phase","body":null,"points":[[1001,0]]}]"#,
        r#"[{"id":"p","type":"phase","body":{},"points":[[1.0,0]]}]"#,
        r#"[{"id":"p","type":"file","body":{},"path":"x","size":0,"points":[]}]"#,
    ] {
        assert!(
            collect(
                &format!("{{\"event\":\"result\",\"samples\":[0,0],\"binary\":{binary}}}\n"),
                0
            )
            .is_err()
        );
    }
    let result = |size| {
        format!(
            "{{\"event\":\"result\",\"samples\":[0,0],\"binary\":[{{\"id\":\"x\",\"type\":\"opaque\",\"body\":null,\"path\":\"x\",\"size\":{size}}}]}}\n"
        )
    };
    assert!(matches!(
        collect(&(result(1) + &result(2)), 0),
        Err(Error::ConflictingAsset(_))
    ));
    assert!(
        collect(
            &(result(1)
                + &result(1)
                + "{\"event\":\"progress\",\"frame\":0}\n{\"record\":\"finalization\"}\n"),
            0
        )
        .is_ok()
    );
}

struct Observed {
    source: Cursor<Vec<u8>>,
    reads: std::sync::Arc<std::sync::Mutex<Vec<(u64, usize)>>>,
}
impl Read for Observed {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        let at = self.source.position();
        let n = self.source.read(buffer)?;
        self.reads.lock().unwrap().push((at, n));
        Ok(n)
    }
}
impl Seek for Observed {
    fn seek(&mut self, p: SeekFrom) -> std::io::Result<u64> {
        self.source.seek(p)
    }
}
#[test]
fn setup_does_not_read_unused_content_or_scan_journal() {
    // Keep the unused payload larger than the bounded ZIP tail lookup window.
    let payload = vec![0xa5; 100_000];
    let journal = "{\"record\":\"finalization\"}\n";
    let bytes = archive(META, journal.as_bytes(), Some(&payload));
    let needle = b"payloads/files/message.bin";
    let header = bytes
        .windows(needle.len())
        .position(|b| b == needle)
        .unwrap();
    let content = header + needle.len();
    let reads = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let recording = Recording::from_reader(
        Observed {
            source: Cursor::new(bytes),
            reads: reads.clone(),
        },
        0,
    )
    .unwrap();
    let accesses = reads.lock().unwrap();
    // Index discovery may read the final 65,557 archive bytes. It must not touch
    // this earlier payload interior, and no content decompression is requested.
    let interior = content as u64 + 100;
    assert!(
        !accesses
            .iter()
            .any(|(at, len)| *at <= interior && interior < *at + *len as u64)
    );
    let journal_header = 30 + "metadata.json".len() + META.len();
    let journal_content = journal_header + 30 + "journal.ndjson".len();
    assert!(
        !accesses
            .iter()
            .any(|(at, len)| *at <= journal_content as u64
                && (journal_content as u64) < *at + *len as u64)
    );
    drop(accesses);
    let mut reader = recording.journal().unwrap();
    assert_eq!(reader.next_record().unwrap(), Some(Record::Finalization));
    assert_eq!(reader.next_record().unwrap(), None);
}
#[test]
fn journal_line_limit_accepts_equality_and_rejects_excess() {
    const LIMIT: usize = 16_777_216;
    let prefix = "{\"event\":\"unknown\",\"text\":\"";
    let suffix = "\"}";
    for (length, terminator, valid) in [
        (LIMIT, "\n", true),
        (LIMIT, "\r\n", true),
        (LIMIT + 1, "\n", false),
    ] {
        let line = format!(
            "{prefix}{}{suffix}{terminator}{{\"record\":\"finalization\"}}\n",
            "a".repeat(length - prefix.len() - suffix.len())
        );
        assert_eq!(collect(&line, 0).is_ok(), valid);
    }
}
#[test]
fn malformed_utf8_and_missing_archive_are_explicit() {
    let data = archive(
        META,
        b"{\"record\":\"finalization\",\"x\":\"\xff\"}\n",
        None,
    );
    let recording = Recording::from_reader(Cursor::new(data), 0).unwrap();
    assert!(matches!(
        recording.journal().unwrap().next_record(),
        Err(Error::Framing(_))
    ));
    let missing =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("does-not-exist.replay");
    assert!(matches!(Recording::open(missing, 0), Err(Error::Io(_))));
}

#[cfg(unix)]
#[test]
fn replacing_archive_pathname_does_not_redirect_payload_reads() {
    let directory = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join(format!("identity-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("recording.replay");
    let moved = directory.join("selected.replay");
    std::fs::write(
        &path,
        archive(META, b"{\"record\":\"finalization\"}\n", Some(b"TEST")),
    )
    .unwrap();
    let recording = Recording::open(&path, 0).unwrap();
    std::fs::rename(&path, &moved).unwrap();
    std::fs::write(
        &path,
        archive(META, b"{\"record\":\"finalization\"}\n", Some(b"EVIL")),
    )
    .unwrap();
    let descriptor = Descriptor::File {
        id: "different-from-path".into(),
        kind: "opaque".into(),
        body: Json::Null,
        path: "files/message.bin".into(),
        size: 4,
    };
    let mut asset = recording.asset(&descriptor).unwrap();
    let mut output = [0; 4];
    assert_eq!(asset.read_chunk(&mut output).unwrap(), 4);
    assert_eq!(&output, b"TEST");
    assert_eq!(asset.read_chunk(&mut output).unwrap(), 0);
    assert!(asset.verified());
    std::fs::remove_file(path).unwrap();
    std::fs::remove_file(moved).unwrap();
    std::fs::remove_dir(directory).unwrap();
}
struct ReportedLength {
    length: u64,
}
impl Read for ReportedLength {
    fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
        panic!("archive limit must precede every read")
    }
}
impl Seek for ReportedLength {
    fn seek(&mut self, p: SeekFrom) -> std::io::Result<u64> {
        assert!(matches!(p, SeekFrom::End(0)));
        Ok(self.length)
    }
}
#[test]
fn oversized_archive_and_directory_fail_before_allocation() {
    assert!(matches!(
        Recording::from_reader(
            ReportedLength {
                length: (1 << 40) + 1
            },
            0
        ),
        Err(Error::Resource("archive bytes"))
    ));
    let mut data = archive(META, b"{\"record\":\"finalization\"}\n", None);
    let end = data.len() - 22;
    data[end + 12..end + 16].copy_from_slice(&((1u32 << 26) + 1).to_le_bytes());
    assert!(matches!(
        Recording::from_reader(Cursor::new(data), 0),
        Err(Error::Resource("central directory"))
    ));
}
