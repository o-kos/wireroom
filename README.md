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

The [run protocol](docs/protocol.md) and [replay recording contract](docs/replay.md)
are specified, with producer schemas still deferred. The shared
[`dmd-protocol`](crates/protocol/README.md) library implements individual messages
and bounded NDJSON codecs. The read-only [`dmd-replay`](crates/replay/README.md)
library validates companion archives and metadata, reads journal boundaries and
streams payloads with integrity checks.

Runtime, playback controls and the Wireroom interface remain future work; the CLI
commands still report that they are not implemented. The interface will use GPUI;
its accepted window concept is in [`docs/ui`](docs/ui/README.md). See `docs/plans/`
for the active work.

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
