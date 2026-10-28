# dmd run protocol

This document specifies run protocol **1.0** between `dmd` and Wireroom (`wr`).
It is a public contract for independently supplied `dmd` implementations. One child
process handles one run. The current CLI does not yet implement it.

Transport, lifecycle, controls and the result envelope are defined here. Exact
producer schemas for masks, ALE, result `params`, binary `body` and other producer
keys are deferred: their interpretation requires agreement with the producer.
This version does **not** establish complete cross-vendor result interoperability.
The command parameter map is specified separately from deferred result `params`.

The replay recording format, result-directory layout, discovery, other CLI commands
and C ABI are separate contracts. Recording, ABI and protocol versions are independent.
Spectrum and sonogram are computed by `wr` and are outside this protocol.

## Launch and transport

`wr` starts the selected executable with:

```sh
dmd run --message-format json
```

The ordinary output mode is `human`; `json` enables this protocol. Run settings
arrive in `start`, after version checking. Executable discovery and supplier-specific
launch settings are outside scope. Failures must never silently change the backend;
a native run must not silently ignore replay-only options.

| Stream | Contents |
| --- | --- |
| Child stdin | Commands from `wr`, one JSON object per line. |
| Child stdout | Events from `dmd`, one JSON object per line, exclusively. |
| Child stderr | Unstructured diagnostic text; never a protocol channel. |

Both protocol streams use UTF-8 newline-delimited JSON (NDJSON), without a byte-order
mark. Each object occupies one line terminated by LF; receivers also accept CRLF.
Newlines inside strings must be JSON escapes. Blank lines, duplicate object keys,
non-object top-level values and invalid JSON are invalid messages. Writers flush
each complete message; readers must handle arbitrary pipe read boundaries and
several messages in one read. JSON member order has no meaning.

The limit is 16,777,216 bytes per line, excluding LF or CRLF, including inline phase
points and JSON escapes. Oversized input is fatal; the receiver need not buffer or
drain the remainder. An oversized output result is also fatal; do not split a producer
result into unrelated results to evade the limit.

`dmd` redirects incidental library output to stderr. `wr` drains both output pipes
concurrently, including while paused and terminating, and displays stderr as
uninterpreted diagnostic text. `dmd` accepts commands while paused or waiting for a
pacing deadline. Pipe backpressure may delay work but must not silently drop events.

## Common types and units

All listed fields are required unless marked optional. JSON `null` is not a
substitute for an omitted optional field. Strings are Unicode strings. Paths and
identifiers must not contain NUL.

| Type | JSON representation and meaning |
| --- | --- |
| `u32` | Integer from 0 through 4,294,967,295. |
| `u64` | Integer from 0 through 18,446,744,073,709,551,615. |
| Finite number | JSON number with a finite value representable as binary64; NaN and infinities are invalid. |
| Frame | `u64` position in the original source file. |
| Frequency | Finite number in Hz, in the source recording's frequency coordinate system. |
| Speed | Positive finite number, or the exact string `"max"`. |
| Parameter map | JSON object mapping parameter names to string values. No implicit conversion from numbers, booleans or null. |
| Identifier | Nonempty opaque string; never a path. |

Integer fields use integer JSON tokens without fractions or exponents. Receivers
preserve them exactly, including values above binary64's exact integer range.

One source frame contains one real sample or one I/Q pair per channel. Channels,
IQ components, conversion and resampling never multiply or change frame coordinates.
A run begins at frame 0 and processes the source in order.

`frame` is the first source frame not yet submitted for processing: monotonic and
at most the source's total `frames`. Result `samples` is the JSON array `[start, end]`,
representing the mathematical half-open source interval `[start, end)`, with
`0 <= start <= end <= frames`; start is included and end excluded. Spans may overlap
or repeat, including during finalization.

The source rate is a rational pair `[numerator, denominator]`, both positive `u64`
integers, in source frames per second. Thus source duration is
`frames * denominator / numerator` seconds. `wr` derives progress percentage from
`frame / frames` and achieved processing rate from frame deltas and elapsed wall
clock time; it handles an empty source without dividing by zero.

Numeric speed requests a multiple of source time per wall clock time: `1` is
real-time pacing, without a throughput guarantee. `"max"` removes deliberate pacing.
Speed changes neither source units nor decoded content. UI speed steps are not a
protocol restriction.

## Version handshake

The first stdout event is exactly this stable shape:

```json
{"event":"hello","protocol":{"major":1,"minor":0}}
```

`major` is a positive `u32`; `minor` is a `u32`. These are the only fields: the
version-only shape is stable across all versions, allowing rejection before `start`.

Each `dmd` implements one version and knows no client versions. A `wr` with maximum
major M supports every major from 1 through M, selects behavior by major, ignores
minor, and rejects a major above M. Major 0 or an invalid greeting is a protocol
failure. On rejection, send no `start`, close stdin and drain output until exit.

Minor revisions may add ignorable events, optional event fields and result keys.
They must not change the meaning or requirements of existing messages. New
commands and incompatible changes require a new major. `wr` ignores unknown
event names and unknown fields in otherwise valid known events, including unknown
result keys. Ignoring a field does not permit accepting an invalid required field.
The stable `hello` shape is not an extension point.

## Commands

Every command has a string `command` discriminator. The following table lists all
other fields. Unknown command names, unknown command fields, missing fields,
wrong types and rejected values produce a nonfatal `error` without changing the
run. A syntactically valid `start` that cannot set up its requested run is the
fatal setup failure described below.

| Command | Fields | Successful response and effect |
| --- | --- | --- |
| `start` | `recording`: nonempty path string; `module`: identifier; `params`: parameter map; `freq`: frequency; `speed`: speed; `paused`: boolean; `output`: nonempty directory path string | `started`; open the recording, select the module, validate settings and prepare the output directory. Enter running or paused as requested. |
| `pause` | None | `state` with `state: "paused"`; finish any submitted block, then submit no further blocks. |
| `resume` | None | `state` with `state: "playing"`; allow processing to continue. |
| `set_speed` | `speed`: speed | `state` with the new speed, preserving playing/paused state. |
| `tune` | `freq`: frequency | `state` with the new frequency, preserving playing/paused state. |
| `set_params` | `params`: parameter map | `state` with the full resulting parameter map, preserving playing/paused state. |
| `stop` | None | `finished` after finalization and result draining; no intermediate `state` acknowledgement. |

`recording` and `output` are filesystem paths, relative to the child's known launch
working directory or absolute. Result paths follow stricter rules below. `module`
is an unambiguous module identifier, never a path.

Initial `params` overrides module defaults; `started.params` reports the full map.
Unknown parameters, invalid values or unsupported initial tuning fail setup; do not
silently ignore them or clip frequency. An unusable output directory also fails setup.
It is a destination, not permission to clear it or overwrite unrelated files.

`pause`, `resume`, `set_speed`, `tune` and `set_params` apply serially between
processing blocks. Their acknowledgement is emitted after the change takes effect
and before the next block is submitted. An in-flight block may finish and emit
results and progress first. Already queued results retain producer order and may
be delivered after a state change; a state acknowledgement is not a result flush
or a new result segment.

`tune` updates the existing processing instance without recreating it. Mask
frequency offsets remain relative to the current `freq`. Tuning also works while
paused; it does not resume processing. All parameters may change during a run.
`set_params` atomically patches the existing map: supplied values replace their
names, omitted names retain their values, and an empty map is a no-op. There is no
deletion operation or special null value. If any name or value is rejected, the
entire patch is rejected and every old value remains. It also applies while
paused, without requiring another processing block or resuming the run.

Every successful control returns a complete snapshot, including no-op pause, resume,
unchanged speed or frequency, and empty parameter patches.

## Lifecycle and command states

After `hello`, the process waits for start, runs or pauses, finalizes, then finishes.
It handles no second run. `started` precedes run logs, results, progress and state
acknowledgements.

| Command | Waiting for start | Running | Paused | Finalizing |
| --- | --- | --- | --- | --- |
| `start` | Set up the run; emit `started`, or fatal setup error and `finished`. | Nonfatal error: already started. | Nonfatal error: already started. | Nonfatal error: finalizing. |
| `pause` | Nonfatal error: not started. | Pause; emit `state`. | No-op; emit `state`. | Nonfatal error: finalizing. |
| `resume` | Nonfatal error: not started. | No-op; emit `state`. | Resume; emit `state`. | Nonfatal error: finalizing. |
| `set_speed` | Nonfatal error: not started. | Apply; emit `state`. | Apply; emit `state`. | Nonfatal error: finalizing. |
| `tune` | Nonfatal error: not started. | Apply; emit `state`. | Apply; emit `state`. | Nonfatal error: finalizing. |
| `set_params` | Nonfatal error: not started. | Apply atomically; emit `state`. | Apply atomically; emit `state`. | Nonfatal error: finalizing. |
| `stop` | Emit `finished` at frame 0 with reason `stopped`; do not open the recording. | Begin finalization; acknowledge with `finished`. | Begin finalization; acknowledge with `finished`. | Keep finalizing; share the same terminal `finished`. |

The finalizing column applies to commands actually read. After `finished`, no
commands are read and no events emitted; unprocessed commands get no individual reply.

At normal source exhaustion, `dmd` finalizes processing and drains pending results.
A stop or stdin EOF stops further source consumption, completes any in-flight work,
finalizes processing and drains pending results. Conversion buffers may still
produce blocks for source frames already consumed; each submitted block still has
its progress event. Stop must work while paused. Results are not discarded merely
because the run was stopped. Once finalization starts, a later `stop` does not
change a normal-end reason to `stopped`; a fatal failure can change either reason
to `error`. Finalization runs only once.

### FIFO acknowledgements and terminal completion

There are no request IDs. `dmd` reads and applies commands in stdin order. For
ordinary commands, `started`, `state` or a nonfatal `error` acknowledges one command
in that order. A malformed, complete input line also occupies one rejected command
position. Result, log and progress events may interleave with acknowledgements;
`hello`, data events and fatal errors are not ordinary command acknowledgements.
`state` is emitted only as a successful control acknowledgement, not spontaneously.

Terminal completion is the exception to one-response-per-command: `finished`
acknowledges a stop and closes all outstanding requests. Repeated stops during
finalization share this single event. If commands are read during finalization,
non-stop commands receive nonfatal errors in their read order, even while a
preceding stop is awaiting `finished`. These errors skip the outstanding stops;
a client tracks those stops as waiting for terminal completion. For example,
`stop`, `pause`, `stop` read during finalization produces one nonfatal error for
`pause` and then one shared `finished`, not three terminal responses.

Terminal completion supersedes unprocessed commands, including controls racing with
source exhaustion or failure. Clear the pending queue on `finished`. A simple client
may send one ordinary command at a time; pipelining follows the same rules.

## Events

Every event has an `event` string. Files and phase points are descriptors in `result`.

### `started`

`started` acknowledges setup before any block submission, also for an empty or
initially paused run. It reports source information and initial state:

| Field | Type and meaning |
| --- | --- |
| `event` | `"started"`. |
| `module` | Identifier of the selected module. |
| `mask` | Producer mask object, preserved without restructuring; exact schema deferred. Frequency offsets within it are relative to `freq`. |
| `stream` | Object with `format`, `channels` and `rate`, describing the original source. |
| `stream.format` | `"real"` or `"iq"`. |
| `stream.channels` | Positive `u32`. |
| `stream.rate` | Positive rational source-frame rate `[numerator, denominator]`. |
| `frames` | `u64` total source-frame count; zero is permitted. |
| `state` | `"playing"` or `"paused"`, matching the initial `paused` setting. |
| `frame` | Frame; always 0 in `started`. |
| `speed` | Effective speed. |
| `freq` | Effective frequency in Hz. |
| `params` | Full current parameter map. |

`frames` and `stream.rate` determine duration; converted-stream metadata cannot
substitute for source metadata. The exact mask schema remains deferred.

### `state`

The required fields are `event: "state"`, `state`, `frame`, `speed`, `freq` and
`params`, with the same types as in `started`. They form a complete snapshot after
the acknowledged command. `state` is either `playing` or `paused`; waiting,
finalizing and finished are lifecycle states, not possible snapshot values.
`frame` need not advance while applying controls.

### `progress`

`progress` contains only `event: "progress"` and `frame`. Emit it after every
submitted block, including conversion-buffer draining. Adjacent frames may be equal
when buffering does not advance the source frontier. Do not thin by timer or include
percentage/rate/time; `wr` computes those and chooses its display update rate.

### `log`

A diagnostic event from `dmd` has `event: "log"`, `frame`: the current source frame,
`level`: one of `debug`, `info`, `warning`, `error`, and `text`: a string. It may
interleave with results and acknowledgements after `started`, including while paused
or finalizing. Severity alone does not reject a command or terminate a run; `error`
events and `finished` govern those transitions. A `log` event is distinct from
producer `result.log`, whose JSON value is preserved, and from raw stderr text.

### `result`

A `result` event retains the producer's keys at the top level, alongside the
reserved `event: "result"` discriminator. A producer must not supply a conflicting
`event` key. Every result requires `samples`; other payload keys are optional and
may occur together.

| Key | Representation |
| --- | --- |
| `samples` (required) | JSON array encoding the half-open source-frame interval defined above. |
| `bits` | String containing `0` and `1` characters; an empty string is allowed. |
| `text` | String fragment, preserved exactly. |
| `log` | Producer string, preserved exactly; separate from `log` events and stderr diagnostics. |
| `ale` | Producer JSON value, preserved; ALE schema deferred. |
| `params` | Producer JSON value, preserved; schema deferred. This is not a state acknowledgement or a parameter update. |
| `binary` | Array of binary descriptors, defined below. |
| Other keys | Producer JSON values, preserved by `dmd`; an unaware `wr` ignores them. |

Preserve producer order without merging or timer-based selection. Text and bits are
fragments, not message lifecycles. No append IDs, completion markers or retune segments
are implied; overlapping spans do not imply replacement.

Each binary descriptor preserves these fields:

| Field | Type and meaning |
| --- | --- |
| `id` | Producer identifier; opaque and not a path or an instruction to create one. |
| `type` | Nonempty producer type string. The exact string `"phase"` selects inline points. |
| `body` | Producer JSON value, preserved without changing its type or fields; type-specific schema deferred. |

A descriptor with `type: "phase"` additionally requires `points`, an array of
pairs `[I, Q]`. Each component is an integer from -1000 through 1000 inclusive.
It has no `path` or `size`. An empty points array is permitted.

Source phase components are already in [-1, 1]. Encode each as
`round(component * 1000)`, rounding ties away from zero; encode negative zero as 0.
Do not normalize the constellation or silently clip components. Nonfinite or
out-of-range source components are a fatal invalid-result error, even if rounding
would bring them into the integer range. No base64 or periodic phase snapshots are
used. All producer phase entries are retained; display thinning belongs to `wr`.

Every descriptor whose `type` is not `"phase"` requires `path`, a safe relative
path string, and `size`, a `u64` byte count. It has no `points`. Its file contains
the original binary payload; `body` remains metadata rather than a replacement
for those bytes.

The complete example below illustrates both descriptor forms. Its `file`, `{}` and
`iqType` values are producer metadata, not standardized type-specific schemas.

### Result-file safety and availability

A binary `path` is relative to the `start.output` directory and uses `/` separators.
It must be nonempty and contain no leading slash, backslash, colon, NUL, empty
component, `.` component or `..` component. It cannot denote a drive path, UNC path,
URL or an absolute path. Identifiers are never interpolated into paths without
independent validation. These rules apply equally to replayed references.

Lexical checks alone are insufficient: both writing and reading must keep the
resolved file inside the output directory, including in the presence of symbolic
links or equivalent filesystem indirections. An implementation must prevent a
check/open race from escaping that directory, not merely check a string prefix.
Reject an unsafe reference; never follow it or silently rewrite it to another file.

Before emitting a descriptor, `dmd` finishes writing and closes the referenced
regular file. Its byte length equals `size`. The file remains available and
unchanged for the rest of the run and after process exit until the caller removes
its output. A later result must not overwrite a file already referenced. This
permits `wr` to open the payload as soon as the event arrives. An inability to
write, close or safely reference it is a fatal run error. Neither participant may
recursively delete an arbitrary directory in order to prepare or clean up output.

### `error`

Required fields: `event: "error"`, `fatal`: boolean, and `message`: nonempty human-readable
string. There are no error codes; do not parse message text for control flow.
A nonfatal error exclusively acknowledges one rejected command, without changing state.
Recoverable runtime diagnostics use `log` events, not unsolicited nonfatal errors.
A fatal error terminates and is followed by `finished` with reason `error` when stdout works.

### `finished`

Required fields: `event: "finished"`, `reason`: `end`, `stopped` or `error`, and
`frame`: the final submitted frontier. Prestart stop/setup failure uses 0; normal end
uses total source frames. Exactly one `finished` is the last stdout event when the
protocol can complete. Normal end/stop first finalizes and drains results. `wr` also
waits for process exit; a terminal event is not an exit status.

## Errors and stream closure

| Condition | Required behavior |
| --- | --- |
| Failure before `hello`, including invalid launch arguments | Write diagnostics to stderr and exit nonzero. There is no protocol greeting or terminal event to promise. |
| Malformed complete stdin line within the length limit, including invalid UTF-8, duplicate keys or an invalid command shape | Emit a nonfatal `error`, discard that line and continue with the next line. Do not partially apply it. |
| Unknown command or a known command invalid in the current lifecycle state | Emit a nonfatal `error`, subject to terminal completion superseding unprocessed commands. |
| Invalid live setting or rejected parameter patch | Emit a nonfatal `error`; retain all previous settings. |
| Valid `start` cannot open the recording, select the module, apply settings or prepare output | Emit a fatal `error`, then `finished` with reason `error` and frame 0; exit nonzero. No `started`. |
| Fatal processing, invalid-result, resource or output-file failure after `started` | Stop processing, emit a fatal `error` then `finished` with reason `error`, and exit nonzero. Do not switch backends. |
| Failure during normal or stopped finalization | Emit a fatal `error`, finish with reason `error`, and exit nonzero. |
| stdin EOF | Process complete preceding command lines in order, then treat EOF as stop. Before start, finish at frame 0 without opening a recording. During finalization, keep finalizing. |
| Unterminated bytes before stdin EOF | Reject the incomplete last line with a nonfatal `error`, then apply EOF as stop. Never execute the truncated command. |
| stdin read failure or oversized command line | Fatal protocol failure: emit `error` and `finished` with reason `error` if stdout works, then exit nonzero. |
| stdout write failure | Stop processing, clean up resources, report to stderr if possible and exit nonzero. Do not claim delivery of `finished` on a broken pipe. |
| stdout EOF before `finished`, including a truncated event | `wr` treats the run as abnormally terminated, even if the child exits 0. Preserve already received results and diagnostic text. |
| Invalid UTF-8, invalid framing, oversized event, or malformed known event on stdout | `wr` treats the run as a protocol failure, closes stdin and drains remaining output while terminating the child. It does not continue interpreting unreliable events as a successful run. |
| stderr EOF | No lifecycle change; continue consuming stdout and waiting for exit. |

After a fatal error, clean up resources without promising successful finalization
or complete results. Emit `finished` directly after the error if stdout works.
Crashes or broken pipes may prevent either event; use stderr and exit status.

Normal end/stop exits 0; fatal failure exits nonzero. Reason `error`, missing
`finished`, or nonzero exit each means failure, even if the other indications suggest
success. Events after `finished` violate the protocol. A consumer may time out and
kill an unresponsive child; the timeout is unspecified and final results are not guaranteed.

## Complete example

A 9,600-frame, one-channel IQ source runs at 4,800 frames/second. Mask and binary
metadata illustrate deferred schemas. Only the JSON lines are transmitted. The
client launches `dmd run --message-format json`.

**stdout:**

```json
{"event":"hello","protocol":{"major":1,"minor":0}}
```

After accepting major 1, **stdin:**

```json
{"command":"start","recording":"rec.wav","module":"navtex","params":{"mode":"fec"},"freq":1855.0,"speed":8,"paused":false,"output":"out"}
```

**stdout:**

```json
{"event":"started","module":"navtex","mask":{"bandwidth":500,"markers":[{"frequency":-185,"type":"info","title":"f0"}]},"stream":{"format":"iq","channels":1,"rate":[4800,1]},"frames":9600,"state":"playing","frame":0,"speed":8,"freq":1855.0,"params":{"mode":"fec"}}
{"event":"log","frame":0,"level":"info","text":"Processing started"}
{"event":"result","samples":[0,2400],"text":"CQ "}
{"event":"progress","frame":2400}
```

The client pauses. **stdin:**

```json
{"command":"pause"}
```

One block was already in flight. Its output precedes the acknowledgement.
**stdout:**

```json
{"event":"result","samples":[3840,4800],"bits":"00000","text":"_","binary":[{"id":"phase","type":"phase","body":{"iqType":"complex_float"},"points":[[712,21],[-694,48],[703,-31]]}]}
{"event":"progress","frame":4800}
{"event":"state","state":"paused","frame":4800,"speed":8,"freq":1855.0,"params":{"mode":"fec"}}
```

The client changes speed, tunes and patches parameters while paused. **stdin:**

```json
{"command":"set_speed","speed":2.5}
{"command":"tune","freq":1900.0}
{"command":"set_params","params":{"mode":"arq"}}
```

FIFO acknowledgements report the full state after each command. **stdout:**

```json
{"event":"state","state":"paused","frame":4800,"speed":2.5,"freq":1855.0,"params":{"mode":"fec"}}
{"event":"state","state":"paused","frame":4800,"speed":2.5,"freq":1900.0,"params":{"mode":"fec"}}
{"event":"state","state":"paused","frame":4800,"speed":2.5,"freq":1900.0,"params":{"mode":"arq"}}
```

**stdin:**

```json
{"command":"resume"}
```

**stdout:**

```json
{"event":"state","state":"playing","frame":4800,"speed":2.5,"freq":1900.0,"params":{"mode":"arq"}}
{"event":"result","samples":[4800,7200],"text":"DE "}
{"event":"progress","frame":7200}
{"event":"progress","frame":9600}
{"event":"result","samples":[4800,9600],"text":"TEST","binary":[{"id":"decoded-1","type":"file","body":{},"path":"payloads/decoded-1.bin","size":4}]}
{"event":"finished","reason":"end","frame":9600}
```

Finalization drained the last result after closing `out/payloads/decoded-1.bin`
with four payload bytes. The file remains available after stdout closes and the
child exits 0. `wr` drains stderr, waits for that exit, and marks the run complete.
