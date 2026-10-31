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
- Strip symbols only from the GUI release package to reduce distribution size;
  keep dependency optimization, CLI release builds and development symbols unchanged.
  Do not add LTO/code-generation changes for this step.
- Limit Cargo compilation to two parallel jobs in the repository configuration.
  Run background validation at lowered scheduler priority with `nice -n 10`.
  Do not start concurrent validation builds or saturate the interactive desktop.
- Keep registry dependency debug symbols disabled in development builds while
  retaining workspace debugging. Remove only confirmed obsolete Cargo build outputs
  before measuring the fresh gate/build footprint; retain fixtures and native evidence.
- Export a small optical icon at 64 pixels, retaining the canonical ring/arrow
  coordinates and four round dots with thinner ring strokes for 16-pixel legibility.
  Keep the large canonical artwork unchanged. Avoid enlarging an intrinsic small
  SVG raster at fractional display scales. Register the
  matching desktop entry/icon only for the current user, with the actual executable
  path and backups for differing prior Wireroom files.
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

- [x] Add the native shell, assets and Argand-based frame with functional window controls.
- [x] Add application menus, toolbar, status bar and shared Recent empty/list states.
- [x] Implement appearance and window-state persistence with visible restoration fallback.
- [x] Update build prerequisites, CI, user documentation and contributor role assignments.
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
- [x] Add only focused state/restoration tests that protect observable behaviour.
- [x] `cargo fmt --all -- --check`
- [x] `cargo clippy --all-targets --locked`
- [x] `cargo test --locked`
- [x] `cargo build --release --locked`, after the gate passes
- [ ] Run full CI on Linux, Windows and macOS; distinguish build coverage from native
  interaction actually exercised, and record any remaining platform limitations.

## Post-completion

Keep the PR in Draft for the owner's visual review. Opening files remains a later
task; do not begin it as part of this change.

## Validation notes

- Native Linux startup succeeded through both Wayland and X11 with desktop access.
  Window-only screenshots confirmed the Argand-style frame, empty Recent, idle
  footer and layout at normal and narrow sizes.
- External input injection requested desktop remote-control permission. It was
  stopped, and no further desktop input automation is used. Menu selection, live
  theme switching and the complete maximize/minimize/relaunch interaction sequence
  remain manual owner checks. Seeded state or a screenshot is not interaction proof.
- The corrected small optical artwork uses the canonical ring/arrow geometry and
  round dots, with coral/outline strokes reduced to 7/9 view-box units. A 64-pixel
  export is displayed at 16 logical pixels. The large canonical artwork is unchanged.
- Actual KWin icon pixels from a fresh native Wayland window verified the canonical
  application icon at 80x80 physical pixels on the fractional-scale display. Named
  theme lookup had produced an empty icon despite matching application identity;
  the generated per-user desktop entry now uses a validated absolute installed PNG
  path. Standard scalable, 48- and 256-pixel icons are also registered. Only Wireroom
  files change, differing prior files are backed up, and old owner windows remain
  untouched. This verifies native icon resolution, not menu or playback interaction.
- The development dependency-symbol reduction addresses avoidable build storage.
  Cargo reported removing 14.4 GiB of stale development artifacts; the targeted
  cleanup also removed the
  separate obsolete Cargo output directories. Fixtures and evidence were retained.
  The fresh development gate/build uses 3.2 GiB; `wr` is 161 MiB rather than the
  previous 694 MiB debug executable. Registry dependency symbols are omitted;
  workspace debug information remains enabled.
- The final formatting and Clippy checks passed. Fresh full tests passed in 3m20s;
  all three focused state tests passed, as did all eight conditional policy tests
  (3.66s execution). The subsequent debug build completed in 7.9s. The corrected release rebuild passed;
  cross-platform CI remains pending at the implementation review snapshot.

## Mechanical review corrections

- Round 1 accepted all three findings: use the same complete-window bounds for
  creation and persistence (inner bounds subtract the client shadow on every run),
  save state on application quit as well as close, and return a failing exit status
  when native window creation fails. No findings were declined.
- The previous manual X11 growth observation did not establish matching creation
  and reported units and was insufficient evidence for using inner bounds globally.
  Wayland source and actual geometry showed the inner-bounds change shrinking the
  visible window by 24 logical pixels per run. Persistence now uses window_bounds
  directly, matching WindowOptions, without a guessed shadow correction.

- Corrected snapshot validation passed formatting, Clippy, full tests and all eight
  conditional policy tests with two jobs at lowered scheduler priority. The GUI-only
  release symbol-strip override is supported by Cargo; its incremental rebuild
  passed in 23.65s and reduced `wr` from 49.68 MiB to 36.31 MiB. The debug binary is
  161.74 MiB. Development artifacts occupy 3.6 GiB and release artifacts 2.0 GiB
  after the clean rebuild. Compilation concurrency is capped at two jobs after the
  owner reported desktop responsiveness problems.
