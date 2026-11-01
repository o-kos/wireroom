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
- Match Argand's actual application-button metrics and component behavior: a
  22-logical-pixel `img`, a 26-pixel button, 6-pixel horizontal padding, `gap_1`,
  measured label width and the reference custom Off/On surface formulas. Use the
  reference trigger wrapper so a popover cannot replace those surfaces with the
  toolkit's selected state. Preserve active/inactive title-bar palette roles.
- Export the small optical artwork at 64 pixels, retaining canonical ring/arrow
  coordinates and four round dots with a thin coral stroke and no navy outlines.
  Match visible logo fill by adjusting view-box padding, not enlarging the 22-pixel
  image box. Keep the large canonical/native artwork unchanged. Register the
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

## Implementation status

The shell implementation, final mechanical and intent reviews, and full cross-platform
CI are complete. The owner explicitly accepted PR #29 for lifecycle completion.
That acceptance does not itemize the interaction checks below; unverified details
remain recorded as such.

- [x] Add the native shell, assets and Argand-based frame with window controls.
- [x] Add application menus, toolbar, status bar and shared Recent empty/list states.
- [x] Implement appearance and window-state persistence with visible restoration fallback.
- [x] Update platform prerequisites, CI, documentation and contributor role assignments.
- [x] Record native evidence and complete the local gate and release build.
- [x] Address independent mechanical review and prepare the Draft PR for owner review.
- [x] Move this implementation record to `docs/plans/completed/` before intent review.
- [x] Complete independent intent review.
- [x] Complete full CI on Linux, Windows and macOS.
- [x] Obtain the owner's acceptance of the implementation for lifecycle completion.

## Validation

- [x] Three focused state tests cover visible restoration, preservation of normal
  bounds across maximization, atomic persistence, malformed data and future versions.
- [x] `cargo fmt --all -- --check`
- [x] `cargo clippy --all-targets --locked`
- [x] `cargo test --locked`
- [x] All eight conditional policy tests.
- [x] `cargo build --release --locked`, after the gate.
- [x] Native Linux startup through Wayland and X11; window-only screenshots of the
  frame, empty Recent, idle footer and normal/narrow layout.
- [x] Native Wayland application icon: actual KWin icon pixels verified at 80x80
  physical pixels on the fractional-scale display.
- [x] Reference-size artwork: canonical ring/arrow coordinates and round marks,
  coral stroke 7 without navy outlines, exported at 64 pixels and displayed at the
  reference 22-logical-pixel size. Compare visible fill at 125% scale.
- [x] Read-only capture of the owned active X11 window confirmed the normal button
  with the corrected image size, optical fill and custom resting surface.

The final gate used two parallel Cargo jobs and lowered scheduler priority with
`nice -n 10`. The fresh full test build completed in 3m20s; later cached checks also
passed. The GUI-only release symbol-strip override rebuilt in 23.65s and reduced
`wr` from 49.68 MiB to 36.32 MiB. The debug binary is 161.94 MiB; workspace debug
information remains enabled. Dependency optimization and CLI release builds are
unchanged; no LTO or code-generation change was needed.

Development dependencies omit debug symbols. After targeted cleanup and the fresh
build, development and release caches occupied approximately 3.6 GiB and 2.0 GiB.
The cleanup removed obsolete Cargo outputs and separately inventoried old task
logs, prompts and fixtures. Current build caches, unknown data and the uncertain
`demo` directory were retained. These are point-in-time build-storage measurements,
not the application's runtime memory use.

## Native verification limits and owner checklist

External input injection requested desktop remote-control permission. It was
stopped; no further desktop input automation was used. No permission for remote
control is required to run Wireroom. Screenshots and seeded state do not establish
interactive behavior. The following checks remain explicitly unverified:

- [ ] Click the application button; navigate File, Recent and Appearance with the
  pointer and keyboard; dismiss menus through their ordinary paths.
- [ ] Select System, Light and Dark; check live System changes and persistence on
  relaunch.
- [ ] Move, resize, minimize, maximize/restore and close the window; verify normal
  bounds and maximized state on relaunch, including application-level Quit.
- [ ] Check the same real Recent list in the menu and main area, clear it and verify
  persistence, without introducing sample product data.
- [ ] Confirm disabled file and processing controls cannot open a picker, open a
  file or start `dmd`.

The desktop identity matched while named theme lookup still produced an empty
native icon. The generated per-user desktop entry now uses the validated absolute
installed PNG path. Standard scalable, 48- and 256-pixel icons are also registered.
Only Wireroom integration files change; differing prior files are backed up. Native
icon resolution was verified on a fresh window; existing owner windows were left
untouched. This evidence does not claim menu or window-lifecycle interaction passed.

## Review and CI handoff

Mechanical round 1 identified three substantive P2 findings. All were accepted:

- Persist the complete `window_bounds()` rectangle in the same units as creation;
  inner bounds would subtract the client shadow on each run.
- Save the latest live window state on application Quit as well as window close.
- Return a failing process exit status when native window creation fails.

No findings were declined. Mechanical round 2 was clean and confirmed the three
corrections. Final mechanical round 3 and the intent follow-up were clean, including
the reference application-button style and visible-fill comparison.

The final application-button code passed [full CI](https://github.com/o-kos/wireroom/actions/runs/41251992928)
on Linux, Windows and macOS, including release and standalone CLI builds and the
`ci/full` status check. Its [quick CI](https://github.com/o-kos/wireroom/actions/runs/41251981324)
also passed. Native interaction coverage remains distinct from cross-platform build
and test coverage.

The owner accepted the implementation and authorized the Ready transition and
squash merge after the required current-head `ci/full` check succeeds. Opening files,
starting `dmd`, playback and domain panels remain separate future work.

## Application-button reference check

The original 16-pixel button image and generic ghost variant did not match Argand.
The final implementation uses the reference's `img` pipeline and exact 22/26/6
image/height/padding metrics, `gap_1`, measured label width, custom Off/On formulas,
TopLeft popover anchor and non-selecting trigger wrapper. Theme adjustments and the
active/inactive title-bar palette are unchanged and match the reference source.
No always-black title-bar override was introduced.

Measured at half-opacity, Argand's actual 24-pixel PNG fills 91.67% of its width and
83.33% of its height. The previous Wireroom export filled 82.81% and 75%, making its
visible logo smaller even in an equal image box. The corrected optical view box
fills 90.63% and 84.38%, within 0.24 logical pixels of the reference at a 22-pixel
image size. Canonical path coordinates, circle locations and the large/native icon
are unchanged. The normal active-window result was captured through read-only X11
access to the task-owned window only; hover/open interaction was not automated.

The final button snapshot passed formatting, Clippy, full tests, all eight conditional
policy tests and the release build with two jobs at lowered priority. The cached
release build completed in 9.78s; its binary is 36.32 MiB. The focused final
mechanical round, intent follow-up and full cross-platform CI subsequently passed.
The owner interaction checklist remains unchecked.
