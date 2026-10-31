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
- [x] Complete mechanical and intent reviews; resolve accepted findings and verify
      full CI on Linux, Windows and macOS.
- [x] Move the focused plan to completed, align Issue #26 and Draft PR #27, and
      prepare the documentation handoff through the installed hook.

## Validation and handoff

Keep a prepared recording end-to-end through actual `dmd` at maximum speed;
pause/resume/stop/EOF; ready payload; absent/corrupt recording errors; containment,
no-overwrite and length limits; ordinary WAVE/truncated header; fragmented NDJSON;
and cancellation/worker termination where still used. Avoid transcript scheduling,
large allocations and exact private operation counts.

The replay/test stage is complete. PR #27 remains Draft for owner review and Issue
#26 remains open until eventual merge. No Ready, merge or next-stage work is part
of this handoff.

## Completion evidence

Pinned formatting, all-target locked Clippy, locked workspace tests and release
build pass with cached dependencies offline. The final code/test revision passed
[full CI on Linux, Windows and macOS](https://github.com/o-kos/wireroom/actions/runs/41183993283),
including formatting, Clippy, tests, release and no-default-features builds. The
plan move and handoff wording follow that run as documentation-only changes;
the local gate and release build are repeated on the final documentation tree.

Warm full test runs observed 0.55–0.60 seconds versus the recorded approximately
9 seconds; these observations are not a benchmark or a claim of global minimality.
Passing checks fall from 131 to 73: product checks 128 to 70 (including two
doctests), plus three unchanged workflow checks. Eight conditional workflow
checks remain ignored and unchanged; no product checks were newly ignored.
CLI checks fall from 45 to 33.

Physical Rust test/support lines fall from 5,637 to 2,986 (2,651 removed), including
590 unchanged workflow support lines. Production falls from 4,021 to 3,404 (617
removed). Count crate Rust files, including examples and existing test seams;
classify test files and inline test modules as test/support, excluding README
doctests and separate policy scripts. This matches the baseline counting method.

Removed mechanisms include Linux PacketProcess and its exact one-second dialogue,
the background SHA worker/hooks/dependency, ZIP64/data-descriptor/comment/multipart/
encryption compatibility, hand-written arbitrary-precision JSON grammar, huge
allocation matrices and private-operation matrices. Classic stored/Deflate ZIP and
ordinary serde JSON retain exact u64 frames, duplicate-key rejection, retained
source/archive identities, bounded lazy access, payload CRC/size checks, safe
paths, no-overwrite publication, lifecycle, controls and EOF. Digest metadata is
informational and unverified. See the [CLI guide](https://github.com/o-kos/wireroom/blob/feat/26-replay-cli/crates/cli/README.md)
for supported behavior and budgets.

No GUI crate or dependency is present, and active documentation makes no
implemented-window claim. The original UI concept remains future work. No new
test framework, lint suppression or threshold change was added. Older completed
plans remain historical.

## Review outcome

Mechanical review accepted two P2 findings: ordinary finite-f64 command round trips
were corrected with standard serde_json `float_roundtrip` and one assertion in the
existing numeric test; the specification fixture loader now normalizes CRLF before
splitting Markdown fences. All 33 existing CLI tests passed against the temporary
CRLF specification, followed by byte-verified restoration. Neither fix added a test
function. No findings were declined. Mechanical round 3 was clean, with no
substantive findings remaining. Final intent review was clean and requested no
corrections; it confirmed the narrowed scope, retained behavior and reported metrics.
