# Wireroom

Wireroom is an operator's console for demodulating recorded signals: tune to a signal,
set processing parameters, run, and watch progress as a log and in separate panes for
decoded text, bits, callsigns and the phase constellation.

Wireroom is the product; its interface binary will be `wr`. It launches a selected
`dmd` executable and consumes the common run protocol. `dmd` is also a standalone
command-line tool. Replay and core are separate implementations of that application.
This repository develops only replay; it does not load a signal core or select
processing implementations internally.

## Status

The [run protocol](docs/protocol.md) and [replay recording contract](docs/replay.md)
are specified, with producer schemas still deferred. The shared
[`dmd-protocol`](crates/protocol/README.md) library implements individual messages
and bounded NDJSON codecs. The read-only [`dmd-replay`](crates/replay/README.md)
library validates companion archives and metadata, reads journal boundaries and
streams payloads with integrity checks. The read-only
[`dmd-source`](crates/source/README.md) library inspects strict PCM WAVE headers,
reports exact original-source frames and retains independent cursors over the
opened signal identity.

`dmd run --message-format json` implements replay lifecycle, FIFO controls, pacing,
asset publication and background diagnostic hashing. Its supported output
configurations and resource limits are documented in the
[CLI runtime guide](crates/cli/README.md). Other commands and the Wireroom interface
remain future work. The interface will use GPUI; its accepted window concept is in
[`docs/ui`](docs/ui/README.md). See `docs/plans/` for active work.

## Building

```sh
cargo build --locked
cargo build -p dmd --no-default-features --locked
```

The workspace needs only Rust and its platform linker. Replay output currently
requires a supported Linux configuration; other platforms build the CLI and report
an explicit setup error. An independently supplied core implementation can implement
the same CLI and protocol contracts without sharing internal runtime interfaces.

## Contributing

`CONTRIBUTING.md` describes the issue-driven workflow, the validation gate and the
review process. `AGENTS.md` carries the architectural invariants that every change must
preserve.

## License

MIT. See `LICENSE`.
