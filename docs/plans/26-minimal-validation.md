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

- [ ] Remove GUI residue and align active documentation with unimplemented `wr`.
- [ ] Remove hashing and packet infrastructure; retain real process lifecycle checks.
- [ ] Finish classic ZIP and ordinary JSON profiles; trim disproportionate tests.
- [ ] Run pinned formatting, all-target Clippy, locked tests and release build;
      record warm test timing and test/support and production reductions.
- [ ] Push through the installed hook; align Issue #26 and Draft PR #27.

## Validation and handoff

Keep a prepared recording end-to-end through actual `dmd` at maximum speed;
pause/resume/stop/EOF; ready payload; absent/corrupt recording errors; containment,
no-overwrite and length limits; ordinary WAVE/truncated header; fragmented NDJSON;
and cancellation/worker termination where still used. Avoid transcript scheduling,
large allocations and exact private operation counts.

Baseline: 131 product tests, approximately 9 seconds. Report workflow checks
separately. Keep this plan active and PR #27 Draft. Root handles correctness and
intent reviews, plan completion and later lifecycle steps; do not run reviewers,
mark Ready or merge in this handoff.
