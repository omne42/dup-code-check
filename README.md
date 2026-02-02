# dup-code-check

[中文文档](README.zh-CN.md)

`dup-code-check` is a toolbox for detecting duplicates/similarity in codebases. It ships as a **Rust CLI binary**; Node.js is used as an installation option via npm.

- CI: `.github/workflows/ci.yml`
- Docs (GitBook-style): `docs/README.md` (English) / `docs/README.zh-CN.md` (中文)

## What it does (MVP)

- Duplicate file detection (ignores ASCII whitespace differences)
  - Single root or multiple roots
  - Optionally only report groups that span >= 2 roots (`--cross-repo-only`)
- Suspected duplicate code spans (`--code-spans`)
  - Normalizes by removing symbols + whitespace (keeps `[A-Za-z0-9_]`)
  - Reports line ranges for occurrences
- Report mode (`--report`): multiple detectors/levels in one run
- Respects `.gitignore` by default

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

> Note: this package builds the Rust binary during `postinstall`, so Rust is required.
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
