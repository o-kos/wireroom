# Issue #18: Implement run protocol 1.0

Resolves #18.

## Overview

Implement the shared run protocol 1.0 types, bounded streaming NDJSON codecs and
individual-message validation in `crates/protocol`. Preserve the accepted contracts
in `docs/protocol.md` and `docs/replay.md`. Replay readers, archive/journal types,
runtime/lifecycle execution, CLI/UI and filesystem operations remain outside scope.

## Context

- Protocol, recording and ABI versions are independent.
- Source coordinates are exact original-file frames, including IQ pairs.
- Commands and the version greeting are strict; other events support ignorable
  extensions while validating known required fields.
- Producer schemas remain deferred. Opaque JSON must retain exact numeric values,
  strings, array order and result keys, including a producer `record` key.
- Lexical paths do not prove filesystem containment or file availability.

## Decisions

- Represent all known commands/events and common wire types publicly. Expose a
  separate unknown-event outcome, not an invented lifecycle event.
- Keep shape/value validation separate from optional source-frame-count checks.
  Leave monotonicity, command execution and event ordering to future runtime work.
- Parse JSON with duplicate-key detection at every depth and exact opaque numbers.
  Never round arbitrary producer integers or decimals through binary64.
- Decode complete lines independently so a rejected command can be followed by a
  valid one. Distinguish incomplete EOF, oversize and I/O failures for callers.
- Bound each line to 16,777,216 bytes excluding LF/CRLF. Validate and serialize a
  complete outgoing object before writing; flush each message.
- Keep replay-only resource limits outside the run codec. Document any explicit
  implementation resource budgets and caller-owned safety guarantees.

## Rejected alternatives

- Implementing a replay reader or runtime now: these have separate contracts and
  would expand the Issue beyond the shared boundary.
- Parsing opaque numbers as floating point or accepting duplicate keys through a
  generic map parser: both lose producer data before validation can inspect it.
- Treating source spans as scheduling timestamps or requiring disjoint spans:
  overlapping and finalization results are explicitly valid.
- Filesystem canonicalization inside protocol validation: it cannot establish safe
  access and would conflate lexical validity with runtime obligations.

## Implementation steps

- [ ] Implement exact JSON handling, public wire types and message validation.
- [ ] Implement bounded incremental NDJSON reading and validated writing.
- [ ] Add conformance/error tests and public API documentation; map all 11 Issue
      criteria to tests or documented caller obligations.
- [ ] Complete the local gate, mechanical review and intent review.
- [ ] Move this plan to `docs/plans/completed/` before final owner review.

## Validation

- [ ] Verify each Issue acceptance criterion, including specification examples.
- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --all-targets --locked`
- [ ] Native Clippy with an empty core directory, as in `CONTRIBUTING.md`.
- [ ] `cargo test --locked`
- [ ] `cargo test -p dmd-native --locked`
- [ ] `cargo build --release --locked`, after the gate passes.

## Post-completion

Obtain final owner approval, mark Ready, wait for successful `ci/full` on an
up-to-date branch and squash-merge. Runtime sequencing, replay and safe filesystem
access remain future work; no producer schemas are standardized by these types.
