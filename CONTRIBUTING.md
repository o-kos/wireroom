# Contributing to Wireroom

Wireroom's interface binary will be `wr`, and `dmd` is its command-line tool.

## 1. Define the change

Create an Issue using the bug or change form. Describe observable behavior, scope,
constraints and acceptance criteria that a reviewer can verify. Route unrelated bugs,
design questions and improvements to separate Issues; link them without expanding the
current scope silently.

## 2. Plan before implementation

Create a branch from current `main` named `<type>/<issue>-<short-description>` (for
example, `docs/1-project-workflow`). Copy `docs/plans/TEMPLATE.md` to a numbered plan in
`docs/plans/`. Record decisions, rejected alternatives, steps and validation. Commit
that plan before implementation. This bootstrap's pre-existing branch is the sole
exception to that ordering.

All work, including the implementer's, happens on the Issue branch in the main
checkout. Use a separate worktree only when the owner asks for one.

Open a Draft Pull Request early. Include a closing keyword such as `Resolves #1`, a
link to the plan and the intended behavior. Keep the description aligned with the final
change. Use English throughout.

## 3. Implement and validate

Do not name or imply other repositories, internal hosts, organisations or filesystem
paths outside this repository, or describe prior or parallel implementations. Public
developer infrastructure, published standards and conventions, and tool-mandated
filenames are outside this disclosure rule's scope.

Make atomic commits with imperative English messages. Tick each plan step in the same
commit that completes it. Never record commit hashes in plans. Keep the authoritative
plan current when a decision changes, and explain the technical reason.

Never state or imply in commit messages, Pull Requests, issues or release notes that
an AI tool was used during development. Do not add co-author trailers or generation
notices. Contributor and agent instructions describing the tooling are outside this
rule's scope.

Install the local gate once per checkout:

```sh
git config core.hooksPath .githooks
```

The pre-push hook refuses direct pushes to `main`: changes reach it through a Pull
Request. It checks outgoing commit messages, filenames and added text for private
filesystem locations (home directories, home-relative paths, mount locations, Windows
drive paths and UNC paths), paths escaping the tree, e-mail addresses and unapproved
URL or remote-path hosts. Other slash-prefixed tokens and bare host-like names pass.
The small generic host allowlist lives in `scripts/content_policy.rs`.
Environment-specific names belong in `dmd.localPolicy`. This is a text hygiene check,
not a secrets scanner. Avoid binary additions that require text review without a
separate review of their contents.

The hook reports whether `dmd.localPolicy` is configured. When configured, it must name
an executable file; empty or invalid values fail the gate. Relative values are relative
to the repository top level. The hook resolves the value to an absolute path before
validating it and executes exactly that path, with the same arguments and ref-update
input as pre-push, propagating failure. Keep environment-specific policy outside the tree.
Configure it with `git config dmd.localPolicy <executable>`.

The content policy runs on every push. The hook caches its compiled binary in
`target/policy/`, rebuilding when the source hash changes or the binary is missing.
Policy and hook tests are ignored by the default test run. Run them by hand with:

```sh
cargo test -p dmd-protocol --test policy --locked -- --ignored
```

The hook runs these tests when any outgoing commit touches `scripts/content_policy.rs`,
`.githooks/pre-push` or `crates/protocol/tests/policy.rs`. For a new branch, it considers
only commits not already on the destination remote. Both CI tiers run the same command
when those paths differ from the Pull Request base SHA, the push event's `before` SHA
for a push to `main`, or `HEAD^` for a manual run. If the base is empty, all zeros or
unavailable in the fetched history, CI runs the policy tests unconditionally. The
`status` job tests its own decision logic separately.

Run the full local gate before every push. It stops at the first failed command and
explains the failure:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --locked
mkdir -p target
native_lib_dir=$(mktemp -d "$PWD/target/native-clippy.XXXXXX")
DMD_CORE_LIB_DIR="$native_lib_dir" cargo clippy -p dmd --features native --all-targets --locked
rmdir "$native_lib_dir"
cargo test --locked
cargo test -p dmd-native --locked
```

The native Clippy step type-checks and lints native code without a core. Its library
directory is empty: Clippy checks the native feature without building or linking the
application binary. Both CI tiers run this step with Bash: on Linux for Draft Pull
Requests and on Linux, Windows and macOS for the full tier.
The separate native test step runs the core-independent tests that default members exclude.

Use `git push --no-verify` only as an explicit escape hatch. State the reason and the
checks bypassed in the Pull Request. A bypass does not replace validation or review.
Server-side protection requires a Pull Request to change `main`. No approving review
count is required; stale approvals are dismissed when new commits arrive. The required
`ci/full` check must come from GitHub Actions. History must be linear and all review
conversations must be resolved. Force pushes to `main` and its deletion are forbidden.
Administrators are not included in the protection, so an administrator can still push
to or merge into `main` directly. For that reason the hook, an early local copy of the
gate, also refuses direct pushes to `main`. Server-side merges do not execute it.

Draft Pull Requests run formatting, default and native lints, conditional policy tests
and core-independent native tests on Linux, producing `ci/quick`. Ready Pull Requests,
pushes to `main` and manual runs require Linux, Windows and macOS tests and release
builds, producing `ci/full`. Skipped results do not count as successes. The required
check on `main` is `ci/full`, and the branch must be up to date with `main` before
merging. Draft Pull Requests publish only `ci/quick`, so a Pull Request cannot merge
until it is Ready and `ci/full` succeeds.

### Lint policy

`[workspace.lints]` names every lint enforced beyond Clippy's defaults. Every crate
inherits it with `lints.workspace = true`. Thresholds live in `clippy.toml`, with one
comment per value explaining what it protects against. Warnings are errors as a
repository property, never through a `-D warnings` flag passed to Cargo.

Enable lints one at a time. Never enable the `pedantic`, `nursery` or `restriction`
groups wholesale: they contain lints that contradict this codebase and each other,
and a group grows silently on a toolchain bump. Adding a lint or changing a threshold
is deliberate; explain why in the Pull Request.

When a maintainability lint fires, change the code first. A long function is a
sequence of stages that has not been named; a wide signature is a type that has not
been written. Raising a threshold requires the agreement described under
[Lint suppressions](#lint-suppressions).

Watch the surveying trap: `warnings = "deny"` makes findings errors, and a crate that
fails to compile is never linted, so findings in dependants stay hidden. Survey with:

```sh
cargo clippy --all-targets --locked -- -W warnings
```

### Lint suppressions

Every suppression needs project-owner agreement before it is pushed: `#[allow]`,
`#[expect]`, `-A` flags, and levels relaxed in `Cargo.toml` or `clippy.toml`, including
raised thresholds. Refactor first; inconvenience is not an argument that a lint is
wrong. When a suppression is unavoidable, ask explicitly, say what was tried, and
write `#[expect(..., reason = "...")]` so it fails once unneeded and records the
reason. An unexplained suppression nobody re-reads turns the gate into a formality.
The `allow_attributes` and `allow_attributes_without_reason` lints enforce the shape;
owner agreement is a review obligation that no lint checks.

### CI configuration

The workflow never restates formatting or lint commands with different flags.
Configuration lives in the repository where Cargo finds it, so local and
merge-blocking results cannot drift. Tighten a rule by changing the configuration,
not the workflow.

### Nested conditionals and comments

Do not write nested, multi-level, opaque `if` chains. A reader must be able to tell
what a branch does without holding three conditions and a later early return in mind
at once. No lint catches this; it is a review obligation. Two recurring shapes need
particular attention.

First, a flag can play two roles: selecting stdout output and controlling a later
early return. This repeats the decision about `quiet`:

```rust
if quiet {
    record_completion();
} else if json {
    write_json();
} else {
    write_text();
}
if quiet {
    return;
}
write_summary();
```

Decide once, in one place. Name the outcome with an enum or a `match`, or use a single
early return at the top:

```rust
if quiet {
    record_completion();
    return;
}
if json {
    write_json();
} else {
    write_text();
}
write_summary();
```

Second, a condition can be repeated inside its own `else`. Here, `save_results` is
tested both in the combined condition and inside its `else`:

```rust
if save_results && json {
    write_json_file();
} else {
    if save_results {
        write_text_file();
    }
}
```

Hoist the shared condition so each decision has one job, or split the function so
each half has one job:

```rust
if save_results {
    if json {
        write_json_file();
    } else {
        write_text_file();
    }
}
```

Write code comments only when intent is not evident from the code, and keep them
concise.

## Native builds

The default build and tests require only Rust and the platform's ordinary Rust linker,
with no Python, CMake, C++ compiler or signal core. Optional dependencies keep native
crates out of default builds. Even explicit crate or workspace-wide selection requires
no core: the core linking script is inert unless its default-off `native` feature is
enabled transitively by the CLI native feature.

For a native build, set `DMD_CORE_LIB_DIR` to the directory containing the prebuilt
static library. `DMD_CORE_LIB_NAME` defaults to `dmd_core` (without a filename prefix or
extension). `DMD_CORE_EXTRA_LIBS` is an optional semicolon-separated list of additional
link targets, using Cargo link-library syntax. Then run:

```sh
cargo build -p dmd --features native
```

The ABI contract is `crates/core-sys/include/dmd_core.h`. Its implementation is not
part of this repository. With `native` disabled, the core linking script performs no
environment reads, probing or linking.

## 4. External review

### Roles and invocation settings

Claude is unavailable; use Codex only. These fixed assignments replace previous
roles, including for unfinished work. Pass the model and reasoning effort explicitly
on every invocation; never rely on machine-local defaults. Apply the listed access
mode too.

| Role | Performed by | Model | Reasoning effort and access |
| --- | --- | --- | --- |
| Planning | Codex | `gpt-6.1-sol` | medium |
| Dispatch, Git and Pull Request routine | Codex | `gpt-6.1-sol` | medium |
| Reconnaissance | Codex | `gpt-6.1-sol` | medium, read-only |
| Implementation | Codex | `gpt-6.1-sol` | medium, workspace-write |
| Mechanical code review | Codex | `gpt-6.1-sol` | high, read-only |
| Intent review | Codex | `gpt-6.1-sol` | high, read-only |

Use `gpt-6.1-sol` at high for every review, regardless of who implemented the change.
Keep these reviewer settings fixed across subsequent rounds.

### First tier: mechanical code review

Once the validation gate passes, write `review-prompt.md` for the particular change.
Name the exact diff to inspect (base and head, plus any uncommitted changes included)
and the Issues it closes. Provide the plan, validation results and known limitations.
Tell the reviewer to read `AGENTS.md` and `CONTRIBUTING.md` first. Define the review
categories for the change: rank rarely executed code whose failures are expensive,
such as workflows, hooks and release scripts, first; then cover architecture, error
handling, tests and scope. Exclude checks already covered by the automated gate.
Require an explicit "nothing found" for each category without a substantive finding;
do not ask for a quota of findings or accept invented findings to fill a category.

Invoke the reviewer read-only with the fixed model and effort:

```sh
codex exec -s read-only --model gpt-6.1-sol \
  -c 'model_reasoning_effort="high"' \
  -C "$(git rev-parse --show-toplevel)" "$(cat review-prompt.md)" < /dev/null
```

Stdin must be closed with `< /dev/null`; without it the command waits for input
forever. Do not use `codex review --base <branch>`: it cannot take a custom prompt.

The orchestrator returns findings to the implementer as one structured list per
round, with category, severity, location, reasoning and requested correction. Agents
do not negotiate with each other; the orchestrator decides which findings to accept
or decline. Fix accepted findings in focused commits and rerun affected checks. Route
out-of-scope findings to separate Issues.

Repeat with the same reviewer until a round returns nothing substantive, for at most three
rounds. If substantive findings remain after the third round, stop and give the owner a
summary of them with a recommendation; the owner decides how to proceed. Each later
prompt names what was fixed and what was declined, explains each decision, and asks
the reviewer to challenge the reasoning behind every decline. A reviewer that never
disagrees is worth nothing.

### Second tier: intent review

After the mechanical review is clean, the orchestrator uses Codex `gpt-6.1-sol`,
explicitly at high reasoning effort and read-only, to review the final diff once
against the approved plan, owner
decisions and every invariant. This tier asks whether the result is what was requested
and preserves every invariant; code correctness was covered by the first tier.
Return any findings as one structured list to the implementer under the same
orchestrator decision rule. Allow at most two correction rounds in this tier, then
escalate unresolved findings to the project owner.

### Owner review

When asking the owner to review, summarise the automatic review: its findings, which
were accepted and how they were addressed, which were declined and why, and whether
the final round was clean. Resolve every blocking finding and obtain approval; a
clean automated gate is not review. Check compliance with
[Lint suppressions](#lint-suppressions).

## 5. Complete

Move the finished plan to `docs/plans/completed/` before final review and update its
links. Run the gate, then `cargo build --release --locked`. Mark the Pull Request Ready
and wait for explicit success of `ci/full` and final review approval. Merge by squash
only, using an imperative English title and a summary of the resulting behavior.
Confirm the closing keyword closes the Issue, then remove the merged branch.

Maintain `CHANGELOG.md` in Keep a Changelog style under `Unreleased` for user-visible
changes. Internal workflow plumbing does not earn an entry.
