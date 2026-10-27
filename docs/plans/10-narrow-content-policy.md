# Issue #10: Narrow the content policy to private locations, URLs and e-mail addresses

Resolves #10.

## Overview

The content policy reports every token that starts with a slash as an absolute path and
every `word.suffix` token as a host, so ordinary markup and code are rejected: closing
tags on their own line, regular-expression literals and property access. After this
change the policy reports only what discloses an environment: private filesystem
locations, tree escapes, URLs to unapproved hosts and e-mail addresses. The special
cases this makes unnecessary are removed.

Outside scope: secrets scanning, and environment-specific names, which belong to the
local policy hook (`dmd.localPolicy`).

## Context

- `scripts/content_policy.rs` is one dependency-free Rust file compiled by the pre-push
  hook with `rustc --edition=2024 -D warnings`. Its unit tests run through
  `crates/protocol/tests/policy.rs`. Every commit, including test code, must pass it.
- #7 added an exemption for anchored `.gitignore` patterns and separated diff headers
  from added hunk text. The exemption becomes redundant; the header separation fixes a
  real bypass and stays.
- A first attempt at this Issue added per-file-type rules for each false positive and
  kept finding new ones; the owner chose to narrow the check instead.

Implementer: Codex `gpt-6-astra`, high reasoning effort, workspace-write. Mechanical
reviewer: Codex `gpt-5.6-sol`, high reasoning effort, read-only.

## Decisions

Mechanical review round 1 found gaps in root matching, escaped UNC prefixes and URL
authority handling; the refinements below close them without restoring broad checks.

- A token is a private location when it is one of:
  - a Unix or macOS home directory: a path from the filesystem root through `home`,
    `Users`, `root` or `var` and then `home`, followed by at least one more path segment;
  - a home-relative path: a tilde, an optional user name and a slash at the start of the
    token, followed by at least one more character;
  - a mount location: a path from the filesystem root through `mnt`, `media`, `run` and
    then `media`, or `Volumes`, followed by at least one more path segment;
  - a Windows drive path (a letter, a colon, then a slash or backslash) or a UNC path
    (two backslashes, a host, then a separator), including the spelling with each
    backslash doubled in a string literal.
  Every listed rooted-path component is compared ASCII-case-insensitively, including
  both components of two-component roots. A listed root can start anywhere in a token
  if the preceding character is absent or is not a letter, digit, dot, underscore or
  hyphen; it still requires at least one further nonempty path segment. Punctuation
  does not make arbitrary slash-prefixed text a private location. UNC detection still
  requires a nonempty host and a following separator, so incomplete backslash runs pass.
  It is reported as a "private filesystem location". Any other token that starts with a
  slash is no longer reported.
- Tree escapes through `..` stay reported as today.
- URLs stay checked against the host allowlist, and remote paths (a user, an at sign, a
  host, a colon and a path) and e-mail addresses stay reported. A host-like token without
  a scheme or `@` is no longer reported, except for a scheme-relative URL: a token
  starting with exactly two slashes and a host-like authority whose final label is in
  the DNS-suffix list. Only this check uses that list; comment markers and other text
  after two slashes pass. URLs with an empty authority have no host to check and apply
  the private-location rules to their path. This permits harmless system paths in
  triple-slash `file` URLs while rejecting private paths in both single-slash and
  triple-slash forms; in empty-authority URL paths and the single-slash `file` form,
  drive detection also accepts exactly one leading slash without widening it elsewhere.
- The closing-tag matching (`strip_markup_closing_tag`) and the anchored `.gitignore`
  exemption are removed, together with the file tracking that only the exemption used.
  `check_added_text` keeps separating headers from hunk text by `diff ` and `@@` lines,
  and keeps checking every added line, including one whose text begins with `++`.
- Constants that only the removed checks used are removed, except for the DNS-suffix
  list retained for scheme-relative URLs.
- `CONTRIBUTING.md` describes the narrowed check and points environment-specific names
  to `dmd.localPolicy`.

### Running the policy and its tests (owner decision after review round 4)

The repository has almost no product code yet, while the policy was compiled on every push and
its tests ran on every `cargo test`. The owner kept the Rust policy and chose to run it cheaply:

- The pre-push hook compiles the policy only when its source changed since the last build: it
  records the `git hash-object` of `scripts/content_policy.rs` beside the binary in
  `target/policy/` and reuses the binary while the hash matches.
- The tests that compile or exercise the policy (`policy_scripts_pass_their_tests` for the
  content policy, and the `push_hook_*` tests, which compile it inside fixture repositories) are
  marked `#[ignore]` with a reason, so `cargo test` skips them. `.github/scripts/ci_status.rs`
  leaves `policy_scripts_pass_their_tests`; the `status` job already tests it on every run.
- The policy tests run only when a change touches `scripts/content_policy.rs`,
  `.githooks/pre-push` or `crates/protocol/tests/policy.rs`: the pre-push hook runs
  `cargo test -p dmd-protocol --test policy --locked -- --ignored` when an outgoing commit
  touches one of them, and CI runs the same command in a step that compares the change with its
  base (the Pull Request base, or the previous commit on `main`).
- `CONTRIBUTING.md` names the command for running the policy tests by hand.

A rewrite of the policy in shell was rejected: URL, scheme-case, UNC and tree-escape handling,
which review rounds 1 to 3 had to tighten, would move into `grep` and `awk` expressions without
tests and with GNU and BSD differences.

## Rejected alternatives

- Adding rules per file type for closing tags, regular expressions and property access:
  tried first; each rule exposed another false positive, and the rules made the policy
  harder to reason about than the disclosures it guards against.
- Keeping bare host detection only for prose files: it would bring back per-file-type
  rules, and a bare name without a scheme rarely discloses more than the URLs and
  addresses that stay checked.
- Rejecting every absolute path except an allowlist of system directories: the set of
  harmless slash-prefixed tokens in code is open-ended, so the allowlist would never be
  complete.

## Implementation steps

- [x] Replace the absolute-path check with the private-location check.
- [x] Stop reporting bare host-like tokens; keep URL, remote-path and e-mail checks.
- [x] Remove the closing-tag matching, the `.gitignore` exemption, the file tracking it
      used and constants left unused, keeping the header and hunk separation.
- [x] Update the tests: private locations of every listed kind are rejected; closing
      tags on their own line, regular-expression literals (including one matching a
      less-than sign), property access ending in a DNS suffix, anchored ignore patterns
      and other slash-prefixed tokens pass; tree escapes, unapproved URLs, remote paths
      and e-mail addresses are rejected; header and hunk tests stay.
- [x] Update the policy description in `CONTRIBUTING.md`.
- [ ] Compile the policy in the pre-push hook only when its source hash changed.
- [ ] Mark the policy and hook tests as ignored, drop `ci_status.rs` from the policy test, and
      run the ignored tests in the hook and in CI only when a change touches the policy, the
      hook or its tests; document the manual command in `CONTRIBUTING.md`.
- [ ] Complete validation.
- [ ] Move this plan to `docs/plans/completed/` before final review.

## Validation

- [ ] A multi-line HTML element whose closing tag stands on its own line passes.
- [ ] A JavaScript regular-expression literal starting with a slash passes, including
      one matching a less-than sign.
- [ ] Property access ending in a DNS suffix passes; a URL to an unapproved host is still
      rejected.
- [ ] An anchored `.gitignore` pattern passes without a special case.
- [ ] Home-directory, mount, drive and network paths, tree escapes and e-mail addresses
      are still rejected.
- [ ] The single-file version of the window concept from #9, without its external font
      link, passes as an HTML file (checked by the orchestrator).
- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --all-targets --locked`
      (warnings are denied in `[workspace.lints]`)
- [ ] `cargo clippy -p dmd --features native --all-targets --locked` with an empty core
      directory (see `CONTRIBUTING.md` for exact commands)
- [ ] `cargo test --locked`
- [ ] `cargo test -p dmd-native --locked`
- [ ] `cargo test -p dmd-protocol --test policy --locked -- --ignored`
- [ ] A push whose commits do not touch the policy, the hook or its tests neither compiles the
      policy nor runs the ignored tests when a matching build is cached.
- [ ] `cargo build --release --locked`, after the gate passes

## Post-completion

None.
