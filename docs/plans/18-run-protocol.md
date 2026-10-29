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
- Use a custom exact JSON tree and duplicate-aware parser; serde_json handles
  strings only. The explicit implementation budget is 256 nested containers,
  counting the message root, independently of replay limits.
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

- [x] Implement exact JSON handling, public wire types and message validation.
- [x] Implement bounded incremental NDJSON reading and validated writing.
- [x] Add conformance/error tests and public API documentation; map all 11 Issue
      criteria to tests or documented caller obligations.
- [x] Complete the local validation gate and release build.
- [ ] Complete mechanical review and intent review.
- [ ] Move this plan to `docs/plans/completed/` before final owner review.

## Validation

- [x] Verify each Issue acceptance criterion, including specification examples.
- [x] `cargo fmt --all -- --check`
- [x] `cargo clippy --all-targets --locked`
- [x] Native Clippy with an empty core directory, as in `CONTRIBUTING.md`.
- [x] `cargo test --locked`
- [x] `cargo test -p dmd-native --locked`
- [x] `cargo build --release --locked`, after the gate passes.

Validation completed successfully: protocol-only tests, the full local gate and
release build all passed. The suite adds 23 conformance/error tests plus an
executable API example; existing native-isolation and dependency-policy tests also
pass. The eight existing hook/content-policy tests remain intentionally ignored
because those files were not changed. No lint suppression or threshold changed.

## Criterion coverage

Tests are in `crates/protocol/tests/`; the API guide is
`crates/protocol/README.md` and is included in the crate documentation.

| Criterion | Evidence |
| --- | --- |
| 1: Complete public wire API | `conformance::independent_wire_forms_and_every_kind`, `all_specification_example_lines`; public types and API guide. |
| 2: Types, nulls, ranges, exact integer tokens | `conformance::required_fields_nulls_and_wrong_types`, `exact_integer_tokens_and_ranges`, `numeric_string_and_enum_validation`, `nested_required_shapes_and_ignorable_extensions`; descriptor shape tests. |
| 3: Strict commands/hello, extensible events, versions | `conformance::strict_commands_hello_and_extensible_events`, `nested_required_shapes_and_ignorable_extensions`. |
| 4: Source bounds, spans and runtime separation | `conformance::source_context_is_explicit_and_has_no_sequence_state`; API guide caller obligations. |
| 5: Exact producer JSON and record key preservation | `producer::exact_opaque_json_all_payloads_and_private_marker_keys`, `descriptor_required_field_types_and_mask_precision`; specification examples. |
| 6: Descriptor forms and phase conversion | `producer::phase_conversion_and_encoded_integer_constraints`, `descriptor_shapes_and_paths`, `descriptor_required_field_types_and_mask_precision`; exact phase wire assertion. |
| 7: Lexical paths and launch-path separation | `producer::descriptor_shapes_and_paths`; API guide filesystem obligations. |
| 8: Incremental framing and duplicate detection | `transport::arbitrary_fragment_boundaries_utf8_crlf_and_multiple_messages`, `complete_line_rejections_recover_but_incomplete_eof_does_not_execute`; `producer::duplicate_keys_at_every_depth_including_ignored_fields`. |
| 9: Byte limits, short I/O and flushing | `transport::input_exact_limit_lf_crlf_fragmentation_and_early_oversize`, `output_exact_limit_including_escapes_and_phase_points`, `exact_output_boundary_counts_coexisting_points`, `short_writes_flush_per_message_and_exact_wire`, I/O failure tests. |
| 10: Structured errors, local validation, exact values | `transport::invalid_locally_constructed_messages_write_nothing`, `io_failures_are_structured_and_fatal`, `write_failure_after_prefix_and_flush_failure_after_full_line`; `producer::explicit_json_budget_grammar_and_arbitrary_numbers`; API guide failure mapping. |
| 11: Examples, independent wire expectations and gate | All 16 command and 46 event example lines are decoded; replay metadata/service records are excluded. Exact wire assertions, executable documentation and validation commands below cover the implementation boundary. Reviews remain pending. |

## Post-completion

Obtain final owner approval, mark Ready, wait for successful `ci/full` on an
up-to-date branch and squash-merge. Runtime sequencing, replay and safe filesystem
access remain future work; no producer schemas are standardized by these types.
