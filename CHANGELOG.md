# Changelog

All notable changes to this project will be documented in this file.

The format is based on *Keep a Changelog*, and this project adheres to *Semantic Versioning*.

## [Unreleased]

### Added
- Initial scaffolding.

### Changed
- Default scan skips files larger than 10 MiB (`DEFAULT_MAX_FILE_SIZE_BYTES`).
- Fallback scanner now respects nested `.gitignore` rules via the `ignore` crate.
- CLI integer options now reject non-integers (e.g. `--max-file-size 1.5`).
- CLI `--help` no longer requires the native module to be present.

### Fixed
- Tolerate `NotFound` during scanning (files deleted mid-scan).
- Avoid panics in `git check-ignore` integration; fall back when it fails.
- Avoid leaking absolute paths in results when path prefix stripping fails.
- NAPI now validates numeric scan options (rejects NaN / fractional / out-of-range values).
