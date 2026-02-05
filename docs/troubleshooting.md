# Troubleshooting

[中文](troubleshooting.zh-CN.md)

This page covers common build/runtime issues and quick ways to diagnose them.

## 1) Missing Rust toolchain / `cargo` not found

Symptoms:

- `npm install` / `npm run build` says Rust toolchain is required
- errors include `ENOENT` / `cargo: not found`

Fix:

1. Install rustup: `https://rustup.rs`
2. Install and use the toolchain pinned by this repo:

```bash
rustup toolchain install 1.92.0
rustup override set 1.92.0
```

Then retry:

```bash
npm run build
```

## 2) Binary missing / not executable

Symptoms:

- `dup-code-check`: “command not found”
- `./bin/dup-code-check`: “No such file or directory”

Cause: the binary hasn’t been built (or isn’t placed where expected).

Fix:

```bash
npm run build
```

Or build directly via Rust:

```bash
cargo build --release -p dup-code-check
```

Then confirm the binary exists:

- macOS/Linux: `bin/dup-code-check`
- Windows: `bin/dup-code-check.exe`

## 3) CLI argument errors (exit code 2)

Symptoms: `Unknown option` or `--xxx must be an integer ...`.

Cause: the CLI validates some flags strictly (e.g. integer flags reject `1.5`).

Fix:

- use `--help` for correct usage
- pass integers to integer flags (bytes/counts, etc.)

## 4) Incomplete scan causes CI failure (`--strict`)

Symptoms: exit code `1`, and stderr prints scan stats containing:

- `permission_denied`
- `outside_root`
- `relativize_failed`
- `walk_errors`
- `bucket_truncated`
- `budget_max_files` / `budget_max_total_bytes`
- `budget_max_normalized_chars` / `budget_max_tokens`

Fix ideas:

- permission issues: adjust scan roots (avoid restricted dirs), or run CI with appropriate permissions
- traversal errors: ensure filesystem stability (container mounts, concurrent writes, etc.)
- bucket truncation: increase `--min-match-len` / `--min-token-len`, or use `--ignore-dir` to skip generated/vendor dirs
- budget limits: increase `--max-files` / `--max-total-bytes` / `--max-normalized-chars` / `--max-tokens`, or reduce roots / add `--ignore-dir`

## 5) `.gitignore` behavior differs from expectations

By default `.gitignore` is respected. When scanning inside a Git repo, ignore rules include `.gitignore`, `.git/info/exclude`, and global Git ignores. To fully scan (including ignored files), use:

```bash
dup-code-check --no-gitignore .
```

## 6) Windows build issues

If building from source fails on Windows, you typically need:

- Visual Studio Build Tools (C/C++ toolchain)
- compatible Rust toolchain and Node versions

Due to environment differences, prefer building in CI with pinned images/containers, or use WSL.

## 7) Overriding the `git` executable (advanced)

By default the scanner invokes `git` from `PATH` to speed up file enumeration inside Git repos. If `git` is missing or can’t be executed, the scanner falls back to the filesystem walker (still correct, usually just slower).

If you *must* use a specific `git` binary, you can override it via environment variables:

- set `DUP_CODE_CHECK_ALLOW_CUSTOM_GIT=1` (explicit opt-in)
- set `DUP_CODE_CHECK_GIT_BIN=/absolute/path/to/git`

The override is intentionally strict:

- the path must be absolute
- it must exist and be a file
- it must not be a symlink
- on Unix: it must be executable and must not be group/world-writable
