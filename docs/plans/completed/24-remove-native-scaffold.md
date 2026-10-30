# Issue #24: Remove the unused native ABI scaffold

Resolves #24.

## Overview

Remove the speculative native integration contract and its unused implementation
scaffold at the owner's request. The CLI currently calls none of it. Removing the
C header alone would leave matching raw declarations, wrappers and build rules
that advertise the same unused contract.

## Context

This cleanup is independent of source metadata work in #22. Preserve the CLI
surface and accepted protocol/replay contracts. Real processing remains an intended
backend; its future integration requires an actual implementation and an agreed
boundary. The owner-directed removal supersedes the existing instructions that
require the scaffold's crates, feature and validation commands.

## Decisions

- Remove `crates/core-sys` and `crates/native` completely, including the C header,
  raw Rust ABI declarations, safe wrappers, build script and dedicated tests.
- Remove their workspace dependencies and CLI Cargo feature wiring. Preserve
  command and backend argument parsing, which do not invoke those crates.
- Remove native-specific hook and CI steps. Retain formatting, Clippy, normal
  tests, conditional hook/policy tests, release builds and CI status aggregation.
- Retain applicable dependency-source and Rust-only dependency policies. Delete
  native-isolation tests whose sole subject is the removed integration scaffold.
- Update active instructions, README and plan template. Keep accepted
  specifications and completed historical plans as historical records.
- Add no replacement ABI, dependency, lint suppression or threshold relaxation.

## Rejected alternatives

- Delete just the header: leaves an unused and misleading contract in Rust.
- Replace it with a new integration abstraction: outside the removal request.
- Remove the CLI backend vocabulary or alter protocol/replay behavior: unrelated
  to the unused implementation scaffold.
- Bypass native checks while keeping their documentation: leaves broken gates.

## Implementation steps

- [x] Remove unused crates, feature wiring and stale lockfile packages.
- [x] Update hook/CI validation, active documentation and plan template.
- [x] Validate CLI behavior, Rust-only builds and all hook/policy tests.
- [x] Complete mechanical review and resolve substantive findings.
- [x] Complete intent review against the removal request and invariants.
- [x] Move this plan to `docs/plans/completed/` before final owner review.

## Criterion-to-evidence map

| Acceptance criterion | Evidence |
| --- | --- |
| Remove complete ABI scaffold | Deleted all nine tracked files under `crates/core-sys` and `crates/native`, plus `native_isolation.rs`. Reference search leaves no live scaffold commands or wiring. |
| Remove dependency/feature wiring | Cargo metadata lists only `dmd`, `dmd-protocol` and `dmd-replay`; CLI features are empty. Lockfile removes only the two deleted packages and the CLI dependency edge. Existing CLI tests pass; manual `auto`, `native` and `replay` backend selections retain the not-implemented response. |
| Preserve meaningful hook/CI gates | Quick CI now runs `cargo test --locked`. Only native-specific steps were removed; policy selection and status aggregation are unchanged. All eight ignored hook/policy tests, including 27 content-policy unit tests, pass. CI decision-logic tests pass. |
| Remove obsolete tests, retain policies | Deleted only the dedicated native-isolation integration test. Unchanged dependency-source and Rust-only build-graph policies pass. |
| Update active docs, preserve contracts | Updated `AGENTS.md`, `CONTRIBUTING.md`, README and plan template; removed obsolete static-library ignore rules. CLI source/tests, replay implementation, accepted specifications and historical completed plans have no diff. |
| Rust-only validation and lifecycle | Full local gate, workspace checks/tests, release and no-default-features builds pass. Full pre-push invocation passes with generic and configured local policy. Mechanical and intent reviews are clean; plan moved before final owner review. |

## Validation

- [x] `cargo fmt --all -- --check`
- [x] `cargo clippy --all-targets --locked`
- [x] `cargo test --locked`
- [x] `cargo test -p dmd-protocol --test policy --locked -- --ignored`
- [x] `cargo check --workspace --all-targets --locked`
- [x] `cargo test --workspace --locked`
- [x] CI status decision-logic tests.
- [x] `cargo build --release --locked` after the gate.
- [x] `cargo build -p dmd --no-default-features --locked`
- [x] Full pre-push hook, including generic and configured local content policy.

Validation completed with the pinned Rust 1.97.1 toolchain. CI decision tests were
compiled with `rustc --test` into `target/ci-status-tests`. Command logs are retained
locally under `target/cleanup-24/`; no new tests, dependencies, lint suppressions or
threshold changes were added.

The full pre-push hook was invoked directly without pushing, using the committed
branch range from its merge base with `main` to `HEAD`. It passed the current Rust
gate, generic content policy and the configured local policy. The eight ignored tests
also cover generic/configured policy behavior and conditional test selection. Content
checks were repeated successfully when the implementation was committed and
pushed, as recorded below.

## Review and final status

Mechanical round 1 was clean across workflow/hook/CI coverage, workspace and
dependency cleanup, architecture and CLI behavior, test gaps and portability,
scope and documentation. No substantive findings were raised, accepted or
declined; no correction round was needed. The actual implementation push also
passed the complete pre-push hook, conditional hook/policy tests and both
generic and configured local content policies without a bypass.

Intent review was clean across all six acceptance criteria, the owner-directed
removal scope, every architectural invariant, evidence, limitations and lifecycle.
No substantive findings were raised, accepted or declined; no correction round
was needed. Both independent reviews are clean. Draft `ci/quick` passed on the
implementation. The completed plan is moved before final owner review.
After the move, the full local gate, all ignored hook/policy tests and release
rebuild passed again. Final reference checks find no live scaffold wiring, and
the final diff preserves CLI source/tests, replay code and accepted contracts.

The owner approved the final removal on 2026-10-30. Ready status, successful
required `ci/full` on an up-to-date branch and squash merge are tracked in
Pull Request #25.

## Post-completion

Complete the release rebuild, mark Ready, require successful `ci/full` on an
up-to-date branch, then squash-merge and remove the merged branch.
Design native integration only when implementing actual runtime processing.
