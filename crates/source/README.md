# dmd-source

Read-only source metadata, independent of the native core, replay archive and
protocol event production. Run `cargo run -p dmd-source --example inspect_source -- capture.wav`.
The executable opens a source and prints actual byte size, `dmd_protocol::Stream`
and exact `u64` frame count.

`Source::open` opens once; `Source::from_file` consumes an opened regular file.
Both inspect that identity. `metadata()` returns the inspection snapshot;
`cursor()` creates reusable independent `Read + Seek` cursors starting at zero,
without reopening a pathname. Dropping Source does not invalidate its cursors.
End-relative cursor seeks use the length observed at cursor creation. Reads use
unbuffered positioned file operations on Unix and Windows.

`inspect(&mut impl Read + Seek)` is the generic inspection boundary. It measures
length by seeking to end, starts at zero and leaves the reader at container end
on success (unspecified on failure). Supply an unbuffered reader: a caller's own
buffering layer could prefetch payload bytes. Ordinary Read/Seek semantics and
stable contents are required. Short/interrupted reads follow `read_exact` behavior.

## Supported profile

| Field | Accepted values |
| --- | --- |
| Container | Little-endian RIFF, form WAVE; declared size equals actual file size |
| Encoding | Integer PCM, format tag 1 |
| fmt body | 16 or 18 bytes; PCM cbSize ignored regardless of value |
| Channels | 1 or 2, both `real`; stereo never implies IQ |
| Container width | 8 or 16 bits |
| Rate | Positive integer, represented exactly as `[sample_rate, 1]` |
| Data | Exactly one chunk after exactly one fmt; empty permitted |

A frame contains one sample per audio channel. Frames are original interleaved
source coordinates, not channel samples or converted/native coordinates. Block
alignment and byte rate must agree exactly; byte rate arithmetic must fit u32.
Partial frames are rejected. Frame count excludes headers, ancillary data and
padding. RIFX, RF64/BW64, extensible, floating/compressed formats, other widths,
other channel counts, raw/headerless and IQ sources are unsupported. Invalid magic
is malformed identification. There are no hints, rate overrides, repairs or fallback.

All top-level headers, bodies and odd-size padding extents must fit both actual
length and RIFF boundary. Ancillary chunks anywhere are opaque, including apparent
embedded signatures. Pad byte values are unconstrained. Inspection reads only the
12-byte container header, 8-byte chunk headers and 16/18-byte fmt body. It seeks
across all payloads and padding, allocating no memory from declared sizes.
`MAX_CHUNKS` is 65,536, including fmt/data. Equality succeeds; an additional chunk
returns `Resource::ChunkBudget` before reading its header or scanning the remainder.
Payload contents are skipped, irrespective of their size.

## Failures and caller obligations

Opening or measuring a handle and required read/seek failures return `Error::Io`
with the original I/O error; unexpected EOF while reading headers is
`Malformed::Truncated`. Identification, extents, order, duplicates, missing chunks,
inconsistent fields and arithmetic failures have typed `Malformed` reasons.
Excluded profiles have typed `Unsupported` reasons; budget exhaustion has a typed
`Resource` reason. An earlier failure takes precedence over later defects.

An opened identity survives pathname replacement on supporting platforms, but does
not freeze externally modified contents. Callers must ensure stability throughout
inspection and subsequent use, and handle later I/O errors. Skipped bytes are not
read-verified. This API neither hashes nor authenticates the source.
Pass `source.metadata().frames` to `dmd_replay::Recording::open` or `from_reader`;
replay/protocol contracts and their independent versions remain unchanged.

## Provenance and license

Adapted from public MIT Argand `crates/io/src/riff.rs` and `shared_file.rs`, revision
`520c933531f1710ecbe7962d34224855142c5b72`. The fmt field-decoding structure,
word-aligned RIFF walk and retained Arc<File> positioned independent cursors reuse
actual source code. Traversal now validates the whole container with checked u64
extents and bounded unbuffered header reads. Argand's stereo IQ mapping, broader
formats, clipping, metadata overrides and floating rates are deliberately excluded.
Copyright (c) 2026 Oleg Kosyankov. MIT permission and warranty notice: [LICENSE](LICENSE).
