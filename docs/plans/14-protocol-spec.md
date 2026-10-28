# Issue #14: Specify the protocol between dmd and wr

Resolves #14.

## Overview

Specify the public run protocol between an independently supplied `dmd` and `wr`.
Define process launch, framing, version handling, run settings, events, commands,
failure handling and a complete exchange in `docs/protocol.md`.

This is a documentation-only change. Rust types, command implementation, the replay
recording format, result-directory layout and C ABI changes are outside scope. Keep
the existing session-handoff guidance in `AGENTS.md` in this branch.

## Context

- `wr` launches one process per run. Different suppliers may provide `dmd`; the
  specification must stand alone without depending on this implementation's ABI.
- Positions and result spans use source-file frames; one IQ frame contains an I/Q
  pair per channel. Resampling must not change that coordinate system.
- Protocol, recording-format and C ABI versions remain independent.
- Processing failures never silently select a different backend.
- Result paths resolve inside the output directory.
- The owner requested individual design questions before the plan. Decisions below
  reflect the final answers to questions 1 through 18, including revisions.

## Decisions

- Launch with `dmd run --message-format json`. The ordinary output mode is `human`;
  removing the existing `--events` option belongs to implementation work.
- Use UTF-8 NDJSON over stdout for events and stdin for commands. Reserve stdout
  for the protocol; redirect incidental implementation output to stderr. `wr`
  displays stderr as uninterpreted diagnostic text and drains both pipes.
- `hello` contains only `event` and `protocol: {major, minor}`. Its shape is stable.
  Each `dmd` supports one version and knows nothing about client versions. `wr`
  supports every major from 1 through its maximum, selects the matching behavior,
  ignores minor and rejects a higher major.
- Minor changes may add ignorable events, optional fields and result keys. New
  commands and incompatible changes require a new major. Unknown events and fields
  are ignored by `wr`; unknown commands receive a nonfatal error.
- After checking `hello`, `wr` sends `start` with recording, module, string-valued
  parameters, frequency, speed, initial pause state and output directory. Success
  produces `started`; setup failure produces a fatal error and `finished`.
- Results retain `samples`, `bits`, `text`, `log` and other producer keys. Preserve
  binary descriptors: `phase` entries carry integer I/Q `points` in -1000..1000;
  all other entries carry a relative `path` and byte `size`. Preserve `id`, `type`
  and `body`. No base64, phase snapshots or timer-based thinning are required.
- Phase values come from components already in -1..1; multiply by 1000 and round.
  Do not normalize or silently clip them. `wr` chooses display thinning.
- The owner deferred exact mask, ALE, params and other binary-type schemas until
  implementation. Carry those producer objects without inventing schemas. State
  this interoperability limitation explicitly rather than claim the whole original
  Issue is specified without further decisions.
- `tune` changes `freq` between blocks without recreating the processing instance.
  It also works while paused. Frequency is in Hz; mask offsets are relative to it.
- Commands are `start`, `pause`, `resume`, `set_speed`, `tune`, `set_params`, `stop`.
  Speed is any positive finite number or `"max"`; UI button steps are not protocol.
- All parameters may change during a run. `set_params` atomically patches string
  values between blocks, including while paused. Rejection preserves all values.
- Successful control commands return a complete `state` snapshot, even for a no-op.
  It includes state, frame, speed, freq and the full current parameter map.
- Commands and responses are FIFO without request IDs. `started`, `state`, a
  nonfatal `error`, or `finished` acknowledges the corresponding command. Data
  events may interleave. Repeated stops during finalization share the terminal
  `finished`; terminal completion supersedes commands not processed before exit.
- Before start, only `start` and `stop` are accepted. Other commands receive a
  nonfatal error. Stop or stdin EOF before start finishes at frame 0 with reason
  `stopped`, without opening the recording. Repeated start is a nonfatal error.
- During finalization, received commands are rejected nonfatally; repeated stop
  leaves finalization in progress. Nothing is read after `finished`.
- `progress` contains only the first unprocessed source `frame`, emitted after
  every submitted processing block. `wr` computes percentage and achieved rate.
- Normal end and stop finalize processing, drain results and emit one final
  `finished` with reason `end` or `stopped`, then exit 0. Fatal failures emit `error`
  and `finished` with reason `error`, then exit nonzero. stdin EOF means stop;
  stdout EOF without `finished` means abnormal termination. Pre-hello failures
  use stderr and a nonzero exit status.
- Spectrum and sonogram are computed by `wr` and are outside this protocol.

## Rejected alternatives

- Rust types in this PR: defer until a consumer can validate their usefulness.
- A persistent server, sockets and event files: one child process with stdio meets
  the run lifecycle without transport setup or an additional event-output option.
- Binary framing and base64: readable JSON points and closed output files cover
  the agreed data types.
- Timed phase snapshots and progress: preserve producer order without coupling
  event selection to display refresh or playback speed.
- Separate typed result events, append IDs and retune segments: they invent a
  lifecycle absent from the producer's result objects.
- Client-version arguments, capability lists and vendor fields in hello: the
  owner chose a version-only greeting and adaptation exclusively in `wr`.
- Fixed speed steps: they are a UI choice, not a restriction on the protocol.

## Implementation steps

- [ ] Write the public specification, including lifecycle tables and field types.
- [ ] Add a complete exchange with pause, speed change, retune and normal end.
- [ ] Link the specification from the repository introduction.
- [ ] Check the specification against owner decisions and Issue criteria, recording
      the owner-approved schema deferrals explicitly.
- [ ] Complete validation and mechanical review.
- [ ] Move this plan to `docs/plans/completed/` before final review.

## Validation

- [ ] Verify each Issue acceptance criterion against the owner-approved scope.
- [ ] Check every example JSON line and its ordering against the document.
- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --all-targets --locked`
- [ ] `cargo clippy -p dmd --features native --all-targets --locked` with an empty core
      directory (see `CONTRIBUTING.md` for exact commands)
- [ ] `cargo test --locked`
- [ ] `cargo test -p dmd-native --locked`
- [ ] `cargo build --release --locked`, after the gate passes

## Post-completion

- Implement protocol types and the replay consumer in subsequent work.
- Specify the replay recording format independently.
- Resolve the deferred producer schemas before claiming cross-vendor result
  interoperability. The original Issue's complete-schema criterion is narrowed
  by the owner's explicit decision to defer these shapes.
- For this implementation only, later native work needs binary retrieval by ID,
  live tuning and atomic live parameter updates. These are implementation needs,
  not requirements on another supplier's internal interface.
- Mechanical review and owner review remain required before marking Ready or merging.
