# Introduction

`dup-code-check` is a toolbox for detecting duplicates/similarity in codebases, delivered as a **Rust CLI binary**.

It is designed to work well in:

- local refactoring (find duplication hotspots quickly)
- CI guardrails (set scan budgets + fail on incomplete scans)
- multi-repo / mono-repo audits (scan multiple roots and only report cross-root duplication)

## What it can find (MVP)

- **Duplicate files** (whitespace-insensitive)
- **Suspected duplicate code spans** (`--code-spans`) with line ranges
- **Report mode** (`--report`) that runs multiple detectors in one pass

## Quick example

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

## Next steps

- [Getting Started](getting-started.md)
- [CLI Usage](cli.md)
- [Scan Options](scan-options.md)
- [CI Integration](ci.md)

