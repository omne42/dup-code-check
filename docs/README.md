# dup-code-check docs

[中文](README.zh-CN.md)

`dup-code-check` is a toolbox for duplicate/similarity detection, delivered as a **Rust CLI binary**. Node.js is only used as an installation option via npm.

## Start here

- Want to run it quickly: see [Getting Started](getting-started.md)
- Want to integrate into CI: see [CI Integration](ci.md)
- Want to understand detectors/algorithms: see [Detectors & Algorithms](detectors.md)

## Quick examples

```bash
# Scan current directory (default: duplicate files)
dup-code-check .

# Only report groups spanning >= 2 roots
dup-code-check --cross-repo-only /repoA /repoB

# Suspected duplicate code spans (with line ranges)
dup-code-check --code-spans --cross-repo-only /repoA /repoB

# JSON output (for CI / post-processing)
dup-code-check --json --report .
```

## Navigation

GitBook-style TOC: `docs/SUMMARY.md` (English + 中文).

## Build docs locally

To preview docs locally (without requiring Rust), install dependencies without running install scripts:

```bash
npm install --ignore-scripts
npm run docs:serve
```
