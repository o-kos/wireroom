
Run `cargo run -p dmd-replay --example inspect -- recording.replay source_frames`
for an executable inspection example using caller-supplied original source frames.
It reports boundaries on stderr and streams assets without protocol output.

`dmd-replay` implements only recording format 1's read-only data boundary.
Metadata and the bounded ZIP index are validated at setup, together with the
journal local header. Setup retains one opened archive identity and does not scan
journal content or decompress unused assets/extensions.

```no_run
use dmd_replay::{Recording, Record};
use dmd_protocol::{Event, Descriptor};
# fn inspect() -> Result<(), dmd_replay::Error> {
// The source reader, owned by the caller, independently derived eight frames.
let recording = Recording::open("example.wav.replay", 8)?;
let metadata = recording.metadata();
let overrides = [("mode".into(), "fec".into())].into_iter().collect();
metadata.match_initial("demo", 0.0, &overrides)?;
let mut journal = recording.journal()?;
while let Some(record) = journal.next_record()? {
    match record {
        Record::Boundary { from, to } => {
            // This closes one validated processing block, even when from == to.
            let _source_advance = to - from;
        }
        Record::Finalization => { /* separate recording transition */ }
        Record::Event(Event::Result(result)) => {
            // Prefix records are provisional until their closing Boundary.
            for descriptor in result.binary.iter().flatten() {
                if matches!(descriptor, Descriptor::File { .. }) {
                    let mut asset = recording.asset(descriptor)?;
                    let mut buffer = [0; 8192];
                    while asset.read_chunk(&mut buffer)? != 0 {
                        // Consume bytes without publishing output files.
                    }
                    assert!(asset.verified());
                }
            }
        }
        Record::Event(_) => {}
    }
}
// Only complete EOF certifies journal structure, decoded size and CRC.
# Ok(())
# }
```

Journal reads validate complete envelopes, producer JSON, replay depth, paths and
prefix/suffix structure incrementally. Prefix results/logs returned before the
closing progress are provisional: later logs or the closing frontier can invalidate
the block. Callers needing complete blocks must spool or buffer them with their own
explicit resource budget. Validation/lookahead does not submit source frames.
Unknown kinds are validated and skipped; their apparent assets are never opened.
`Journal::set_cancellation` installs a shared atomic flag checked between records
and at buffer refills; cancellation returns `Error::Cancelled` and poisons the reader.

Payload reads check exact unchanged paths, declared descriptor size, local headers,
consumed range overlap, decompression, actual decoded size and CRC. Journal decoding
does not verify or materialize referenced files. Only full asset EOF certifies bytes;
dropping a partially read asset or journal performs no mandatory scan. Failures poison
a reader. Unseen contents remain uncertified, including when the last frontier was N.
The archive identity is retained; callers must prevent concurrent modification of its
contents if they require a stable snapshot.

The portable limits in `docs/replay.md` apply. Central directory memory is bounded
by 64 MiB; entry/reference maps by 65,536 names; streaming buffers by 8 KiB per
reader; metadata and each journal line by 16 MiB, plus their parsed exact JSON trees.
Archive setup also has an implementation work budget of 64 MiB of aggregate
central-directory bytes read and parsed across all EOCD candidates. A single
candidate can use the full portable 64 MiB directory limit. Opaque comments can
require retrying candidates; exhausting the aggregate budget fails explicitly with
`Error::Resource` before reading another candidate directory. The budget does not
change the format's per-directory limit, and a profile-valid archive with many
ambiguous comment candidates may exceed this implementation budget. The same
last-22-byte candidate is never validated twice during fallback.

JSON has at most 128 containers on a nesting path. No whole journal/block buffer,
compression-ratio heuristic, or extraction directory is used. Allocation failure
at the process allocator remains platform-defined; explicit format budgets fail
with `Error::Resource` before trusting unchecked declared sizes.

Callers own signal opening/header decoding, original-frame context, digest computation
and its diagnostic outcome, runtime lifecycle/controls/pacing and protocol output.
The recorded digest is validated metadata, not authentication or certification of a
matching signal or successful playback. Output path checks, exclusive publication,
close-before-emission, reuse and published-file lifetime belong to the caller.
The [CLI runtime](https://github.com/o-kos/wireroom/blob/main/crates/cli/README.md)
uses portable staging under caller-selected output whose ancestry and contents
remain stable during the run. Archive matching always uses exact UTF-8 names;
filesystem alias collisions must fail rather than overwrite or reuse a different
asset. The reader rejects known Windows syntax/device/component aliases on Windows.
This library never writes into `start.output`.
