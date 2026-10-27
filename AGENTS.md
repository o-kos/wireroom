# Project context

## Purpose and architecture

Wireroom is the product: an application for demodulating recorded signals and
inspecting results. It starts `dmd` and consumes its events. `dmd` is also a complete
command-line tool. Only the optional native layer reaches the signal core through the
C ABI. Everything implementing that ABI is outside this repository's scope. The
interface toolkit is GPUI, through GPUI Kit, following the component principles of
[Argand](https://github.com/o-kos/argand). The interface crate builds the `wr` binary,
while `dmd` keeps its name.

## Workspace

- `crates/protocol`: shared protocol boundary; currently only its version.
- `crates/cli`: command-line surface and backend selection.
- `crates/native`: safe resource ownership over the C ABI.
- `crates/core-sys`: C header, matching raw declarations and static linking.
- `assets/icons/`: colour and monochrome Wireroom icon sources, with optical variants
  for small sizes.
- `docs/plans`: active plans; `docs/plans/completed`: finished plans.
- `docs/ui`: the accepted window concept and its interactive mock-up.
- `.github/scripts`: CI decision logic; `scripts`: local policy checks.

The event protocol, recording format and result layout belong to later work.

## Commands

Use the toolchain pinned by `rust-toolchain.toml`.

```sh
cargo build
cargo run -p dmd -- --help
cargo run -p dmd -- --backend replay list
cargo fmt --all -- --check
cargo clippy --all-targets --locked
# Native Clippy with an empty core directory: see CONTRIBUTING.md for exact commands.
cargo test --locked
cargo test -p dmd-native --locked
cargo build --release --locked
cargo build -p dmd --no-default-features
cargo build -p dmd --features native
```

The CLI currently reports that each command is not implemented. Native builds require
`DMD_CORE_LIB_DIR`; `DMD_CORE_LIB_NAME` defaults to `dmd_core`, and
`DMD_CORE_EXTRA_LIBS` optionally lists additional link targets separated by semicolons.

## Architectural invariants

- `native` is not a default feature. Optional dependencies isolate default builds;
  the core linking script is inert without its own `native` feature, even when selected
  explicitly or through `--workspace`.
- Replay-only builds and tests require no Python, CMake or C++ toolchain.
- There is no silent fallback. A core that fails to load or a corrupt recording is an
  error, never a quiet switch to the other backend.
- Sample positions are frames of the source file. For IQ, one frame is an I/Q pair.
- Paths from recordings and result identifiers stay inside the output directory.
  Never delete an arbitrary directory recursively.
- No STL types or C++ exceptions cross the C ABI. Memory is released by its allocator's
  side, and Rust RAII closes resources. Panics must not unwind across FFI boundaries.
- Replay-only options are never silently ignored during a native run.
- Event protocol, recording format and C ABI versions are independent.
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

Use the fixed roles in `CONTRIBUTING.md`, passing model and reasoning effort explicitly
on every invocation: Claude Opus at high for planning and intent review; Claude Sonnet
at medium for dispatch, Git and Pull Request routine; Codex `gpt-6-astra` at medium,
read-only for reconnaissance and at high, workspace-write for implementation. The
mechanical reviewer is Codex `gpt-5.6-sol` at high for Codex GPT-6 Astra implementations,
or Codex `gpt-6-astra` at high for Claude implementations; keep that choice for all rounds.
Run the reviewer read-only with `codex exec`, an explicit model and effort, a
change-specific prompt and closed stdin as documented in `CONTRIBUTING.md`.

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
