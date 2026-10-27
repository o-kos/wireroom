# Issue #7: Accept anchored patterns in .gitignore in the content policy

Resolves #7.

## Overview

`scripts/content_policy.rs` reports every token that starts with `/` as an absolute
filesystem path, so an anchored ignore pattern (a pattern whose first character is a
slash, which ties it to the directory of its ignore file) added to a
`.gitignore` cannot be pushed. After this change an anchored pattern added to any
`.gitignore` in the tree passes, while the same text in any other file is still
rejected. Host, URL, e-mail and tree-escape checks keep applying to `.gitignore` lines.

Outside scope: other ignore or attribute files, and any wider change to path
detection. False positives in markup and scripts are tracked in #10.

## Context

- The policy is a single dependency-free Rust file compiled by the pre-push hook with
  `rustc --edition=2024 -D warnings`; its unit tests run through
  `crates/protocol/tests/policy.rs` (`policy_scripts_pass_their_tests`).
- `check_commit` reads each commit as one `git show --unified=0` diff and
  `check_added_text` checks every added line in isolation, without knowing its file.
- Every commit must pass the policy on its own, including the commits of this change.

Implementer: Codex `gpt-6-astra`, high reasoning effort, workspace-write. Mechanical
reviewer: Codex `gpt-5.6-sol`, high reasoning effort, read-only.

## Decisions

- `check_added_text` tracks the file each added line belongs to from the diff's
  `+++` header. A header of `/dev/null` belongs to no file; a quoted header is
  unquoted before its `b/` prefix is removed. A file whose last path component is
  `.gitignore` is an ignore file.
- Headers are told from content by the diff's structure: a line starting with `diff `
  opens a file's header section, the only place a `+++` line names the file, and a line
  starting with `@@` opens hunk content, where every `+` line is added text. Review
  round 1 showed that matching `+++` anywhere let an added line beginning with `++`
  escape the check or re-attribute later lines to an ignore file.
- In an ignore file, a pattern line that starts with `/`, or with `!/` for a negated
  pattern, is checked without that anchoring `/`. Every other check, including tree
  escapes through `..`, still applies to the rest of the line.
- Comment lines (`#`) and lines that do not start with an anchor are checked
  unchanged, so prose and stray paths in an ignore file are still reported.
- The reported diff line number keeps its current meaning.

## Rejected alternatives

- Exempting whole `.gitignore` files from the absolute-path check: it would also
  admit absolute paths in comments, which the Issue keeps rejecting.
- Treating every token that starts with `/` and ends with `/` as a pattern in any
  file: it widens path detection, which the Issue puts outside scope.
- Reading file names from a separate `git diff --name-only` pass: it cannot tie a line
  to its file without re-parsing hunks, which the `+++` header already gives.

## Implementation steps

- [x] Track the current file in `check_added_text` and strip the anchoring `/` from
      pattern lines of `.gitignore` files before the check.
- [x] Add unit tests: an anchored and a negated anchored pattern pass in a root and in
      a nested `.gitignore`; the same line is rejected in another file and in a
      `.gitignore` comment; an e-mail address, an unapproved host and a `..` escape are
      rejected in a `.gitignore`; a deleted file's lines and a quoted header are handled.
- [x] Complete validation.
- [x] Move this plan to `docs/plans/completed/` before final review.

## Validation

- [x] A diff adding an anchored pattern to `.gitignore` or to a nested `.gitignore` passes
      the content policy.
- [x] The same line added to any other file is still rejected as an absolute path.
- [x] An e-mail address or unapproved host added to `.gitignore` is still rejected.
- [x] `cargo fmt --all -- --check`
- [x] `cargo clippy --all-targets --locked`
      (warnings are denied in `[workspace.lints]`)
- [x] `cargo clippy -p dmd --features native --all-targets --locked` with an empty core
      directory (see `CONTRIBUTING.md` for exact commands)
- [x] `cargo test --locked`
- [x] `cargo test -p dmd-native --locked`
- [x] `cargo build --release --locked`, after the gate passes

## Post-completion

None.
