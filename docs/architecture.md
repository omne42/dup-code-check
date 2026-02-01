# Architecture (Quick Overview)

[中文](architecture.zh-CN.md)

## Layers

- `crates/core`: pure Rust core logic (scan files, normalize, compute fingerprints, produce results)
- `crates/cli`: Rust CLI binary (entrypoint, argument parsing, output formatting, exit codes)

## Data flow (CLI → results)

1. `dup-code-check` parses CLI args → builds `roots + options`
2. Rust core:
   - collects file paths (respects `.gitignore` by default; skips common dependency/output dirs)
   - reads file contents; skips huge/binary files
   - runs detectors and returns results (optionally with `ScanStats`)
3. CLI formats output (text or JSON) and decides exit code based on `--strict` / stats

## Key abstractions (conceptual)

- `roots`: multiple scan roots (cross-repo / cross-directory)
- `ScanOptions`: ignore rules, budgets, thresholds, output limits
- `ScanStats`: scan statistics and “scan completeness” signal for CI/strict mode

Related docs:

- Options: [Scan Options](scan-options.md)
- Detectors: [Detectors & Algorithms](detectors.md)

## Local build (via npm)

`npm run build`:

- `cargo build --release -p dup-code-check`
- copies the binary to `bin/dup-code-check`

## Where to look in code

- CLI: `crates/cli/src/main.rs`
  - arg parsing: `parse_args()`
  - exit code policy: `--strict` + `ScanStats`
- Core: `crates/core/src/lib.rs`
  - file collection: `collect_repo_files*`
  - duplicate files: `find_duplicate_files*`
  - code spans: `find_duplicate_code_spans*`
  - report: `generate_duplication_report*`

## Extensibility

Suggested path to add a new detector:

1. add core logic under `crates/core` (preferably with unit tests)
2. expose it via CLI (flags/output) under `crates/cli` when needed
3. update docs (`docs/`) and `CHANGELOG.md`
