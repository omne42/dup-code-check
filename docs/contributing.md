# Contributing

[中文](contributing.zh-CN.md)

Contributions are welcome. This page documents a minimal collaboration contract so changes stay maintainable, traceable, and verifiable.

## Setup

```bash
./scripts/bootstrap.sh
```

It will:

- initialize git (if this directory isn’t a repo yet)
- configure git hooks (`core.hooksPath=githooks`)
- install Node dependencies (`npm install`)

## Before you change things

1. Run the gate once to verify your local environment:

```bash
./scripts/gate.sh
```

2. Be explicit about what layer you’re changing:

- core scanning/detectors → `crates/core`
- CLI experience/output → `crates/cli`

## Commit conventions

### Branch naming

Allowed branch prefixes include:

- `feat/...`, `fix/...`, `docs/...`, `refactor/...`, `perf/...`, `test/...`, `chore/...`, `build/...`, `ci/...`, `revert/...`

### Commit messages

We use Conventional Commits, for example:

- `feat(core): add new detector`
- `fix(cli): handle invalid option`
- `docs(readme): add usage examples`

## CHANGELOG rules

The `pre-commit` hook enforces:

- every commit must update the `[Unreleased]` section in `CHANGELOG.md`
- released sections are immutable (unless an explicit env var is set)

## Pre-submit checks

Recommended:

```bash
./scripts/gate.sh
```

Or at least:

```bash
cargo test
npm test
```
