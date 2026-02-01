# Changelog

[中文](CHANGELOG.zh-CN.md)

All notable changes to this project will be documented in this file.

The format is based on *Keep a Changelog*, and this project adheres to *Semantic Versioning*.

## [Unreleased]

### Added
- Initial scaffolding.
- Rust CLI binary: `dup-code-check`.
- CLI i18n: `--localization <en|zh>` (default: `en`).
- Scan budgets: `maxFiles` / `maxTotalBytes` (CLI: `--max-files` / `--max-total-bytes`).
- Scan stats + strict mode in CLI (`--stats`, `--strict`).
- GitHub Actions: CI (Linux/macOS/Windows), docs (GitHub Pages), and release (GitHub Release + npm publish).
- Docs: bilingual (EN/ZH) with cross-links.

### Changed
- Rename project: `dup-check` → `dup-code-check`.
- Dev: rename the released-changelog edit override env var to `DUP_CODE_CHECK_ALLOW_CHANGELOG_RELEASE_EDIT`.
- Default scan skips files larger than 10 MiB (`DEFAULT_MAX_FILE_SIZE_BYTES`).
- Fallback scanner now respects nested `.gitignore` rules via the `ignore` crate.
- CLI integer options now reject non-integers (e.g. `--max-file-size 1.5`).
- `--report` memory usage reduced by avoiding large intermediate clones.
- Invalid roots now fail early instead of producing empty results.
- Docs: add GitBook-style documentation under `docs/`.

### Fixed
- Tolerate `NotFound` during scanning (files deleted mid-scan).
- Avoid panics in `git check-ignore` integration; fall back when it fails.
- Avoid leaking absolute paths in results when path prefix stripping fails.
- `--follow-symlinks` now works reliably by using the walker path when enabled.
- Token-based detectors now record the start line for multi-line string tokens.
- CLI now supports `--` to terminate option parsing (allows roots that start with `-`).
- Scanning now skips `PermissionDenied` and walker traversal errors instead of aborting.
- CLI now catches runtime scan failures and exits with code 1.
- Remove unstable rustfmt config options to avoid warnings on stable toolchains.
- `--max-report-items` now applies consistently across all report sections and prefers larger groups.
- CLI now supports `--no-gitignore` to disable `.gitignore` filtering.
- When path prefix stripping fails, output uses `<external:...>/name` to keep paths distinguishable without leaking absolute paths.
