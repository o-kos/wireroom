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
        let root = std::env::temp_dir().join(format!(
            "dmd-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
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
        .unwrap()
        .replace("\r\n", "\n");
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
        .unwrap()
        .replace("\r\n", "\n");
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
#[test]
fn specification_example_preserves_payload_and_producer_values() {
    let fixture = Fixture::example();
    let mut p = fixture.process();
    p.send(fixture.start(false, json!("max")));
    assert_eq!(p.next()["event"], "started");
    let mut events = Vec::new();
    loop {
        let event = p.next();
        if event["binary"][0]["type"] == "file" {
            assert_eq!(
                fs::read(fixture.root.join("out/files/message.bin")).unwrap(),
                b"TEST"
            );
        }
        let done = event["event"] == "finished";
        events.push(event);
        if done {
            break;
        }
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap();
    let spec = fs::read_to_string(root.join("docs/replay.md"))
        .unwrap()
        .replace("\r\n", "\n");
    let journal = spec
        .split("`journal.ndjson` contains")
        .nth(1)
        .unwrap()
        .split("```json\n")
        .nth(1)
        .unwrap()
        .split("```")
        .next()
        .unwrap();
    let expected: Vec<Value> = journal
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .filter(|value| value.get("event").is_some())
        .collect();
    assert_eq!(&events[..events.len() - 1], expected);
    assert_eq!(
        events.last().unwrap(),
        &json!({"event":"finished","reason":"end","frame":8})
    );
    p.success();
    assert_eq!(
        fs::read(fixture.root.join("out/files/message.bin")).unwrap(),
        b"TEST"
    );
}
#[test]
fn paused_resume_completes_at_max_speed() {
    let fixture = Fixture::example();
    let mut p = fixture.process();
    p.send(fixture.start(true, json!("max")));
    assert_eq!(p.next()["event"], "started");
    p.send(json!({"command":"resume"}));
    assert_eq!(p.next()["state"], "playing");
    assert_eq!(p.finish().last().unwrap()["reason"], "end");
    p.success();
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
    let events = p.finish();
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
    let events = p.finish();
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

fn result(path: &str, size: u64) -> Value {
    json!({"event":"result","samples":[0,8],"binary":[{"id":"opaque/id","type":"custom","body":{"n":18446744073709551615u64},"path":path,"size":size}]})
}
fn journal(lines: &[Value]) -> String {
    lines.iter().map(|v| format!("{v}\n")).collect()
}
#[test]
fn repeated_asset_reuses_file_without_rewriting() {
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
    let events = p.finish();
    assert_eq!(
        events,
        [json!({"event":"finished","reason":"stopped","frame":0})]
    );
    p.success();
}
#[test]
fn absent_source_or_companion_and_corrupt_archive_fail_before_started() {
    for invalid in ["source", "companion", "corrupt"] {
        let fixture = Fixture::example();
        match invalid {
            "source" => fs::remove_file(fixture.root.join("example.wav")).unwrap(),
            "companion" => fs::remove_file(fixture.root.join("example.wav.replay")).unwrap(),
            _ => fs::write(fixture.root.join("example.wav.replay"), b"broken ZIP").unwrap(),
        }
        let mut process = fixture.process();
        process.send(fixture.start(false, json!("max")));
        let events = process.finish();
        assert!(events.iter().all(|event| event["event"] != "started"));
        assert_eq!(events[0]["fatal"], true);
        assert_eq!(events.last().unwrap()["reason"], "error");
        assert!(!process.child.wait().unwrap().success());
    }
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
    let events = p.finish();
    assert_eq!(events[0]["fatal"], false);
    assert_eq!(events[1]["reason"], "stopped");
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
#[cfg(unix)]
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
fn existing_destinations_and_nondirectory_ancestors_are_preserved() {
    for conflict in ["files", "files/message.bin"] {
        let f = Fixture::example();
        let path = f.root.join("out").join(conflict);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, b"KEEP").unwrap();
        let mut p = f.process();
        p.send(f.start(false, json!("max")));
        let events = p.finish();
        assert_eq!(events.last().unwrap()["reason"], "error");
        assert!(!p.child.wait().unwrap().success());
        assert_eq!(fs::read(&path).unwrap(), b"KEEP");
        assert!(events.iter().all(|e| {
            e["binary"]
                .as_array()
                .is_none_or(|descriptors| descriptors.iter().all(|d| d["path"].is_null()))
        }));
    }
}

#[test]
#[cfg(unix)]
fn symlink_payload_ancestors_and_destinations_are_rejected() {
    use std::os::unix::fs::symlink;
    for ancestor in [false, true] {
        let f = Fixture::example();
        fs::create_dir(f.root.join("out")).unwrap();
        fs::create_dir(f.root.join("elsewhere")).unwrap();
        fs::write(f.root.join("elsewhere/message.bin"), b"KEEP").unwrap();
        if ancestor {
            symlink(f.root.join("elsewhere"), f.root.join("out/files")).unwrap();
        } else {
            fs::create_dir(f.root.join("out/files")).unwrap();
            symlink(
                f.root.join("elsewhere/message.bin"),
                f.root.join("out/files/message.bin"),
            )
            .unwrap();
        }
        let mut p = f.process();
        p.send(f.start(false, json!("max")));
        let events = p.finish();
        assert_eq!(events.last().unwrap()["reason"], "error");
        assert!(!p.child.wait().unwrap().success());
        assert_eq!(
            fs::read(f.root.join("elsewhere/message.bin")).unwrap(),
            b"KEEP"
        );
        assert!(events.iter().all(|e| {
            e["binary"]
                .as_array()
                .is_none_or(|descriptors| descriptors.iter().all(|d| d["path"].is_null()))
        }));
    }
}

#[test]
fn unsafe_descriptor_paths_fail_without_materializing() {
    for path in [
        concat!(".", ".", "/escape"),
        "/absolute",
        "files/../escape",
        "files//alias",
        "files/./alias",
        r"files\escape",
        "C:alias",
    ] {
        let f = Fixture::new(
            &journal(&[
                json!({"event":"progress","frame":8}),
                json!({"record":"finalization"}),
                result(path, 4),
            ]),
            8,
        );
        let mut p = f.process();
        p.send(f.start(false, json!("max")));
        let events = p.finish();
        assert_eq!(events.last().unwrap()["reason"], "error");
        assert!(!p.child.wait().unwrap().success());
        assert!(events.iter().all(|e| e["event"] != "result"));
        assert_eq!(fs::read_dir(f.root.join("out")).unwrap().count(), 0);
    }
}

#[test]
fn all_result_files_are_ready_before_descriptor_and_after_exit() {
    let mut whole = result("files/message.bin", 4);
    whole["binary"]
        .as_array_mut()
        .unwrap()
        .push(result("files/second.bin", 4)["binary"][0].clone());
    let f = Fixture::new(
        &journal(&[
            json!({"event":"progress","frame":8}),
            json!({"record":"finalization"}),
            whole.clone(),
        ]),
        8,
    );
    let mut p = f.process();
    p.send(f.start(false, json!("max")));
    loop {
        let event = p.next();
        if event["event"] == "result" {
            assert_eq!(event, whole);
            break;
        }
        assert_ne!(event["event"], "finished");
    }
    for name in ["message.bin", "second.bin"] {
        assert_eq!(
            fs::read(f.root.join("out/files").join(name)).unwrap(),
            b"TEST"
        );
    }
    assert_eq!(p.finish().last().unwrap()["reason"], "end");
    p.success();
    for name in ["message.bin", "second.bin"] {
        assert_eq!(
            fs::read(f.root.join("out/files").join(name)).unwrap(),
            b"TEST"
        );
    }
}

#[test]
fn payload_failure_cleans_stage_and_preserves_published_and_unrelated_files() {
    let mut whole = result("files/message.bin", 4);
    whole["binary"]
        .as_array_mut()
        .unwrap()
        .push(result("files/second.bin", 4)["binary"][0].clone());
    let f = Fixture::new(
        &journal(&[
            json!({"event":"progress","frame":8}),
            json!({"record":"finalization"}),
            whole,
        ]),
        8,
    );
    let archive = f.root.join("example.wav.replay");
    let mut bytes = fs::read(&archive).unwrap();
    let index = bytes
        .windows(4)
        .enumerate()
        .filter(|(_, w)| *w == b"TEST")
        .nth(1)
        .unwrap()
        .0;
    bytes[index] ^= 1;
    fs::write(archive, bytes).unwrap();
    fs::create_dir(f.root.join("out")).unwrap();
    fs::write(f.root.join("out/keep"), b"KEEP").unwrap();
    let mut p = f.process();
    p.send(f.start(false, json!("max")));
    let events = p.finish();
    assert_eq!(events.last().unwrap()["reason"], "error");
    assert!(!p.child.wait().unwrap().success());
    assert!(events.iter().all(|e| e["event"] != "result"));
    assert_eq!(
        fs::read(f.root.join("out/files/message.bin")).unwrap(),
        b"TEST"
    );
    assert_eq!(fs::read(f.root.join("out/keep")).unwrap(), b"KEEP");
    let files: Vec<_> = fs::read_dir(f.root.join("out/files"))
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(files, [std::ffi::OsString::from("message.bin")]);
    assert_eq!(fs::read_dir(f.root.join("out")).unwrap().count(), 2);
}

#[test]
#[cfg(unix)]
fn public_caller_ancestry_is_accepted_without_permission_changes() {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let f = Fixture::example();
    fs::set_permissions(&f.root, fs::Permissions::from_mode(0o777)).unwrap();
    fs::create_dir(f.root.join("out")).unwrap();
    fs::set_permissions(f.root.join("out"), fs::Permissions::from_mode(0o755)).unwrap();
    let mut p = f.process();
    p.send(f.start(false, json!("max")));
    assert_eq!(p.finish().last().unwrap()["reason"], "end");
    p.success();
    assert_eq!(fs::metadata(&f.root).unwrap().mode() & 0o777, 0o777);
    assert_eq!(
        fs::metadata(f.root.join("out")).unwrap().mode() & 0o777,
        0o755
    );
}
