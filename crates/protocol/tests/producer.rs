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
fn exact_opaque_json_all_payloads_and_private_marker_keys() {
    let payload = r#""record":"finalization","bits":"01","text":" \nλ","log":"\t!","ale":null,"params":[1,true,null],"vendor":{"$serde_json::private::Number":"not a number","n":18446744073709551616000000000000000,"d":0.123456789012345678901234567890123456789,"e":1e9999999999999999999999999},"binary":[{"id":"p","type":"phase","body":null,"points":[]},{"id":"f","type":"opaque","body":{"n":1.000000000000000000000000000001},"path":"a/λ%2F.bin","size":18446744073709551615}]"#;
    let DecodedEvent::Known(event) = result(payload).unwrap() else {
        panic!()
    };
    let encoded = event.encode(CONTEXT).unwrap();
    let Json::Object(fields) = Json::parse(&encoded).unwrap() else {
        panic!()
    };
    assert_eq!(fields["record"], Json::String("finalization".into()));
    assert_eq!(fields["text"], Json::String(" \nλ".into()));
    assert_eq!(fields["log"], Json::String("\t!".into()));
    assert_eq!(fields["ale"], Json::Null);
    let original = Json::parse(&format!(
        r#"{{"event":"result","samples":[0,0],{payload}}}"#
    ))
    .unwrap();
    assert_eq!(Json::Object(fields), original);
    assert!(encoded.contains("18446744073709551616000000000000000"));
    assert!(encoded.contains("0.123456789012345678901234567890123456789"));
    assert!(encoded.contains("1e9999999999999999999999999"));
    assert!(encoded.contains("1.000000000000000000000000000001"));
    for bits in [r#""bits":"""#, r#""bits":"0101""#] {
        result(bits).unwrap();
    }
    for bits in [r#""bits":"012""#, r#""bits":" 01""#, r#""bits":1"#] {
        assert!(result(bits).is_err());
    }
    let mut local = ResultEvent::new(Span { start: 0, end: 0 });
    for key in [
        "event", "samples", "bits", "text", "log", "ale", "params", "binary",
    ] {
        local.extra.insert(key.into(), Json::Null);
        assert!(Event::Result(local.clone()).encode(CONTEXT).is_err());
        local.extra.clear();
    }
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
            matches!(error, ValidationError::DuplicateKey(_)),
            "{line}: {error:?}"
        );
    }
    assert!(Json::parse(r#"{"$serde_json::private::Number":1,"x":2}"#).is_ok());
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
    for points in ["[]", "[[1000,-1000],[0,0],[-0,1]]"] {
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

#[test]
fn explicit_json_budget_grammar_and_arbitrary_numbers() {
    let deep = format!("{}0{}", "[".repeat(MAX_DEPTH), "]".repeat(MAX_DEPTH));
    assert_eq!(
        Json::parse(&deep).unwrap().to_compact_string().unwrap(),
        deep
    );
    let deeper = format!("[{deep}]");
    assert_eq!(
        Json::parse(&deeper).unwrap_err(),
        ValidationError::DepthLimit
    );
    let mut local = Json::Null;
    for _ in 0..=MAX_DEPTH {
        local = Json::Array(vec![local]);
    }
    assert_eq!(
        local.to_compact_string().unwrap_err(),
        ValidationError::DepthLimit
    );
    for token in [
        "0",
        "-0",
        "123456789012345678901234567890",
        "-1.2345678901234567890123456789e-9999999",
    ] {
        let number = Number::new(token).unwrap();
        assert_eq!(number.as_str(), token);
    }
    for token in [
        "", " 1", "1 ", "NaN", "Infinity", "+1", "01", "-", ".1", "1.", "1e", "1e+", "--1", "0x1",
    ] {
        assert!(Number::new(token).is_err(), "{token}");
    }
    for json in [
        "[1,]",
        "{\"a\":1,}",
        "true false",
        "{}{}",
        "\u{feff}{}",
        r#""\uD800""#,
        r#""\x""#,
        "\"a\nb\"",
    ] {
        assert!(Json::parse(json).is_err(), "{json}");
    }
}

#[test]
fn descriptor_required_field_types_and_mask_precision() {
    let phase = r#"{"id":"p","type":"phase","body":null,"points":[]}"#;
    let file = r#"{"id":"f","type":"custom","body":null,"path":"a","size":0}"#;
    for line in [phase, file] {
        let Json::Object(fields) = Json::parse(line).unwrap() else {
            panic!()
        };
        for key in fields.keys() {
            let mut missing = fields.clone();
            missing.remove(key);
            assert!(
                result(&format!(
                    "\"binary\":[{}]",
                    Json::Object(missing).to_compact_string().unwrap()
                ))
                .is_err()
            );
            if key == "body" {
                continue;
            }
            for invalid in [Json::Null, Json::Bool(true)] {
                let mut wrong = fields.clone();
                wrong.insert(key.clone(), invalid);
                assert!(
                    result(&format!(
                        "\"binary\":[{}]",
                        Json::Object(wrong).to_compact_string().unwrap()
                    ))
                    .is_err()
                );
            }
        }
    }
    descriptor(r#""id":"p","type":"phase","body":null,"points":[],"future":null"#).unwrap();
    let line = r#"{"event":"started","module":"m","mask":{"n":123456789012345678901234567890,"d":0.12345678901234567890123456789,"array":[null,true,"λ"]},"stream":{"format":"real","channels":1,"rate":[1,1]},"frames":0,"state":"paused","frame":0,"speed":"max","freq":0,"params":{}}"#;
    let DecodedEvent::Known(event) = Event::decode(line, CONTEXT).unwrap() else {
        panic!()
    };
    assert_eq!(
        Json::parse(line).unwrap(),
        Json::parse(&event.encode(CONTEXT).unwrap()).unwrap()
    );
}
