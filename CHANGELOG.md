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
- Tests: add regression coverage for safe relative-path validation and mid-read budget/file-size enforcement.

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
- Docs: exclude `README*.md` pages from `llms` bundles.
- Docs: nav language switch now points to the Chinese home page.
- npm: align metadata with npm (drop pnpm hints) and avoid shipping `docs/` in the npm package.
- Dev: regenerate `package-lock.json` with `registry.npmjs.org` resolved URLs (avoid `npmmirror`).
- Docs: derive `llms` bundle ordering from VitePress sidebar config (warn on missing/unlinked pages).
- Docs: localize the `llms.zh-CN.txt` bundle header.
- Docs: strip VitePress frontmatter from `llms` bundles.
- Docs: README now links to the hosted docs and clarifies docs are not shipped in the npm package.
- Docs: `LLMS_STRICT=1` makes `docs:llms` fail on ordering/sidebar inconsistencies.
- Docs workflow: skip GitHub Pages deployment when the repository is private.
- Docs workflow: auto-enable GitHub Pages (Actions) when deploying docs.
- Release workflow: publish to crates.io on tag releases (and skip when token is missing).
- Release workflow: avoid `secrets.*` in `if:` to prevent workflow validation issues.
- Release workflow: retry `cargo publish` for the CLI to tolerate crates.io index propagation delay.
- Repo links: update GitHub owner / Pages URLs (`omne42`).
- Metadata: use MIT-only license identifier.
- Scan budgets: `maxFiles` now stops scanning once the file-count budget is hit (`skippedBudgetMaxFiles` becomes non-zero).
- Node installer: `postinstall` builds the Rust binary for global installs; project installs build on first run (or set `DUP_CODE_CHECK_BUILD_ON_INSTALL=1`).
- Scan pipeline: stream `git ls-files` enumeration in the Git fast path (avoid collecting full lists; stop early under `maxFiles`).
- CLI: clarify `--strict` semantics (fatal skips only: permission/traversal/budget/bucket) and add smoke coverage.
- CLI: always emit fatal-skip warnings to stderr (even with `--stats`; the `--stats` re-run hint is shown only when needed).
- Rust: de-duplicate code-span (winnowing) and file-duplicate grouping logic via a shared internal helper.
- Core: tighten `DUP_CODE_CHECK_GIT_BIN` override validation (absolute path required; must exist and be a file).
- Core: further tighten `DUP_CODE_CHECK_GIT_BIN` override validation (must not be a symlink; must be executable and not world-writable on Unix).
- Core: require `DUP_CODE_CHECK_ALLOW_CUSTOM_GIT=1` to honor `DUP_CODE_CHECK_GIT_BIN` (opt-in).
- Report: set a default `maxTotalBytes` budget (256 MiB) to bound memory use; override via `--max-total-bytes`.
- Docs: mention the `--report` default `--max-total-bytes` budget in `--help` and `README`.
- CLI: resolve roots via `canonicalize()` (fail if it fails) to reduce symlink ambiguity.
- Docs: add a security note that npm `postinstall` runs a native build (Cargo) and may execute dependency build scripts.
- Docs: add the same `postinstall` security note to Getting Started.
- Scan stats: record detector bucket truncation as `skippedBucketTruncated`.
- Core: reduce memory usage for file-duplicate grouping by avoiding storing full normalized samples.
- Core: split the scan module into smaller files (no behavior change).
- CLI: treat `skippedBucketTruncated` as a fatal skip (scan incomplete) for warnings/`--strict`.
- CLI: `--strict` now treats `outside_root` traversal skips as fatal (scan incomplete).
- Scan: `ignoreDirs` matching is case-insensitive on Windows (ASCII).
- Tokenizer: treat `#` as a comment only at line start (after optional whitespace).
- CI: docs build now runs with `LLMS_STRICT=1`.
- CI: pin Rust toolchain to `1.92.0` (match `rust-toolchain.toml`).
- Scan: when Git streaming enumeration fails, fall back to the walker (avoid early aborts / double-scans).
- CLI: resolve roots by directly `canonicalize()`-ing user paths (preserve symlink semantics).
- Report: reduce memory usage by avoiding storing full file text for previews (generate previews from files on demand).
- Rust: refactor the winnowing detector API to pass params as a struct (no behavior change).
- Normalization: code-span and line-span detectors now keep only ASCII word chars (`[A-Za-z0-9_]`) to match the docs.
- Report: avoid extra `String` allocations when scanning/tokenizing files for `--report`.
- Scan: remove the redundant `git check-ignore` step in the Git fast path (less overhead; same results).

### Fixed
- Tolerate `NotFound` during scanning (files deleted mid-scan).
- Avoid leaking absolute paths in results when path prefix stripping fails.
- Avoid potential panics in winnowing match selection on unexpected empty windows.
- `--follow-symlinks` now works reliably by using the walker path when enabled.
- When `--follow-symlinks` is enabled, skip symlinked directories that resolve outside the root.
- Harden file reads under `--follow-symlinks` against symlink/TOCTOU races.
- Token-based detectors now record the start line for multi-line string tokens.
- CLI now supports `--` to terminate option parsing (allows roots that start with `-`).
- CLI now errors when `--report` and `--code-spans` are both specified.
- CLI: `--cross-repo-only` now errors when fewer than 2 roots are provided.
- Scanning now skips `PermissionDenied` and walker traversal errors instead of aborting.
- Scanning now skips per-file read I/O failures instead of aborting.
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
- CLI: `--version` now respects `--` (treats `--version` after `--` as a root).
- Scan pipeline: when `git ls-files` exits non-zero during streaming, fall back to the walker (fail closed).
- Scan pipeline: when `git ls-files` outputs non-UTF-8 paths in streaming mode, fall back to the walker (even after scanning has started).
- npm build: fail early with a clear error when `bin/dup-code-check.mjs` wrapper is missing.
- npm build: improve Cargo build failure diagnostics.
- npm build: resolve `cargo` from PATH excluding `node_modules/.bin` (supply-chain hardening).
- npm build: avoid running non-executable or world-writable `cargo` binaries found on PATH (Unix).
- Node smoke: verify the wrapper can execute `--version` when deciding whether to rebuild.
- Node smoke: avoid following symlink directories when probing source mtimes (more stable in unusual worktrees).
- npm package: include `rust-toolchain.toml` so installs use the pinned Rust toolchain.
- CLI: localize `Number.MAX_SAFE_INTEGER` errors for integer options.
- CLI: improve fatal-skip warnings with a reason summary and actionable hints when `--stats` is not enabled.
- CLI: fatal-skip warnings now include JSON-compatible `scanStats` keys (camelCase), with snake_case aliases.
- Core: reduce the risk of false file-duplicate grouping due to hash collisions by adding prefix/suffix samples to fingerprints.
- Tests: make the PermissionDenied scanning test robust in environments where `chmod 000` is ineffective (e.g. running as root).
- Core: re-verify whitespace-normalized file contents before emitting file-duplicate groups (avoid false positives from hash collisions or file changes).
- Scan budgets: binary files no longer bypass `maxFiles` / `maxTotalBytes`, and binary detection avoids reading entire binaries.
- Scan budgets: enforce `maxTotalBytes` / `maxFileSize` during reads to avoid budget overruns when files grow mid-scan.
- Report: avoid panics when truncating previews containing non-ASCII characters.
