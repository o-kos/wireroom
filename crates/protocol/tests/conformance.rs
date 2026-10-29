use dmd_protocol::*;

const CONTEXT: Context = Context { frames: None };
const START: &str = r#"{"command":"start","recording":"rec.wav","module":"navtex","params":{"mode":"fec"},"freq":1855.0,"speed":8,"paused":false,"output":"out"}"#;
const STARTED: &str = r#"{"event":"started","module":"demo","mask":{},"stream":{"format":"iq","channels":1,"rate":[4800,1]},"frames":9600,"state":"playing","frame":0,"speed":8,"freq":1855.0,"params":{}}"#;
const STATE: &str =
    r#"{"event":"state","state":"paused","frame":4,"speed":1,"freq":0,"params":{}}"#;

fn event(line: &str) -> Event {
    match Event::decode(line, CONTEXT).unwrap() {
        DecodedEvent::Known(event) => event,
        other => panic!("unexpected {other:?}"),
    }
}
fn replace(line: &str, key: &str, replacement: Option<&str>) -> String {
    let Json::Object(mut fields) = Json::parse(line).unwrap() else {
        panic!()
    };
    if let Some(replacement) = replacement {
        fields.insert(key.into(), Json::parse(replacement).unwrap());
    } else {
        fields.remove(key);
    }
    Json::Object(fields).to_compact_string().unwrap()
}
fn rejects_event(line: &str) {
    assert!(Event::decode(line, CONTEXT).is_err(), "{line}");
}
fn rejects_command(line: &str) {
    assert!(Command::decode(line).is_err(), "{line}");
}

#[test]
fn all_specification_example_lines() {
    let mut commands = 0;
    let mut events = 0;
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
                commands += 1;
            } else if line.starts_with(r#"{"event":"#) {
                let value = event(line);
                assert_eq!(event(&value.encode(CONTEXT).unwrap()), value);
                events += 1;
            }
        }
    }
    assert_eq!(commands, 16);
    assert_eq!(events, 46);
}

#[test]
fn independent_wire_forms_and_every_kind() {
    for (command, wire) in [
        (Command::Pause, r#"{"command":"pause"}"#),
        (Command::Resume, r#"{"command":"resume"}"#),
        (Command::Stop, r#"{"command":"stop"}"#),
        (
            Command::SetSpeed(Speed::Max),
            r#"{"command":"set_speed","speed":"max"}"#,
        ),
        (Command::Tune(-1.5), r#"{"command":"tune","freq":-1.5}"#),
        (
            Command::SetParams(Parameters::new()),
            r#"{"command":"set_params","params":{}}"#,
        ),
    ] {
        assert_eq!(command.encode().unwrap(), wire);
    }
    assert_eq!(
        Event::Hello(CURRENT_VERSION).encode(CONTEXT).unwrap(),
        r#"{"event":"hello","protocol":{"major":1,"minor":0}}"#
    );
    assert_eq!(
        Event::Progress {
            frame: 9_007_199_254_740_993
        }
        .encode(CONTEXT)
        .unwrap(),
        r#"{"event":"progress","frame":9007199254740993}"#
    );
    for level in ["debug", "info", "warning", "error"] {
        event(&format!(
            r#"{{"event":"log","frame":0,"level":"{level}","text":""}}"#
        ));
    }
    for reason in ["end", "stopped", "error"] {
        event(&format!(
            r#"{{"event":"finished","frame":0,"reason":"{reason}"}}"#
        ));
    }
    event(r#"{"event":"error","fatal":true,"message":"failed"}"#);
    event(r#"{"event":"error","fatal":false,"message":"rejected"}"#);
    let start = Command::decode(START).unwrap();
    assert_eq!(
        start.encode().unwrap(),
        r#"{"command":"start","freq":1855,"module":"navtex","output":"out","params":{"mode":"fec"},"paused":false,"recording":"rec.wav","speed":8}"#
    );
}

#[test]
fn required_fields_nulls_and_wrong_types() {
    let known = [
        STARTED,
        STATE,
        r#"{"event":"hello","protocol":{"major":1,"minor":0}}"#,
        r#"{"event":"progress","frame":0}"#,
        r#"{"event":"log","frame":0,"level":"info","text":""}"#,
        r#"{"event":"error","fatal":false,"message":"x"}"#,
        r#"{"event":"finished","reason":"end","frame":0}"#,
        r#"{"event":"result","samples":[0,0]}"#,
    ];
    for line in known {
        let Json::Object(fields) = Json::parse(line).unwrap() else {
            panic!()
        };
        for key in fields.keys() {
            rejects_event(&replace(line, key, None));
            rejects_event(&replace(line, key, Some("null")));
            rejects_event(&replace(line, key, Some("[]")));
        }
    }
    for line in [
        START,
        r#"{"command":"set_speed","speed":1}"#,
        r#"{"command":"tune","freq":1}"#,
        r#"{"command":"set_params","params":{}}"#,
    ] {
        let Json::Object(fields) = Json::parse(line).unwrap() else {
            panic!()
        };
        for key in fields.keys() {
            rejects_command(&replace(line, key, None));
            rejects_command(&replace(line, key, Some("null")));
            rejects_command(&replace(line, key, Some("[]")));
        }
    }
    for key in ["bits", "text", "log", "binary"] {
        rejects_event(&replace(
            r#"{"event":"result","samples":[0,0]}"#,
            key,
            Some("null"),
        ));
    }
}

#[test]
fn strict_commands_hello_and_extensible_events() {
    rejects_command(r#"{"command":"unknown"}"#);
    for command in [
        START,
        r#"{"command":"pause"}"#,
        r#"{"command":"resume"}"#,
        r#"{"command":"stop"}"#,
        r#"{"command":"set_speed","speed":1}"#,
        r#"{"command":"tune","freq":0}"#,
        r#"{"command":"set_params","params":{}}"#,
    ] {
        rejects_command(&replace(command, "extension", Some("7")));
    }
    rejects_event(r#"{"event":"hello","protocol":{"major":1,"minor":0},"x":1}"#);
    rejects_event(r#"{"event":"hello","protocol":{"major":1,"minor":0,"x":1}}"#);
    assert!(
        matches!(Event::decode(r#"{"event":"future","x":null}"#, CONTEXT).unwrap(),
        DecodedEvent::Unknown { name, .. } if name == "future")
    );
    rejects_event(r#"{"event":1}"#);
    rejects_event(r#"{"x":"unknown"}"#);
    assert_eq!(event(&replace(STATE, "future", Some("null"))), event(STATE));
    rejects_event(&replace(
        &replace(STATE, "future", Some("null")),
        "frame",
        Some("null"),
    ));
    assert!(Version { major: 0, minor: 0 }.validate().is_err());
    assert!(
        Version {
            major: 2,
            minor: u32::MAX
        }
        .validate()
        .is_ok()
    );
    assert!(Version { major: 2, minor: 0 }.check_supported(1).is_err());
    assert!(
        Version {
            major: 1,
            minor: u32::MAX
        }
        .check_supported(1)
        .is_ok()
    );
    assert!(Version { major: 2, minor: 0 }.check_supported(2).is_ok());
}

#[test]
fn exact_integer_tokens_and_ranges() {
    for token in ["0", "9007199254740993", "18446744073709551615"] {
        let line = format!(r#"{{"event":"progress","frame":{token}}}"#);
        assert_eq!(event(&line).encode(CONTEXT).unwrap(), line);
    }
    for token in [
        "-0",
        "-1",
        "18446744073709551616",
        "1.0",
        "1e0",
        "1E+0",
        "true",
        "\"1\"",
    ] {
        rejects_event(&format!(r#"{{"event":"progress","frame":{token}}}"#));
        rejects_event(&replace(STARTED, "frames", Some(token)));
    }
    for major in ["1", "4294967295"] {
        event(&format!(
            r#"{{"event":"hello","protocol":{{"major":{major},"minor":4294967295}}}}"#
        ));
    }
    for token in ["0", "4294967296", "-1", "1.0", "1e0"] {
        rejects_event(&format!(
            r#"{{"event":"hello","protocol":{{"major":{token},"minor":0}}}}"#
        ));
    }
    for token in ["4294967296", "-1", "0.0", "0e0", "null"] {
        rejects_event(&format!(
            r#"{{"event":"hello","protocol":{{"major":1,"minor":{token}}}}}"#
        ));
    }
    let Json::Object(mut fields) = Json::parse(STARTED).unwrap() else {
        panic!()
    };
    for channels in ["0", "4294967296", "1.0", "1e0", "-1", "null"] {
        fields.insert(
            "stream".into(),
            Json::parse(&format!(
                r#"{{"format":"iq","channels":{channels},"rate":[1,1]}}"#
            ))
            .unwrap(),
        );
        rejects_event(&Json::Object(fields.clone()).to_compact_string().unwrap());
    }
    for rate in [
        "[0,1]",
        "[1,0]",
        "[1.0,1]",
        "[1,1e0]",
        "[-1,1]",
        "[1]",
        "[1,1,1]",
        "[18446744073709551616,1]",
    ] {
        fields.insert(
            "stream".into(),
            Json::parse(&format!(
                r#"{{"format":"real","channels":1,"rate":{rate}}}"#
            ))
            .unwrap(),
        );
        rejects_event(&Json::Object(fields.clone()).to_compact_string().unwrap());
    }
    fields.insert("stream".into(), Json::parse(r#"{"format":"real","channels":4294967295,"rate":[18446744073709551615,18446744073709551615]}"#).unwrap());
    event(&Json::Object(fields).to_compact_string().unwrap());
}

#[test]
fn numeric_string_and_enum_validation() {
    for token in ["-1", "0", "-0.0", "1e999", "\"MAX\"", "null", "true"] {
        rejects_command(&format!(r#"{{"command":"set_speed","speed":{token}}}"#));
    }
    for token in ["1", "2.5", "1e-300", "1.7976931348623157e308", "\"max\""] {
        Command::decode(&format!(r#"{{"command":"set_speed","speed":{token}}}"#)).unwrap();
    }
    for token in ["1e999", "\"1\"", "false"] {
        rejects_command(&replace(START, "freq", Some(token)));
    }
    for token in ["-1e308", "0", "1e308"] {
        Command::decode(&replace(START, "freq", Some(token))).unwrap();
    }
    for field in ["recording", "output", "module"] {
        for value in ["\"\"", r#""a\u0000b""#] {
            rejects_command(&replace(START, field, Some(value)));
        }
    }
    Command::decode(&replace(START, "params", Some(r#"{"":""}"#))).unwrap();
    for value in [r#"{"x":0}"#, r#"{"x":null}"#, r#"{"x":true}"#, "[]"] {
        rejects_command(&replace(START, "params", Some(value)));
    }
    rejects_event(&replace(STATE, "state", Some(r#""finalizing""#)));
    rejects_event(r#"{"event":"log","frame":0,"level":"warn","text":"x"}"#);
    rejects_event(r#"{"event":"finished","reason":"done","frame":0}"#);
    rejects_event(r#"{"event":"error","fatal":false,"message":""}"#);
    rejects_event(&replace(STARTED, "mask", Some("null")));
    rejects_event(&replace(
        STARTED,
        "stream",
        Some(r#"{"format":"complex","channels":1,"rate":[1,1]}"#),
    ));
}

#[test]
fn source_context_is_explicit_and_has_no_sequence_state() {
    rejects_event(&replace(STARTED, "frame", Some("1")));
    for total in [0, 8] {
        let context = Context {
            frames: Some(total),
        };
        for frame in [0, total] {
            let event = Event::Progress { frame };
            event.validate(context).unwrap();
            event.validate(context).unwrap();
        }
        assert!(
            Event::Progress { frame: total + 1 }
                .validate(context)
                .is_err()
        );
        for span in [
            Span { start: 0, end: 0 },
            Span {
                start: 0,
                end: total,
            },
            Span {
                start: total,
                end: total,
            },
        ] {
            Event::Result(ResultEvent::new(span))
                .validate(context)
                .unwrap();
        }
        assert!(
            Event::Finished {
                reason: TerminalReason::End,
                frame: total
            }
            .validate(context)
            .is_ok()
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
    assert!(
        Event::Finished {
            reason: TerminalReason::End,
            frame: 0
        }
        .validate(Context { frames: Some(8) })
        .is_err()
    );
    assert!(
        Event::Finished {
            reason: TerminalReason::End,
            frame: 0
        }
        .validate(CONTEXT)
        .is_ok()
    );
    for span in [
        "[4,3]", "[0,9]", "[-1,0]", "[0.0,1]", "[0,1e0]", "[0]", "null",
    ] {
        assert!(
            Event::decode(
                &format!(r#"{{"event":"result","samples":{span}}}"#),
                Context { frames: Some(8) }
            )
            .is_err()
        );
    }
}

#[test]
fn nested_required_shapes_and_ignorable_extensions() {
    for stream in [
        r#"{"channels":1,"rate":[1,1]}"#,
        r#"{"format":"real","rate":[1,1]}"#,
        r#"{"format":"real","channels":1}"#,
        r#"{"format":null,"channels":1,"rate":[1,1]}"#,
        r#"{"format":"real","channels":null,"rate":[1,1]}"#,
        r#"{"format":"real","channels":1,"rate":null}"#,
    ] {
        rejects_event(&replace(STARTED, "stream", Some(stream)));
    }
    event(&replace(
        STARTED,
        "stream",
        Some(r#"{"format":"real","channels":1,"rate":[1,1],"future":null}"#),
    ));
    for line in [
        STARTED,
        STATE,
        r#"{"event":"progress","frame":0}"#,
        r#"{"event":"log","frame":0,"level":"info","text":""}"#,
        r#"{"event":"error","fatal":false,"message":"x"}"#,
        r#"{"event":"finished","reason":"end","frame":0}"#,
    ] {
        assert_eq!(
            event(line),
            event(&replace(
                line,
                "extension",
                Some("[null,true,{\"n\":1e999}]")
            ))
        );
    }
    for version in [
        "{}",
        r#"{"major":1}"#,
        r#"{"minor":0}"#,
        r#"{"major":null,"minor":0}"#,
        r#"{"major":1,"minor":null}"#,
    ] {
        rejects_event(&format!(r#"{{"event":"hello","protocol":{version}}}"#));
    }
}
