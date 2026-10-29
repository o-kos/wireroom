# Issue #20: Replay recording reader

Resolves #20.

## Overview

Add a read-only `dmd-replay` library for recording format 1.0. It validates the
archive and metadata, reads the journal incrementally with explicit block and
finalization boundaries, and streams lazily verified payloads. Runtime, controls,
UI, signal decoding/hashing and output materialization remain outside scope.

## Context

- `docs/replay.md` and `docs/protocol.md` are the accepted contracts.
- `dmd-protocol` supplies exact JSON and individual-message validation, but does
  not validate recording structure or filesystem safety.
- The caller supplies source frames; all coordinates remain original-file frames.
- Unread contents are not certified. Full journal/asset integrity requires EOF.
- A verified payload is not a safely published output file.

## Decisions

- Isolate recording concerns in `crates/replay`, included in default validation.
- Retain an opened archive identity, bound its index before allocation, and check
  local structure and CRC only for consumed members.
- Reuse public protocol types and exact JSON; enforce replay depth separately.
- Expose journal records and boundaries incrementally, documenting provisional
  block validation rather than requiring a whole block in memory.
- Keep resource budgets explicit and errors inspectable; never repair data.
- Use registry dependencies with a Rust-only decompression backend (`flate2` and
  `crc32fast`). Track raw Deflate stream end and exact compressed consumption,
  independently of the declared decoded size and CRC.
- Expose prefix progress as `Record::Boundary`, suffix progress as its stored
  protocol event, and the service marker as `Record::Finalization`.
- Cap distinct remembered asset references at 65,536; exceeding this independent
  bookkeeping budget fails explicitly instead of growing with journal length.

## Rejected alternatives

- Implementing process execution, playback controls, source parsers or output
  extraction: separate responsibilities explicitly excluded by the Issue.
- Eager extraction or a complete journal scan at startup: defeats lazy validation.
- A generic lossy JSON tree: cannot preserve producer numbers and duplicate keys.
- Reopening archive paths per member: does not retain the selected identity.
- Relaxing lint limits or suppressing warnings: refactor within existing policy.

## Implementation steps

- [x] Implement archive/index/member integrity and bounded lazy asset access.
- [x] Implement metadata, settings matching and incremental journal validation.
- [x] Complete public documentation, conformance fixtures and criterion coverage.
- [x] Complete the local validation gate and release build.
- [ ] Complete mechanical review and resolve substantive findings.
- [ ] Complete intent review against all criteria and invariants.
- [ ] Move this plan to `docs/plans/completed/` before final owner review.

## Validation

- [x] Map all 12 Issue criteria to tests or explicit caller obligations.
- [x] `cargo fmt --all -- --check`
- [x] `cargo clippy --all-targets --locked`
- [x] Native Clippy with an empty core directory, as in `CONTRIBUTING.md`.
- [x] `cargo test --locked`
- [x] `cargo test -p dmd-native --locked`
- [x] `cargo build --release --locked`, after the gate passes.

## Criterion coverage

| Issue criterion | Evidence |
| --- | --- |
| Public read-only boundary | `examples/inspect.rs`; README compile-checked public API example; conformance companion/native-byte and archive-path replacement tests; missing archive error test. |
| Metadata and compatibility | Conformance required-field/type/null/duplicate/version/high-minor fixtures; present `info:null`; producer values use protocol exact JSON. |
| Fixed initial settings | Conformance complete-map/default/explicit-required/extra/module/frequency tests. |
| ZIP index and namespace | Adversarial namespace, duplicate, flag, kind, UTF-8, local-index mismatch, ZIP64 and descriptor fixtures; scanner bounds before allocations. |
| Incremental framing and dispatch | Conformance LF/CRLF/one-byte reads, split UTF-8, discriminators, forbidden events, malformed/duplicate JSON and unknown kind fixtures. |
| Processing prefix | Independent example boundaries `(0,4)`, `(4,4)`, `(4,8)`; provisional-result test; log/regression/range/unclosed-block negative fixtures. |
| Marker, suffix and completion | Empty-source/repeated-zero/source-boundary/suffix fixtures; marker failures and EOF integrity tests; README states partial consumption is not certification. |
| Exact producer preservation | Independent documented example expectations plus coexisting fragments, null values and arbitrary-precision opaque number fixtures; exact phase integer assertions and malformed point tests. |
| Lazy asset access | Adversarial stored/Deflate integrity, declared/actual mismatch, partial reads and local-header laziness; conformance repeated-reference conflict and exact `TEST` bytes through asset EOF. |
| Consumed structure and setup | Adversarial overlapping consumed local ranges, CRC, stream-end and compressed trailing data tests; instrumented setup verifies journal and unused payload interior are not read. |
| Resource profile | Scanner synthetic ZIP64 per-entry, total, metadata and 65,536-entry equality/excess fixtures; virtual overlarge archive and central-directory declarations; exact 16 MiB LF/CRLF lines; UTF-8 byte/path-depth and 128-container equality/excess tests. |
| Errors, integration and obligations | Public structured `Error`; Rust-only fixtures and runnable example; crate README documents stage-specific validation, provisional blocks, memory bounds and caller-owned signal/runtime/materializer obligations. Full gate/release evidence is recorded below when run. |

The scanner tests construct declared large sizes without allocating decoded payloads.
The entry-count equality fixture uses a few MiB of index data, not a giant archive.
Journal/asset readers keep 8 KiB streaming buffers; only a bounded metadata/line JSON
value and bounded index/reference maps are retained. No whole block is collected.
ZIP comment lookup can inspect a bounded encoded tail; a comment-free archive reads
only its last 22 locator bytes before reading the central directory.

## Validation results

The full local gate and release build passed on the implementation. The new crate
has 31 Rust tests and one compile-checked documentation example; its runnable
`inspect` example builds with the normal all-targets checks. Existing protocol,
CLI, dependency-policy and core-independent native tests pass. The eight existing
hook/content-policy tests remain intentionally ignored because their source was
not changed. No lint suppression, threshold change or hook bypass was added.

## Review

Record accepted and declined findings, their reasons, corrections and final-round
status. No implementation review has run yet.

## Post-completion

Obtain final owner approval after a concrete reviewable result. Rebuild release,
mark Ready, wait for successful required `ci/full` on a branch up to date with
`main`, then squash-merge. No bypass or lint suppression is planned.
