# Run protocol API

`dmd-protocol` implements the individual-message boundary in `docs/protocol.md`.
`CURRENT_VERSION` is protocol 1.0; it is independent of the package version,
recording version and C ABI. The crate does not implement a running process.

`Command` and `Event` represent every known wire message. `Started` contains a
`Snapshot`; `ResultEvent` contains independent optional payloads and an `extra`
object for producer keys, including `record`. `Descriptor::Phase` carries encoded
integer points; `PhasePoint::from_source` explicitly quantizes source components.
`Descriptor::File` supports every other nonempty producer type.

```rust
use dmd_protocol::{Command, Context, DecodedEvent, Event, Reader, Writer};
use std::io::Cursor;

let mut reader = Reader::new(Cursor::new(b"{\"command\":\"pause\"}\r\n"));
assert_eq!(reader.read_command()?, Some(Command::Pause));
assert_eq!(reader.read_command()?, None);

let context = Context { frames: Some(8) };
let decoded = Event::decode("{\"event\":\"progress\",\"frame\":8}", context)?;
assert_eq!(decoded, DecodedEvent::Known(Event::Progress { frame: 8 }));
let mut writer = Writer::new(Vec::new());
writer.write_event(&Event::Progress { frame: 8 }, context)?;
assert_eq!(writer.into_inner(), b"{\"event\":\"progress\",\"frame\":8}\n");
# Ok::<(), Box<dyn std::error::Error>>(())
```

`Command::decode` and `Event::decode` parse a single object without a transport
terminator. Commands and hello are strict; other events ignore extension fields.
`DecodedEvent::Unknown` is an ignorable event name, not a runtime event. Every
input value, including unknown events and ignored fields, is syntax-checked and
checked for duplicate decoded keys before shape validation. Missing, null and
wrong-type fields remain distinct from opaque JSON nulls.

`Json` uses serde's ordinary JSON parser/serializer with a small duplicate-key
visitor. Numbers are i64/u64 integers or finite f64; numeric spelling and arbitrary
precision are not preserved. Strings, array order and ordinary producer values
remain intact. Integer fields reject fractional/exponent values and retain exact
u64 coordinates, including above binary64's exact integer range. `Json::parse`
accepts any JSON root; message decoding requires an object discriminator.

`validate` and `encode` check locally built messages as well as decoded ones.
`Context::default()` checks shape and local values; `Context { frames: Some(n) }`
additionally checks source bounds and normal-end equality. Started always requires
frame zero. Context never infers or tracks the current frontier. Empty, overlapping
and repeated spans and frontiers are valid individual values.

`Reader<R: BufRead>` preserves partial lines across reads, including split UTF-8
and CRLF. Each call returns one complete message, clean EOF (`Ok(None)`), or a
structured `DecodeError`. `is_complete_line_rejection()` identifies a malformed
complete command line that the caller can reject nonfatally and then continue.
For events, every malformed known message, framing failure or UTF-8 error is a
protocol failure. Unknown event names can be ignored. `Incomplete` means an
unterminated final line that must never execute; the caller rejects it and applies
its own EOF handling. `TooLong` and `Io` are fatal. After these failures or
incomplete EOF, further reads return `Ended`; the reader does not drain oversized
input. No error sends an acknowledgement or terminates a process.

`Writer<W: Write>` checks validity and encoded length before any bytes are written,
then writes one compact UTF-8 object and LF and flushes. `EncodeError::Invalid` and
`TooLong` write nothing. `Io` may follow a partial write or a failed flush; the
caller must treat that output stream as unusable. No result splitting occurs.
Short writes and interrupted reads/writes follow Rust's standard I/O behavior.

The transport limit is 16,777,216 bytes excluding LF/CRLF, with equality accepted.
Input buffering retains at most that many payload bytes and one provisional CR;
the caller's `BufRead` buffer is separate. Output is fully buffered before writing,
so escaping and all inline points count toward the limit. Parsed JSON and typed
values allocate additional memory proportional to their content; the wire-byte
limit is not a total heap budget. Allocation exhaustion follows the Rust allocator
behavior. Standalone message/JSON parsing and string encoding APIs do not enforce
the transport byte limit; use `Reader` and `Writer` for bounded transport.

Parsing uses serde_json's default recursion guard; locally built JSON also checks
the conservative `MAX_DEPTH` budget of 127 containers before serialization. This
budget is independent of replay archive limits.

Callers still own lifecycle ordering, FIFO acknowledgement tracking, monotonic
frontiers, equality with the current frontier, command execution, finalization,
draining, EOF-as-stop, concurrent stderr draining, process termination and exit
status. Individual validation cannot certify a successful run.

`validate_result_path` is lexical only: it never rewrites, normalizes, case-folds,
percent-decodes or interpolates an identifier. Launch paths have separate nonempty,
NUL-free rules and may be absolute. Callers must enforce filesystem containment
without check/open races or unsafe indirections, regular-file status, on-disk size,
publication before emission, no overwrite and the promised file lifetime. A valid
lexical path proves none of these filesystem guarantees.
