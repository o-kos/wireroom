use super::*;
use serde_json::{Value, json};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    mpsc::SyncSender,
};
mod zip {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/support/archive.rs"
    ));
}

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture(std::path::PathBuf);
impl Fixture {
    fn new(journal: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "dmd-unit-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        let spec = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .ancestors()
                .nth(2)
                .unwrap()
                .join("docs/replay.md"),
        )
        .unwrap()
        .replace("\r\n", "\n");
        let hex = spec
            .split("```text\n")
            .nth(1)
            .unwrap()
            .split("```")
            .next()
            .unwrap();
        let wav: Vec<_> = hex
            .split_whitespace()
            .map(|s| u8::from_str_radix(s, 16).unwrap())
            .collect();
        std::fs::write(root.join("example.wav"), wav).unwrap();
        let meta = spec
            .split("`example.wav.replay`")
            .nth(1)
            .unwrap()
            .split("```json\n")
            .nth(1)
            .unwrap()
            .split("\n```")
            .next()
            .unwrap();
        std::fs::write(
            root.join("example.wav.replay"),
            zip::archive(meta, journal.as_bytes(), Some(b"TEST")),
        )
        .unwrap();
        Self(root)
    }
    fn start(&self) -> Command {
        Command::Start(Start {
            recording: self.0.join("example.wav").to_str().unwrap().into(),
            module: "demo".into(),
            params: [("mode".into(), "fec".into())].into(),
            freq: 0.0,
            speed: dmd_protocol::Speed::Max,
            paused: false,
            output: self.0.join("out").to_str().unwrap().into(),
        })
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}
fn runtime_run(fixture: &Fixture) -> (Runtime<Vec<u8>>, Run, SyncSender<Input>) {
    let Command::Start(start) = fixture.start() else {
        panic!()
    };
    let (run, _) = Run::setup(start).unwrap();
    let (tx, input) = mpsc::sync_channel(16);
    (
        Runtime {
            writer: Writer::new(Vec::new()),
            input,
            frame: 0,
            context: Context { frames: Some(8) },
        },
        run,
        tx,
    )
}

#[path = "cancellation_tests.rs"]
mod cancellation_tests;
