# Issue #16: Specify the replay recording format

Resolves #16.

## Overview

Write `docs/replay.md` as the replay recording contract based on
[`protocol.md`](https://github.com/o-kos/wireroom/blob/main/docs/protocol.md). Define the recording structure, stored data,
validation and observable playback behavior, and link it from `README.md`.

This change is documentation only. Rust types, readers, writers, recording tools,
CLI/UI implementation, C ABI changes, general discovery and the general result
directory layout are outside scope. No implementation starts with this plan.

## Context

- The owner discussed the design one question at a time before this plan.
- Keep `wr` launching `dmd`, with native processing behind its optional native
  boundary. Direct Wasm execution in `wr` is not adopted.
- Replay must work without a native signal core. Source reading must not require
  native demodulation modules.
- Run-protocol, recording-format and C ABI versions are independent. Apply the
  established compatibility principles without introducing a second handshake.
- Source positions are original-file frames; one IQ frame contains an I/Q pair
  per channel. Preserve exact integers and producer order.
- Retain the protocol's deferred producer schemas. Carrying a complete module
  `info` JSON does not establish a universal schema for its contents.

## Decisions

### Recording set and stored data

- One recording represents one normally completed run with fixed results.
  Stopped and failed source runs are outside this format's initial scope.
- The set consists of a signal file and a companion ZIP archive. Append `.replay`
  to the complete signal filename: `signal.wav` uses `signal.wav.replay`.
  `start.recording` points to the signal, not the archive.
- Read size, signal format, channel count, rate and frame count from the signal
  itself. Do not duplicate these properties in archive metadata.
- Store the signal's SHA-256 digest in the archive for background verification.
- Store the module's complete `info` JSON, including its mask and parameter
  definitions, without selectively copying producer fields. Also store the module
  identity, recorded tuning frequency and full actual parameter values needed to
  identify and reproduce the selected run.
- Store `result`, `log` and `progress` in one ordered NDJSON journal, using their
  run-protocol JSON forms. Progress entries retain processing-block boundaries.
  Include a service marker identifying the start of finalization.
- Include binary result payload files in the archive. Phase points retain their
  inline protocol representation. Raw stderr from the original run is excluded;
  structured `log` events and producer `result.log` remain in the journal.

### Playback and controls

- Generate `hello`, `started`, command acknowledgements and `finished` for the
  current invocation; do not replay old lifecycle events or acknowledgements.
- Before `started`, reject a module, frequency or effective parameter map that
  differs from the recorded run. Apply initial overrides to module defaults before
  comparing the full map. Speed and initial pause state are freely selectable.
- A live `tune` or `set_params` request changing recorded settings receives a
  nonfatal command error and preserves state. A request preserving current values
  succeeds with a complete `state`, including an empty parameter patch.
- Pace playback by source-frame advancement, source rate and requested speed.
  Do not record or reproduce original processing/file-write delays. Speed 1 follows
  source time without a throughput guarantee; `max` removes deliberate pacing.
  Pause, controls and stop remain responsive while waiting for a pacing deadline.
- Early stop completes the current block, emits its journal entries through its
  progress event, then finishes with reason `stopped`. Stop while paused completes
  immediately. Emit recorded full-run finalization results only after reaching the
  end of the signal. Once finalization starts, retain the protocol's terminal-reason
  and command-handling rules. Treat stdin EOF as stop.
- Preserve journal order, including overlapping result spans, repeated progress
  frontiers, delayed results and finalization results. Result spans are not playback
  scheduling timestamps.
- Before emitting a file descriptor, materialize and close its regular payload
  file inside the requested output directory, verify its size and retain it
  unchanged through and after exit. Prevent path escapes and filesystem races;
  never overwrite unrelated or already referenced files or clear arbitrary output.

### Validation and failures

- Start after required structural/setup checks; do not scan the entire journal,
  extract all payloads or hash the whole signal as a startup prerequisite.
- Verify signal SHA-256 in the background. A mismatch or inability to complete
  verification produces a `log` at `warning` level and playback continues. The
  digest is diagnostic, not a mandatory proof of correspondence.
- Invalid/unreadable journal data, invalid coordinates, unsafe paths and unreadable
  or invalid required payloads are fatal when detected. Setup failures precede
  `started`; failures discovered during playback use the protocol's fatal error
  and terminal-completion rules. Never silently repair data or switch backends.
- Nonfatal `error` remains exclusively a rejected-command acknowledgement.
  Runtime warnings use `log`, not unsolicited nonfatal `error` events.

## Rejected alternatives

- A directory as the recording container: the owner selected a ZIP archive beside
  the signal. Embedding it in a special signal chunk was also rejected.
- A `.replay.zip` suffix: the owner selected `.replay`; ZIP is the internal format.
- Duplicating signal metadata and byte size in archive metadata: derive them from
  the signal. The digest supplies the agreed diagnostic correspondence check.
- Recomputing results or accepting changed decoding settings: replay has fixed
  results and cannot reproduce processing under different settings.
- Replaying the entire process transcript: generate lifecycle/control events for
  the current invocation and exclude original raw stderr.
- Full blocking validation before playback: avoid startup delay; distinguish
  background verification warnings from fatal data failures.
- Reproducing original machine timing: pace by signal time and selected speed.

## Implementation steps

- [ ] Write `docs/replay.md`: scope, version identification/compatibility, companion
      lookup, ZIP entry layout, metadata fields, preserved module info, NDJSON
      framing, finalization marker and binary asset references.
- [ ] Define the stored/generated event mapping and playback state/command table,
      including initial settings, no-op controls, pacing, pause, stop, EOF, normal
      finalization, an empty source and repeated source-frame frontiers.
- [ ] Define lazy validation, background digest verification, ZIP/path/resource
      limits, payload materialization and setup/runtime failure tables. Keep file
      access safe without requiring eager extraction or whole-recording validation.
- [ ] Add a complete illustrative archive and playback exchange plus valid/invalid
      cases covering the Issue criteria. Label producer info as illustrative rather
      than inventing a standardized module-info or result-body schema.
- [ ] Link the contract from `README.md` and cross-check every acceptance criterion,
      owner decision and applicable protocol invariant.
- [ ] Run the full validation gate, mechanical review and intent review using the
      fixed roles and round limits in `CONTRIBUTING.md`.
- [ ] Move this plan to `docs/plans/completed/` before final owner review.

## Validation

- [ ] Verify every Issue #16 criterion against a specific section or example.
- [ ] Parse all JSON/NDJSON examples and check their types, source coordinates,
      event ordering, file sizes and terminal behavior against both contracts.
- [ ] Check the failure cases: malformed/truncated journal, unsafe/duplicate ZIP
      paths, missing/corrupt payloads, unsupported versions, mismatched settings,
      output collisions and background digest mismatch/failure.
- [ ] Check control cases: no-op tune/patch, rejected changes, pause/resume,
      speed/max, early stop, paused stop, EOF and stop during finalization.
- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --all-targets --locked`
- [ ] `cargo clippy -p dmd --features native --all-targets --locked` with an empty
      core directory, as documented in `CONTRIBUTING.md`
- [ ] `cargo test --locked`
- [ ] `cargo test -p dmd-native --locked`
- [ ] `cargo build --release --locked`, after the gate passes

## Post-completion

Implement replay types/readers/writers and recording tools in subsequent Issues.
Resolve producer-schema deferrals separately before claiming full cross-vendor
interoperability. Follow the Draft PR, owner review, Ready, successful `ci/full`
and squash-merge lifecycle in `CONTRIBUTING.md`.
