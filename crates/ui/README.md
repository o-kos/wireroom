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
changes, normal close and application quit. Missing state uses a centred 1100×720 window. Invalid or
unreadable state reports a diagnostic and does not prevent startup; unknown versions
are left unchanged. Atomic replacement preserves the previous complete file if a
write fails. Write errors appear in the status bar and stderr. Concurrent instances
use last-writer-wins state; the shell does not merge sessions.

Restoration fits the saved rectangle to available displays. Wayland compositors own
window position; only size and maximized state can be requested. Window managers may
also override geometry. Saved bounds use the same complete-window rectangle as window creation;
hidden-window reports do not replace the last visible normal bounds. Fullscreen is not exposed or persisted separately.

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

The existing SVG artwork is authoritative. The application button follows the
public Argand component exactly: a 22-logical-pixel `img`, a 26-pixel button,
6-pixel horizontal padding and the same measured label/gap sizing. A 64-pixel PNG
export supplies the icon. The small optical source retains canonical ring/arrow
coordinates and four round dots, with a thin coral stroke and no navy outlines.
Its view-box padding matches the reference logo's visible fill without enlarging
the button box. X11 receives a 256-pixel primary-artwork window icon through the
toolkit.
Linux identifies the window as `io.github.o_kos.wireroom`; run
`scripts/install-wr-desktop.sh target/debug/wr` to register the current user's
desktop entry, scalable SVG and 48/256-pixel PNG icons. The helper records the selected executable's absolute
path and a direct installed PNG icon path, preserves different prior Wireroom
files as backups and refuses an unrelated
desktop entry. Rerun it with `target/release/wr` to use the release binary. It does
not install or copy the executable. Remove the installed Wireroom entry and icon files to
undo registration, restoring any backups. An uninstalled desktop
entry may leave the task switcher's generic icon, depending on the compositor.

Windows builds embed resource 1 from `assets/wireroom.ico`, matching GPUI's native
window icon lookup. On macOS, run `scripts/bundle-wr-macos.sh` after a release build
and open `target/Wireroom.app` for the Dock/application icon; an unbundled executable
inherits the launching application's icon. The helper creates an unsigned local
bundle and does not install it. The toolbar PNG is exported with `rsvg-convert -w 64 -h 64` from
`assets/icons/wireroom-16.svg`. The small optical SVG retains the primary ring/arrow
geometry and round marks with thinner ring strokes for small-size legibility.
ICO and ICNS assets are raster exports of
`assets/icons/wireroom.svg`, with no build-time image conversion.

## Reference

The application button uses Argand's custom Off/On surfaces and a popover trigger
that does not force toolkit selection. Off hover/pressed surfaces use the toolbar
accent at 32%/44% opacity; open surfaces use 30%/40%/52%. The accent is `blue_light`
in Dark and `blue.darken(0.2)` in Light. Active and inactive title bars retain the
reference's separate `title_bar` and `background` roles.

Frame geometry and neutral theme surfaces are adapted from the public
[Argand](https://github.com/o-kos/argand) project. Its MIT notice is preserved in
[ARGAND-LICENSE](ARGAND-LICENSE). Ordinary controls use GPUI Kit components; this
shell contains no plot rendering or signal-processing integration.

## Development build size

Cargo limits compilation to two parallel jobs in `.cargo/config.toml` to keep the
interactive desktop responsive. For a background validation run, use
`nice -n 10 cargo build --locked` (and the same prefix for tests or Clippy).

The development profile omits debug symbols for registry dependencies while keeping
workspace crate debugging. The GUI dependency graph is substantially larger than
the replay CLI; Cargo retains separate check, build, test and release artifacts.
Changing profiles or dependencies can leave obsolete artifacts until Cargo cleanup.
This affects build storage, not the executable's runtime memory.

The GUI release package strips symbols, reducing the measured executable from
49.68 MiB to 36.32 MiB. Dependency optimization, CLI release builds and development
debug information are unchanged; no additional LTO or code-generation changes are
used to obtain that reduction.
