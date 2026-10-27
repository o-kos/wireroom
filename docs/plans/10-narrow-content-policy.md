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

- A token is a private location when it is one of:
  - a Unix or macOS home directory: a path from the filesystem root through `home`,
    `Users`, `root` or `var` and then `home`, followed by at least one more path segment;
  - a home-relative path: a tilde, an optional user name and a slash at the start of the
    token, followed by at least one more character;
  - a mount location: a path from the filesystem root through `mnt`, `media`, `run` and
    then `media`, or `Volumes`, followed by at least one more path segment;
  - a Windows drive path (a letter, a colon, then a slash or backslash) or a UNC path
    (two backslashes, a host, then a separator).
  It is reported as a "private filesystem location". Any other token that starts with a
  slash is no longer reported.
- Tree escapes through `..` stay reported as today.
- URLs stay checked against the host allowlist, and remote paths (a user, an at sign, a
  host, a colon and a path) and e-mail addresses stay reported. A host-like token without
  a scheme or `@` is no longer reported.
- The closing-tag matching (`strip_markup_closing_tag`) and the anchored `.gitignore`
  exemption are removed, together with the file tracking that only the exemption used.
  `check_added_text` keeps separating headers from hunk text by `diff ` and `@@` lines,
  and keeps checking every added line, including one whose text begins with `++`.
- Constants that only the removed checks used are removed.
- `CONTRIBUTING.md` describes the narrowed check and points environment-specific names
  to `dmd.localPolicy`.

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

- [ ] Replace the absolute-path check with the private-location check.
- [ ] Stop reporting bare host-like tokens; keep URL, remote-path and e-mail checks.
- [ ] Remove the closing-tag matching, the `.gitignore` exemption, the file tracking it
      used and constants left unused, keeping the header and hunk separation.
- [ ] Update the tests: private locations of every listed kind are rejected; closing
      tags on their own line, regular-expression literals (including one matching a
      less-than sign), property access ending in a DNS suffix, anchored ignore patterns
      and other slash-prefixed tokens pass; tree escapes, unapproved URLs, remote paths
      and e-mail addresses are rejected; header and hunk tests stay.
- [ ] Update the policy description in `CONTRIBUTING.md`.
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
- [ ] `cargo build --release --locked`, after the gate passes

## Post-completion

None.
