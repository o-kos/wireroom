# Issue #26: Complete replay CLI execution

Resolves #26.

## Outcome and scope

Implement `dmd run --message-format json` using the source, recording and protocol
libraries. Preserve exact producer values, source coordinates and payload bytes,
FIFO controls, bounded lazy lookahead, pacing, cancellation, hashing and terminal
semantics. Other commands, GUI, discovery, exports and signal processing are outside
scope. PR #27 remains Draft, ready for final owner review. Ready and merge
transitions are outside this handoff.

## Decisions

The owner requests a simpler output trust model. Caller-selected output and its
ancestry are trusted and stable throughout the run; local directory relocation and
output mutation are outside the contract. Ordinary portable filesystem operations
are sufficient under this assumption. Archive data remains untrusted.

Validate descriptor paths with the shared lexical validator and reject platform
names that cannot represent them exactly. Check payload ancestors for evident
symlinks/non-directories and reject existing destinations. Stream assets with exact
size/CRC validation to temporary staging files, close writing handles, then publish
without overwrite at exact descriptor paths. Prepare every file before emitting a
result. Reuse the same exact archive asset and size without rewriting. Keep only
path/size bookkeeping, preserve published files and clean only invocation-owned
unpublished temporary files. Use `tempfile` for staging and bounded spooling.

Retain the existing 256 MiB single-block spool, 64 GiB new-payload budget and 65,536
published-path limit. Keep deterministic runtime, pacing, hashing and cancellation
coverage. Common real-process tests should run across platforms where practical;
the normative packet-pipe exchange remains Linux-specific. Do not retain tests for
excluded local mutation, UID/mount restrictions or descriptor exhaustion.

## Steps

- [x] Implement replay lifecycle, transport, settings, controls, lookahead, pacing,
      finalization and cancellable hashing.
- [x] Revise Issue/PR scope and this plan for the owner-requested trust model before
      changing implementation.
- [x] Replace Linux output machinery with portable staging/publication and simplify
      output tests while preserving replay behavior.
- [x] Align active contracts, architecture instructions, CLI guides and changelog.
- [x] Run pinned formatting, all-target locked Clippy, locked workspace tests and
      release build; record actual platform limits.
- [x] Complete independent mechanical and intent review, then move this plan to
      completed for final owner review while PR #27 remains Draft.

## Validation

Check archive path rejection, existing destinations/non-directory ancestors,
symlinks where supported, exact repeated assets, exact bytes, all files before a
result, cleanup after payload failure and publication availability after exit.
Retain the documented process exchange and deterministic controls, pacing, hashing
and cancellation tests. No test quota or hostile-concurrency guarantee is intended.

Run `cargo fmt --all -- --check`, `cargo clippy --all-targets --locked`,
`cargo test --locked`, then `cargo build --release --locked`. Cached offline registry
use is acceptable. The pinned local gate and subsequent release build passed on
Rust 1.97.1 with locked offline dependencies on x86_64 Linux, without lint
suppressions or hook bypass.

[Full CI](https://github.com/o-kos/wireroom/actions/runs/41115478768) succeeded on
the final code revision on Linux, Windows and macOS, including formatting, Clippy,
tests, release and no-default-features builds. This run precedes only the final
documentation handoff; it does not validate that later documentation commit.

## Review outcome

Accepted runtime findings were fixed: Deflate cancellation and complete real-process
example, pacing, hash and active-worker coverage. Output-specific findings were
superseded by the owner-approved trusted/stable-output replacement described above;
they are not unresolved findings. No findings were declined.

The final simplification review accepted one P2 for CRLF specification fixtures.
All four specification readers now normalize CRLF; the actual 48-test dmd suite
passed against a temporarily CRLF-converted specification, and the original bytes
were restored and verified. All other categories were clean. Mechanical round 2
and the complete-change intent review were clean, with no substantive findings or
requested corrections.

The completed plan and Draft PR are ready for final owner review. Marking Ready
and merging are outside the authorized final state.
