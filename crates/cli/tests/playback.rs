#![cfg(target_os = "linux")]
use serde_json::{Value, json};
use std::{
    fs,
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    process::{Child, ChildStdin, Command, Stdio},
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc::{self, Receiver},
    },
    time::Duration,
};

#[path = "support/archive.rs"]
mod zip;
use zip::archive;

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture {
    root: PathBuf,
}
impl Fixture {
    fn new(journal: &str, frames: u32) -> Self {
        use std::os::unix::fs::DirBuilderExt;
        let root = std::env::temp_dir().join(format!(
            "dmd-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::DirBuilder::new().mode(0o700).create(&root).unwrap();
        let mut wav = b"RIFF".to_vec();
        wav.extend((36 + frames * 2).to_le_bytes());
        wav.extend(b"WAVEfmt ");
        wav.extend(16u32.to_le_bytes());
        wav.extend(1u16.to_le_bytes());
        wav.extend(1u16.to_le_bytes());
        wav.extend(4u32.to_le_bytes());
        wav.extend(8u32.to_le_bytes());
        wav.extend(2u16.to_le_bytes());
        wav.extend(16u16.to_le_bytes());
        wav.extend(b"data");
        wav.extend((frames * 2).to_le_bytes());
        wav.resize(44 + frames as usize * 2, 0);
        fs::write(root.join("example.wav"), &wav).unwrap();
        let spec = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .ancestors()
                .nth(2)
                .unwrap()
                .join("docs/replay.md"),
        )
        .unwrap();
        let section = spec.split("`example.wav.replay`").nth(1).unwrap();
        let meta = section
            .split("```json\n")
            .nth(1)
            .unwrap()
            .split("\n```")
            .next()
            .unwrap();
        fs::write(
            root.join("example.wav.replay"),
            archive(meta, journal.as_bytes(), Some(b"TEST")),
        )
        .unwrap();
        Self { root }
    }
    fn example() -> Self {
        let spec = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .ancestors()
                .nth(2)
                .unwrap()
                .join("docs/replay.md"),
        )
        .unwrap();
        let section = spec.split("`example.wav.replay`").nth(1).unwrap();
        let journal = section
            .split("```json\n")
            .nth(2)
            .unwrap()
            .split("```")
            .next()
            .unwrap();
        Self::new(journal, 8)
    }
    fn start(&self, paused: bool, speed: Value) -> Value {
        json!({"command":"start","recording":"example.wav","module":"demo","params":{"mode":"fec"},"freq":0,"speed":speed,"paused":paused,"output":"out"})
    }
    fn process(&self) -> Process {
        Process::new(&self.root)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}
struct Process {
    child: Child,
    input: Option<ChildStdin>,
    events: Receiver<Value>,
}
impl Process {
    fn new(root: &PathBuf) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_dmd"))
            .args(["run", "--message-format", "json"])
            .current_dir(root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let input = child.stdin.take();
        let stdout = child.stdout.take().unwrap();
        let (tx, events) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                tx.send(serde_json::from_str(&line.unwrap()).unwrap())
                    .unwrap();
            }
        });
        let process = Self {
            child,
            input,
            events,
        };
        assert_eq!(
            process.next(),
            json!({"event":"hello","protocol":{"major":1,"minor":0}})
        );
        process
    }
    fn send(&mut self, value: Value) {
        self.raw(&format!("{value}\n"));
    }
    fn raw(&mut self, value: &str) {
        self.input
            .as_mut()
            .unwrap()
            .write_all(value.as_bytes())
            .unwrap();
        self.input.as_mut().unwrap().flush().unwrap();
    }
    fn next(&self) -> Value {
        self.events.recv_timeout(Duration::from_secs(15)).unwrap()
    }
    fn finish(&mut self) -> Vec<Value> {
        let mut events = Vec::new();
        loop {
            let event = self.next();
            let done = event["event"] == "finished";
            events.push(event);
            if done {
                break;
            }
        }
        events
    }
    fn success(&mut self) {
        assert!(self.child.wait().unwrap().success());
        assert!(
            self.events
                .recv_timeout(Duration::from_millis(100))
                .is_err()
        );
    }
}
impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
fn stored(events: &[Value]) -> Vec<Value> {
    events
        .iter()
        .filter(|e| !(e["event"] == "log" && e["level"] == "warning"))
        .cloned()
        .collect()
}

#[test]
fn specification_example_preserves_payload_and_producer_values() {
    let fixture = Fixture::example();
    let mut process = fixture.process();
    process.send(fixture.start(false, json!("max")));
    let events = process.finish();
    assert_eq!(events[0]["frames"], 8);
    assert_eq!(
        events[0]["stream"],
        json!({"format":"real","channels":1,"rate":[4,1]})
    );
    let events = stored(&events);
    assert_eq!(
        events
            .iter()
            .filter(|e| e["event"] == "progress")
            .map(|e| e["frame"].as_u64().unwrap())
            .collect::<Vec<_>>(),
        [4, 4, 8]
    );
    let results: Vec<_> = events.iter().filter(|e| e["event"] == "result").collect();
    assert_eq!(results[0]["record"], "finalization");
    assert_eq!(results[0]["vendorTag"], json!({"sequence":1}));
    assert_eq!(
        results[1]["binary"][0]["points"],
        json!([[1000, 0], [-500, 250]])
    );
    assert_eq!(results[2]["params"], json!({"confidence":0.9}));
    assert_eq!(
        fs::read(fixture.root.join("out/files/message.bin")).unwrap(),
        b"TEST"
    );
    assert_eq!(
        events.last().unwrap(),
        &json!({"event":"finished","reason":"end","frame":8})
    );
    process.success();
    assert_eq!(
        fs::read(fixture.root.join("out/files/message.bin")).unwrap(),
        b"TEST"
    );
}
#[test]
fn paused_fifo_controls_are_atomic_and_eof_stops() {
    let fixture = Fixture::example();
    let mut p = fixture.process();
    p.send(fixture.start(true, json!(1)));
    assert_eq!(p.next()["event"], "started");
    for (command, event) in [
        (json!({"command":"pause"}), "state"),
        (json!({"command":"set_speed","speed":2}), "state"),
        (json!({"command":"tune","freq":1}), "error"),
        (
            json!({"command":"set_params","params":{"mode":"fec","unknown":"x"}}),
            "error",
        ),
        (json!({"command":"set_params","params":{}}), "state"),
    ] {
        p.send(command);
        let answer = p.next();
        assert_eq!(answer["event"], event);
        if event == "state" {
            assert_eq!(answer["state"], "paused");
            assert_eq!(answer["params"], json!({"mode":"fec"}));
            assert_eq!(answer["frame"], 0);
        }
    }
    p.raw("{invalid}\n");
    assert_eq!(p.next()["fatal"], false);
    p.input.take();
    let events = p.finish();
    assert_eq!(events.last().unwrap()["reason"], "stopped");
    assert_eq!(events.last().unwrap()["frame"], 0);
    assert!(!fixture.root.join("out/files/message.bin").exists());
    p.success();
}
#[test]
fn stop_cancels_pending_pacing_without_submitting() {
    let fixture = Fixture::example();
    let mut p = fixture.process();
    p.send(fixture.start(false, json!(0.000001)));
    assert_eq!(p.next()["event"], "started");
    p.send(json!({"command":"stop"}));
    let events = stored(&p.finish());
    assert_eq!(
        events,
        [json!({"event":"finished","reason":"stopped","frame":0})]
    );
    p.success();
}
#[test]
fn empty_paused_stop_drains_zero_frontier_suffix() {
    let fixture = Fixture::new(
        "{\"event\":\"progress\",\"frame\":0}\n{\"record\":\"finalization\"}\n{\"event\":\"result\",\"samples\":[0,0],\"text\":\"drain\"}\n",
        0,
    );
    let mut p = fixture.process();
    p.send(fixture.start(true, json!(1)));
    assert_eq!(p.next()["event"], "started");
    p.send(json!({"command":"stop"}));
    let events = stored(&p.finish());
    assert_eq!(events[0], json!({"event":"progress","frame":0}));
    assert_eq!(events[1]["text"], "drain");
    assert_eq!(events[2]["reason"], "stopped");
    p.success();
}
#[test]
fn invalid_closing_boundary_does_not_emit_provisional_result() {
    let fixture = Fixture::new(
        "{\"event\":\"result\",\"samples\":[0,8],\"text\":\"provisional\"}\n{\"event\":\"progress\",\"frame\":9}\n",
        8,
    );
    let mut p = fixture.process();
    p.send(fixture.start(false, json!("max")));
    let events = p.finish();
    assert!(events.iter().all(|e| e["event"] != "result"));
    assert_eq!(events[events.len() - 2]["fatal"], true);
    assert_eq!(events.last().unwrap()["frame"], 0);
    assert!(!p.child.wait().unwrap().success());
}
#[test]
fn collision_and_symlink_ancestors_never_overwrite() {
    use std::os::unix::fs::symlink;
    for link in [false, true] {
        let fixture = Fixture::example();
        fs::create_dir(fixture.root.join("out")).unwrap();
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(fixture.root.join("out"), fs::Permissions::from_mode(0o700))
                .unwrap();
        }
        fs::create_dir(fixture.root.join("elsewhere")).unwrap();
        if link {
            symlink(
                fixture.root.join("elsewhere"),
                fixture.root.join("out/files"),
            )
            .unwrap();
        } else {
            fs::create_dir(fixture.root.join("out/files")).unwrap();
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(
                    fixture.root.join("out/files"),
                    fs::Permissions::from_mode(0o700),
                )
                .unwrap();
            }
            fs::write(fixture.root.join("out/files/message.bin"), b"KEEP").unwrap();
        }
        let mut p = fixture.process();
        p.send(fixture.start(false, json!("max")));
        let events = p.finish();
        assert_eq!(events.last().unwrap()["reason"], "error");
        assert!(!p.child.wait().unwrap().success());
        assert!(!fixture.root.join("elsewhere/message.bin").exists());
        if !link {
            assert_eq!(
                fs::read(fixture.root.join("out/files/message.bin")).unwrap(),
                b"KEEP"
            );
        }
        assert_eq!(events.iter().filter(|e| e["event"] == "result").count(), 2);
    }
}
#[test]
fn writable_ancestry_is_rejected_at_setup() {
    use std::os::unix::fs::PermissionsExt;
    let fixture = Fixture::example();
    fs::set_permissions(&fixture.root, fs::Permissions::from_mode(0o777)).unwrap();
    let mut p = fixture.process();
    p.send(fixture.start(false, json!("max")));
    let events = p.finish();
    assert_eq!(events[0]["fatal"], true);
    assert_eq!(events[1]["reason"], "error");
    assert!(!p.child.wait().unwrap().success());
}

fn result(path: &str, size: u64) -> Value {
    json!({"event":"result","samples":[0,8],"binary":[{"id":"opaque/id","type":"custom","body":{"n":18446744073709551615u64},"path":path,"size":size}]})
}
fn journal(lines: &[Value]) -> String {
    lines.iter().map(|v| format!("{v}\n")).collect()
}
#[test]
fn repeated_asset_reuses_retained_file_without_rewriting() {
    let data = journal(&[
        json!({"event":"progress","frame":8}),
        json!({"record":"finalization"}),
        result("files/message.bin", 4),
        result("files/message.bin", 4),
    ]);
    let f = Fixture::new(&data, 8);
    let mut p = f.process();
    p.send(f.start(false, json!("max")));
    let events = p.finish();
    assert_eq!(events.iter().filter(|e| e["event"] == "result").count(), 2);
    assert_eq!(events.last().unwrap()["reason"], "end");
    p.success();
}
#[test]
fn missing_or_wrong_size_asset_rejects_entire_result() {
    for bad in [
        result("files/missing.bin", 4),
        result("files/message.bin", 5),
    ] {
        let mut whole = result("files/message.bin", 4);
        whole["binary"]
            .as_array_mut()
            .unwrap()
            .push(bad["binary"][0].clone());
        let f = Fixture::new(
            &journal(&[
                json!({"event":"progress","frame":8}),
                json!({"record":"finalization"}),
                whole,
            ]),
            8,
        );
        let mut p = f.process();
        p.send(f.start(false, json!("max")));
        let events = p.finish();
        assert!(events.iter().all(|e| e["event"] != "result"));
        assert_eq!(events.last().unwrap()["reason"], "error");
        assert!(!p.child.wait().unwrap().success());
    }
}
#[test]
fn missing_marker_and_bad_suffix_are_fatal_at_exhaustion() {
    for data in [
        "{\"event\":\"progress\",\"frame\":8}\n",
        "{\"event\":\"progress\",\"frame\":8}\n{\"record\":\"finalization\"}\n{\"event\":\"progress\",\"frame\":7}\n",
        "{\"event\":\"progress\",\"frame\":8}\n{\"record\":\"finalization\"}",
    ] {
        let f = Fixture::new(data, 8);
        let mut p = f.process();
        p.send(f.start(false, json!("max")));
        let events = p.finish();
        assert_eq!(
            events.last().unwrap(),
            &json!({"event":"finished","reason":"error","frame":8})
        );
        assert!(!p.child.wait().unwrap().success());
    }
}
#[test]
fn paused_early_stop_leaves_bad_suffix_unread() {
    let f = Fixture::new("invalid unread journal", 8);
    let mut p = f.process();
    p.send(f.start(true, json!("max")));
    assert_eq!(p.next()["event"], "started");
    p.send(json!({"command":"stop"}));
    let events = stored(&p.finish());
    assert_eq!(
        events,
        [json!({"event":"finished","reason":"stopped","frame":0})]
    );
    p.success();
}
#[test]
fn invalid_initial_settings_fail_before_started() {
    for patch in [
        json!({"params":{}}),
        json!({"module":"other"}),
        json!({"freq":1}),
    ] {
        let f = Fixture::example();
        let mut start = f.start(false, json!("max"));
        for (k, v) in patch.as_object().unwrap() {
            start[k] = v.clone();
        }
        let mut p = f.process();
        p.send(start);
        let events = p.finish();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0]["fatal"], true);
        assert_eq!(events[1]["frame"], 0);
        assert!(!p.child.wait().unwrap().success());
    }
}
#[test]
fn virtual_frontier_comes_from_logs_before_asset_failure() {
    let f = Fixture::new(
        &journal(&[
            json!({"event":"log","frame":2,"level":"info","text":"advance"}),
            result("missing.bin", 4),
            json!({"event":"progress","frame":4}),
        ]),
        8,
    );
    let mut p = f.process();
    p.send(f.start(false, json!("max")));
    let events = p.finish();
    assert_eq!(
        events.last().unwrap(),
        &json!({"event":"finished","reason":"error","frame":2})
    );
    assert!(!p.child.wait().unwrap().success());
}
#[test]
fn corrupt_required_asset_and_journal_crc_fail() {
    for corrupt_asset in [true, false] {
        let f = Fixture::example();
        let path = f.root.join("example.wav.replay");
        let mut zip = fs::read(&path).unwrap();
        let needle: &[u8] = if corrupt_asset {
            b"TEST"
        } else {
            b"Recorded run"
        };
        let index = zip.windows(needle.len()).position(|w| w == needle).unwrap();
        zip[index] ^= 1;
        fs::write(path, zip).unwrap();
        let mut p = f.process();
        p.send(f.start(false, json!("max")));
        let events = p.finish();
        assert_eq!(events.last().unwrap()["reason"], "error");
        assert!(!p.child.wait().unwrap().success());
    }
}
#[test]
fn incomplete_input_is_rejected_before_eof_stop() {
    let f = Fixture::example();
    let mut p = f.process();
    p.send(f.start(true, json!(1)));
    assert_eq!(p.next()["event"], "started");
    p.raw("{\"command\":\"resume\"}");
    p.input.take();
    let events = stored(&p.finish());
    assert_eq!(events[0]["fatal"], false);
    assert_eq!(events[1]["reason"], "stopped");
    p.success();
}
#[test]
fn published_file_is_available_when_descriptor_arrives() {
    let f = Fixture::example();
    let mut p = f.process();
    p.send(f.start(false, json!("max")));
    loop {
        let event = p.next();
        if event["binary"][0]["type"] == "file" {
            assert_eq!(
                fs::read(f.root.join("out/files/message.bin")).unwrap(),
                b"TEST"
            );
            break;
        }
    }
    assert_eq!(p.finish().last().unwrap()["reason"], "end");
    p.success();
}

#[test]
fn signal_and_archive_path_replacement_keeps_opened_identities() {
    let f = Fixture::example();
    let mut p = f.process();
    p.send(f.start(true, json!("max")));
    assert_eq!(p.next()["event"], "started");
    for name in ["example.wav", "example.wav.replay"] {
        fs::rename(f.root.join(name), f.root.join(format!("{name}.retained"))).unwrap();
        fs::write(f.root.join(name), b"not the opened identity").unwrap();
    }
    p.send(json!({"command":"resume"}));
    let events = p.finish();
    assert_eq!(events.last().unwrap()["reason"], "end");
    assert_eq!(
        fs::read(f.root.join("out/files/message.bin")).unwrap(),
        b"TEST"
    );
    p.success();
}

#[test]
fn symlink_output_root_and_nonregular_source_fail_before_started() {
    use std::os::unix::fs::symlink;
    let f = Fixture::example();
    fs::create_dir(f.root.join("elsewhere")).unwrap();
    symlink("elsewhere", f.root.join("out")).unwrap();
    let mut p = f.process();
    p.send(f.start(false, json!("max")));
    let events = p.finish();
    assert_eq!(events[0]["fatal"], true);
    assert!(!p.child.wait().unwrap().success());
    let mut p = f.process();
    let mut start = f.start(false, json!("max"));
    start["recording"] = json!(".");
    p.send(start);
    let events = p.finish();
    assert_eq!(events[0]["fatal"], true);
    assert!(!p.child.wait().unwrap().success());
}

#[test]
fn preexisting_public_output_is_rejected_instead_of_changing_permissions() {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let f = Fixture::example();
    fs::create_dir(f.root.join("out")).unwrap();
    fs::set_permissions(f.root.join("out"), fs::Permissions::from_mode(0o755)).unwrap();
    let mut p = f.process();
    p.send(f.start(false, json!("max")));
    let events = p.finish();
    assert_eq!(events[0]["fatal"], true);
    assert!(!p.child.wait().unwrap().success());
    assert_eq!(
        fs::metadata(f.root.join("out")).unwrap().mode() & 0o777,
        0o755
    );
}

#[test]
fn stdout_failure_exits_nonzero_without_waiting_for_stdin_eof() {
    let f = Fixture::example();
    let mut child = Command::new(env!("CARGO_BIN_EXE_dmd"))
        .args(["run", "--message-format", "json"])
        .current_dir(&f.root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut reader = BufReader::new(child.stdout.take().unwrap());
    let mut hello = String::new();
    reader.read_line(&mut hello).unwrap();
    assert!(hello.contains("hello"));
    drop(reader);
    let mut input = child.stdin.take().unwrap();
    writeln!(input, "{}", f.start(false, json!("max"))).unwrap();
    input.flush().unwrap();
    assert!(!child.wait().unwrap().success());
}
