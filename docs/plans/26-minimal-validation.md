# Issue #26: Usable replay with proportionate validation

Resolves #26.

## Scope

Finish test and infrastructure reductions while preserving usable replay. No UI
implementation, GUI dependency, window, exporter or new test framework belongs here.
The existing UI concept remains unimplemented for a separate owner task.

## Decisions

Remove Linux packet-pipe test machinery, expensive boundary matrices, private-stage
checks and background source hashing. Retain small public behavior checks, exact
u64 source coordinates, retained file identities, lazy bounded journal/index access,
payload CRC/size checks, safe paths and publication without overwrite.

Use classic single-disk stored/Deflate ZIP; reject ZIP64, descriptors, comments,
multipart and encryption. Use ordinary serde JSON with duplicate-key rejection;
arbitrary numeric precision and exotic exponents are deferred. Digests are
informational and unverified. Preserve existing workflow policy and hook tests.

## Steps

- [x] Remove GUI residue and align active documentation with unimplemented `wr`.
- [x] Remove hashing and packet infrastructure; retain real process lifecycle checks.
- [x] Finish classic ZIP and ordinary JSON profiles; trim disproportionate tests.
- [x] Run pinned formatting, all-target Clippy, locked tests and release build;
      record warm test timing and test/support and production reductions.
- [x] Align Issue #26 and Draft PR #27; prepare push through the installed hook.

## Validation and handoff

Keep a prepared recording end-to-end through actual `dmd` at maximum speed;
pause/resume/stop/EOF; ready payload; absent/corrupt recording errors; containment,
no-overwrite and length limits; ordinary WAVE/truncated header; fragmented NDJSON;
and cancellation/worker termination where still used. Avoid transcript scheduling,
large allocations and exact private operation counts.

Baseline: 131 total passing tests, approximately 9 seconds. Report workflow checks
separately. Keep this plan active and PR #27 Draft. Root handles correctness and
intent reviews, plan completion and later lifecycle steps; do not run reviewers,
mark Ready or merge in this handoff.

## Focused completion evidence

Pinned formatting, all-target locked Clippy, locked workspace tests and release
build pass with cached dependencies offline. A sequential warm full test run
observed 0.55 seconds real (0.50 user, 0.32 system), versus the recorded 9.00 seconds;
this is an observation, not a benchmark. Total passing tests fall from 131 to 73:
product checks 128 to 70 (including two doctests), workflow checks three unchanged.
Eight conditional workflow tests remain ignored; unchanged policy/hook paths do
not require them. CLI checks fall from 45 to 33.

Physical Rust test/support lines fall from 5,637 to 2,979 (2,658 removed), including
590 unchanged workflow support lines. Production falls from 4,021 to 3,404 (617
removed). Count crate Rust files, including examples and existing test seams;
classify test files and inline test modules as test/support, excluding README
doctests and separate policy scripts. This matches the earlier audit's baseline.

Removed scenarios include large real-boundary allocations, private operation counts,
ZIP64/descriptor/comment compatibility, hash outcomes, schema combinations and
private runtime stages. Retained public checks cover the behaviors listed above;
the oversized-command check streams bytes without constructing a 16 MiB fixture.
Existing cancellation support remains; no new hooks or test framework were added.
There is no GUI crate or dependency in the tree/manifests, and active documentation
makes no implemented-window claim. The original UI concept is unchanged. No lint
suppression, threshold change, reviewer invocation, Ready or merge is included.
