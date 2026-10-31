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
    for chunk in [1, 7] {
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
