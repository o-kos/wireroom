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
        use std::os::unix::fs::DirBuilderExt;
        let root = std::env::temp_dir().join(format!(
            "dmd-unit-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&root)
            .unwrap();
        let spec = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .ancestors()
                .nth(2)
                .unwrap()
                .join("docs/replay.md"),
        )
        .unwrap();
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
struct Reactive {
    events: Vec<Value>,
    tx: SyncSender<Input>,
    trigger: &'static str,
    commands: Vec<Command>,
}
impl Write for Reactive {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let value: Value = serde_json::from_slice(bytes).unwrap();
        if value["event"] == self.trigger {
            for command in std::mem::take(&mut self.commands) {
                self.tx.send(Ok(Some(command))).unwrap();
            }
        }
        self.events.push(value);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
fn execute(journal: &str, trigger: &'static str, commands: Vec<Command>) -> Vec<Value> {
    let fixture = Fixture::new(journal);
    let (tx, input) = mpsc::sync_channel(16);
    tx.send(Ok(Some(fixture.start()))).unwrap();
    let writer = Writer::new(Reactive {
        events: Vec::new(),
        tx,
        trigger,
        commands,
    });
    let mut runtime = Runtime {
        writer,
        input,
        frame: 0,
        context: Context::default(),
    };
    runtime.lifecycle().unwrap();
    runtime.writer.into_inner().events
}
#[test]
fn stop_in_flight_finishes_block_and_discards_unread_suffix() {
    let events = execute(
        "{\"event\":\"log\",\"frame\":0,\"level\":\"info\",\"text\":\"trigger\"}\n{\"event\":\"result\",\"samples\":[0,4],\"text\":\"retained\"}\n{\"event\":\"progress\",\"frame\":4}\ninvalid unread suffix",
        "log",
        vec![Command::Stop],
    );
    assert_eq!(
        events.iter().find(|e| e["event"] == "result").unwrap()["text"],
        "retained"
    );
    assert_eq!(
        events.last().unwrap(),
        &json!({"event":"finished","reason":"stopped","frame":4})
    );
}
#[test]
fn stop_in_flight_at_n_drains_repeated_boundaries_and_suffix() {
    let events = execute(
        "{\"event\":\"log\",\"frame\":8,\"level\":\"info\",\"text\":\"trigger\"}\n{\"event\":\"progress\",\"frame\":8}\n{\"event\":\"progress\",\"frame\":8}\n{\"record\":\"finalization\"}\n{\"event\":\"result\",\"samples\":[0,8],\"text\":\"drained\"}\n",
        "log",
        vec![Command::Stop],
    );
    assert_eq!(
        events.iter().filter(|e| e["event"] == "progress").count(),
        2
    );
    assert_eq!(
        events.iter().find(|e| e["event"] == "result").unwrap()["text"],
        "drained"
    );
    assert_eq!(events.last().unwrap()["reason"], "stopped");
}
#[test]
fn in_flight_controls_acknowledge_fifo_after_progress() {
    let events = execute(
        "{\"event\":\"log\",\"frame\":0,\"level\":\"info\",\"text\":\"trigger\"}\n{\"event\":\"progress\",\"frame\":4}\n{\"event\":\"progress\",\"frame\":8}\n{\"record\":\"finalization\"}\n",
        "log",
        vec![
            Command::Pause,
            Command::SetSpeed(dmd_protocol::Speed::Max),
            Command::Resume,
        ],
    );
    let states: Vec<_> = events.iter().filter(|e| e["event"] == "state").collect();
    assert_eq!(states.len(), 3);
    assert_eq!(states[0]["state"], "paused");
    assert_eq!(states[1]["state"], "paused");
    assert_eq!(states[2]["state"], "playing");
    assert!(states.iter().all(|e| e["frame"] == 4));
    assert_eq!(events[2], json!({"event":"progress","frame":4}));
}
#[test]
fn finalization_preserves_end_and_rejects_nonstop_in_order() {
    let events = execute(
        "{\"event\":\"progress\",\"frame\":8}\n{\"record\":\"finalization\"}\n{\"event\":\"result\",\"samples\":[0,8],\"text\":\"trigger\"}\n",
        "result",
        vec![Command::Stop, Command::Pause, Command::Stop],
    );
    let errors: Vec<_> = events.iter().filter(|e| e["event"] == "error").collect();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0]["fatal"], false);
    assert_eq!(
        events.last().unwrap(),
        &json!({"event":"finished","reason":"end","frame":8})
    );
}

#[test]
fn retained_publication_rejects_replacement_and_hardlink_alias() {
    let fixture =
        Fixture::new("{\"event\":\"progress\",\"frame\":8}\n{\"record\":\"finalization\"}\n");
    let Command::Start(start) = fixture.start() else {
        panic!()
    };
    let (mut run, _) = Run::setup(start).unwrap();
    let DecodedEvent::Known(Event::Result(result))=Event::decode(r#"{"event":"result","samples":[0,8],"binary":[{"id":"id","type":"file","body":{},"path":"files/message.bin","size":4}]}"#,Context {frames:Some(8)}).unwrap() else { panic!() };
    run.output.materialize(&run.recording, &result).unwrap();
    let path = fixture.0.join("out/files/message.bin");
    std::fs::hard_link(&path, fixture.0.join("alias")).unwrap();
    assert!(run.output.materialize(&run.recording, &result).is_err());
    std::fs::remove_file(fixture.0.join("alias")).unwrap();
    std::fs::rename(&path, fixture.0.join("retained")).unwrap();
    std::fs::write(&path, b"FAKE").unwrap();
    assert!(run.output.materialize(&run.recording, &result).is_err());
    assert_eq!(std::fs::read(fixture.0.join("retained")).unwrap(), b"TEST");
}
#[test]
fn journal_cancellation_is_checked_before_scanning_extensions() {
    let fixture = Fixture::new(
        "{\"record\":\"extension\"}\n{\"event\":\"progress\",\"frame\":8}\n{\"record\":\"finalization\"}\n",
    );
    let recording = Recording::open(fixture.0.join("example.wav.replay"), 8).unwrap();
    let mut journal = recording.journal().unwrap();
    journal.set_cancellation(std::sync::Arc::new(std::sync::atomic::AtomicBool::new(
        true,
    )));
    assert!(matches!(
        journal.next_record(),
        Err(dmd_replay::Error::Cancelled)
    ));
}
#[test]
fn max_then_numeric_in_one_fifo_batch_removes_pending_wait() {
    let fixture =
        Fixture::new("{\"event\":\"progress\",\"frame\":8}\n{\"record\":\"finalization\"}\n");
    let Command::Start(start) = fixture.start() else {
        panic!()
    };
    let (mut run, _) = Run::setup(start).unwrap();
    let (tx, input) = mpsc::sync_channel(4);
    let mut runtime = Runtime {
        writer: Writer::new(Vec::new()),
        input,
        frame: 0,
        context: Context { frames: Some(8) },
    };
    run.clock.begin();
    run.snapshot.state = State::Paused;
    tx.send(Ok(Some(Command::SetSpeed(dmd_protocol::Speed::Max))))
        .unwrap();
    tx.send(Ok(Some(Command::SetSpeed(dmd_protocol::Speed::Numeric(
        1.0,
    )))))
    .unwrap();
    runtime.commands(&mut run).unwrap();
    let mut budget = Budget::new(0, 8, run.rate);
    budget.credit(run.clock.take());
    assert!(budget.ready());
    assert_eq!(run.snapshot.state, State::Paused);
}

#[test]
fn unsupported_filesystem_is_rejected_without_fallback() {
    assert!(Output::open("/proc").is_err());
}

#[test]
fn spool_budget_accepts_exact_boundary_and_rejects_excess() {
    let fixture =
        Fixture::new("{\"event\":\"progress\",\"frame\":8}\n{\"record\":\"finalization\"}\n");
    let Command::Start(start) = fixture.start() else {
        panic!()
    };
    let (run, _) = Run::setup(start).unwrap();
    let context = Context { frames: Some(8) };
    let length = Event::Progress { frame: 8 }.encode(context).unwrap().len() as u64 + 1;
    for (limit, valid) in [(length, true), (length - 1, false)] {
        let mut worker =
            Lookahead::with_limit(run.recording.journal().unwrap(), context, limit).unwrap();
        worker.request(run.output.temporary().unwrap()).unwrap();
        let result = worker.results.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(result.is_ok(), valid);
        worker.finish().unwrap();
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
#[test]
fn pending_clock_accounts_lookahead_controls_and_submission_deterministically() {
    use std::sync::{Arc, Mutex};
    let fixture = Fixture::new(
        "{\"event\":\"progress\",\"frame\":4}\n{\"event\":\"progress\",\"frame\":8}\n{\"record\":\"finalization\"}\n",
    );
    let (mut runtime, mut run, tx) = runtime_run(&fixture);
    let now = Arc::new(Mutex::new(Instant::now()));
    let clock = now.clone();
    run.clock.now = Box::new(move || *clock.lock().unwrap());
    let advance = |millis| {
        *now.lock().unwrap() += Duration::from_millis(millis);
    };
    run.snapshot.speed = dmd_protocol::Speed::Numeric(1.0);
    run.clock.begin();
    // Production lookahead completion accounts time before constructing its budget.
    run.lookahead
        .request(run.output.temporary().unwrap())
        .unwrap();
    advance(250);
    let unit = runtime.lookahead(&mut run).unwrap().unwrap();
    let Ending::Boundary { from, to } = unit.ending else {
        panic!()
    };
    let mut budget = Budget::new(from, to, run.rate);
    budget.credit(run.clock.take());
    assert_eq!(runtime.pending(&mut run, &mut budget).unwrap(), None);
    advance(250);
    tx.send(Ok(Some(Command::Pause))).unwrap();
    assert_eq!(runtime.pending(&mut run, &mut budget).unwrap(), None);
    advance(100_000);
    tx.send(Ok(Some(Command::SetSpeed(dmd_protocol::Speed::Numeric(
        2.0,
    )))))
    .unwrap();
    tx.send(Ok(Some(Command::Resume))).unwrap();
    assert_eq!(runtime.pending(&mut run, &mut budget).unwrap(), None);
    advance(249);
    assert_eq!(runtime.pending(&mut run, &mut budget).unwrap(), None);
    advance(2);
    assert!(runtime.wait(&mut run, budget).unwrap());
    run.clock.active = false;
    runtime.submit(&mut run, &unit).unwrap();
    assert_eq!(runtime.frame, 4);
    // Changing speed halfway accounts the elapsed interval at the old speed.
    run.snapshot.speed = dmd_protocol::Speed::Numeric(1.0);
    run.clock.begin();
    let mut budget = Budget::new(4, 8, run.rate);
    advance(500);
    tx.send(Ok(Some(Command::SetSpeed(dmd_protocol::Speed::Numeric(
        2.0,
    )))))
    .unwrap();
    assert_eq!(runtime.pending(&mut run, &mut budget).unwrap(), None);
    advance(249);
    assert_eq!(runtime.pending(&mut run, &mut budget).unwrap(), None);
    advance(2);
    assert!(runtime.wait(&mut run, budget).unwrap());
    check_repeated_max_and_stop(&mut runtime, &mut run, &tx);
    runtime.complete(&mut run, Ok(())).unwrap();
    let events: Vec<Value> = runtime
        .writer
        .into_inner()
        .split(|b| *b == b'\n')
        .filter(|line| !line.is_empty())
        .map(|line| serde_json::from_slice(line).unwrap())
        .collect();
    assert_eq!(
        events.iter().filter(|e| e["event"] == "progress").count(),
        1
    );
    assert_eq!(
        events.last().unwrap(),
        &json!({"event":"finished","reason":"stopped","frame":4})
    );
}

fn check_repeated_max_and_stop(
    runtime: &mut Runtime<Vec<u8>>,
    run: &mut Run,
    tx: &SyncSender<Input>,
) {
    // A repeated frontier costs no source time, but pause still prevents submission.
    run.clock.begin();
    tx.send(Ok(Some(Command::Pause))).unwrap();
    let mut repeated = Budget::new(4, 4, run.rate);
    assert_eq!(runtime.pending(run, &mut repeated).unwrap(), None);
    tx.send(Ok(Some(Command::Resume))).unwrap();
    assert!(runtime.wait(run, repeated).unwrap());
    // A transient max clears a pending budget even within one FIFO batch.
    run.clock.begin();
    tx.send(Ok(Some(Command::SetSpeed(dmd_protocol::Speed::Max))))
        .unwrap();
    tx.send(Ok(Some(Command::SetSpeed(dmd_protocol::Speed::Numeric(
        1.0,
    )))))
    .unwrap();
    let budget = Budget::new(4, 8, run.rate);
    assert!(runtime.wait(run, budget).unwrap());
    // Stop wins over a ready budget; no second progress is submitted.
    tx.send(Ok(Some(Command::Stop))).unwrap();
    let budget = Budget::new(4, 4, run.rate);
    assert!(!runtime.wait(run, budget).unwrap());
}

#[test]
fn retained_hash_outcomes_are_ordered_once_before_unchanged_terminal() {
    for mode in ["match", "mismatch", "read", "modified", "cancel"] {
        for stop in [false, true] {
            hash_case(mode, stop);
        }
    }
}
fn hash_case(mode: &'static str, stop: bool) {
    let fixture = Fixture::new(
        "{\"event\":\"progress\",\"frame\":4}\n{\"event\":\"progress\",\"frame\":8}\n{\"record\":\"finalization\"}\n",
    );
    let (mut runtime, mut run, tx) = runtime_run(&fixture);
    run.hashing = Some(start_hash_case(&fixture, &run, mode));
    if mode != "cancel" {
        let deadline = Instant::now() + Duration::from_secs(5);
        while !run.hashing.as_ref().unwrap().completed() {
            assert!(Instant::now() < deadline);
            thread::yield_now();
        }
    }
    if stop {
        // Submit one actual unit before stopping at its closing frontier.
        run.lookahead
            .request(run.output.temporary().unwrap())
            .unwrap();
        let unit = runtime.lookahead(&mut run).unwrap().unwrap();
        runtime.submit(&mut run, &unit).unwrap();
        tx.send(Ok(Some(Command::Stop))).unwrap();
    }
    runtime.play(&mut run).unwrap();
    runtime.complete(&mut run, Ok(())).unwrap();
    assert!(run.hashing.as_mut().unwrap().finish().is_none());
    assert!(run.hashing.as_ref().unwrap().completed());
    let events: Vec<Value> = runtime
        .writer
        .into_inner()
        .split(|b| *b == b'\n')
        .filter(|line| !line.is_empty())
        .map(|line| serde_json::from_slice(line).unwrap())
        .collect();
    let frame = if stop { 4 } else { 8 };
    assert_eq!(
        events.last().unwrap(),
        &json!({"event":"finished","reason":if stop { "stopped" } else { "end" },"frame":frame})
    );
    let warnings: Vec<_> = events
        .iter()
        .enumerate()
        .filter(|(_, e)| e["level"] == "warning")
        .collect();
    assert_eq!(warnings.len(), usize::from(mode != "match"), "{mode}");
    if let Some((index, warning)) = warnings.first() {
        assert_eq!(*index, events.len() - 2);
        assert_eq!(warning["frame"], frame);
        let text = warning["text"].as_str().unwrap();
        assert!(
            text.contains(match mode {
                "mismatch" => "differs",
                "read" => "injected retained read failure",
                "modified" => "changed",
                "cancel" => "cancelled",
                _ => unreachable!(),
            }),
            "{text}"
        );
    }
}

fn start_hash_case(fixture: &Fixture, run: &Run, mode: &'static str) -> Hashing {
    use crate::hashing::VerificationPoint;
    use std::sync::atomic::AtomicBool;
    let retained = fixture.0.join("retained.wav");
    std::fs::rename(fixture.0.join("example.wav"), &retained).unwrap();
    std::fs::write(fixture.0.join("example.wav"), b"replacement").unwrap();
    let expected = if mode == "mismatch" {
        "0".repeat(64)
    } else {
        run.expected.clone()
    };
    let (began, started) = mpsc::sync_channel(1);
    let mut reads = 0;
    let hashing = Hashing::start_with(
        run.source.clone(),
        expected,
        move |point, flag: &AtomicBool| {
            match point {
                VerificationPoint::Read => {
                    reads += 1;
                    if reads == 1 {
                        began.send(()).unwrap();
                    }
                    if mode == "read" && reads == 2 {
                        anyhow::bail!("injected retained read failure");
                    }
                    if mode == "cancel" {
                        while !flag.load(Ordering::Relaxed) {
                            thread::yield_now();
                        }
                    }
                }
                VerificationPoint::Digested if mode == "modified" => {
                    std::fs::OpenOptions::new()
                        .write(true)
                        .open(&retained)?
                        .set_len(61)?;
                }
                VerificationPoint::Digested => {}
            }
            Ok(())
        },
    );
    started.recv_timeout(Duration::from_secs(5)).unwrap();
    hashing
}

#[path = "cancellation_tests.rs"]
mod cancellation;
