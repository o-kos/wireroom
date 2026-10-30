# Project context

## Purpose and architecture

Wireroom is the product: an application for demodulating recorded signals and
inspecting results. It starts `dmd` and consumes its events. `dmd` is also a complete
command-line tool. Native processing is future work; its integration boundary will
be agreed when an actual implementation is available. The interface toolkit is GPUI,
through GPUI Kit, following the component principles of
[Argand](https://github.com/o-kos/argand). The planned interface crate will build
the `wr` binary, while `dmd` keeps its name.

## Workspace

- `crates/protocol`: shared event types and bounded NDJSON codecs.
- `crates/replay`: read-only recording validation and payload streaming.
- `crates/source`: read-only source metadata and retained file access.
- `crates/cli`: command-line surface and backend selection.
- `assets/icons/`: colour and monochrome Wireroom icon sources, with optical variants
  for small sizes.
- `docs/plans`: active plans; `docs/plans/completed`: finished plans.
- `docs/ui`: the accepted window concept and its interactive mock-up.
- `.github/scripts`: CI decision logic; `scripts`: local policy checks.

The event protocol and recording contract are specified in `docs/protocol.md` and
`docs/replay.md`. Runtime execution and playback controls remain future work.

## Commands

Use the toolchain pinned by `rust-toolchain.toml`.

```sh
cargo build
cargo run -p dmd -- --help
cargo run -p dmd -- --backend replay list
cargo fmt --all -- --check
cargo clippy --all-targets --locked
cargo test --locked
cargo build --release --locked
cargo build -p dmd --no-default-features
```

The CLI currently reports that each command is not implemented. There is no native
Cargo feature or signal-core integration in the current workspace.

## Architectural invariants

- Current workspace builds and tests require no Python, CMake, C++ toolchain or signal
  core. Future native integration must keep replay-only builds Rust-only.
- There is no silent fallback. A core that fails to load or a corrupt recording is an
  error, never a quiet switch to the other backend.
- Sample positions are frames of the source file. For IQ, one frame is an I/Q pair.
- Paths from recordings and result identifiers stay inside the output directory.
  Never delete an arbitrary directory recursively.
- Any future C ABI must keep STL types and C++ exceptions behind the boundary. Memory
  must be released by its allocator's side, with Rust RAII closing resources. Panics
  must not unwind across FFI boundaries.
- Future native execution must never silently ignore replay-only options.
- Event protocol and recording format versions are independent; any future C ABI
  must have its own version.
- Use `thiserror` in libraries and `anyhow` in binaries. Non-test code must not
  `unwrap` or `expect` on external data.
- Every lint suppression, including a relaxed threshold, requires project-owner
  agreement before pushing. Refactor first; use `#[expect(..., reason = "...")]`
  when unavoidable. See `CONTRIBUTING.md` for the full suppression rule.

## Engineering and workflow

Write all repository content in English.

Ask the owner questions one at a time or number them, so each can be answered by number.

Recommend starting a new session when it helps, for example when a task is finished and the
next one is unrelated, or when the context has grown long. Offer a ready prompt for that session
at the same time.

Do not name or imply other repositories, internal hosts, organisations or filesystem
paths outside this repository, or describe prior or parallel implementations. Public
developer infrastructure, published standards and conventions, tool-mandated
filenames and the public Argand project are outside this disclosure rule's scope.

Dependencies may use the package registry or workspace paths, never external path or
git sources.

- Warnings are errors as a repository property, never a `-D warnings` flag to Cargo.
- Run the full local gate before every push.
- Select workspace lints individually and inherit them in every crate; document each
  threshold in `clippy.toml`. Never enable lint groups wholesale.
- Review for opaque nested conditional chains; see `CONTRIBUTING.md` for examples.
- Comment only intent that is not evident from the code, and keep comments concise.

Start with an Issue containing verifiable acceptance criteria, then a branch named
`<type>/<issue>-<short-description>`. Commit a plan before implementation and open a
Draft Pull Request early with a closing keyword. Complete each plan checkbox in the
commit that completes its work; never record commit hashes. Commit atomic changes with
English imperative messages and no attribution trailers or generation notices.

Never state or imply in commit messages, Pull Requests, issues or release notes that
an AI tool was used during development. Do not add co-author trailers or generation
notices. Contributor and agent instructions describing the tooling are outside this
rule's scope.

Claude is unavailable. Use Codex only, with the fixed roles in `CONTRIBUTING.md`.
Pass model and reasoning effort explicitly on every invocation: `gpt-6.1-sol` at
medium for planning, dispatch, Git and Pull Request routine, reconnaissance and
implementation; `gpt-6.1-sol` at high for mechanical and intent review. Reconnaissance
and reviews are read-only; implementation uses workspace-write. These assignments
replace previous roles, including for unfinished work. Keep the reviewer settings
fixed across subsequent rounds. Run the reviewer read-only with `codex exec`, an
explicit model and effort, a change-specific prompt and closed stdin as documented
in `CONTRIBUTING.md`.

Mechanical review repeats until nothing substantive remains, challenging every declined
finding in later rounds, for at most three rounds. If substantive findings remain after the
third round, stop and give the owner a summary of them with a recommendation; the owner
decides how to proceed. The orchestrator then checks the final diff against the plan
and invariants, with at most two correction rounds before escalation to the owner.
Return one structured findings list to the implementer per round; agents do not
negotiate, and the orchestrator decides. Summarise accepted and declined findings,
their reasons and fixes, and whether the final round was clean when requesting owner
review.

Install `.githooks` with `git config core.hooksPath .githooks`. Run the validation gate
before review. Route out-of-scope findings to separate Issues. Complete external review,
move the plan to `completed/`, rebuild release, mark Ready, and merge by squash only.
A merge waits for the required `ci/full` check to succeed on a branch that is up to date
with `main`.
See `CONTRIBUTING.md` for the complete lifecycle and bypass rules.
