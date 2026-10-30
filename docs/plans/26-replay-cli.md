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

### Implemented configuration and runtime decisions

- Require Linux output on ext-family, Btrfs or tmpfs, with non-casefold inode flags
  checked on every directory, including tmpfs (which supports casefolding),
  statx mount identities, anonymous O_TMPFILE spooling and exclusive renameat2
  publication. Reject overlay/network/FUSE and unknown ownership mappings. Output
  paths cannot contain parent components. All ancestry is root/caller owned and
  non-writable by other users, except root-owned sticky traversal ancestors.
- Require the output root and every payload directory to be caller-owned and private
  (no group/other access). This protects relocation and hard-link creation without
  assuming a system-wide protected-hardlinks setting. Reject descendant mount-ID
  changes, including bind mounts. Same-credential caller modification and privileged
  administration follow the contract's caller/OS trust boundary; directory handles
  alone are never treated as relocation protection.
- Limit the single anonymous block spool to 256 MiB, materialized output to 64 GiB
  and 65,536 files, and the input queue to one decoded command. Worker-side decoding
  holds one additional bounded line. Check commands in bounded FIFO batches and at
  5 ms polling intervals, subject to bounded parsing, scheduling and backpressure.
- Credit source time consumed during lookahead and account at every control before
  changing settings. A transient max request clears pending deliberate pacing even
  if another numeric speed follows in the same FIFO batch. Markers validated by
  lookahead become lifecycle transitions only when consumed by the runtime.
- Add journal cancellation checked between records and 8 KiB refills, with a distinct
  cancellation error. Join lookahead before terminal completion and retain any
  already detected non-cancellation failure. Hashing starts after started; join it
  after cancellation, compare retained-file metadata, and omit optional diagnostics
  on fatal completion so fatal error directly precedes finished.
- Use deterministic event-writer command injection for in-flight and finalization
  tests, and injected elapsed durations for pacing. The real-process suite reads
  the normative recording example from the contract instead of maintaining a copy.
- Preserve all owner AGENTS.md edits and add runtime status updates in place. Leave
  that entire file unstaged for the orchestrator to separate from the owner changes.

## Rejected alternatives

Whole-journal buffering or eager extraction violates lazy validation and resource
bounds. Uninterruptible sleeps prevent controls. Result spans are not pacing
coordinates. Path canonicalization and child handles alone cannot establish safe
publication. Silently accepting an unprotected filesystem is not an acceptable
portability fallback. A blocking startup digest is diagnostic work in the wrong
lifecycle phase.

## Implementation steps

- [x] Implement launch surface, bounded transport and lifecycle/settings controls.
- [x] Implement bounded lookahead/spooling, remaining-time pacing and finalization.
- [x] Implement safe output publication/reuse with explicit platform limits.
- [x] Implement cancellable retained-identity SHA-256 diagnostics.
- [x] Add deterministic runtime and real-process contract/error/safety tests.
- [x] Update active architecture/runtime documentation and CHANGELOG in English.
- [x] Complete the full validation gate and release build.
- [x] Leave the active plan ready for orchestrator review; defer Draft PR management
      to the orchestrator.

## Validation

- [x] Run the docs/replay.md example through a real process; verify exact producer
      values, file bytes, close-before-descriptor availability and exit/termination.
- [x] Cover normal end, empty/repeated boundaries, controls/FIFO, EOF and malformed
      input, stop during wait/in-flight/finalization and virtual-frontier failures.
- [x] Cover lazy journal/assets failures, all-result validation, collision/reuse,
      symlinks, aliases and relocation protection or explicit configuration rejection.
- [x] Cover digest match, mismatch, read/modification failure and cancellation.
- [x] Verify pacing with deterministic elapsed-time tests, including pause and
      speed changes, max, repeated frontiers and large coordinates.
- [x] `cargo fmt --all -- --check`
- [x] `cargo clippy --all-targets --locked` (repository warning policy)
- [x] `cargo test --locked`
- [x] `cargo build --release --locked`, after the gate passes

## Implementation validation evidence

The pinned Rust 1.97.1 toolchain passes formatting, all-target Clippy, the complete
locked workspace test suite (129 passed including doctests, eight unchanged
policy/hook tests ignored), then the locked release build. Forty tests exercise the
CLI runtime and process surface. No lint suppressions or threshold changes were
introduced. The normative example runs in a real process and publishes the exact
TEST payload, retains producer values and completes with end at frame 8.

Filesystem tests ran with the host ownership view because the sandbox maps trusted
root-owned ancestors to an unmapped UID; rejection of that mapped view is deliberate.
The run exercises Btrfs ancestry and tmpfs output, including mandatory inode-flag
inspection on tmpfs. Other platform/filesystem configurations are not certified;
unsupported configurations fail closed according to the runtime guide. Registry
access was unavailable; cached registry dependencies resolved offline. Local Git
writes required the authorized sandbox override. Neither restriction stopped local
implementation. Owner AGENTS.md changes and integrated runtime updates remain
unstaged; all other task changes are committed. Push, Draft PR state and independent
reviews remain the orchestrator's responsibility. No reviews were spawned.

## Post-completion

Independent mechanical and intent reviews are performed by the orchestrator.
Plan completion/move, Ready state and merge remain deferred to that workflow;
the owner explicitly requests a Draft Pull Request as this task's final state.
Remote operations blocked by the sandbox are reported and do not stop local work.
