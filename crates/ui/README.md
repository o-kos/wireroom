# Wireroom window shell

```sh
cargo run -p wireroom --bin wr --locked
```

`wr` opens the native Wireroom shell: the Argand-style frame, a toolbar in the
34-pixel title bar, the application menu, Recent and an idle status bar. File
opening, processing and playback are unavailable; disabled controls cannot open a
picker or start `dmd`. No sample recent files or signal data are supplied.

Click the Wireroom button or press F10 for the menu. File contains Open (disabled),
Recent and Quit. Recent displays stored history and can clear it; entries cannot
open files in this step. Appearance selects System, Light or Dark. System follows
live platform appearance changes. Menus use the toolkit's arrows, Enter and Escape.

Window placement, last normal size, maximized state, display identity when available,
appearance and at most ten recent paths are stored in `wireroom/session.json` under
the platform's per-user configuration directory. State is saved on appearance/history
changes and normal close. Missing state uses a centred 1100×720 window. Invalid or
unreadable state reports a diagnostic and does not prevent startup; unknown versions
are left unchanged. Atomic replacement preserves the previous complete file if a
write fails. Write errors appear in the status bar and stderr. Concurrent instances
use last-writer-wins state; the shell does not merge sessions.

Restoration fits the saved rectangle to available displays. Wayland compositors own
window position; only size and maximized state can be requested. Window managers may
also override geometry. Bounds exclude the client shadow to prevent growth across
relaunches; hidden-window reports do not replace the last visible normal bounds. Fullscreen is not exposed or persisted separately.

## Platform prerequisites

Use the pinned Rust toolchain. Dependencies come from the package registry through
GPUI Kit 0.6. The replay CLI can still be built independently with
`cargo build -p dmd --locked` and requires only Rust and its ordinary linker.

On Debian/Ubuntu, the GUI additionally needs:

```sh
sudo apt-get install libasound2-dev libfontconfig-dev libvulkan-dev libwayland-dev \
  libxcb1-dev libxkbcommon-dev libxkbcommon-x11-dev
```

A working Wayland or X11 session and a Vulkan-capable graphics driver are needed to
run the Linux window. CI builds and state tests do not require a display or GPU.
macOS needs Xcode command-line tools and its SDK; Windows needs the usual MSVC build
tools and Windows SDK, including the resource compiler. Python, CMake and a signal
core are not required. The GUI's platform dependencies can compile small C helpers.

## Application icon

The existing SVG artwork is authoritative. The toolbar uses a 64-pixel PNG export of its small optical
variant, displayed at 16 logical pixels to keep fractional-DPI rendering sharp. Linux identifies the window as `io.github.o_kos.wireroom`; run
`scripts/install-wr-desktop.sh target/debug/wr` to register the current user's
desktop entry and SVG icon. The helper records the selected executable's absolute
path, preserves different prior Wireroom files as backups and refuses an unrelated
desktop entry. Rerun it with `target/release/wr` to use the release binary. It does
not install or copy the executable. Remove the two installed Wireroom files to
undo registration, restoring any backups. An uninstalled desktop
entry may leave the task switcher's generic icon, depending on the compositor.

Windows builds embed resource 1 from `assets/wireroom.ico`, matching GPUI's native
window icon lookup. On macOS, run `scripts/bundle-wr-macos.sh` after a release build
and open `target/Wireroom.app` for the Dock/application icon; an unbundled executable
inherits the launching application's icon. The helper creates an unsigned local
bundle and does not install it. The toolbar PNG is exported with `rsvg-convert -w 64 -h 64` from
`assets/icons/wireroom-16.svg`. ICO and ICNS assets are raster exports of
`assets/icons/wireroom.svg`, with no build-time image conversion.

## Reference

Frame geometry and neutral theme surfaces are adapted from the public
[Argand](https://github.com/o-kos/argand) project. Its MIT notice is preserved in
[ARGAND-LICENSE](ARGAND-LICENSE). Ordinary controls use GPUI Kit components; this
shell contains no plot rendering or signal-processing integration.

## Development build size

The development profile omits debug symbols for registry dependencies while keeping
workspace crate debugging. The GUI dependency graph is substantially larger than
the replay CLI; Cargo retains separate check, build, test and release artifacts.
Changing profiles or dependencies can leave obsolete artifacts until Cargo cleanup.
This affects build storage, not the executable's runtime memory.
