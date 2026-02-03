# Getting Started

[中文](getting-started.zh-CN.md)

Goal: run a scan with minimal setup, and understand the shape of the output.

## 0) Prerequisites

- Rust toolchain `1.92.0` (pinned by `rust-toolchain.toml`)
- (Optional) Node.js `>= 22` (only if you want to install/build via npm)
- (Optional) Git: by default we respect `.gitignore` (and `.git/info/exclude` / global ignores when in a Git repo), and when available we use `git ls-files` to speed up file collection. Use `--no-gitignore` to include ignored files.

## 1) Run from source (recommended)

From the repo root:

```bash
cargo build --release -p dup-code-check
./target/release/dup-code-check --help
```

Or build via npm and run:

```bash
npm install
npm run build
./bin/dup-code-check --help
```

Or run via `npx` (will compile during `postinstall`):

```bash
npx dup-code-check --help
```

> Security note: npm installs run `postinstall` (Cargo build), which may execute dependency build scripts. Use `--ignore-scripts` / `npm_config_ignore_scripts=true` if you need to avoid running install scripts (see [Installation & Build](installation.md)).

## 2) Scan a directory: duplicate files (default)

```bash
dup-code-check .
```

In text mode you’ll see:

- `duplicate groups: N`
- one line per group: `hash=... normalized_len=... files=...`
- then a list of `- [repoLabel] path`

`normalized_len` is the byte length after removing ASCII whitespace (not the original file size).

## 3) Scan multiple roots: only cross-root duplicates

```bash
dup-code-check --cross-repo-only /repoA /repoB
```

`--cross-repo-only` filters out groups that only occur within a single root.

Roots are exactly the paths you pass; `--cross-repo-only` only works with 2+ roots and does not auto-detect Git repos.

## 4) Suspected duplicate code spans (with line ranges)

```bash
dup-code-check --code-spans --cross-repo-only /repoA /repoB
```

Output contains:

- `duplicate code span groups: N`
- `preview=...` for each group
- occurrences as `path:startLine-endLine`

> This is a lightweight detector: no AST parsing; it uses normalization + matching/fingerprints.

## 5) JSON output (for CI / post-processing)

```bash
dup-code-check --json --stats --strict .
```

- `--json`: machine-readable output
- `--stats`: includes `scanStats` (or prints to stderr in text mode)
- `--strict`: exits non-zero if the scan was incomplete (e.g. permission errors, traversal errors, budget abort)

Tip: non-fatal skips still exit `0`; use `--stats` (check stderr in text mode) and `--strict` to fail CI on incomplete scans.

For a complete field reference, see [Output & Report](output.md).
