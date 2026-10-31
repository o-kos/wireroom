use super::*;
use std::{
    io::{Cursor, Read, Seek, SeekFrom},
    sync::{Arc, Mutex, atomic::AtomicBool},
};

struct Gate {
    cursor: Cursor<Vec<u8>>,
    armed: Arc<AtomicBool>,
    cancel: Arc<Mutex<Option<Arc<AtomicBool>>>>,
    refill: bool,
    began: mpsc::SyncSender<()>,
}
impl Read for Gate {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        if self.armed.load(Ordering::Relaxed) {
            if self.refill {
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
            self.refill = true;
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
fn stop_during_decompression_joins_and_finishes() {
    let journal = "{\"event\":\"progress\",\"frame\":8}\n{\"record\":\"finalization\"}\n";
    let fixture = Fixture::new(journal);
    let (mut runtime, mut run, tx) = runtime_run(&fixture);
    let started = install_gate(&fixture, &mut run, runtime.context);
    let controller = thread::spawn(move || {
        started.recv_timeout(Duration::from_secs(5)).unwrap();
        tx.send(Ok(Some(Command::Stop))).unwrap();
    });
    runtime.play(&mut run).unwrap();
    runtime.complete(&mut run, Ok(())).unwrap();
    controller.join().unwrap();
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

fn install_gate(fixture: &Fixture, run: &mut Run, context: Context) -> mpsc::Receiver<()> {
    let bytes = deflated_recording(fixture);
    let armed = Arc::new(AtomicBool::new(false));
    let cancel = Arc::new(Mutex::new(None));
    let (began, started) = mpsc::sync_channel(1);
    run.recording = Recording::from_reader(
        Gate {
            cursor: Cursor::new(bytes),
            armed: armed.clone(),
            refill: false,
            cancel: cancel.clone(),
            began,
        },
        8,
    )
    .unwrap();
    run.lookahead = Lookahead::new(run.recording.journal().unwrap(), context).unwrap();
    *cancel.lock().unwrap() = Some(run.lookahead.cancellation());
    armed.store(true, Ordering::Relaxed);
    started
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
fn stdout_failure_cancels_and_joins_lookahead() {
    let fixture =
        Fixture::new("{\"event\":\"progress\",\"frame\":8}\n{\"record\":\"finalization\"}\n");
    let (runtime, mut run, tx) = runtime_run(&fixture);
    let mut runtime = Runtime {
        writer: Writer::new(BrokenState(Vec::new())),
        input: runtime.input,
        frame: runtime.frame,
        context: runtime.context,
    };
    let started = install_gate(&fixture, &mut run, runtime.context);
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
    run.lookahead.finish().unwrap();
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
    zip::archive_with_prefix(meta, journal, None, 4000)
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
