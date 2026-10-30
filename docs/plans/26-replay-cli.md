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
- Add journal cancellation checked between records, compressed refills and inflate
  iterations, with a distinct cancellation error. Join lookahead before terminal completion and retain any
  already detected non-cancellation failure. Hashing starts after started; join it
  after cancellation, compare retained-file metadata, and omit optional diagnostics
  on fatal completion so fatal error directly precedes finished.
- Use deterministic event-writer command injection for in-flight and finalization
  tests, and an injected monotonic clock in runtime accounting for pacing. The
  real-process suite reads the normative recording example from the contract.
- Preserve all owner AGENTS.md edits and add runtime status updates in place. Include
  the complete active instructions in the Pull Request, including owner architecture
  edits.

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
- [x] Complete the full validation gate and release build after round 1 corrections.
- [x] Prepare focused corrections and update the existing Draft PR for orchestrator
      review; retain Draft state through publication.

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

## Mechanical review round 1 corrections

All five findings were accepted; none were declined. The separate temporary-name
cleanup correction is also in scope.

- [x] Use the kernel's four-byte inode-flag output and cover its casefold mask;
      disarm cleanup after publication and check unpublished identity before unlink.
- [x] Cancel inside compressed refill/inflate loops; synchronize cancellation after
      empty-block processing begins, bound further reads, join and finish; preserve
      detected integrity failures and validate the uncancelled compressed fixture.
- [x] Execute every documented example request and compare every JSON event through
      the actual process, with speed 1 and packet-pipe boundary synchronization.
- [x] Exercise the production pending-clock/control path with injected monotonic time:
      lookahead credit, pause, old-speed accounting, resume, max, repeats and stop.
- [x] Cover retained-source match, mismatch, read failure, modification and cancellation
      through runtime completion; check warning frontier/order/count and terminal
      reasons. Break stdout with active workers and verify cancellation and joins.
- [x] Commit all owner architecture instructions and runtime documentation together.

## Mechanical review round 2 correction

One P2 finding was accepted; none were declined. Round 2 was not clean. All five
round 1 findings were confirmed fixed. The final mechanical review remains pending
with the orchestrator.

- [x] Arm Stage cleanup immediately after exclusive creation using the original open
      file; retain unpublished identity across explicit writer close, use no-follow
      handle-relative statx without allocating a descriptor during cleanup, and
      disarm only after successful publication.
- [x] Add a dedicated subprocess regression with one free descriptor slot: exclusive
      creation succeeds and the identity open fails with EMFILE. Verify owned-temp
      removal, cleanup after writer close under exhaustion, and preservation of a
      replaced temp, an unrelated file at a reused temp name and a published file.
- [x] Pass the full gate and release build after the correction; retain Draft state
      and update the correction evidence for the orchestrator's next review.

The regression fails against the previous Stage implementation because the owned
new temporary file remains, and passes with the correction. Only the helper lowers
its process-wide descriptor limit and fills real descriptors; the parent test suite
keeps its limits unchanged. Existing publication and replaced-temp tests also pass.
No waits, lint suppressions, threshold changes or output-security restrictions were
added or relaxed. The sandbox ownership mapping still requires the existing host
ownership view for the full gate; the gate and subsequent release build pass there.

### Cleanup portability correction before the final mechanical round

- [x] Replace plain fstatat with the already-required statx identity API. Request
      STATX_INO and require its returned mask bit before comparing the u64 inode;
      map device major/minor through libc::makedev to retained metadata's device.
      Preserve no-follow lookup, no descriptor allocation, original-handle evidence,
      writer-close retention and publication disarming. Keep the EMFILE regression.
- [x] Verify the API against cached libc 0.2.189 sources: linux_like/mod.rs exposes
      statx for GNU and supported musl versions, with u64 inode and u32 device fields;
      linux_l4re_shared.rs maps both device components into Linux's u64 dev_t.
      GNU b32's default ino_t is c_ulong and build.rs defaults GNU time bits to 32,
      so plain fstatat can both mismatch retained u64 inode types and overflow.
- [x] Rerun formatting, all-target locked offline Clippy, the complete locked offline
      test suite and then the locked offline release build with the host ownership
      view. Results remain 139 passed plus the isolated helper; no tests were added.

Only x86_64-unknown-linux-gnu is installed. A 32-bit cross-target build is NOT
certified: cached API inspection does not replace compilation for that target.
No lint suppressions, narrowing casts or dependency changes were introduced. The
remaining mechanical and intent reviews stay with the orchestrator; none were run
for this focused correction. Keep the PR in Draft.

## Implementation validation evidence

The pinned Rust 1.97.1 toolchain passes formatting, all-target Clippy and the full
locked offline workspace suite: 139 passed including two doctests, with nine
tests ignored in the main run: eight unchanged conditional policy/hook tests and
one private FD-exhaustion helper. The helper runs separately and passes inside the
parent regression test. The CLI contributes 50 passing tests in the main run:
24 unit/runtime, six command and 20 process tests. The locked offline release build
passes after the complete gate.

The process example compares all documented requests and complete event values at
speed 1, including FIFO controls, exact payload availability and unchanged bytes
after exit. A single-packet stdout pipe holds the first result while a second stdin
read proves pause has entered the bounded command FIFO; this coordinates the first
block boundary without guessed timing or public protocol additions. The initial
source-time budget remains one second.

Worker tests synchronize cancellation after one compressed refill has been inflated,
then assert exactly two compressed reads, no further refill, joining and terminal
completion. The full uncancelled fixture also validates. Injected monotonic time
exercises runtime clock/control accounting. Retained-source diagnostics cover five
outcomes for both end and stopped terminal reasons, with warning count, order and
frontier checks. A synchronized runtime test breaks stdout with both workers active;
a real-process test breaks it after started and producer activity, keeps stdin open
and requires nonzero termination.

Output tests use the host ownership view: mapped untrusted ancestor ownership is
deliberately rejected. Supported output configurations and the caller/OS trust
boundary remain unchanged; packet-pipe coordination is Linux-specific. Cancellation
cannot interrupt an individual OS operation before that operation returns. No lint
suppressions or threshold relaxations were added. All owner AGENTS.md architecture
edits and integrated runtime updates are included intentionally. Subsequent reviews
remain the orchestrator's responsibility; none were invoked for these corrections.

## Post-completion

Independent mechanical and intent reviews are performed by the orchestrator.
Plan completion/move, Ready state and merge remain deferred to that workflow;
the owner explicitly requests a Draft Pull Request as this task's final state.
Push corrections through the installed hook and preserve the existing Draft state.
