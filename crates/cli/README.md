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

## Supported output and security boundary

Output is deliberately restricted to Linux. Every ancestor, starting at the actual
filesystem root, is opened without following symbolic links. Only ext-family,
Btrfs and tmpfs filesystems are accepted; overlay, network, FUSE and other filesystems
are rejected conservatively. Non-tmpfs directories must expose inode flags and
must not have the casefold flag. This excludes case/Unicode folding configurations
rather than attempting to emulate their filename equivalence. Non-Linux systems
and namespaces exposing unmapped ancestor owners fail setup. Output paths containing parent components are rejected without rewriting.
Kernel/filesystem
support for `statx` mount IDs, `O_TMPFILE` and `renameat2(RENAME_NOREPLACE)` is required.
Anonymous spool support is checked during setup; payload publication failures remain
fatal when encountered.

Every traversed directory must belong to the effective UID or UID 0 and deny group
and other write access. A root-owned sticky ancestor is permitted while traversing
into a protected child, but the output root and every payload directory must belong to the effective UID
and deny all group/other access (mode 0700 or stricter). Newly created directories use mode 0700 and staging files 0600;
POSIX ACL grants are bounded by the checked group-class mask. Private payload
directories also prevent other UIDs from acquiring hard-link aliases, without
relying on the system-wide protected-hardlinks setting. All result
ancestor mount IDs must equal the retained output-root mount ID, including bind
mounts; reused files are checked too. Symbolic links and non-directory ancestors
are rejected with handle-relative no-follow opens. Existing final destinations are
collisions, including hard links and filesystem aliases.

These ownership and access controls prevent a different unprivileged UID from
renaming traversed ancestors while writing or publishing. Holding a directory
handle alone would not do so. The caller security principal includes processes
with the same effective UID: they already share its discretionary access and may
modify its files. Caller modification of its output is outside the contract's
published-file stability guarantee. Administrators with authority to bypass these
permissions or change mounts are trusted, as by the operating system's ordinary
file protection model. Source/archive stability remains the caller's obligation.
This policy does not claim protection against hostile same-credential or privileged
modification, nor accept arbitrary shared writable output as a fallback.

Each asset is streamed with exact size and CRC verification into an exclusive
staging file under its protected destination parent, synchronized and explicitly
closed before exclusive publication. Every result's files are ready before any
of its descriptors are emitted. Read-only file handles retain published identities;
reuse requires the same exact archive path and size, regular-file identity and one
hard link. A replaced path or new hard-link alias fails. Published files are never
rewritten or removed, including on later failure. A failed result can leave already
published files without a descriptor. Cleanup removes only this invocation's
unpublished exclusive staging names; it never clears an output tree.

## Additional resource budgets

The recording/protocol limits still apply. Runtime budgets are smaller where noted:

| Resource | Runtime limit |
| --- | --- |
| Pending block spool | 256 MiB of compact NDJSON including closing progress |
| Spools / outstanding lookahead | One anonymous file / one unit |
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
checks its producer values, exact payload and termination, and checks availability
at descriptor delivery and after exit. Other tests cover controls/EOF, repeated and
empty boundaries, lazy failures, CRC/asset errors, initial settings, collisions,
symlinks, writable ancestry and retained identity replacement. Deterministic event
writers inject controls during submitted blocks and finalization; pacing uses
injected elapsed durations without real sleeps. Hash tests cover match/mismatch,
read failure, cancellation and joining. Linux output tests require a normal owner
view; a sandbox remapping trusted ancestor UIDs is intentionally rejected and must
run these tests outside that mapping. Platform rejection is part of the supported
configuration policy, not a silent downgrade.
