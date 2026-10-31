# Wireroom

Wireroom is an operator's console for demodulating recorded signals: tune to a signal,
set processing parameters, run, and watch progress as a log and in separate panes for
decoded text, bits, callsigns and the phase constellation.

Wireroom is the product; its interface binary is `wr`. It launches a selected
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
asset publication. Its output trust assumption and resource limits are documented
in the [CLI runtime guide](crates/cli/README.md). Source digest verification is
deferred; recordings use the small classic ZIP and ordinary JSON profiles.

The native `wr` shell provides the Argand-based window frame, application menu,
System/Light/Dark appearance, persisted window placement and Recent empty/list view.
File opening and processing controls are unavailable in this step; no file is opened
and no `dmd` process is started. See the [interface guide](crates/ui/README.md).
Other CLI commands remain unimplemented. The accepted full window concept is in
[`docs/ui`](docs/ui/README.md).
See `docs/plans/` for active work.

## Building

```sh
cargo build --locked
cargo build -p dmd --no-default-features --locked
```

Cargo limits compilation to two parallel jobs to keep the desktop responsive.
Background validation commands can use `nice -n 10` to yield scheduler priority.

The replay CLI needs only Rust and its platform linker. The native `wr` window
also needs the [GPUI platform libraries](crates/ui/README.md#platform-prerequisites). Replay uses portable output
publication and trusts caller-selected output and its ancestry to remain stable
during the run. An independently supplied core implementation can implement
the same CLI and protocol contracts without sharing internal runtime interfaces.

## Contributing

`CONTRIBUTING.md` describes the issue-driven workflow, the validation gate and the
review process. `AGENTS.md` carries the architectural invariants that every change must
preserve.

## License

MIT. See `LICENSE`.
