# Wireroom

Wireroom is an operator's console for demodulating recorded signals: tune to a signal,
set processing parameters, run, and watch progress as a log and in separate panes for
decoded text, bits, callsigns and the phase constellation.

Wireroom is the product, and its interface binary will be `wr`. Between Wireroom and
the demodulation libraries sits `dmd`, a command-line tool that gives Wireroom one
uniform way to reach demodulation regardless of what is behind it. Wireroom starts
`dmd` for a run and reads its events; it never loads a demodulation library itself and
never links a signal core.

`dmd` is also a complete tool in its own right, usable directly from a terminal for
demodulating, inspecting modules and checking results.

`dmd` resolves what is behind it from the contents of its search path:

- **Native** performs real demodulation through a signal core and dynamically loaded
  demodulation libraries. It is enabled by the `native` Cargo feature.
- **Replay** reproduces prepared recordings of earlier runs. It is always available and
  needs no native toolchain, so Wireroom can be developed, tested and demonstrated
  without the demodulation stack present.

Both paths share one command-line surface, one event protocol and one set of result
files. Wireroom cannot tell them apart beyond the data they produce, which is what
makes a recording a usable stand-in for a real run.

## Status

Early bootstrap. Work starts from `dmd` and the shared protocol types. The Wireroom
interface will be built with GPUI; its accepted window concept is in
[`docs/ui`](docs/ui/README.md). The event protocol and the recording format are being
specified before implementation. The [run protocol](docs/protocol.md) defines
the process exchange and records the remaining producer-schema limitations. The
[replay recording contract](docs/replay.md) defines companion archives, validation
and fixed-result playback. See `docs/plans/` for the active work.

## Building

```sh
# Command-line tool with replay only: no C++ toolchain required
cargo build -p dmd --no-default-features

# Full build, including native demodulation
cargo build -p dmd --features native
```

The full build links a prebuilt signal core; see `CONTRIBUTING.md` for the environment
variables that locate it.

## Contributing

`CONTRIBUTING.md` describes the issue-driven workflow, the validation gate and the
review process. `AGENTS.md` carries the architectural invariants that every change must
preserve.

## License

MIT. See `LICENSE`.
