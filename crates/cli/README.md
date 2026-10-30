# dmd replay runtime

`cargo run -p dmd -- run --message-format json` implements one replay run using
[protocol 1.0](https://github.com/o-kos/wireroom/blob/main/docs/protocol.md) and [recording format 1.0](https://github.com/o-kos/wireroom/blob/main/docs/replay.md).
Send `start` over stdin after `hello`. `recording` names the PCM WAVE signal; its
literal `.replay` companion supplies fixed settings and results. The source profile
is documented by [dmd-source](https://github.com/o-kos/wireroom/blob/main/crates/source/README.md). Human output and the other
commands remain explicitly unimplemented. There is no backend selector, discovery,
export, native loading or automatic implementation selection.

## Execution

Setup retains regular signal/archive identities, inspects the signal and bounded
archive index, validates metadata/settings, opens the journal without scanning it,
and prepares output access. It does not read payload contents or hash the signal
before `started`. A bounded input worker reads NDJSON; only the runtime applies
commands and writes flushed stdout events. Complete malformed lines are individual
FIFO rejections. EOF acts as stop after preceding input, including rejection of an
incomplete final line. The input worker is process-scoped: stdin can remain blocked
until process exit, but it cannot write protocol output or retain run resources.

A cancellable worker spools exactly one pending block through its validated closing
frontier. Journal cancellation is checked at each streaming-buffer refill and
between records, including skipped extensions. One record and its parsed exact
JSON tree are held in memory at a time. Lookahead never submits frames; consuming
a validated marker in the runtime starts normal finalization. A suffix unit holds
one event. A detected worker failure remains fatal even if stop races with it;
cancellation alone does not certify unread content or turn a detected failure into
success. The runtime cancels and joins lookahead before terminal completion.

Pacing starts with lookahead and credits elapsed playing time at the effective
speed before each control takes effect. Frame differences are computed as integers
before conversion to source seconds. Pauses freeze the remaining source budget;
`max` removes the pending wait even when immediately followed by numeric speed.
Commands are checked between every submitted block, including repeated frontiers,
and at intervals of at most 5 ms during ordinary waits (subject to OS scheduling,
bounded decoding and stdout backpressure). Submitted blocks finish through progress
before controls apply. Logs advance the virtual frontier; result spans do not.

Early stop below N discards unsubmitted work. Stop at N drains remaining N-frontier
blocks and finalization without pacing, including a paused empty source. Consuming
the normal marker fixes reason `end`; later stop cannot replace it. Journal EOF
integrity validation is required for normal completion and full draining. Fatal
errors use the current virtual frontier and directly precede `finished(error)`.

SHA-256 runs after `started` over a cursor of the retained source identity. Length,
modification time and (on Unix) change time are compared around hashing. A matching
completed digest is silent; mismatch, diagnostic read failure, observed change,
worker startup failure or cancellation produces at most one warning before successful
terminal completion. Fatal completion omits this optional warning. Cancellation
checks each 8 KiB read and joins the worker; completion never waits for a full hash.
As with required archive and output I/O, the OS must eventually complete a pending
filesystem operation. This is diagnostic verification, not authentication or a
snapshot: callers must keep source/archive contents stable throughout use.

## Output publication

Caller-selected output and its ancestry are trusted and stable throughout the run.
Local processes moving directories or mutating output are outside the contract.
Archive paths and payload bytes remain untrusted. Shared lexical validation and
recording platform-name checks precede filesystem use. Ordinary filesystem checks
reject evident symlinks/non-directory payload ancestors and existing destinations.
There are no UID, mode, mount or filesystem allowlists.

The portable `tempfile` facility supplies a single bounded spool and exclusive
staging files under destination parents. Assets stream with exact size/CRC checks;
writing handles close before `TempPath::persist_noclobber` publishes at the exact
path without overwrite. Every result's files are ready before its descriptors.
Reuse requires the same exact archive asset and size; no published handles are
retained and no file is rewritten. Published files remain after failure and exit.
A failed result can leave already published files without a descriptor.

Temporary-file cleanup applies only to invocation-owned unpublished files, never
an output tree. Publication is not a multi-file transaction or a crash-durability
guarantee. `tempfile` does not report close errors from dropping a writing handle;
write and synchronization failures are checked before closing. Its no-overwrite
publication API does not promise atomic removal of the old temporary name; a
filesystem cleanup failure can leave that temporary link behind.

## Additional resource budgets

The recording/protocol limits still apply. Runtime budgets are smaller where noted:

| Resource | Runtime limit |
| --- | --- |
| Pending block spool | 256 MiB of compact NDJSON including closing progress |
| Spools / outstanding lookahead | One temporary file / one unit |
| Input queue | One decoded command plus one producer-side bounded line/command |
| Published payloads | 65,536 distinct exact paths |
| Total newly materialized payload bytes | 64 GiB per invocation |
| Streaming buffers | 8 KiB per journal, asset and hash reader |
| Commands handled per polling turn | At most 32, preserving FIFO |

Spool and output disk exhaustion fail explicitly without truncation. A single
protocol/journal line remains bounded at 16 MiB; exact JSON parsing and encoding
can hold several representations of that one line. No full journal, full payload
or unbounded block is retained in memory. Local allocator and kernel failures can
still prevent terminal delivery, as can a broken stdout pipe.

## Validation

The process suite reads the normative example directly from `docs/replay.md`,
checks producer values, exact payload and termination, and checks availability at
descriptor delivery and after exit. Common process tests run across platforms where
practical; the deterministic packet-pipe exchange is Linux-specific and symlink
fixtures use Unix APIs. Tests cover archive paths, destination/ancestor conflicts,
repeated assets, all files before a result, CRC failures and temporary cleanup.
Deterministic event writers inject controls during submitted blocks and finalization;
pacing uses injected elapsed durations. Hash tests cover match/mismatch, read
failure, cancellation and joining. Local validation currently uses the installed
x86_64 Linux target; Windows/macOS execution has not been performed locally.
