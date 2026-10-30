# Issue #26: Complete replay CLI execution

Resolves #26.

## Outcome and scope

Implement `dmd run --message-format json` using the source, recording and protocol
libraries. Preserve exact producer values, source coordinates and payload bytes,
FIFO controls, bounded lazy lookahead, pacing, cancellation, hashing and terminal
semantics. Other commands, GUI, discovery, exports and signal processing are outside
scope. Keep PR #27 in Draft for independent mechanical and intent review.

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
- [ ] Complete independent mechanical and intent review, then move this plan to
      completed (reserved for the orchestrator).

## Validation

Check archive path rejection, existing destinations/non-directory ancestors,
symlinks where supported, exact repeated assets, exact bytes, all files before a
result, cleanup after payload failure and publication availability after exit.
Retain the documented process exchange and deterministic controls, pacing, hashing
and cancellation tests. No test quota or hostile-concurrency guarantee is intended.

Run `cargo fmt --all -- --check`, `cargo clippy --all-targets --locked`,
`cargo test --locked`, then `cargo build --release --locked`. Cached offline registry
use is acceptable. Only x86_64 Linux is installed locally; compilation and execution
on Windows/macOS must not be claimed without running them.

Local validation passed on pinned Rust 1.97.1 using locked offline dependencies,
including the full gate and subsequent release build. Only
`x86_64-unknown-linux-gnu` was installed and exercised. Independent reviews remain
pending; this plan stays active and the Pull Request stays Draft.

## Simplification review round 1 correction

- [x] Accept P2: CRLF specification checkouts break LF-only Markdown fence
      delimiters in runtime and playback fixtures; remote Windows CI independently
      confirmed the failure before runtime behavior could execute. Other review
      categories were clean, and no findings were declined.
- [x] Normalize CRLF before parsing in the runtime fixture and all three playback
      readers: fixture metadata, example journal and normative exchange. Keep
      production behavior and public contracts unchanged.
- [x] Exercise the actual fixture parsers with a temporarily CRLF-converted
      `docs/replay.md`: `cargo test -p dmd --locked --offline` passed all 48 tests,
      including the normative exchange; a `finally` restoration restored the
      original specification bytes and verified byte-for-byte equality.

A brief inspection of path operations and platform gates found no obvious additional
blocker in the newly reachable suites; Unix symlink tests and Linux packet-pipe tests
retain their platform guards. Windows/macOS/Linux CI will be rerun independently.
Round 2 mechanical and intent review remain reserved for the orchestrator; this
correction does not mark either review clean or complete.

After the correction, the pinned formatting check, all-target locked offline Clippy,
locked offline workspace tests and subsequent locked offline release build all
passed on Linux. The installed pre-push hook remains enabled without bypass.
