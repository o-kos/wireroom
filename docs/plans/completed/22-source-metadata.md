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

- [x] Adapt the retained-source and strict RIFF metadata API with typed errors.
- [x] Add Rust fixtures, bounded-work/failure/identity evidence and replay composition.
- [x] Document the public API, support matrix, obligations and provenance.
- [x] Complete the local gate and release build without lint suppressions.
- [x] Complete mechanical review and resolve substantive findings.
- [x] Complete intent review against acceptance criteria and invariants.
- [x] Move the plan to `docs/plans/completed/` before final owner review.

## Criterion-to-evidence map

All test paths below are relative to `crates/source/tests/`.

| Acceptance criterion | Implementation evidence |
| --- | --- |
| Public boundary and exact results | `examples/inspect_source.rs` executed against the documented source: 60 bytes, real, one channel, rate 4/1, 8 frames; `composition.rs` asserts the same independently. |
| Supported profile | `conformance.rs`: `pcm_matrix_has_independent_frame_and_stream_expectations` covers mono/stereo x 8/16-bit, 16/18-byte fmt, ignored cbSize, positive rates and empty data; `maximum_integer_rate_and_short_fmt_body` checks u32::MAX rate. |
| Header/frame consistency | `conformance.rs`: `malformed_fields_have_typed_reasons`, `required_chunk_order_presence_and_uniqueness`, and `maximum_integer_rate_and_short_fmt_body` cover inconsistent fields, rate overflow, partial frames, order, duplicates and missing chunks. |
| Chunk traversal/bounds | `conformance.rs`: `ancillary_positions_padding_and_embedded_signatures_are_opaque` and `every_truncated_prefix_and_inconsistent_extent_is_rejected` cover opaque chunks in all positions, padding, truncated extents and trailing bytes. |
| Unsupported/malformed/errors | `conformance.rs`: `unsupported_profiles_have_typed_reasons` covers excluded containers, tags, channels, widths and fmt sizes; malformed tests cover identification and headerless bytes. `bounded.rs` injects errors and unexpected EOF at every required read and errors at every required seek. Production source has no unwrap/expect or lint suppressions. |
| Identity/bounded inspection | `bounded.rs`: virtual near-4-GiB sample and ancillary payloads reject any payload read; u64::MAX actual length is rejected without overflow; short/interrupted reads preserve metadata. `identity.rs` covers independent/reusable cursors, original identity after pathname replacement on Unix, cursor lifetime and externally modified contents. |
| Work budget | `bounded.rs`: `budget_equality_and_excess_measure_headers_without_scanning_remainder` asserts exact reads, bytes, seeks and stopping position for 65,536, 65,537 and 65,636 chunks. |
| Composition/evidence/lifecycle | `composition.rs` extracts exact source, metadata and journal from `docs/replay.md` with LF and CRLF, passes derived frames to dmd-replay and asserts boundaries/finalization. Stored ZIP helper is reused locally from replay conformance tests without importing their test module. Crate README documents support, failure stages, stability and later hash-worker obligations. Local gate/release passed; external reviews remain pending. |

## Validation

- [x] Verify implementation acceptance criteria with independent expectations;
  external review and later lifecycle steps remain pending below.
- [x] `cargo fmt --all -- --check`
- [x] `cargo clippy --all-targets --locked`
- [x] Native Clippy with an empty core directory per CONTRIBUTING.md.
- [x] `cargo test --locked`
- [x] `cargo test -p dmd-native --locked`
- [x] `cargo build --release --locked` after the gate.

Local Linux validation on 2026-10-30 also passed
`cargo test -p dmd-source --locked` (17 integration tests) and
`cargo clippy -p dmd-source --all-targets --locked`. No suppression or threshold
change was needed. The full gate and release build passed after renaming the
source example to `inspect_source` to avoid the replay example's output name.
The executable example was run successfully against the exact documented source.
Other operating systems remain for CI; the pathname-replacement assertion is
Unix-only. Independent reviews followed this initial implementation gate.

## Review and final status

Mechanical round 1 was clean in all five categories: malformed-input/resource
boundaries and retained identity; architecture and source units; public errors
and API documentation; substantive test gaps and portability; scope, provenance
and maintainability. The reviewer confirmed actual Argand code adaptation and
preservation of the narrower Issue semantics. No substantive finding was raised,
accepted or declined; no correction round was needed. Windows execution remains
for CI.

Intent review was clean across all eight acceptance criteria, the owner's Argand
reuse requirement, every architectural invariant, scope, evidence, limitations
and lifecycle. No substantive finding was raised, accepted or declined; no
correction round was needed. Mechanical round 1 and intent review are both clean.

The completed plan is moved to `docs/plans/completed/` before final owner review.
After the move, the full local gate and release rebuild passed again on Linux.
The final diff matches the plan and preserves the architectural invariants.

### Content-policy correction and final reviews

The first implementation push was refused by the content policy because added
parent-relative path text was treated as escaping the tree. No hook was bypassed.
The replay development dependency now inherits a workspace path; README inclusion
uses the manifest directory; fixture/document paths derive the repository root
with Path ancestors. Only unpushed implementation history was corrected, keeping
the committed plan first. Parser and cursor behavior did not change.

The full pre-push gate, generic and local content policies, all 17 source tests
and release build passed after correction. Mechanical round 2 and a focused
intent follow-up were clean in every category and upheld the earlier full
reviews. No review finding was raised, accepted or declined. Draft `ci/quick`
passed on the implementation; the final documentation push must pass it too.

Keep the Pull Request Draft for final owner review as requested.
Ready status, successful required `ci/full` on an
up-to-date branch and squash merge require the later owner-approved completion.

## Post-completion

Runtime integration and background source hashing remain outside this Issue.
