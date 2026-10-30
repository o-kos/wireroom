# Issue #26: Reduce evidenced duplicate test maintenance

Continues #26 and Draft PR #27. The completed replay implementation plan remains historical.

## Overview and decisions

The owner requested justification of the whole workspace suite before UI work.
The audit reports 137 running tests and eight ignored, with 5,830 physical test/support
lines versus 4,021 production lines (including examples and small test seams,
excluding README doctests; policy scripts counted separately). Maintenance burden,
not the observed 8.87-second workspace run, motivates this modest cleanup.

Remove only six high-confidence duplicates and the source composition ZIP builder.
Preserve source metadata expectations and all production behavior, protocols and
formats. Do not change hashing hooks, fixture architecture, corruption/budget matrices,
cancellation or the trusted stable output model. Do not impose a test-count quota
or claim that the remaining suite is minimal. The full audit stays outside the tree.

## Concrete removals and covering evidence

| Remove | Retain as coverage |
| --- | --- |
| replay `complete_specification_example_accepts_crlf_markdown` | `complete_specification_example_has_independent_expectations` uses the same normalizing helper; `split_utf8_crlf_exact_producer_values_and_pending_validation` tests actual journal CRLF |
| replay `stored_archive_bytes_can_be_supplied_without_a_path` | Independent specification example opens a stored archive through `Recording::from_reader(Cursor)` and reads its journal |
| replay `zip64_empty_descriptor_cannot_hide_a_corrupt_wide_size` | `empty_descriptor_assets_use_the_declared_zip_width` corrupts the same decoded-size field for signed/unsigned ZIP64 empty Stored and Deflate assets |
| CLI `published_file_is_available_when_descriptor_arrives` | `all_result_files_are_ready_before_descriptor_and_after_exit` checks two files at delivery and after exit; normative exchange retains payload and exit assertions |
| CLI `changed_retained_metadata_cannot_certify_digest` | `retained_hash_outcomes_are_ordered_once_before_unchanged_terminal` modifies retained data after digesting and checks warning, frontier, outcome and terminal ordering |
| CLI `remaining_time_survives_pause_and_speed_change` | `pending_clock_accounts_lookahead_controls_and_submission_deterministically` exercises runtime pause/resume and speed accounting; keep `exact_deltas_and_max` for large-integer subtraction |
| source copied ZIP builder, replay traversal and synthetic CRLF loop | Rename to documented-source metadata test; retain independent byte length, frame count, rate, channels and format assertions. Replay example and normative CLI cover composition |

The explicit source-to-replay smoke and direct private-helper checks disappear;
listed broader tests retain their contract evidence. No new tests are planned for
reversible removal. Optional audit trims are deferred to keep scope concrete.

## Implementation steps

- [x] Remove the six duplicates and simplify the source metadata fixture; remove unused dev-dependencies and update the lockfile.
- [x] Record exact reduction, counts, warm workspace/CLI timings and coverage limits; pass the full pinned gate and release build.
- [x] Update Draft PR #27 with cleanup and pending independent reviews; prepare the hook-validated push.
- [ ] Complete independent mechanical and intent reviews, then move this plan to `completed/` under the final review workflow.

## Validation and handoff

Run `cargo fmt --all -- --check`, `cargo clippy --all-targets --locked`,
`cargo test --locked` and then `cargo build --release --locked`; cached dependencies
may be used offline. Policy/hook tests are conditional on changes to their code,
which are outside this cleanup. Measure a warm full test run and a subsequent
CLI-only run without competing Cargo work; report observations, not benchmarks.
Leave the PR Draft and this plan active for independent removal reviews.

## Cleanup evidence

Removed six test functions (99 physical Rust lines) and reduced the source fixture
from 133 to 39 lines (94 net lines), for 193 net Rust test/support lines removed.
The two dev-dependency declarations and their two source lockfile edges were also
removed; the packages remain used elsewhere. Test/support now totals 5,637 lines
under the audit's classification; production remains 4,021 lines.

On the pinned toolchain, formatting, all-target locked Clippy, locked workspace
tests and the subsequent release build passed with cached dependencies offline.
A compilation/warm-up run preceded sequential timed runs with no competing Cargo
work or Cargo lock waits. `/usr/bin/time -p` around `cargo test --locked --offline`
observed 9.00 seconds real (12.69 user, 0.73 system); the subsequent
`cargo test -p dmd --locked --offline` observed 1.14 seconds real (0.20 user,
0.11 system). The supplied pre-cleanup observations were 8.87 and 1.14 seconds;
these observations show no speedup and are not a benchmark.

Workspace results: 131 passed (129 unit/integration plus two doctests), zero failed,
eight ignored, versus 137 passed and eight ignored before cleanup. CLI: 45 passed
versus 48 before. Policy/hook sources were untouched, so conditional ignored policy
tests were not required. No suppressions, thresholds or production behavior changed.

Loss is limited to the explicitly listed redundant variants, isolated private-helper
checks and the separate source-to-replay smoke; source metadata and actual composed
process execution remain checked. This is not proof that every remaining test is
necessary. Normative full-process exchange remains Linux-specific; output file-count
and byte-accounting extreme boundaries still lack direct tests. Hash test hooks and
any broader scope reduction are outside this change. Independent removal mechanical
and intent reviews remain pending; keep this plan active and PR #27 Draft.
