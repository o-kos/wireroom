use dmd_protocol::*;

const CONTEXT: Context = Context { frames: None };

fn event(line: &str) -> Event {
    match Event::decode(line, CONTEXT).unwrap() {
        DecodedEvent::Known(event) => event,
        other => panic!("unexpected {other:?}"),
    }
}

#[test]
fn specification_examples_round_trip() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap();
    for name in ["protocol.md", "replay.md"] {
        let spec = std::fs::read_to_string(root.join("docs").join(name)).unwrap();
        for line in spec.lines().filter(|line| line.starts_with('{')) {
            if line.starts_with(r#"{"command":"#) {
                let command = Command::decode(line).unwrap();
                assert_eq!(
                    Command::decode(&command.encode().unwrap()).unwrap(),
                    command
                );
            } else if line.starts_with(r#"{"event":"#) {
                let value = event(line);
                assert_eq!(event(&value.encode(CONTEXT).unwrap()), value);
            }
        }
    }
}

#[test]
fn independent_wire_forms_and_exact_source_coordinates() {
    assert_eq!(Command::Pause.encode().unwrap(), r#"{"command":"pause"}"#);
    assert_eq!(
        Event::Hello(CURRENT_VERSION).encode(CONTEXT).unwrap(),
        r#"{"event":"hello","protocol":{"major":1,"minor":0}}"#
    );
    for token in ["9007199254740993", "18446744073709551615"] {
        let line = format!(r#"{{"event":"progress","frame":{token}}}"#);
        assert_eq!(event(&line).encode(CONTEXT).unwrap(), line);
    }
    let line = r#"{"event":"result","samples":[9007199254740993,18446744073709551615]}"#;
    assert_eq!(event(line).encode(CONTEXT).unwrap(), line);
    for token in ["-1", "18446744073709551616", "1.0", "1e0", "null"] {
        assert!(
            Event::decode(
                &format!(r#"{{"event":"progress","frame":{token}}}"#),
                CONTEXT
            )
            .is_err()
        );
    }
}

#[test]
fn malformed_required_values_and_unsupported_numbers_fail_explicitly() {
    for line in [
        r#"{"command":"start"}"#,
        r#"{"command":"set_speed","speed":0}"#,
        r#"{"command":"set_speed","speed":1e999}"#,
        r#"{"command":"set_params","params":{"mode":1}}"#,
        r#"{"command":"tune","freq":null}"#,
    ] {
        assert!(Command::decode(line).is_err(), "{line}");
    }
    for line in [
        r#"{"event":"progress"}"#,
        r#"{"event":"log","frame":0,"level":"warn","text":"x"}"#,
        r#"{"event":"result","samples":[0,1],"text":null}"#,
        r#"{"event":"hello","protocol":{"major":1}}"#,
        r#"{"event":"finished","reason":"done","frame":0}"#,
    ] {
        assert!(Event::decode(line, CONTEXT).is_err(), "{line}");
    }
    Command::decode(r#"{"command":"set_speed","speed":2.5}"#).unwrap();
    let command = Command::Tune(94.52706955539223);
    assert_eq!(
        Command::decode(&command.encode().unwrap()).unwrap(),
        command
    );
}

#[test]
fn commands_are_strict_and_events_allow_extensions() {
    assert!(Command::decode(r#"{"command":"future"}"#).is_err());
    assert!(Command::decode(r#"{"command":"pause","extra":1}"#).is_err());
    assert!(
        Event::decode(
            r#"{"event":"hello","protocol":{"major":1,"minor":0},"extra":1}"#,
            CONTEXT
        )
        .is_err()
    );
    assert!(matches!(
        Event::decode(r#"{"event":"future","extra":null}"#, CONTEXT).unwrap(),
        DecodedEvent::Unknown { name, .. } if name == "future"
    ));
    assert_eq!(
        event(r#"{"event":"progress","frame":4,"extra":[null,true,{"n":1.25}]}"#),
        Event::Progress { frame: 4 }
    );
    assert!(Version { major: 2, minor: 0 }.check_supported(1).is_err());
}

#[test]
fn source_context_bounds_frames_and_spans() {
    let context = Context { frames: Some(8) };
    assert!(Event::Progress { frame: 8 }.validate(context).is_ok());
    assert!(Event::Progress { frame: 9 }.validate(context).is_err());
    for span in ["[4,3]", "[0,9]"] {
        assert!(
            Event::decode(
                &format!(r#"{{"event":"result","samples":{span}}}"#),
                context
            )
            .is_err()
        );
    }
    assert!(
        Event::Finished {
            reason: TerminalReason::End,
            frame: 0
        }
        .validate(context)
        .is_err()
    );
    assert!(
        Event::Finished {
            reason: TerminalReason::Stopped,
            frame: 0
        }
        .validate(context)
        .is_ok()
    );
}
