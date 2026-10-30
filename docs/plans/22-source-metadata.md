# Issue #22: Source metadata from PCM WAVE

Resolves #22.

## Overview

Add a read-only, core-independent `dmd-source` library that retains an opened
signal identity and reports actual byte size, protocol stream and exact source
frame count. Playback, runtime/CLI/UI integration, conversion, hashing, native
changes and replay/protocol contract changes remain outside scope.

## Context

Source coordinates are original interleaved frames. An opened handle preserves
identity across pathname replacement, but does not freeze externally changed
contents. Callers own stability; a later worker must hash the retained source.
The owner explicitly requested reuse of Argand's file-reading code.

## Decisions

- Place `dmd-source` in `crates/source`, included in default workspace validation.
- Adapt public MIT-licensed Argand `crates/io/src/riff.rs` and
  `shared_file.rs` from revision `520c933531f1710ecbe7962d34224855142c5b72`.
  Preserve its license notice and document provenance. Use local source rather
  than an external path or Git dependency.
- Reuse format-field decoding, aligned RIFF traversal and independent cursors
  over a retained file. Adapt traversal to unbuffered bounded header reads and
  checked `u64` offsets, validating the complete top-level container.
- Apply the Issue's stricter profile: PCM tag 1, mono/stereo, 8/16-bit,
  16/18-byte fmt, positive exact integer rate. Ignore PCM cbSize. Stereo remains
  two real channels. Reject truncation and partial frames rather than clipping.
- Reject broader Argand formats, hints, fallback and metadata rate overrides.
  Reusing those behaviors would violate the accepted Issue contract.
- Use shared protocol Stream/Rate/Frame types and structured thiserror errors.
- Enforce a documented 65,536 top-level chunk budget before reading each header;
  equality succeeds and excess stops without scanning the remainder.
- Skip opaque data/ancillary payloads by seeking. No allocation depends on a
  declared chunk length. Expose retained access without reopening the pathname.

## Rejected alternatives

- A new independent file-reader implementation: owner requested Argand reuse.
- Importing the complete Argand sample-decoding stack: exceeds metadata scope
  and introduces sample scans, floating rates and incompatible acceptance rules.
- Filename inference, stereo-to-IQ inference, recovery or backend fallback:
  violates explicit support and consistency requirements.
- Hashing during setup: separate worker responsibility, not header inspection.

## Implementation steps

- [ ] Adapt the retained-source and strict RIFF metadata API with typed errors.
- [ ] Add Rust fixtures, bounded-work/failure/identity evidence and replay composition.
- [ ] Document the public API, support matrix, obligations and provenance.
- [ ] Complete the local gate and release build without lint suppressions.
- [ ] Complete mechanical review and resolve substantive findings.
- [ ] Complete intent review against acceptance criteria and invariants.
- [ ] Move the plan to `docs/plans/completed/` before final owner review.

## Criterion-to-evidence map

| Acceptance criterion | Planned evidence |
| --- | --- |
| Public boundary and exact results | Runnable inspect example; independent byte-size, Stream and frame assertions. |
| Supported profile | Mono/stereo x 8/16-bit fixtures, 16/18-byte fmt, ignored cbSize, unusual rates, empty data, exact documented 60-byte source. |
| Header/frame consistency | Wrong fields/order/duplicates/missing chunks, checked rate/alignment arithmetic and partial-frame failures. |
| Chunk traversal/bounds | Ancillary chunks in all positions, odd padding, opaque embedded signatures, every truncated extent and trailing bytes. |
| Unsupported/malformed/errors | Typed negative fixtures for all excluded formats, I/O and seek failures, truncation and no panics on external data. |
| Identity/bounded inspection | Instrumented seekable reader, synthetic large offsets, short/interrupted reads, injected failures and pathname replacement. |
| Work budget | Exact chunk-budget equality and one-over-limit measured header reads/seeks with no remaining scan. |
| Composition/evidence/lifecycle | Derived frame count passed to dmd-replay, LF/CRLF document fixtures, README failure stages and obligations, full gate/release and independent reviews. |

## Validation

- [ ] Verify each Issue acceptance criterion with independent expectations.
- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --all-targets --locked`
- [ ] Native Clippy with an empty core directory per CONTRIBUTING.md.
- [ ] `cargo test --locked`
- [ ] `cargo test -p dmd-native --locked`
- [ ] `cargo build --release --locked` after the gate.

## Review and final status

Mechanical and intent review are pending. Keep the Pull Request Draft for final
owner review as requested. Ready status, successful required `ci/full` on an
up-to-date branch and squash merge require the later owner-approved completion.

## Post-completion

Runtime integration and background source hashing remain outside this Issue.
