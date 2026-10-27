# Issue #10: Stop reporting markup and script syntax as paths and hosts in the content policy

Resolves #10.

## Overview

`scripts/content_policy.rs` checks each added line on its own and rejects ordinary
HTML and JavaScript: a closing tag on a line without its opening tag, a regular
expression literal, and property access whose last segment is a listed DNS suffix
(an object name, a dot and `info`). After this change that syntax passes in markup and script files, while real
absolute paths, hosts, URLs and e-mail addresses are still reported everywhere.

Outside scope: CSS, other languages, and any change to how prose and configuration
files are checked.

## Context

- Since #7, `check_added_text` knows the file each added line belongs to. That file's
  extension is what lets the policy read markup and script syntax without loosening
  the check for prose.
- The policy stays a single dependency-free Rust file compiled with
  `rustc --edition=2024 -D warnings`; its unit tests run through
  `crates/protocol/tests/policy.rs`. Every commit, including test code, must pass it.
- The window concept in `docs/ui` was restructured in #9 only to dodge these false
  positives; it is not changed here.

Implementer: Codex `gpt-6-astra`, high reasoning effort, workspace-write. Mechanical
reviewer: Codex `gpt-5.6-sol`, high reasoning effort, read-only.

## Decisions

- File kinds come from the extension of the current file, case-insensitively:
  `html` and `htm` are markup and script; `svg` and `xml` are markup; `js`, `mjs`,
  `cjs`, `jsx`, `ts` and `tsx` are script. Every other file, and a line with no known
  file, keeps today's checks exactly.
- In markup and script files a well-formed closing tag (a slash and a tag name of the
  form the policy already accepts, then `>`) is markup even without an earlier opening
  tag on the same line. Elsewhere the existing earlier-opening-tag rule stays.
- In script files a regular-expression literal is removed before the check. A literal
  starts at the beginning of the line or after an operator or opening bracket
  (`(`, `,`, `=`, `:`, `[`, `!`, `&`, `|`, `?`, `{`, `}`, `;`, `+`, `-`, `*`, `%`, `<`,
  `>`, `~`, `^`) with optional whitespace, runs to the next unescaped slash outside a
  character class on the same line, may end with flags from `dgimsuvy`, and its body
  contains at least one escape or one of `\ ( ) [ ] { } ? * + ^ $ | .`; a slash-delimited
  token with none of these, such as a plain path, is still checked as a path.
- In script files a bare host-like token (no scheme, no `@`) is reported only when it is
  the whole content of a quoted string literal. Property access whose last segment
  happens to be a DNS suffix is code. URLs and e-mail addresses are checked as today in every file.

## Rejected alternatives

- Treating every closing tag as markup in every file: it would admit path-like text
  such as a closing tag around a private directory name in prose.
- Skipping script and markup files entirely: private paths, hosts and addresses in
  strings and comments of those files must still be caught.
- A full JavaScript or HTML tokenizer: it breaks the dependency-free, generic design and
  is far larger than the three false positives need.
- Accepting closing tags only for a list of HTML element names: SVG and custom elements
  would need endless additions, while the file kind already says the line is markup.

## Implementation steps

- [ ] Derive the file kind from the current file and pass it to the line check.
- [ ] Accept well-formed closing tags in markup and script files.
- [ ] Remove regular-expression literals in script files before the check.
- [ ] Report bare hosts in script files only as whole quoted string literals.
- [ ] Unit tests for each rule, including that the same text is still rejected in a
      prose file, that plain paths, URLs, e-mail addresses and quoted hosts are still
      rejected in script and markup files, and that a closing tag in a markup file does
      not hide a path elsewhere on its line.
- [ ] Complete validation.
- [ ] Move this plan to `docs/plans/completed/` before final review.

## Validation

- [ ] A multi-line HTML element whose closing tag stands on its own line passes.
- [ ] A JavaScript regular-expression literal starting with a slash passes.
- [ ] Property access ending in `info` or `dev` passes as code, while an unapproved host
      is still reported in prose and as a quoted string in a script.
- [ ] Absolute paths, unapproved URLs and e-mail addresses are still rejected.
- [ ] The single-file version of the window concept from #9 passes line by line as an
      HTML file (checked by the orchestrator).
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
