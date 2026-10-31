# Issue #28: Build the Wireroom window shell

Resolves #28.

## Overview

Deliver a native `wr` window whose frame, appearance and everyday window behaviour
follow [Argand](https://github.com/o-kos/argand). The workspace is empty apart from
Recent. File opening, processing, plots and results are separate future work.

## Context

The accepted concept in `docs/ui` establishes a toolbar inside the title bar and a
bottom status bar. This step implements that shell using registry GPUI Kit and the
existing Wireroom icons. It does not connect the interface to `dmd` or change replay.

## Decisions

- Add `crates/ui` with the `wr` binary, inheriting workspace lints. Use registry
  GPUI Kit 0.6 components and assets; no external path or Git dependencies.
- Follow Argand's actual client frame: compact title bar, rounded frame, restrained
  shadow, resize edges, platform window controls and maximized/tiled behaviour.
  Use standard components for menus, buttons and other ordinary controls. Keep any
  required custom frame code small and acknowledge the public reference as needed.
- The Wireroom icon/name button opens the application menu. File contains the
  unavailable Open action, Recent and an operational close/quit action. Recent in
  the menu and main area share the same persisted list and an honest empty state.
  Existing entries may be displayed and cleared, but cannot open files in this step.
  Do not populate sample entries or introduce a file picker.
- Reserve the title-bar toolbar for the accepted layout, with unavailable processing
  controls visibly disabled. Keep enough draggable space and avoid narrow-window
  overlap. The status bar reports the actual idle state, with no invented signal data.
- Offer System, Light and Dark appearance through the menu, using Argand's neutral
  surfaces and Wireroom branding. Persist the choice; System follows live changes.
- Persist normal window bounds, display identity where supported and maximized state
  in the platform's per-user configuration location. Restore usable visible bounds
  when a display disappears or the saved bounds are unsuitable. Preserve normal bounds
  through maximize/minimize; use a sensible centred default on first run.
- Keep state persistence simple. Missing state is normal; malformed state must not
  prevent launching. Surface meaningful write failures without repeated modal dialogs.
- Distinguish the GUI's platform build prerequisites from the existing Rust-only CLI.
  Include `wr` in workspace validation and update CI setup as required by GPUI Kit.
- Record the owner's current roles in contributor instructions: Astra Medium for
  planning and decisions, Sol 6.1 Medium for implementation and Git routine, Sol 6.1
  High for independent read-only mechanical and intent reviews. The coordinator does
  not write implementation code or automatically accept every review suggestion.

## Rejected alternatives

- Opening a recording or starting `dmd`: explicitly outside this step.
- Recreating the full concept with fake plots, files or results: the deliverable is
  an empty, usable application shell.
- Using the default toolkit frame unchanged when its geometry differs from Argand:
  the owner explicitly chose Argand as the frame reference.
- General settings frameworks, speculative platform abstraction or a large GUI test
  harness: unnecessary for this window. Prefer manual native interaction plus a few
  focused tests of restoration and persisted state.

## Implementation steps

- [ ] Add the native shell, assets and Argand-based frame with functional window controls.
- [ ] Add application menus, toolbar, status bar and shared Recent empty/list states.
- [ ] Implement appearance and window-state persistence with visible restoration fallback.
- [ ] Update build prerequisites, CI, user documentation and contributor role assignments.
- [ ] Validate the native window manually and complete the local gate and release build.
- [ ] Address independent mechanical and intent review, and prepare a Draft PR for owner review.
- [ ] Move this plan to `docs/plans/completed/` before final review.

## Validation

- [ ] Run `wr`; inspect the frame, icon, toolbar, Recent and status bar at normal and
  narrow sizes in both themes. Open and dismiss the application menu and submenus.
- [ ] Check move, resize, minimize, maximize/restore and close. Relaunch after changing
  bounds and appearance; verify persistence and normal bounds after maximize/restore.
- [ ] Check missing/malformed saved state and off-screen saved bounds. Verify Recent
  menu/list consistency with temporary isolated state, without adding fake product data.
- [ ] Confirm unavailable file/processing controls cannot open a file or launch `dmd`.
- [ ] Add only focused state/restoration tests that protect observable behaviour.
- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --all-targets --locked`
- [ ] `cargo test --locked`
- [ ] `cargo build --release --locked`, after the gate passes
- [ ] Run full CI on Linux, Windows and macOS; distinguish build coverage from native
  interaction actually exercised, and record any remaining platform limitations.

## Post-completion

Keep the PR in Draft for the owner's visual review. Opening files remains a later
task; do not begin it as part of this change.
