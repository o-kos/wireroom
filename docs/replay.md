# Replay recording contract

This document specifies recording format **1.0**, for replay through
[run protocol 1.0](protocol.md). It defines data and behavior. The [runtime guide](https://github.com/o-kos/wireroom/blob/main/crates/cli/README.md)
documents the implemented CLI, supported configurations and additional budgets. One recording represents one normally completed run with fixed
results. Recordings of stopped or failed source runs are outside version 1.
The recorded effective tuning and parameters are fixed throughout that source run;
runs that changed them are outside version 1. Replay reads source metadata without
a native signal core and never decodes again.
`wr` launches the selected `dmd` executable; replay and core are separate
implementations of the application.

The general result-directory layout, discovery, recording tools, C ABI and producer
schemas are outside scope. Masks, module info, ALE, result `params`, binary `body`
and other producer values retain their producer-defined interpretation. This
contract does not establish complete cross-vendor producer interoperability.

## Recording set and versions

The entry point is `start.recording`, a signal file. Append the literal suffix
`.replay` to its complete pathname to find its companion ZIP: `signal.wav` uses
`signal.wav.replay`, not `signal.replay` or `signal.wav.replay.zip`. Both files are
required. Relative launch paths have the protocol's working-directory meaning.
There is no fallback search or alternate processing implementation on failure. The archive contains
all replay assets; it must not refer to another external file besides this signal.

Read byte size, real/IQ format, channels, rational frame rate and total frames from
the signal itself. None is duplicated in archive metadata. A supported source
reader must derive these unambiguously, without loading demodulation modules;
unsupported, inconsistent or unreadable source headers fail setup. This format
does not prescribe a new signal encoding or make every signal encoding supported.
All positions are original-source frames, with one I/Q pair per channel in an IQ
frame. No conversion changes their units. Source rate is frames per second.

`metadata.json` identifies both the recording version and the protocol under which
the journal was written. These are independent of each other and of the C ABI.
Each version object is exactly `{"major":1,"minor":0}` in this revision: `major`
is a positive `u32`, `minor` a `u32`. A reader supporting recording major M must
understand majors 1 through M, select rules by major and ignore minor for
compatibility. Reject zero or unsupported majors before `started`. The journal
protocol major must also be supported and have a defined mapping to the emitted
protocol. This revision defines only protocol major 1 to emitted major 1. A reader
must reject an unsupported pair rather than guess a translation. The emitted
`hello` reports the implementation's run protocol, never the recording version;
there is no second process handshake.

Minor recording revisions may add optional fields, ignorable journal kinds and
extension assets, without altering existing requirements or playback meaning.
Unknown fields in known recording objects are ignored, except version objects have
only their two fields. Unknown journal kinds are skipped as described below.
Unknown producer fields are preserved, not discarded. A change requiring new
semantics, a new required field, a different boundary interpretation or an
incompatible protocol mapping requires a new recording major.

## ZIP layout and metadata

ZIP names are case-sensitive UTF-8 names with the UTF-8 flag set; reject invalid
UTF-8. Entries are regular files, with no explicit directory entries. Order in the
ZIP has no meaning. The exact version-1 namespace is:

| Entry | Requirement and contents |
| --- | --- |
| `metadata.json` | Exactly one UTF-8 JSON object, without BOM. |
| `journal.ndjson` | Exactly one UTF-8 NDJSON journal, without BOM. |
| `payloads/<path>` | Raw payload bytes for a non-phase descriptor whose protocol `path` is `<path>`. No JSON wrapper or base64. |
| `extensions/<path>` | Optional opaque extension assets; a version-1 reader ignores their contents. |

No other names are permitted. Every `<path>` follows the path rules below.
Unreferenced payload entries are allowed and need not be read. An extension must
not be necessary to interpret a version-1 recording. Reject duplicate entry names,
even identical duplicates, and file/directory prefix conflicts such as `payloads/a`
and `payloads/a/b`. Do not silently choose one central-directory entry.

Metadata has the following required fields; no optional version-1 fields are
currently defined. Protocol common types apply, including exact integer tokens and
finite binary64 numbers. Duplicate object keys at every nesting level are invalid.
JSON whitespace, including a final newline, is allowed around the metadata object;
additional JSON values or trailing non-whitespace are invalid.

| Field | JSON type and meaning |
| --- | --- |
| `format` | Exact string `"dmd-replay"`. |
| `version` | Recording version object. |
| `protocol` | Stored run-protocol version object. |
| `signal_sha256` | String of exactly 64 lowercase hexadecimal digits: SHA-256 of every byte of the signal file, including its headers. |
| `module` | Nonempty identifier string, never a path. |
| `info` | Complete producer module-info JSON value, including its mask and parameter definitions, with no selective omission. Its shape is deferred. |
| `mask` | Producer mask object used by this run, copied without restructuring for `started.mask`. |
| `defaults` | Parameter map: object of string values giving the module defaults used when resolving initial overrides. |
| `freq` | Recorded finite tuning frequency in Hz in the source frequency coordinate system. |
| `params` | Parameter map of the full actual settings used for the recorded run, including settings equal to defaults. |

`mask` and `defaults` are explicit replay bindings, not a universal schema imposed
on `info`. A recording producer knows how to obtain them from its complete info
and selected module. Store the full info even though the replay mask and defaults
are also provided. The mask must be the same producer value used for the recorded
run; defaults must agree with the producer's parameter definitions. Do not invent
JSON paths into arbitrary info or infer defaults from actual settings. A generic
reader can validate the binding types but cannot validate opaque producer schemas;
a producer-specific reader that detects a contradiction must reject it.
Every default name must occur in `params`; a parameter without a default can occur
only in `params` and must be supplied explicitly at start. The actual value may
differ from its default. Neither replay bindings nor deferred schemas permit
converting parameter strings to numbers or replacing producer JSON values.

Preservation means JSON value and array order, exact integers, strings (including
text fragments), result order and raw payload bytes. It does not require identical
whitespace, object-member order or equivalent number spelling. Do not round
integers through binary64. Opaque JSON numbers must retain their exact value;
re-encoding cannot silently lose precision. No `null` substitutes for required
objects, strings or maps; `info` and opaque producer values may themselves be null
if that is what the producer supplies.

## Journal framing and boundaries

Each record is one JSON object on one line terminated by LF; CRLF is also accepted.
The last record also requires a terminator. Blank lines, BOM, invalid UTF-8,
duplicate keys, incomplete final lines, non-object values and trailing bytes on a
line are fatal. Embedded newlines in strings are escaped. The limit is 16,777,216
uncompressed bytes per line, excluding LF or CRLF, exactly as on the run protocol.
Readers handle arbitrary decompression/read boundaries. All JSON, including ignored
fields and kinds, must be syntactically valid within the limit.

Dispatch a journal record by `event` whenever that key is present; its value must
be a string. Only when `event` is absent does `record` act as the service
discriminator, and its value must then be a string. A record with neither key or
with an invalid discriminator type is fatal; an invalid `event` never falls back
to `record`. Known `event` values are `result`, `log` and `progress`, using their
protocol JSON forms, required types, units and constraints. Unknown optional
fields follow protocol compatibility; unknown result keys are producer data and
must be retained. In particular, a result's `record` may have any producer JSON
value, including `"finalization"`, and is preserved on stdout without triggering a
service transition. Known lifecycle/control events (`hello`, `started`, `state`,
`error`, `finished`) are forbidden in the journal, not ignored. An unrecognized
`event` string, or an unrecognized `record` string when `event` is absent, is an
ignorable extension record. It cannot count as a block boundary, replace a
required record, advance time or create an asset reference. Writers must not hide
necessary run data in an extension kind.

The single version-1 service record is:

```json
{"record":"finalization"}
```

The marker has no `event` key. It is never emitted on stdout. Exactly one is
required, even for an empty source.
It marks entry into the original normally completed run's finalization. EOF closes
the journal; there is no stored terminal event. Missing or repeated markers are
fatal when detected. Unknown fields on the marker are ignored.

The prefix before the marker is a sequence of processing blocks. Each block is
zero or more `result`, `log` or ignored extension records followed by exactly one
`progress`. Thus a progress-only block is valid. The marker occurs at a block
boundary: no unterminated block may precede it. Extension-only records between
boundaries do not themselves require a progress. Logs/results before the first
progress belong to that first block. There is no block ID or fixed block length.

Let N be the signal's frame count and F the last completed progress frontier,
initially 0. Each prefix block ends with a frontier G satisfying `F <= G <= N`.
Repeated frontiers are real boundaries and must not be merged or skipped. All
stored log frames in that block lie between F and G inclusive and are
nondecreasing, followed by progress G. A log may describe the frontier before
submission or after submission; it must not move the reported frontier backwards.
Results require `0 <= samples[0] <= samples[1] <= N`; their spans need not be
monotonic, disjoint or bounded by that block's frontier. A delayed result with an
earlier or overlapping span is not reordered or paced by its span.

For N > 0, the prefix must include a progress at N before the marker. It may have
additional blocks with frontier N, representing buffering without source advance.
For N = 0 the prefix can be empty, or contain any number of blocks at frontier 0.
All result spans in an empty source are `[0,0]`. The suffix after the marker is
finalization output: results, logs, repeated `progress` at N for draining submitted
buffers, and ignored extensions, in stored order. Every suffix log frame is N.
Suffix results still use source coordinates and may overlap earlier results.
Suffix records need no final progress. A decreasing or out-of-range frontier,
a marker before source exhaustion, or a non-N suffix progress is fatal.

## Stored data and generated events

| Data | Replay behavior |
| --- | --- |
| Signal header and length | Derive `started.stream` and `started.frames`; do not substitute archive or converted-stream properties. |
| Complete `info`, module, mask, defaults, frequency, actual parameters | Retain info; select and validate the fixed run; generate `started` with the preserved mask and full effective parameters. No module-info event is added to the run protocol. |
| Journal results | Emit in producer order, including all fragments, overlapping spans, producer `log`, ALE, result `params`, binary descriptors and unknown producer keys. |
| Journal logs and progress | Emit their protocol forms in journal order; every stored progress remains a block boundary. |
| Finalization marker | Internal lifecycle transition only; never a protocol event. |
| File assets and inline phase | Materialize files before their descriptors; retain inline points without re-quantization, normalization, clipping or thinning. |
| Original lifecycle, controls, timing, stderr | Not stored. Generate the current invocation's `hello`, `started`, acknowledgements and single `finished`; do not reproduce original machine delays or raw stderr. |
| Background verification diagnostic | Generate a `log` at `warning` level if needed, after `started` and before `finished`; it may interleave without changing the relative order of stored entries. |

## Settings, pacing and controls

For a syntactically valid `start`, compare `module` with the recorded identifier and
`freq` numerically with the recorded finite frequency. Do not clip or accept a
frequency tolerance. Form the effective parameter map by copying `defaults` and
replacing/adding each `start.params` member. Require exactly the same names and
string values as recorded `params`, independent of member order. Any difference,
unknown name or unsupported value fails setup, before `started`. This also means
an empty override map fails if actual settings differed from defaults. A reader
does not need to implement a universal parameter-definition schema to compare the
fixed map. Initial speed and pause state are freely selected and not recorded.

The protocol's command syntax, rejection rules and FIFO handling apply unchanged.
The following table gives the replay-specific behavior; every successful ordinary
control emits a full `state` snapshot, even a no-op.

| Input | Waiting for start | Playing or paused | Finalizing |
| --- | --- | --- | --- |
| `start` | Validate and emit `started` at frame 0, or fatal setup error and `finished(error,0)`. | Nonfatal error: already started. | Nonfatal error: finalizing. |
| `pause` | Nonfatal error. | Finish any submitted block through its progress, then acknowledge paused; if already paused, acknowledge immediately. | Nonfatal error. |
| `resume` | Nonfatal error. | Acknowledge playing and allow the next block; already playing is a no-op. | Nonfatal error. |
| `set_speed` | Nonfatal error. | Accept positive finite numeric speed or `"max"`, acknowledge, preserve pause state. | Nonfatal error. |
| `tune` | Nonfatal error. | Equal recorded frequency: acknowledge unchanged state. Any change: nonfatal error with no state change. | Nonfatal error. |
| `set_params` | Nonfatal error. | Atomically patch current map; accept only if the full map remains identical to recorded `params`. An empty patch succeeds. Otherwise reject the whole patch with a nonfatal error. | Nonfatal error. |
| `stop` | `finished(stopped,0)` without opening either recording file. | Finish any submitted block through its progress; submit no next source-advancing block. While paused, begin terminal draining immediately. See exhaustion rule below. | Keep finalizing; share the one terminal event. |
| stdin EOF | Process preceding complete commands, then act as stop. | Same as stop, including while paused. | Keep finalizing. |

Malformed complete command lines produce nonfatal errors; unterminated input at
EOF is rejected before EOF acts as stop. Oversized input or read failure is fatal.
A successful no-op tune/patch does not decode again, retune a core or regenerate
results. A rejected change never changes frequency, parameters or playing state.

Pace blocks using source advancement only. For a block from F to G at numeric
speed s and source rate `[a,b]`, its nominal time budget is
`(G - F) * b / (a * s)` seconds. Use exact frame arithmetic without integer overflow
and a monotonic clock. Delay before submitting that block, then emit its entries
in order; do not sleep separately for its results, logs or file writes. A reader
may look ahead into the block with bounded buffering or spooling to find G. That
lookahead does not submit it or advance the current frontier. Initially paused
runs submit nothing until resumed, including on an empty source.

Maintain the remaining source-time budget for the pending block. Time spent paused
never consumes it. On a speed change, account for elapsed playing time at the old
speed, then consume the remainder at the new speed. `"max"` removes the remaining
deliberate wait; a later numeric speed applies to still-unconsumed source time.
Repeated-frontier blocks have zero budget but remain separately controllable
boundaries. Finalization has no deliberate pacing. File I/O, decoding the journal
and pipe backpressure can make replay slower than requested; speed 1 is source-time
pacing without a throughput guarantee, not reproduction of original processing time.
Do not accumulate an intentional additional wait for an already elapsed budget.

Read and handle commands while waiting on deadlines or paused; never use one
uninterruptible sleep for a block. Apply controls serially between submitted
blocks, acknowledge before submitting the next, and check pending commands at
zero-duration boundaries too. Background work and bounded I/O must not starve
control handling. Once submitted, a block's entries through progress form the
in-flight work: pause/stop can wait for it, but cannot discard or split it.
A stop during the wait before submission cancels that pending block. No pacing
wait is required just to honor a stop. stdout backpressure can delay delivery.

Replay block submission means committing to this journal unit, not calling a
signal core. Within that unit the virtual submitted source frontier starts at F,
advances to each stored log frame immediately before emitting that log, and reaches
G immediately before its progress. Results do not themselves advance it. This
preserves the protocol meaning of a log's current frame even for a leading log at
F before the block advances. Generated diagnostics and a fatal terminal event use
that current virtual frontier; ordinary control snapshots occur after the whole
block and therefore report G. Background lookahead never advances this frontier.

### Exhaustion and terminal semantics

At normal exhaustion, consume the marker and drain its suffix exactly once, then
emit `finished` with reason `end` and frame N and exit 0. An empty, initially
playing source also emits `started` first, then any zero-frontier blocks, marker,
suffix and terminal event. A paused empty source waits for resume or stop.

On early stop with frontier F < N, emit no unsubmitted blocks or full-run suffix;
finish with reason `stopped`, frame F and exit 0. This fixed recording has no
separate stopped-run drain to synthesize. Completed blocks' results remain valid.
If stop has already been accepted, completing its in-flight block at N still
uses reason `stopped`, but reaching N permits draining the remaining recorded
N-frontier blocks and full-run finalization suffix without pacing. A paused stop
at N (including N = 0) likewise drains this end-of-source output immediately,
without a resume. At F < N, paused stop has no further recorded drain.

If normal finalization has already begun, a later stop or EOF keeps reason `end`.
For replay, normal finalization begins when the marker is consumed; an accepted
stop begins stopped finalization and fixes that reason first. Commands actually
read during either finalization use the table's last column: non-stop requests
get nonfatal errors in read order, repeated stops share `finished`. Terminal
completion supersedes commands not yet read/processed; clear their pending queue.
A fatal failure can change either terminal reason to `error`.

After a fatal error, emit `finished(error,F)` directly after that error when stdout
works, exit nonzero and promise neither successful draining nor complete results.
F is the current virtual submitted frontier defined above, including any
advancement recorded by a log in an unfinished block, not a result span endpoint.
Setup failures use 0. Exactly one `finished` is the last stdout event. Broken stdout or
a crash may prevent it; report to stderr if possible and exit nonzero. No events,
including verification warnings, are emitted after `finished`. Consumers still
wait for process exit and treat nonzero exit or missing terminal completion as
failure, following the protocol.

## Assets, containment and resource limits

A non-phase descriptor's `path` is the protocol output-relative path. Its archive
asset is exactly `payloads/` followed by that path: `files/a.bin` maps to
`payloads/files/a.bin`. `id` is opaque and is never a path construction instruction.
Require descriptor `size` to be a `u64`, match the ZIP uncompressed size, and match
the actual decoded byte count. Preserve the descriptor path; never silently rename
it to avoid a collision. All non-phase types use this rule, not only `"file"`.
Phase descriptors contain integer points in [-1000,1000], and neither `path` nor
`size`; other types have no `points`. Validate the protocol envelope even though
binary `body` and type-specific producer schemas remain deferred.

For all ZIP names and descriptor paths, reject leading slash, backslash, colon,
NUL, empty components, `.` and `..`. Reject absolute, drive, UNC and URL forms.
Do not percent-decode, normalize Unicode or case-fold names to find assets.
Matching is exact. Before filesystem use also reject names that the target platform
cannot represent safely, including reserved names, trailing-character aliases or
collisions under its filename equivalence rules. A reader must fail rather than
materialize a different name. These checks apply to ignored ZIP entries too.

Use a single opened archive and signal identity during a run; do not resolve their
pathnames afresh for each read. A reader must not follow archive entries as
filesystem links. Reject symbolic links, device nodes, sockets, FIFOs, reparse-like
entries and any declared non-regular entry. Entries without Unix mode attributes
are ordinary files if their remaining ZIP attributes do not declare another kind.
For consumed entries, validate local headers against the central directory, bound
compressed ranges within the archive, reject overlapping consumed entry ranges,
and validate decompression, decoded size and ZIP CRC-32 through entry EOF. Do not
trust only the central-directory size or a descriptor's size. An invalid archive
structure is fatal, not a SHA warning.

Version 1 accepts only single-disk, unencrypted ZIP using stored (method 0) or
Deflate (method 8) entries. ZIP64 and data descriptors are allowed; ZIP64 counts
and offsets must be checked without truncation. Reject unsupported methods,
encryption or multi-disk layouts during index setup. ZIP metadata extra fields
are not replay extension fields: ignore well-framed unknown ZIP extras, reject
malformed ones. No archive extraction is needed to read metadata or journal.

The following hard limits define the portable version-1 profile. Count bytes after
decompression unless specified otherwise. Equality with a limit is permitted.

| Resource | Limit |
| --- | --- |
| Archive byte size | 1 TiB (1,099,511,627,776 bytes). |
| Central directory size | 64 MiB (67,108,864 bytes). |
| Number of entries | 65,536, including extensions. |
| UTF-8 entry name or descriptor path | 4,096 bytes each; archive prefix counts for an entry name. |
| Path depth | 64 components, including archive prefixes. |
| `metadata.json` | 16,777,216 bytes. |
| Journal line | 16,777,216 bytes, excluding terminator. |
| JSON nesting | 128 object/array levels, counting the root as level 1. |
| Any single entry, including journal | 64 GiB (68,719,476,736 bytes). |
| Sum of declared uncompressed entry sizes | 1 TiB, including ignored entries. |

Enforce declared limits at index/setup time where available and decoded limits
incrementally; never allocate directly from an unchecked ZIP size or JSON count.
No compression-ratio heuristic is needed: bound actual decompressed bytes, memory,
lookahead spooling and disk usage. Stream or spool blocks rather than requiring
an entire block or journal in RAM. An implementation may have smaller explicit
resource budgets (including output space or spool space); exhaustion is a fatal
resource failure at the point detected, not silent truncation. It must document
those budgets, not claim every profile-valid recording will run with any resources.

The caller-selected output directory and its ancestry are trusted and stable
throughout the run. Local processes relocating directories or mutating output are
outside the contract. Archive paths and bytes remain untrusted. Validate the output
root during setup. Under output, reject evident symlink/junction or non-directory
payload ancestors and existing destinations, even if existing bytes match.
Ordinary filesystem checks suffice under the stability assumption; do not silently
rewrite a descriptor path to work around a conflict.

Stream each required asset into an exclusively owned staging file under its
destination parent, verify exact decoded size and CRC, finish writing and close the
writing handle, then publish at the exact path without overwriting any destination.
Do not emit a result until all its referenced regular files are ready. Reuse only
the same exact previously materialized archive asset and size, without rewriting;
reject conflicting references. A filesystem alias to another path does not permit
reuse. These requirements do not imply crash durability or an atomic multi-file
transaction.

Keep published files unchanged through and after exit until the caller removes or
modifies output. A failed result may leave files already published before its
failure. Cleanup removes only invocation-owned unpublished temporary files; never
clear output or recursively delete an arbitrary directory. Failure to prepare
output or perform exclusive publication is an explicit error.

## Lazy validation and failure classification

Before `started`, open the signal and companion, derive source metadata, inspect
the bounded ZIP index and its names/types/declared limits, read and validate all
metadata including its CRC, check version compatibility and settings, open the
journal for streaming, and prepare output access. Index enumeration is
required structural work; it is not a scan/decompression of every archive member.
Do not scan the whole journal, extract all payloads or hash the whole signal as a
startup prerequisite. Content/local-header checks for unopened payloads are lazy.

After `started`, validate each journal line and ordering before emitting its data,
and each required asset before its descriptor. Validate a whole result and all
its descriptors before emitting that result. Lookahead can discover a fatal
problem sooner than emission would; failure occurs when detected. Validate journal
CRC and uncompressed length at journal EOF before declaring normal completion.
An early stop need not inspect unread journal content, unused payloads or the
missing marker in an unread suffix. It does not certify the full recording valid.
No requirement for lazy access permits ignoring invalid data already encountered.

Verify SHA-256 of the opened signal's entire contents in background after startup,
without blocking controls. A mismatch emits a generated warning log and continues.
An I/O error affecting only hashing, inability to obtain a stable digest (including
signal modification detected during hashing), or inability to finish before the
run terminates also emits a warning. Do not delay terminal completion just to
finish hashing; cancel/join the worker and emit a warning before `finished` if no
conclusive result is available. A matching completed digest needs no event. Emit
at most one verification outcome warning per invocation. Use the current submitted
frontier as its `frame`. Do not schedule a late worker write to stdout. Fatal
completion takes precedence: emit any pending warning before the fatal error,
or omit it so that the fatal error is immediately followed by `finished`.

The digest is diagnostic, not authentication or mandatory correspondence proof.
A syntactically invalid `signal_sha256` is malformed metadata and fatal; a valid
but differing digest is only a warning. Required source-header reads and replay
asset reads remain fatal even if an independent background hash also fails.

| Condition | Detection stage and observable behavior |
| --- | --- |
| Missing/unreadable signal or archive, unsupported source, invalid metadata or unsupported version pair | Setup: fatal `error`, `finished(error,0)`, nonzero exit, no `started`. |
| Duplicate/unsafe ZIP name, invalid entry kind, index damage, unsupported compression, declared limit excess | Setup index check: same fatal setup behavior. |
| Module/frequency/effective initial map differs, unusable output root | Setup: same fatal setup behavior. |
| Invalid/missing newline, JSON/type/range error, forbidden stored event, invalid block order, premature/duplicate marker | Fatal when read: before `started` if encountered opening required data, otherwise fatal `error` then `finished(error,F)`, nonzero exit. |
| Journal EOF without marker, unterminated block, journal CRC/size failure or truncated compressed bytes | Fatal when encountered; normal end is never inferred merely from reaching source frame N. |
| Missing referenced asset, decoded size mismatch, CRC/decompression/read failure, unsafe descriptor path, invalid phase points | Fatal when required/decoded, before its result is emitted. Previously emitted results remain valid. |
| Output collision, symlink/non-directory ancestor, detected write/validation/publication failure, resource exhaustion | Fatal when detected; no partial result descriptor and no unrelated overwrite. |
| Invalid live tune/patch or command | Nonfatal command `error`, no state change, continue playback. |
| Valid digest differs or background verification cannot complete | `log` with `level: "warning"`, continue playback; never an unsolicited nonfatal `error`. |
| stdout failure | Stop work, stderr diagnostic if possible, nonzero exit; no promise of terminal delivery. |

There is no repair, dropped result, rewritten unsafe path, inferred marker,
reordered producer output or switch to another implementation on any failure.

## Complete illustrative recording and playback

All producer fields below are illustrative, including module info structure,
parameter definitions, mask, `vendorTag`, `ale` and binary `body`. They do not
standardize producer schemas. The replay metadata envelope and journal boundaries
are normative examples.

The signal `example.wav` is a PCM WAVE file with one real channel, rate 4 frames/s,
8 signed 16-bit little-endian samples, all zero. Its complete 60 bytes are given
here in hexadecimal (spaces/newlines are presentation only):

```text
52 49 46 46 34 00 00 00 57 41 56 45 66 6d 74 20
10 00 00 00 01 00 01 00 04 00 00 00 08 00 00 00
02 00 10 00 64 61 74 61 10 00 00 00 00 00 00 00
00 00 00 00 00 00 00 00 00 00 00 00
```

`example.wav.replay` contains exactly three regular entries, using ZIP method 0
or 8 with correct CRCs. `payloads/files/message.bin` is four bytes: hexadecimal
`54 45 53 54` (ASCII `TEST`, no newline). `metadata.json` is:

```json
{"format":"dmd-replay","version":{"major":1,"minor":0},"protocol":{"major":1,"minor":0},"signal_sha256":"896e28eebf9cd2df518466bc293bc311b6e3e6fd8dd3c44fd37453eb7fe0e813","module":"demo","info":{"name":"demo","mask":{"bandwidth":2},"parameters":[{"name":"mode","default":"raw","choices":["raw","fec"]}],"vendorTag":{"revision":7}},"mask":{"bandwidth":2},"defaults":{"mode":"raw"},"freq":0,"params":{"mode":"fec"}}
```

`journal.ndjson` contains all the following lines, including the final LF:

```json
{"event":"log","frame":0,"level":"info","text":"Recorded run"}
{"event":"result","samples":[0,4],"record":"finalization","text":"CQ ","vendorTag":{"sequence":1}}
{"event":"progress","frame":4}
{"event":"result","samples":[2,4],"bits":"01","log":"producer note","binary":[{"id":"p","type":"phase","body":{"iqType":"complex_float"},"points":[[1000,0],[-500,250]]}]}
{"event":"progress","frame":4}
{"event":"progress","frame":8}
{"record":"finalization"}
{"event":"result","samples":[0,8],"text":"TEST","ale":{"call":"DEMO"},"params":{"confidence":0.9},"binary":[{"id":"message","type":"file","body":{},"path":"files/message.bin","size":4}]}
{"event":"log","frame":8,"level":"info","text":"Recorded finalization complete"}
```

The first result deliberately has both `event:"result"` and
`record:"finalization"`. It remains a result in the first processing block and
retains the producer's `record` on stdout. Only the later record without `event`
is the finalization marker; it occurs after progress reaches frame 8.

Launch `dmd run --message-format json`. **stdout:**

```json
{"event":"hello","protocol":{"major":1,"minor":0}}
```

**stdin:**

```json
{"command":"start","recording":"example.wav","module":"demo","params":{"mode":"fec"},"freq":0,"speed":1,"paused":false,"output":"out"}
```

**stdout**, with the first block submitted after its one-second nominal budget:

```json
{"event":"started","module":"demo","mask":{"bandwidth":2},"stream":{"format":"real","channels":1,"rate":[4,1]},"frames":8,"state":"playing","frame":0,"speed":1,"freq":0,"params":{"mode":"fec"}}
{"event":"log","frame":0,"level":"info","text":"Recorded run"}
{"event":"result","samples":[0,4],"record":"finalization","text":"CQ ","vendorTag":{"sequence":1}}
{"event":"progress","frame":4}
```

Suppose `pause` is read before the next block is submitted. **stdin:**

```json
{"command":"pause"}
{"command":"set_speed","speed":2}
{"command":"tune","freq":0}
{"command":"set_params","params":{}}
{"command":"tune","freq":1}
{"command":"set_params","params":{"mode":"raw"}}
{"command":"set_params","params":{"mode":"fec"}}
{"command":"set_speed","speed":"max"}
```

**stdout**, in FIFO acknowledgement order:

```json
{"event":"state","state":"paused","frame":4,"speed":1,"freq":0,"params":{"mode":"fec"}}
{"event":"state","state":"paused","frame":4,"speed":2,"freq":0,"params":{"mode":"fec"}}
{"event":"state","state":"paused","frame":4,"speed":2,"freq":0,"params":{"mode":"fec"}}
{"event":"state","state":"paused","frame":4,"speed":2,"freq":0,"params":{"mode":"fec"}}
{"event":"error","fatal":false,"message":"Replay frequency is fixed"}
{"event":"error","fatal":false,"message":"Replay parameters are fixed"}
{"event":"state","state":"paused","frame":4,"speed":2,"freq":0,"params":{"mode":"fec"}}
{"event":"state","state":"paused","frame":4,"speed":"max","freq":0,"params":{"mode":"fec"}}
```

**stdin:**

```json
{"command":"resume"}
```

**stdout**, with no deliberate wait at `max`:

```json
{"event":"state","state":"playing","frame":4,"speed":"max","freq":0,"params":{"mode":"fec"}}
{"event":"result","samples":[2,4],"bits":"01","log":"producer note","binary":[{"id":"p","type":"phase","body":{"iqType":"complex_float"},"points":[[1000,0],[-500,250]]}]}
{"event":"progress","frame":4}
{"event":"progress","frame":8}
{"event":"result","samples":[0,8],"text":"TEST","ale":{"call":"DEMO"},"params":{"confidence":0.9},"binary":[{"id":"message","type":"file","body":{},"path":"files/message.bin","size":4}]}
{"event":"log","frame":8,"level":"info","text":"Recorded finalization complete"}
{"event":"finished","reason":"end","frame":8}
```

Before the file-bearing result appears, `out/files/message.bin` is a closed regular
file containing exactly `TEST`; it remains unchanged after exit 0. This exchange
assumes background hashing completed successfully; if it did not, a warning log
would appear before `finished`. `wr` drains stderr and waits for exit too.

### Boundary, invalid-data and control cases

The following are independent changes to the example, not extra records appended
to its successful exchange. Fatal examples assume stdout works.

| Case | Expected acceptance or behavior |
| --- | --- |
| Empty supported signal, N = 0; journal is only the finalization marker plus LF | Valid. Generate `started(frame=0)` then `finished(end,0)` after marker/EOF validation, with no division by zero. Adjust the diagnostic digest for that signal. |
| Empty source with two `progress(0)` blocks, marker, result `samples:[0,0]`, then `progress(0)` | Valid repeated boundaries and empty finalization span. Preserve all three progress events. |
| Example's overlapping `[0,4]`, `[2,4]`, `[0,8]` spans | Valid; preserve order and both phase and file descriptors. Spans do not drive timing. |
| Prefix progress 4, 4, 8, 8, marker | Valid; keep both N-frontier blocks before normal finalization. |
| Progress 4 then 3, frame 9, span `[4,3]`, span ending at 9, or noninteger frame token `4.0` | Fatal when read, even if a result otherwise appears useful. |
| Marker at frontier 4 with N = 8, duplicate marker, prefix result without closing progress, or no marker at full journal EOF | Fatal ordering/structure error; never claim `end`. |
| Suffix progress 7 or suffix log frame 4 for N = 8 | Fatal when read. A suffix result with span `[0,4]` remains valid. |
| Missing LF, truncated UTF-8/JSON/Deflate, duplicate JSON key, invalid points `[1001,0]`, or stored `finished` | Fatal when encountered, before emission of the invalid record. |
| Recording major 0 or 2 on a major-1 reader, unsupported stored protocol major, or malformed digest string | Fatal setup failure, no `started`. |
| Higher minor, unknown metadata field, opaque extra result key, `{"record":"vendor-note","value":7}` at a boundary | Valid ignorable extension except the result key, which is preserved. Required fields remain validated. |
| Unknown root ZIP entry, duplicate `metadata.json`, a payload entry name containing a parent-directory component, a symlink entry, or an encrypted member | Fatal setup index failure, even if the member would be unused. |
| Descriptor `files/missing.bin`, CRC-damaged required asset, size 5 for the four-byte asset, or a descriptor path beginning with a parent-directory component | Fatal when the result/asset is read; emit no such result. |
| Existing `out/files/message.bin`, a payload ancestor symlink/junction or non-directory, or a filesystem alias collision | Fatal before descriptor emission; do not overwrite, follow or rename to work around it. |
| Missing unused payload contents or an unread corrupt journal suffix on early stop | No eager verification obligation. An indexed structural defect already detected still fails; an early stop does not certify unseen contents. |
| Declared size/entry-count limit exceeded | Fatal setup failure. Actual decompression exceeding limits, spool exhaustion or full output disk is fatal when encountered. |
| Valid digest differs, or background hash read fails/cannot finish | Warning log only, playback and its terminal reason continue. Required signal-header or asset failure is separately fatal. |
| Start with `params:{}` (defaults are raw), another module, frequency 1 or an extra parameter | Fatal setup mismatch. Explicit `mode:fec` with the recorded module and frequency succeeds. |
| No-op pause/resume, unchanged speed, tune 0, patch `{}` or `{"mode":"fec"}` | Successful full state snapshot, including while paused where applicable. Resume/pause still set their respective states. |
| Patch combines `mode:fec` with an unknown name, tune 1, speed 0, unknown command/field | One nonfatal error per rejected command, atomically unchanged state. |
| Pause arrives during a block | Complete that block through progress before paused acknowledgement. No subsequent block is submitted until resume. |
| Pause/speed change arrives halfway through the wait for frontier 8 from 4 | Commands respond before submission. Pause freezes the remaining budget; at speed 2, half a source-second still unconsumed takes 0.25 wall seconds after resume. |
| Stop during the wait for the first block | Cancel it; `finished(stopped,0)`, no stored block output or full-run suffix. A verification warning may precede completion. |
| Stop while the first block is in flight | Emit its log, result and progress 4, then `finished(stopped,4)`; no phase block or finalization file. |
| Stop/EOF while paused at 4 | Immediate `finished(stopped,4)` without another block or full-run suffix. |
| Stop accepted with the block ending at 8 in flight | Complete it, drain N-frontier remainder and full-run suffix without pacing, then `finished(stopped,8)`. |
| Stop while paused on an empty source | `started` already exists; drain its zero-frontier/end-of-source output without resume, then `finished(stopped,0)`. |
| Stop after the normal marker has been consumed | Drain suffix, `finished(end,8)`; do not change the reason to stopped. |
| During finalization, read stop, pause, stop | One nonfatal error for pause, then one shared terminal event with the already selected reason. |
| EOF before start, or stop before start | `hello`, then `finished(stopped,0)`, exit 0; never open the recording. |
| Unterminated command at EOF | One nonfatal rejection for the incomplete line, then EOF acts as stop in the current lifecycle state. |
| Missing terminal event, nonzero exit, or event after `finished` | Consumer treats the run as failed under the protocol even if some results were delivered. |

These examples cover the format's replay envelope. Agreement on deferred producer
schemas remains separate work; no example makes its module-specific keys universal.
