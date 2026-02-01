# Development

[中文](development.zh-CN.md)

This page is for contributors who want to modify `dup-code-check`: how to build, test, and navigate the repo structure.

## Repo layout

- `crates/core`: Rust core (scanning, normalization, detectors, report generation)
- `crates/cli`: Rust CLI binary
- `bin/`: built binary output (`bin/dup-code-check`)
- `scripts/`: build/validation scripts

## Build

```bash
npm install
npm run build
```

The binary is written to `bin/dup-code-check`.

## Run

```bash
./bin/dup-code-check --help
./bin/dup-code-check .
```

## Tests & validation

### Rust tests

```bash
cargo test
```

### CLI smoke tests

```bash
npm test
```

`npm test` runs `scripts/smoke.mjs`, which creates small temporary repos/files to validate:

- duplicate file scan is correct
- argument validation (e.g. `--max-file-size`) is strict
- `--` option terminator works
- `.gitignore` is respected by default and can be disabled via `--no-gitignore`

### Unified gate (for local/hooks)

```bash
./scripts/gate.sh
```

It auto-detects project markers and runs:

- `cargo fmt --check`
- `cargo check`
- `npm run check`

## Git hooks (optional but recommended)

```bash
./scripts/bootstrap.sh
```

This configures `core.hooksPath=githooks` and enables:

- `pre-commit`: requires updating `CHANGELOG.md` for every commit, then runs the gate
- `commit-msg`: enforces Conventional Commits + branch prefix rules
