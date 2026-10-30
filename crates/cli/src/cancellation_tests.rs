use super::*;
use std::{
    io::{Cursor, Read, Seek, SeekFrom},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize},
    },
};

struct Gate {
    cursor: Cursor<Vec<u8>>,
    armed: Arc<AtomicBool>,
    cancel: Arc<Mutex<Option<Arc<AtomicBool>>>>,
    reads: Arc<AtomicUsize>,
    began: mpsc::SyncSender<()>,
}
impl Read for Gate {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        if self.armed.load(Ordering::Relaxed) {
            let count = self.reads.fetch_add(1, Ordering::Relaxed) + 1;
            if count == 2 {
                // One compressed refill has already been inflated without producing bytes.
                self.began.send(()).unwrap();
                let flag = self.cancel.lock().unwrap().as_ref().unwrap().clone();
                let deadline = Instant::now() + Duration::from_secs(5);
                while !flag.load(Ordering::Relaxed) {
                    assert!(
                        Instant::now() < deadline,
                        "runtime did not cancel active decompression"
                    );
                    thread::yield_now();
                }
            }
        }
        self.cursor.read(bytes)
    }
}
impl Seek for Gate {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.cursor.seek(position)
    }
}

#[test]
fn stop_during_empty_deflate_prefix_bounds_reads_joins_and_finishes() {
    let journal = "{\"event\":\"progress\",\"frame\":8}\n{\"record\":\"finalization\"}\n";
    let fixture = Fixture::new(journal);
    let (mut runtime, mut run, tx) = runtime_run(&fixture);
    let (reads, started) = install_gate(&fixture, &mut run, runtime.context);
    let controller = thread::spawn(move || {
        started.recv_timeout(Duration::from_secs(5)).unwrap();
        tx.send(Ok(Some(Command::Stop))).unwrap();
    });
    runtime.play(&mut run).unwrap();
    runtime.complete(&mut run, Ok(())).unwrap();
    controller.join().unwrap();
    assert_eq!(
        reads.load(Ordering::Relaxed),
        2,
        "no further compressed reads after cancellation"
    );
    run.lookahead.finish().unwrap();
    let events: Vec<Value> = serde_json::Deserializer::from_slice(&runtime.writer.into_inner())
        .into_iter()
        .map(Result::unwrap)
        .collect();
    assert_eq!(
        events,
        [json!({"event":"finished","reason":"stopped","frame":0})]
    );
}

#[test]
fn joining_retains_detected_integrity_failure_despite_cancellation() {
    let fixture = Fixture::new("{\"event\":\"progress\",\"frame\":9}\n");
    let (mut runtime, mut run, _) = runtime_run(&fixture);
    run.lookahead
        .request(run.output.temporary().unwrap())
        .unwrap();
    // Returning the detected error to the queue keeps finish responsible for it.
    // Wait for the worker to finish rather than cancelling before it validates.
    run.lookahead.wait_finished();
    assert!(runtime.complete(&mut run, Ok(())).is_err());
    assert!(runtime.writer.into_inner().is_empty());
}

fn install_gate(
    fixture: &Fixture,
    run: &mut Run,
    context: Context,
) -> (Arc<AtomicUsize>, mpsc::Receiver<()>) {
    let bytes = deflated_recording(fixture);
    let armed = Arc::new(AtomicBool::new(false));
    let reads = Arc::new(AtomicUsize::new(0));
    let cancel = Arc::new(Mutex::new(None));
    let (began, started) = mpsc::sync_channel(1);
    run.recording = Recording::from_reader(
        Gate {
            cursor: Cursor::new(bytes),
            armed: armed.clone(),
            reads: reads.clone(),
            cancel: cancel.clone(),
            began,
        },
        8,
    )
    .unwrap();
    run.lookahead = Lookahead::new(run.recording.journal().unwrap(), context).unwrap();
    *cancel.lock().unwrap() = Some(run.lookahead.cancellation());
    armed.store(true, Ordering::Relaxed);
    (reads, started)
}

struct BrokenState(Vec<u8>);
impl Write for BrokenState {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let event: Value = serde_json::from_slice(bytes).unwrap();
        if event["event"] == "state" {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "injected active stdout failure",
            ));
        }
        self.0.write(bytes)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
#[test]
fn stdout_failure_cancels_and_joins_both_active_workers() {
    let fixture =
        Fixture::new("{\"event\":\"progress\",\"frame\":8}\n{\"record\":\"finalization\"}\n");
    let (runtime, mut run, tx) = runtime_run(&fixture);
    let mut runtime = Runtime {
        writer: Writer::new(BrokenState(Vec::new())),
        input: runtime.input,
        frame: runtime.frame,
        context: runtime.context,
    };
    let (reads, started) = install_gate(&fixture, &mut run, runtime.context);
    let (began, hashing) = mpsc::sync_channel(1);
    run.hashing = Some(Hashing::start_with(
        run.source.clone(),
        run.expected.clone(),
        move |_, flag| {
            began.send(()).unwrap();
            let deadline = Instant::now() + Duration::from_secs(5);
            while !flag.load(Ordering::Relaxed) {
                assert!(Instant::now() < deadline, "hash worker was not cancelled");
                thread::yield_now();
            }
            anyhow::bail!("verification cancelled")
        },
    ));
    hashing.recv_timeout(Duration::from_secs(5)).unwrap();
    let controller = thread::spawn(move || {
        started.recv_timeout(Duration::from_secs(5)).unwrap();
        tx.send(Ok(Some(Command::Pause))).unwrap();
    });
    let result = runtime.play(&mut run);
    assert!(matches!(
        result
            .as_ref()
            .unwrap_err()
            .downcast_ref::<dmd_protocol::EncodeError>(),
        Some(dmd_protocol::EncodeError::Io(_))
    ));
    assert!(runtime.complete(&mut run, result).is_err());
    controller.join().unwrap();
    assert_eq!(reads.load(Ordering::Relaxed), 2);
    run.lookahead.finish().unwrap();
    assert!(run.hashing.as_ref().unwrap().completed());
    assert!(run.hashing.as_mut().unwrap().finish().is_none());
    assert!(
        runtime.writer.into_inner().0.is_empty(),
        "no diagnostic or terminal is written to broken stdout"
    );
}

fn deflated_recording(fixture: &Fixture) -> Vec<u8> {
    let metadata = std::fs::read(fixture.0.join("example.wav.replay")).unwrap();
    let start = 30 + "metadata.json".len();
    let size = u32::from_le_bytes(metadata[22..26].try_into().unwrap()) as usize;
    let meta = std::str::from_utf8(&metadata[start..start + size]).unwrap();
    let journal = b"{\"event\":\"progress\",\"frame\":8}\n{\"record\":\"finalization\"}\n";
    zip::archive_with_prefix(meta, journal, None, 100_000)
}
#[test]
fn empty_deflate_prefix_is_valid_when_not_cancelled() {
    let fixture = Fixture::new("");
    let recording = Recording::from_reader(Cursor::new(deflated_recording(&fixture)), 8).unwrap();
    let mut journal = recording.journal().unwrap();
    assert_eq!(
        journal.next_record().unwrap(),
        Some(dmd_replay::Record::Boundary { from: 0, to: 8 })
    );
    assert_eq!(
        journal.next_record().unwrap(),
        Some(dmd_replay::Record::Finalization)
    );
    assert_eq!(journal.next_record().unwrap(), None);
}
