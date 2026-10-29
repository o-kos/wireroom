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
- Use registry dependencies with a Rust-only decompression backend.

## Rejected alternatives

- Implementing process execution, playback controls, source parsers or output
  extraction: separate responsibilities explicitly excluded by the Issue.
- Eager extraction or a complete journal scan at startup: defeats lazy validation.
- A generic lossy JSON tree: cannot preserve producer numbers and duplicate keys.
- Reopening archive paths per member: does not retain the selected identity.
- Relaxing lint limits or suppressing warnings: refactor within existing policy.

## Implementation steps

- [ ] Implement archive/index/member integrity and bounded lazy asset access.
- [ ] Implement metadata, settings matching and incremental journal validation.
- [ ] Complete public documentation, conformance fixtures and criterion coverage.
- [ ] Complete the local validation gate and release build.
- [ ] Complete mechanical review and resolve substantive findings.
- [ ] Complete intent review against all criteria and invariants.
- [ ] Move this plan to `docs/plans/completed/` before final owner review.

## Validation

- [ ] Map all 12 Issue criteria to tests or explicit caller obligations.
- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --all-targets --locked`
- [ ] Native Clippy with an empty core directory, as in `CONTRIBUTING.md`.
- [ ] `cargo test --locked`
- [ ] `cargo test -p dmd-native --locked`
- [ ] `cargo build --release --locked`, after the gate passes.

## Criterion coverage

Record concrete evidence for each of the 12 Issue criteria with the implementation.
Include independent example expectations, malformed archive and journal cases,
lazy-read instrumentation, resource boundaries, payload EOF verification and
explicit source/runtime/filesystem obligations.

## Review

Record accepted and declined findings, their reasons, corrections and final-round
status. No implementation review has run yet.

## Post-completion

Obtain final owner approval after a concrete reviewable result. Rebuild release,
mark Ready, wait for successful required `ci/full` on a branch up to date with
`main`, then squash-merge. No bypass or lint suppression is planned.
