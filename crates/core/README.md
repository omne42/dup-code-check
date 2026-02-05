# dup-code-check-core

Core library for `dup-code-check` duplication scanning.

- Repo: https://github.com/omne42/dup-code-check
- CLI: https://crates.io/crates/dup-code-check (after publishing)
- Docs: https://omne42.github.io/dup-code-check/

## API stability note

This crate is currently developed primarily as an internal library for the `dup-code-check` CLI.
Public types may change between versions.

In particular, some output structs use `Arc<str>` for `repo_label` / `path` to reduce allocations
and make cloning cheap when generating large reports.

These fields are not exposed directly; use accessor methods (e.g. `repo_label()` / `path()`)
instead of accessing struct fields.

## License

MIT
