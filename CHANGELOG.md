# Changelog

[中文](CHANGELOG.zh-CN.md)

All notable changes to this project will be documented in this file.

The format is based on *Keep a Changelog*, and this project adheres to *Semantic Versioning*.

## [Unreleased]

### Added
- Initial scaffolding.
- Rust CLI binary: `dup-code-check`.
- CLI i18n: `--localization <en|zh>` (default: `en`).
- CLI: `--version` / `-V`.
- Scan budgets: `maxFiles` / `maxTotalBytes` (CLI: `--max-files` / `--max-total-bytes`).
- Scan stats + strict mode in CLI (`--stats`, `--strict`).
- CLI: `--gitignore` to explicitly enable `.gitignore` filtering (default on; mainly useful in scripts).
- npm: `bin/dup-code-check.mjs` launcher script (enables cross-platform `dup-code-check` via npm).
- Core: allow overriding the `git` executable via `DUP_CODE_CHECK_GIT_BIN`.
- GitHub Actions: CI (Linux/macOS/Windows), docs (GitHub Pages), and release (GitHub Release + npm publish).
- Docs: bilingual (EN/ZH) with cross-links.
- Docs: add `/llms.en.txt` + `/llms.zh-CN.txt` bundles and an `LLMs` docs page.

### Changed
- Rename project: `dup-check` → `dup-code-check`.
- Dev: rename the released-changelog edit override env var to `DUP_CODE_CHECK_ALLOW_CHANGELOG_RELEASE_EDIT`.
- Rust: refactor core/report and CLI into smaller modules (no behavior change).
- Rust: de-duplicate scan file reading/skip logic via a shared helper.
- Tests: add regression coverage for the Git fast path under scan budgets.
- Default scan skips files larger than 10 MiB (`DEFAULT_MAX_FILE_SIZE_BYTES`).
- Fallback scanner now respects nested `.gitignore` rules via the `ignore` crate.
- CLI integer options now reject non-integers (e.g. `--max-file-size 1.5`).
- `--report` memory usage reduced by avoiding large intermediate clones.
- Invalid roots now fail early instead of producing empty results.
- Docs: migrate docs site from Honkit to VitePress (with `llms.txt`).
- Docs: improve `llms.txt` generation order and add a prompt template header.
- Docs workflow: skip GitHub Pages deployment when the repository is private.
- Docs workflow: auto-enable GitHub Pages (Actions) when deploying docs.
- Release workflow: publish to crates.io on tag releases (and skip when token is missing).
- Release workflow: avoid `secrets.*` in `if:` to prevent workflow validation issues.
- Release workflow: retry `cargo publish` for the CLI to tolerate crates.io index propagation delay.
- Repo links: update GitHub owner / Pages URLs (`omne42`).
- Metadata: use MIT-only license identifier.
- Scan budgets: `maxFiles` now stops scanning once the file-count budget is hit (`skippedBudgetMaxFiles` becomes non-zero).
- Node installer: `postinstall` builds Rust binary with `cargo build --locked`.
- Scan pipeline: stream `git ls-files` enumeration when `maxFiles` is set (stop early without collecting full lists).

### Fixed
- Tolerate `NotFound` during scanning (files deleted mid-scan).
- Avoid panics in `git check-ignore` integration; fall back when it fails.
- Avoid leaking absolute paths in results when path prefix stripping fails.
- `--follow-symlinks` now works reliably by using the walker path when enabled.
- When `--follow-symlinks` is enabled, skip symlinked directories that resolve outside the root.
- Token-based detectors now record the start line for multi-line string tokens.
- CLI now supports `--` to terminate option parsing (allows roots that start with `-`).
- Scanning now skips `PermissionDenied` and walker traversal errors instead of aborting.
- CLI now catches runtime scan failures and exits with code 1.
- Remove unstable rustfmt config options to avoid warnings on stable toolchains.
- `--max-report-items` now applies consistently across all report sections and prefers larger groups.
- CLI: `--max-files` now rejects values that don’t fit into `usize` instead of truncating.
- CLI now supports `--no-gitignore` to disable `.gitignore` filtering.
- When path prefix stripping fails, output uses `<external:...>/name` to keep paths distinguishable without leaking absolute paths.
- Ignore unsafe relative paths from `git ls-files` (absolute paths, `..`, etc.) instead of attempting to read them.
- Scan budgets: keep the Git fast path enabled when `maxFiles` / `maxTotalBytes` are set.
- npm install: make the `dup-code-check` bin work on Windows by launching the platform binary from the Node wrapper.
- Docs: document `maxFiles` stop behavior and `skippedBudgetMaxFiles` semantics.
