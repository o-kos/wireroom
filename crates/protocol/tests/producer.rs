use dmd_protocol::*;
const CONTEXT: Context = Context { frames: None };

fn result(payload: &str) -> Result<DecodedEvent, ValidationError> {
    Event::decode(
        &format!(r#"{{"event":"result","samples":[0,0],{payload}}}"#),
        CONTEXT,
    )
}
fn descriptor(fields: &str) -> Result<DecodedEvent, ValidationError> {
    result(&format!(r#""binary":[{{{fields}}}]"#))
}

#[test]
fn ordinary_producer_values_and_u64_integers_round_trip() {
    let payload = r#""text":" \nλ","record":"finalization","vendor":{"integer":18446744073709551615,"confidence":0.9,"items":[null,true,"x"]}"#;
    let DecodedEvent::Known(event) = result(payload).unwrap() else {
        panic!()
    };
    let encoded = event.encode(CONTEXT).unwrap();
    assert!(encoded.contains("18446744073709551615"));
    assert_eq!(
        Event::decode(&encoded, CONTEXT).unwrap(),
        DecodedEvent::Known(event)
    );
    assert!(result(r#""vendor":1e9999"#).is_err());
}

#[test]
fn duplicate_keys_at_every_depth_including_ignored_fields() {
    for line in [
        r#"{"command":"pause","command":"pause"}"#,
        r#"{"command":"set_params","params":{"x":"a","x":"b"}}"#,
        r#"{"event":"future","ignored":[{"x":1,"\u0078":2}]}"#,
        r#"{"event":"progress","frame":0,"ignored":{"a":{"b":1,"b":2}}}"#,
        r#"{"event":"result","samples":[0,0],"vendor":[{"x":1,"x":2}]}"#,
        r#"{"event":"result","samples":[0,0],"binary":[{"id":"p","type":"phase","body":{"x":null,"x":null},"points":[]}]}"#,
    ] {
        let error = if line.contains("command") {
            Command::decode(line).unwrap_err()
        } else {
            Event::decode(line, CONTEXT).unwrap_err()
        };
        assert!(
            matches!(error, ValidationError::Json(_)),
            "{line}: {error:?}"
        );
    }
}

#[test]
fn phase_conversion_and_encoded_integer_constraints() {
    assert_eq!(
        PhasePoint::from_source(0.0005, -0.0005).unwrap(),
        PhasePoint { i: 1, q: -1 }
    );
    assert_eq!(
        PhasePoint::from_source(-0.0, 0.0).unwrap(),
        PhasePoint { i: 0, q: 0 }
    );
    assert_eq!(
        PhasePoint::from_source(1.0, -1.0).unwrap(),
        PhasePoint { i: 1000, q: -1000 }
    );
    for source in [
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        1.00000001,
        -1.00000001,
    ] {
        assert!(PhasePoint::from_source(source, 0.0).is_err());
        assert!(PhasePoint::from_source(0.0, source).is_err());
    }
    for points in ["[]", "[[1000,-1000],[0,0],[0,1]]"] {
        descriptor(&format!(
            r#""id":"p","type":"phase","body":null,"points":{points}"#
        ))
        .unwrap();
    }
    for points in [
        "[[1001,0]]",
        "[[-1001,0]]",
        "[[1.0,0]]",
        "[[1e0,0]]",
        "[[0,1.0]]",
        "[[0]]",
        "[[0,0,0]]",
        "[0,0]",
        "null",
        "[[true,0]]",
        "[[32768,0]]",
    ] {
        assert!(
            descriptor(&format!(
                r#""id":"p","type":"phase","body":null,"points":{points}"#
            ))
            .is_err()
        );
    }
    let points = vec![PhasePoint { i: 712, q: -31 }, PhasePoint { i: 0, q: 0 }];
    let mut local = ResultEvent::new(Span { start: 0, end: 0 });
    local.binary = Some(vec![Descriptor::Phase {
        id: "p".into(),
        body: Json::Null,
        points,
    }]);
    assert_eq!(
        Event::Result(local).encode(CONTEXT).unwrap(),
        r#"{"binary":[{"body":null,"id":"p","points":[[712,-31],[0,0]],"type":"phase"}],"event":"result","samples":[0,0]}"#
    );
}

#[test]
fn descriptor_shapes_and_paths() {
    for fields in [
        r#""id":"","type":"phase","body":null,"points":[]"#,
        r#""id":"a\u0000","type":"phase","body":null,"points":[]"#,
        r#""id":"p","type":"","body":null,"points":[]"#,
        r#""id":"p","type":"phase","points":[]"#,
        r#""id":"p","type":"phase","body":null"#,
        r#""id":"p","type":"phase","body":null,"points":[],"path":null"#,
        r#""id":"p","type":"phase","body":null,"points":[],"size":0"#,
        r#""id":"f","type":"file","body":null,"path":"a","size":0,"points":null"#,
        r#""id":"f","type":"file","body":null,"path":"a""#,
        r#""id":"f","type":"file","body":null,"size":0"#,
    ] {
        assert!(descriptor(fields).is_err(), "{fields}");
    }
    for size in ["-1", "1.0", "1e0", "18446744073709551616", "null", "\"1\""] {
        assert!(
            descriptor(&format!(
                r#""id":"f","type":"file","body":null,"path":"a","size":{size}"#
            ))
            .is_err()
        );
    }
    let opaque_id = ["..", "opaque"].join("/");
    for path in ["a", "files/a.bin", " λ /é.bin", "%2e%2e/%2f", "A/é"] {
        validate_result_path(path).unwrap();
        let path_json = Json::String(path.into()).to_compact_string().unwrap();
        descriptor(&format!(
            r#""id":"{opaque_id}","type":"PHASE","body":null,"path":{path_json},"size":0"#
        ))
        .unwrap();
    }
    for path in [
        "", "/a", "a/", "a//b", ".", "..", "a/./b", "a/../b", "a\\b", "C:a", "https:a", "a\0b",
    ] {
        assert!(validate_result_path(path).is_err(), "{path:?}");
    }
    // Launch paths have a separate contract and remain untouched.
    let windows_path = ["C:", "recording.wav"].join("\\");
    for path in ["/absolute/a", "relative/../a", windows_path.as_str()] {
        let command = Command::Start(Start {
            recording: path.into(),
            output: path.into(),
            module: "m".into(),
            params: Parameters::new(),
            freq: 0.0,
            speed: Speed::Max,
            paused: false,
        });
        let decoded = Command::decode(&command.encode().unwrap()).unwrap();
        assert_eq!(decoded, command);
    }
}
