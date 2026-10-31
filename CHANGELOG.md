# Changelog

All notable user-visible changes are documented here, following Keep a Changelog.

## [Unreleased]

### Added

- JSON replay execution with serial controls, source-time pacing, bounded lazy block
  spooling and portable no-overwrite payload publication under trusted stable output.

- Read-only PCM WAVE source metadata library with exact frame counts, retained
  source identity and bounded header inspection.

- Read-only replay recording library with bounded ZIP indexing, validated metadata,
  incremental journal boundaries and lazy payload integrity verification.

- Shared run protocol 1.0 command/event types, ordinary producer JSON support with
  exact u64 coordinates, individual-message validation and bounded streaming NDJSON codecs.

### Changed

- Narrow pre-release replay to classic stored/Deflate ZIP without ZIP64, data
  descriptors, multipart layout, encryption or entry/archive comments, with smaller bounded archive limits. Defer source
  digest verification and arbitrary-precision producer numbers.

- Remove the legacy `--backend` selector; replay is a separate `dmd` implementation.

- The product is named Wireroom, its interface binary will be `wr`, and the `dmd`
  command-line tool keeps its name.
