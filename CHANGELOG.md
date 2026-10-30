# Changelog

All notable user-visible changes are documented here, following Keep a Changelog.

## [Unreleased]

### Added

- JSON replay execution with serial controls, source-time pacing, bounded lazy block
  spooling, portable no-overwrite payload publication under trusted stable output and
  cancellable background SHA-256 diagnostics.

- Read-only PCM WAVE source metadata library with exact frame counts, retained
  source identity and bounded header inspection.

- Read-only replay recording library with bounded ZIP indexing, validated metadata,
  incremental journal boundaries and lazy payload integrity verification.

- Shared run protocol 1.0 command/event types, exact producer JSON preservation,
  individual-message validation and bounded streaming NDJSON codecs.

### Changed

- Remove the legacy `--backend` selector; replay is a separate `dmd` implementation.

- The product is named Wireroom, its interface binary will be `wr`, and the `dmd`
  command-line tool keeps its name.
