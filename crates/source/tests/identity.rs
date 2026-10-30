use dmd_source::{Error, Source};
use std::{
    fs,
    io::{Read, Seek, SeekFrom},
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .unwrap()
            .join("target/source-identity");
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join(format!(
            "{}-{}.wav",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        use std::io::Write;
        file.write_all(&document_bytes()).unwrap();
        Self(path)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        match fs::remove_file(&self.0) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => panic!("fixture cleanup: {error}"),
        }
    }
}
fn document_bytes() -> Vec<u8> {
    let document = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap()
        .join("docs/replay.md");
    fs::read_to_string(document)
        .unwrap()
        .replace("\r\n", "\n")
        .split("The signal `example.wav`")
        .nth(1)
        .unwrap()
        .split("```text\n")
        .nth(1)
        .unwrap()
        .split("```")
        .next()
        .unwrap()
        .split_whitespace()
        .map(|v| u8::from_str_radix(v, 16).unwrap())
        .collect()
}

#[test]
fn retained_cursors_are_independent_reusable_and_outlive_source() {
    let fixture = Fixture::new();
    let mut file = fs::File::open(&fixture.0).unwrap();
    file.seek(SeekFrom::Start(50)).unwrap();
    let source = Source::from_file(file).unwrap();
    assert_eq!(source.metadata().frames, 8);
    let mut a = source.cursor().unwrap();
    let mut b = source.clone().cursor().unwrap();
    drop(source);
    let mut buffer = [0; 4];
    a.read_exact(&mut buffer).unwrap();
    assert_eq!(&buffer, b"RIFF");
    b.seek(SeekFrom::End(-4)).unwrap();
    b.read_exact(&mut buffer).unwrap();
    assert_eq!(buffer, [0; 4]);
    a.seek(SeekFrom::Current(4)).unwrap();
    a.read_exact(&mut buffer).unwrap();
    assert_eq!(&buffer, b"WAVE");
    a.seek(SeekFrom::Start(0)).unwrap();
    a.read_exact(&mut buffer).unwrap();
    assert_eq!(&buffer, b"RIFF");
    assert!(a.seek(SeekFrom::Current(-5)).is_err());
    assert_eq!(a.stream_position().unwrap(), 4);
    a.seek(SeekFrom::Start(u64::MAX)).unwrap();
    assert!(a.seek(SeekFrom::Current(1)).is_err());
    assert_eq!(a.read(&mut []).unwrap(), 0);
}

#[cfg(unix)]
#[test]
fn pathname_replacement_keeps_original_open_identity() {
    let original = Fixture::new();
    let replacement = Fixture::new();
    let source = Source::open(&original.0).unwrap();
    // Exchange the original path for unsupported bytes without modifying its inode.
    fs::write(&replacement.0, b"different source").unwrap();
    fs::rename(&replacement.0, &original.0).unwrap();
    drop(replacement);
    assert!(Source::open(&original.0).is_err());
    let mut cursor = source.cursor().unwrap();
    assert_eq!(dmd_source::inspect(&mut cursor).unwrap(), source.metadata());
    cursor.rewind().unwrap();
    let mut bytes = Vec::new();
    cursor.read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, document_bytes());
}

#[test]
fn opened_handle_does_not_freeze_external_modification() {
    let fixture = Fixture::new();
    let source = Source::open(&fixture.0).unwrap();
    fs::OpenOptions::new()
        .write(true)
        .open(&fixture.0)
        .unwrap()
        .set_len(0)
        .unwrap();
    assert_eq!(source.metadata().byte_size, 60);
    let mut cursor = source.cursor().unwrap();
    assert_eq!(cursor.seek(SeekFrom::End(0)).unwrap(), 0);
    assert!(dmd_source::inspect(&mut cursor).is_err());
}

#[test]
fn open_failures_are_io_and_directories_are_unsupported() {
    let fixture = Fixture::new();
    assert!(matches!(
        Source::open(fixture.0.join("absent")),
        Err(Error::Io(_))
    ));
    #[cfg(unix)]
    assert!(matches!(
        Source::open(fixture.0.parent().unwrap()),
        Err(Error::Unsupported(dmd_source::Unsupported::FileKind))
    ));
}
