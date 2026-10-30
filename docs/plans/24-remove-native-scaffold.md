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

- [ ] Remove unused crates, feature wiring and stale lockfile packages.
- [ ] Update hook/CI validation, active documentation and plan template.
- [ ] Validate CLI behavior, Rust-only builds and all hook/policy tests.
- [ ] Complete mechanical review and resolve substantive findings.
- [ ] Complete intent review against the removal request and invariants.
- [ ] Move this plan to `docs/plans/completed/` before final owner review.

## Criterion-to-evidence map

| Acceptance criterion | Evidence to collect |
| --- | --- |
| Remove complete ABI scaffold | Deleted-file inventory and search for remaining live references. |
| Remove dependency/feature wiring | Cargo metadata/lockfile diff; existing CLI parser and command tests. |
| Preserve meaningful hook/CI gates | Hook/workflow diff, ignored policy/hook tests, CI decision-logic tests. |
| Remove obsolete tests, retain policies | Deleted native-isolation tests and passing dependency/toolchain policies. |
| Update active docs, preserve contracts | Documentation/reference search; unchanged accepted specifications and historical plans. |
| Rust-only validation and lifecycle | Full local gate, workspace checks/tests, release build, independent reviews and Draft PR. |

## Validation

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --all-targets --locked`
- [ ] `cargo test --locked`
- [ ] `cargo test -p dmd-protocol --test policy --locked -- --ignored`
- [ ] `cargo check --workspace --all-targets --locked`
- [ ] `cargo test --workspace --locked`
- [ ] CI status decision-logic tests.
- [ ] `cargo build --release --locked` after the gate.
- [ ] `cargo build -p dmd --no-default-features --locked`
- [ ] Full pre-push hook, including generic and configured local content policy.

The plan-only push uses the existing gate before the scaffold is removed. Later
pushes use the documented gate with only obsolete native-specific steps removed.

## Review and final status

Mechanical and intent reviews are pending. Keep the Pull Request Draft for final
owner review. Ready status, successful required `ci/full` on an up-to-date branch
and squash merge remain pending explicit owner approval.

## Post-completion

Design native integration only when implementing actual runtime processing.
