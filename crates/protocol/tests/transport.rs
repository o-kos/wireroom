use dmd_protocol::*;
use std::io::{self, BufReader, Cursor, Read, Write};
const CONTEXT: Context = Context { frames: None };

struct ShortRead {
    bytes: Cursor<Vec<u8>>,
    chunk: usize,
    interrupted: bool,
}
impl Read for ShortRead {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if self.interrupted {
            self.interrupted = false;
            return Err(io::ErrorKind::Interrupted.into());
        }
        let count = output.len().min(self.chunk);
        self.bytes.read(&mut output[..count])
    }
}
fn fragmented(bytes: &[u8], chunk: usize) -> Reader<BufReader<ShortRead>> {
    Reader::new(BufReader::with_capacity(
        7,
        ShortRead {
            bytes: Cursor::new(bytes.to_vec()),
            chunk,
            interrupted: true,
        },
    ))
}
#[derive(Default)]
struct Sink {
    bytes: Vec<u8>,
    flushes: usize,
    chunk: usize,
    fail_write: bool,
    interrupted: bool,
    fail_flush: bool,
}
impl Write for Sink {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.interrupted {
            self.interrupted = false;
            return Err(io::ErrorKind::Interrupted.into());
        }
        if self.fail_write {
            return Err(io::ErrorKind::BrokenPipe.into());
        }
        let count = bytes.len().min(self.chunk);
        self.bytes.extend_from_slice(&bytes[..count]);
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.flushes += 1;
        if self.fail_flush {
            return Err(io::ErrorKind::Other.into());
        }
        Ok(())
    }
}

#[test]
fn arbitrary_fragment_boundaries_utf8_crlf_and_multiple_messages() {
    let bytes = "{\"event\":\"log\",\"frame\":0,\"level\":\"info\",\"text\":\"λ😀\"}\r\n{\"event\":\"progress\",\"frame\":1}\n".as_bytes();
    for chunk in 1..=bytes.len() {
        let mut reader = fragmented(bytes, chunk);
        assert_eq!(
            reader.read_event(CONTEXT).unwrap(),
            Some(DecodedEvent::Known(Event::Log {
                frame: 0,
                level: LogLevel::Info,
                text: "λ😀".into(),
            }))
        );
        assert_eq!(
            reader.read_event(CONTEXT).unwrap(),
            Some(DecodedEvent::Known(Event::Progress { frame: 1 }))
        );
        assert_eq!(reader.read_event(CONTEXT).unwrap(), None);
        assert_eq!(reader.read_event(CONTEXT).unwrap(), None);
    }
    let mut reader = Reader::new(Cursor::new(
        b"{\"command\":\"pause\"}\n{\"command\":\"stop\"}\r\n",
    ));
    assert_eq!(reader.read_command().unwrap(), Some(Command::Pause));
    assert_eq!(reader.read_command().unwrap(), Some(Command::Stop));
    assert_eq!(reader.read_command().unwrap(), None);
}

#[test]
fn complete_line_rejections_recover_but_incomplete_eof_does_not_execute() {
    for malformed in [
        b"\xef\xbb\xbf{}".as_slice(),
        b"",
        b" ",
        b"\xff",
        b"{",
        b"[]",
        b"null",
        b"{}{}",
        b"{\"command\":\"pause\",\"command\":\"pause\"}",
        b"{\"command\":1}",
    ] {
        let mut input = malformed.to_vec();
        input.extend_from_slice(b"\n{\"command\":\"pause\"}\n");
        let mut reader = fragmented(&input, 1);
        assert!(
            reader
                .read_command()
                .unwrap_err()
                .is_complete_line_rejection(),
            "{malformed:?}"
        );
        assert_eq!(reader.read_command().unwrap(), Some(Command::Pause));
    }
    for incomplete in [
        b"{\"command\":\"stop\"}".as_slice(),
        b"\r",
        b"\xf0\x9f",
        b"{}\r",
    ] {
        let mut reader = fragmented(incomplete, 1);
        assert!(matches!(
            reader.read_command(),
            Err(DecodeError::Incomplete)
        ));
        assert!(matches!(reader.read_command(), Err(DecodeError::Ended)));
    }
    for line in [
        "null\n",
        "[]\n",
        "{\"event\":\"progress\",\"frame\":null}\n",
    ] {
        let mut reader = Reader::new(Cursor::new(line));
        assert!(matches!(
            reader.read_event(CONTEXT),
            Err(DecodeError::Invalid(_))
        ));
    }
}

fn padded_line(size: usize, ending: &[u8]) -> Vec<u8> {
    let mut bytes = b"{\"command\":\"pause\"}".to_vec();
    bytes.resize(size, b' ');
    bytes.extend_from_slice(ending);
    bytes
}

#[test]
fn input_exact_limit_lf_crlf_fragmentation_and_early_oversize() {
    for ending in [b"\n".as_slice(), b"\r\n"] {
        let bytes = padded_line(MAX_LINE_BYTES, ending);
        for capacity in [8192, MAX_LINE_BYTES, MAX_LINE_BYTES + 1] {
            let mut reader = Reader::new(BufReader::with_capacity(capacity, Cursor::new(&bytes)));
            assert_eq!(reader.read_command().unwrap(), Some(Command::Pause));
            assert_eq!(reader.read_command().unwrap(), None);
        }
        let bytes = padded_line(MAX_LINE_BYTES + 1, ending);
        let mut reader = Reader::new(Cursor::new(bytes));
        assert!(matches!(reader.read_command(), Err(DecodeError::TooLong)));
        assert!(matches!(reader.read_command(), Err(DecodeError::Ended)));
        assert!(reader.into_inner().position() <= (MAX_LINE_BYTES + 1) as u64);
    }
    let mut bytes = padded_line(MAX_LINE_BYTES, b"\rX");
    bytes.resize(MAX_LINE_BYTES * 2, b'x');
    let mut reader = Reader::new(Cursor::new(bytes));
    assert!(matches!(reader.read_command(), Err(DecodeError::TooLong)));
    assert_eq!(reader.into_inner().position(), (MAX_LINE_BYTES + 1) as u64);
}

#[test]
fn input_exact_limit_eof_counts_provisional_cr_as_content() {
    for capacity in [8192, MAX_LINE_BYTES, MAX_LINE_BYTES + 1, MAX_LINE_BYTES + 2] {
        let mut reader = Reader::new(BufReader::with_capacity(
            capacity,
            Cursor::new(padded_line(MAX_LINE_BYTES, b"")),
        ));
        assert!(matches!(
            reader.read_command(),
            Err(DecodeError::Incomplete)
        ));
        assert!(matches!(reader.read_command(), Err(DecodeError::Ended)));

        let mut reader = Reader::new(BufReader::with_capacity(
            capacity,
            Cursor::new(padded_line(MAX_LINE_BYTES, b"\r")),
        ));
        assert!(matches!(reader.read_command(), Err(DecodeError::TooLong)));
        assert!(matches!(reader.read_command(), Err(DecodeError::Ended)));

        let mut reader = Reader::new(BufReader::with_capacity(
            capacity,
            Cursor::new(padded_line(MAX_LINE_BYTES, b"\r\n")),
        ));
        assert_eq!(reader.read_command().unwrap(), Some(Command::Pause));
        assert_eq!(reader.read_command().unwrap(), None);
    }
}

#[test]
fn short_writes_flush_per_message_and_exact_wire() {
    let mut writer = Writer::new(Sink {
        interrupted: true,
        chunk: 1,
        ..Sink::default()
    });
    writer.write_command(&Command::Pause).unwrap();
    writer
        .write_event(&Event::Hello(CURRENT_VERSION), CONTEXT)
        .unwrap();
    let sink = writer.into_inner();
    assert_eq!(
        sink.bytes,
        b"{\"command\":\"pause\"}\n{\"event\":\"hello\",\"protocol\":{\"major\":1,\"minor\":0}}\n"
    );
    assert_eq!(sink.flushes, 2);
}

fn text_result(text: String) -> Event {
    let mut result = ResultEvent::new(Span { start: 0, end: 0 });
    result.text = Some(text);
    Event::Result(result)
}
#[test]
fn output_exact_limit_including_escapes_and_phase_points() {
    let overhead = text_result(String::new()).encode(CONTEXT).unwrap().len();
    let mut writer = Writer::new(Sink {
        chunk: 8192,
        ..Sink::default()
    });
    writer
        .write_event(&text_result("x".repeat(MAX_LINE_BYTES - overhead)), CONTEXT)
        .unwrap();
    let sink = writer.into_inner();
    assert_eq!(sink.bytes.len(), MAX_LINE_BYTES + 1);
    assert_eq!(sink.bytes.last(), Some(&b'\n'));
    assert_eq!(sink.flushes, 1);
    let mut reader = Reader::new(Cursor::new(sink.bytes));
    assert!(reader.read_event(CONTEXT).unwrap().is_some());
    assert!(reader.read_event(CONTEXT).unwrap().is_none());
    // NUL needs six encoded bytes; count the actual escaped wire length.
    let available = MAX_LINE_BYTES - overhead;
    let escaped = format!(
        "{}{}",
        "\0".repeat(available / 6),
        "x".repeat(available % 6)
    );
    let mut writer = Writer::new(Sink {
        chunk: 65536,
        ..Sink::default()
    });
    writer
        .write_event(&text_result(escaped.clone()), CONTEXT)
        .unwrap();
    assert_eq!(writer.into_inner().bytes.len(), MAX_LINE_BYTES + 1);
    for text in [
        "x".repeat(MAX_LINE_BYTES - overhead + 1),
        format!("{escaped}x"),
    ] {
        let mut writer = Writer::new(Sink {
            chunk: 65536,
            ..Sink::default()
        });
        assert!(matches!(
            writer.write_event(&text_result(text), CONTEXT),
            Err(EncodeError::TooLong)
        ));
        let sink = writer.into_inner();
        assert!(sink.bytes.is_empty());
        assert_eq!(sink.flushes, 0);
    }
    phase_output_limit();
}
fn phase_output_limit() {
    let mut result = ResultEvent::new(Span { start: 0, end: 0 });
    result.binary = Some(vec![Descriptor::Phase {
        id: "p".into(),
        body: Json::Null,
        points: vec![PhasePoint { i: -1000, q: 1000 }; MAX_LINE_BYTES / 13 + 1],
    }]);
    let mut writer = Writer::new(Vec::new());
    assert!(matches!(
        writer.write_event(&Event::Result(result), CONTEXT),
        Err(EncodeError::TooLong)
    ));
    assert!(writer.into_inner().is_empty());
}

struct BrokenRead;
impl Read for BrokenRead {
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
        Err(io::ErrorKind::Other.into())
    }
}
#[test]
fn io_failures_are_structured_and_fatal() {
    let mut reader = Reader::new(BufReader::new(BrokenRead));
    assert!(matches!(reader.read_command(), Err(DecodeError::Io(_))));
    assert!(matches!(reader.read_command(), Err(DecodeError::Ended)));
    for sink in [
        Sink {
            fail_write: true,
            ..Sink::default()
        },
        Sink {
            chunk: 3,
            fail_flush: true,
            ..Sink::default()
        },
        Sink::default(),
    ] {
        let mut writer = Writer::new(sink);
        assert!(matches!(
            writer.write_command(&Command::Stop),
            Err(EncodeError::Io(_))
        ));
    }
}

#[test]
fn invalid_locally_constructed_messages_write_nothing() {
    let commands = [
        Command::SetSpeed(Speed::Numeric(0.0)),
        Command::SetSpeed(Speed::Numeric(f64::NAN)),
        Command::Tune(f64::INFINITY),
        Command::Start(Start {
            recording: "".into(),
            module: "m".into(),
            params: Parameters::new(),
            freq: 0.0,
            speed: Speed::Max,
            paused: false,
            output: "out".into(),
        }),
    ];
    for command in commands {
        let mut writer = Writer::new(Vec::new());
        assert!(matches!(
            writer.write_command(&command),
            Err(EncodeError::Invalid(_))
        ));
        assert!(writer.into_inner().is_empty());
    }
    let mut result = ResultEvent::new(Span { start: 0, end: 0 });
    result.binary = Some(vec![Descriptor::Phase {
        id: "p".into(),
        body: Json::Null,
        points: vec![PhasePoint { i: 1001, q: 0 }],
    }]);
    let mut writer = Writer::new(Vec::new());
    assert!(matches!(
        writer.write_event(&Event::Result(result), CONTEXT),
        Err(EncodeError::Invalid(_))
    ));
    assert!(writer.into_inner().is_empty());
}

struct PrefixFailure {
    bytes: Vec<u8>,
}
impl Write for PrefixFailure {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if !self.bytes.is_empty() {
            return Err(io::ErrorKind::BrokenPipe.into());
        }
        self.bytes.extend_from_slice(&bytes[..3]);
        Ok(3)
    }
    fn flush(&mut self) -> io::Result<()> {
        panic!("must not flush after a write failure")
    }
}
#[test]
fn write_failure_after_prefix_and_flush_failure_after_full_line() {
    let mut writer = Writer::new(PrefixFailure { bytes: Vec::new() });
    assert!(matches!(
        writer.write_command(&Command::Stop),
        Err(EncodeError::Io(_))
    ));
    assert_eq!(writer.into_inner().bytes, b"{\"c");
    let mut writer = Writer::new(Sink {
        chunk: 3,
        fail_flush: true,
        ..Sink::default()
    });
    assert!(matches!(
        writer.write_command(&Command::Stop),
        Err(EncodeError::Io(_))
    ));
    let sink = writer.into_inner();
    assert_eq!(sink.bytes, b"{\"command\":\"stop\"}\n");
    assert_eq!(sink.flushes, 1);
}

#[test]
fn exact_output_boundary_counts_coexisting_points() {
    let mut result = ResultEvent::new(Span { start: 0, end: 0 });
    result.text = Some(String::new());
    result.binary = Some(vec![Descriptor::Phase {
        id: "p".into(),
        body: Json::Null,
        points: vec![PhasePoint { i: 1000, q: -1000 }; 2],
    }]);
    let overhead = Event::Result(result.clone()).encode(CONTEXT).unwrap().len();
    result.text = Some("x".repeat(MAX_LINE_BYTES - overhead));
    let mut writer = Writer::new(Vec::new());
    writer
        .write_event(&Event::Result(result.clone()), CONTEXT)
        .unwrap();
    assert_eq!(writer.into_inner().len(), MAX_LINE_BYTES + 1);
    result.text.as_mut().unwrap().push('x');
    let mut writer = Writer::new(Vec::new());
    assert!(matches!(
        writer.write_event(&Event::Result(result), CONTEXT),
        Err(EncodeError::TooLong)
    ));
    assert!(writer.into_inner().is_empty());
}
