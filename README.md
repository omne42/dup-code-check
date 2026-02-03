# dup-code-check

[中文文档](README.zh-CN.md)

`dup-code-check` is a toolbox for detecting duplicates/similarity in codebases. It ships as a **Rust CLI binary**; Node.js is used as an installation option via npm.

- CI: `.github/workflows/ci.yml`
- Docs: https://omne42.github.io/dup-code-check/ (VitePress; sources in `docs/`; not shipped in the npm package)

## What it does (MVP)

- Duplicate file detection (ignores ASCII whitespace differences)
  - Single root or multiple roots
  - Optionally only report groups that span >= 2 roots (`--cross-repo-only`)
- Suspected duplicate code spans (`--code-spans`)
  - Normalizes by removing symbols + whitespace (keeps `[A-Za-z0-9_]`)
  - Reports line ranges for occurrences
- Report mode (`--report`): multiple detectors/levels in one run
- Respects `.gitignore` by default
  - Note: in a Git repo, ignore rules include `.gitignore`, `.git/info/exclude`, and global Git ignores. Use `--no-gitignore` to include ignored files.

## Install

### Option A: Rust (recommended)

```bash
cargo build --release -p dup-code-check
./target/release/dup-code-check --help
```

Or install locally:

```bash
cargo install --path . --bin dup-code-check
dup-code-check --help
```

### Option B: npm (builds from source on install)

```bash
npm i -D dup-code-check
npx dup-code-check --help
```

Requires:

- Node.js `>= 22`
- Rust toolchain `1.92.0` (pinned by `rust-toolchain.toml`)

> Note: this package builds the Rust binary during `postinstall`, so Rust is required.
>
> Security note: `postinstall` runs a native build (Cargo), which may execute dependency build scripts.
>
> You can also set `DUP_CODE_CHECK_SKIP_BUILD=1` to skip the build during install (then run `npm run build` manually inside `node_modules/dup-code-check/`).
>
> To avoid running install scripts, use `npm_config_ignore_scripts=true` and build manually:
>
> ```bash
> npm_config_ignore_scripts=true npm i -D dup-code-check
> npm run build
> ```

## Quick examples

```bash
dup-code-check .
dup-code-check --cross-repo-only /repoA /repoB
dup-code-check --code-spans --cross-repo-only /repoA /repoB
dup-code-check --json --report .
```

## Docs

To preview docs locally, clone the repo and run:

```bash
npm install --ignore-scripts
npm run docs:serve
```
