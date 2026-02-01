# Scan Options (`ScanOptions`)

[中文](scan-options.zh-CN.md)

The CLI and the Node.js API share the same scan options. CLI flags are mapped into a `ScanOptions` struct passed into the native core.

> Defaults follow Rust `ScanOptions::default()`; `--help` also shows some defaults.

## Directories & ignore rules

### `ignoreDirs` / `--ignore-dir`

Ignores specific directory names (matches by path segment). Commonly used to skip dependencies and build outputs.

Default includes (partial):

- `.git`, `node_modules`, `target`, `dist`, `build`, `out`, `.next`, `.turbo`, `.cache`

Repeatable in CLI:

```bash
dup-code-check --ignore-dir vendor --ignore-dir .venv .
```

### `respectGitignore` / `--no-gitignore`

Default `true`: respects `.gitignore` rules (and uses `git` to accelerate file collection when available).

Disable:

```bash
dup-code-check --no-gitignore .
```

Re-enable (mostly useful in scripts):

```bash
dup-code-check --gitignore .
```

Notes:

- even when `.gitignore` is disabled, `ignoreDirs` still applies
- `.gitignore` behavior depends on implementation details (e.g. whether the root is a Git repo)

### `followSymlinks` / `--follow-symlinks`

Default `false` (don’t follow symlinks). Enable to scan symlinked dirs/files:

```bash
dup-code-check --follow-symlinks .
```

> In monorepos or build outputs with many symlinks, enable carefully to avoid exploding scan scope or cycles.

## Scan budgets

Budgets help control scan cost, especially in CI.

### `maxFiles` / `--max-files`

Scan at most `n` files. Once exceeded, scanning stops early; reflected in `scanStats.skippedBudgetMaxFiles`.

> With `--strict`, an early stop due to `maxFiles` is treated as “incomplete scan” and will fail.

### `maxTotalBytes` / `--max-total-bytes`

Total bytes budget: if reading a file would make `scannedBytes + fileSize > maxTotalBytes`, that file is skipped and counted in `scanStats.skippedBudgetMaxTotalBytes`.

> Unlike `maxFiles` (which stops scanning), `maxTotalBytes` continues scanning but may skip many files.

### `maxFileSize` / `--max-file-size`

Skips files larger than `n` bytes (default `10 MiB`). Counted in `scanStats.skippedTooLarge`.

## Detector thresholds

### `minMatchLen` / `--min-match-len`

Affects:

- `--code-spans` minimum normalized length
- `--report` `codeSpanDuplicates`
- `--report` `lineSpanDuplicates` filtering (prevents tiny line fragments from being treated as duplicates)

Default `50`.

### `minTokenLen` / `--min-token-len`

Affects token/block detectors in report mode:

- `tokenSpanDuplicates`
- `blockDuplicates`
- `astSubtreeDuplicates`
- `similarBlocksMinhash`
- `similarBlocksSimhash`

Default `50`.

### `similarityThreshold` / `--similarity-threshold`

Similarity detectors (MinHash/SimHash). Default `0.85` (range `0..1`).

### `simhashMaxDistance` / `--simhash-max-distance`

SimHash maximum Hamming distance (default `3`, range `0..64`).

## Output controls (only for `--report`)

### `maxReportItems` / `--max-report-items`

Maximum items per report section (default `200`).

- larger values: more complete, but larger output and higher memory/time
- `0`: outputs an empty report (fast way to “disable report”)

## Cross-root only

### `crossRepoOnly` / `--cross-repo-only`

When `true`, only output groups spanning `>= 2` roots (for both file duplicates and span duplicates).
