# Issue #26: Complete replay CLI execution

Resolves #26.

## Overview

Implement `dmd run --message-format json` as one complete replay run using the
existing source, recording and protocol libraries. Preserve producer values and
payload bytes, support serial controls and pacing, and finish with the contracted
terminal event and exit status. Other commands, discovery, exports, GUI and signal
processing implementations are outside scope. Keep the Pull Request in Draft for
independent review; do not mark Ready or merge.

## Context

Replay is an implementation of the dmd application, not a backend selected inside
one application. Source frames and rational rate come from the retained signal
identity. The companion archive supplies fixed settings and journal data. Setup
must remain lazy: no journal scan, payload extraction or mandatory digest pass.
The source and archive libraries remain read-only boundaries.

## Decisions

- Use the shared protocol codecs for bounded input and flushed event output. Keep
  lifecycle, snapshots, FIFO command processing and the virtual submitted frontier
  explicit. A reader worker may queue bounded input, but only the runtime applies
  commands and writes protocol output.
- Spool one processing block with explicit disk and memory limits. Validate its
  closing frontier before submission. Keep controls responsive during lookahead
  and required bounded I/O; stop cancels an unsubmitted block. Preserve repeated
  progress boundaries and complete every submitted block before applying controls.
- Model pacing as remaining source time using overflow-safe frame differences and
  rational rate, accounting for elapsed playing time before each setting change.
  Inject elapsed time for deterministic tests; do not test timing with real sleeps.
- Distinguish stopped finalization from normal marker consumption. Early stop below
  N discards unread work; stop at N drains remaining N-frontier blocks and suffix
  without pacing. Validate complete journal EOF before claiming normal completion.
- Materialize all files in a result before emitting any descriptor. Stream and
  verify assets, close successfully, and publish exclusively at exact paths.
  Retain identities for reuse; reject collisions, aliases and replaced files.
- Output safety requires both handle-relative no-follow operations and protected
  ancestry. Directory handles alone do not prevent relocation. Support only
  platform/filesystem/access-control configurations for which the implementation
  can establish this protection; reject the others at setup. Document the actual
  supported platforms, ownership/permission assumptions, filesystem restrictions,
  resource limits and threat boundary alongside tests. Do not weaken the contracts
  with canonicalize-then-open, prefix checks or post-write race detection.
- Start SHA-256 diagnostics after startup on the retained source identity. Detect
  observable modification, report at most one warning, cancel and join without
  waiting for a full digest at termination. Workers never emit stdout events.
- Remove the legacy backend selector and stale native-integration descriptions.
  Preserve the owner's AGENTS.md changes while updating runtime status in place.

## Rejected alternatives

Whole-journal buffering or eager extraction violates lazy validation and resource
bounds. Uninterruptible sleeps prevent controls. Result spans are not pacing
coordinates. Path canonicalization and child handles alone cannot establish safe
publication. Silently accepting an unprotected filesystem is not an acceptable
portability fallback. A blocking startup digest is diagnostic work in the wrong
lifecycle phase.

## Implementation steps

- [ ] Implement launch surface, bounded transport and lifecycle/settings controls.
- [ ] Implement bounded lookahead/spooling, remaining-time pacing and finalization.
- [ ] Implement safe output publication/reuse with explicit platform limits.
- [ ] Implement cancellable retained-identity SHA-256 diagnostics.
- [ ] Add deterministic runtime and real-process contract/error/safety tests.
- [ ] Update active architecture/runtime documentation and CHANGELOG in English.
- [ ] Complete the full validation gate and release build.
- [ ] Leave the active plan and Draft PR ready for orchestrator review.

## Validation

- [ ] Run the docs/replay.md example through a real process; verify exact producer
      values, file bytes, close-before-descriptor availability and exit/termination.
- [ ] Cover normal end, empty/repeated boundaries, controls/FIFO, EOF and malformed
      input, stop during wait/in-flight/finalization and virtual-frontier failures.
- [ ] Cover lazy journal/assets failures, all-result validation, collision/reuse,
      symlinks, aliases and relocation protection or explicit configuration rejection.
- [ ] Cover digest match, mismatch, read/modification failure and cancellation.
- [ ] Verify pacing with deterministic elapsed-time tests, including pause and
      speed changes, max, repeated frontiers and large coordinates.
- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --all-targets --locked` (repository warning policy)
- [ ] `cargo test --locked`
- [ ] `cargo build --release --locked`, after the gate passes

## Post-completion

Independent mechanical and intent reviews are performed by the orchestrator.
Plan completion/move, Ready state and merge remain deferred to that workflow;
the owner explicitly requests a Draft Pull Request as this task's final state.
Remote operations blocked by the sandbox are reported and do not stop local work.
