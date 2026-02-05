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
- when scanning inside a Git repo, ignore rules include `.gitignore`, `.git/info/exclude`, and global Git ignores

### `followSymlinks` / `--follow-symlinks`

Default `false` (don’t follow symlinks). Enable to scan symlinked dirs/files:

```bash
dup-code-check --follow-symlinks .
```

> In monorepos or build outputs with many symlinks, enable carefully to avoid exploding scan scope or cycles.

## Scan budgets

Budgets help control scan cost, especially in CI.

### `maxFiles` / `--max-files`

Stop scanning after reading/processing `n` files. When the limit is reached, the scan ends early and `scanStats.skippedBudgetMaxFiles` becomes non-zero.

> With `--strict`, hitting `maxFiles` is treated as an “incomplete scan” and will fail.

### `maxTotalBytes` / `--max-total-bytes`

Total bytes budget: if reading a file would make `scannedBytes + fileSize > maxTotalBytes`, that file is skipped and counted in `scanStats.skippedBudgetMaxTotalBytes`.

> Unlike `maxFiles` (which stops scanning once the limit is reached), `maxTotalBytes` continues scanning but may skip many files.

### `maxFileSize` / `--max-file-size`

Skips files larger than `n` bytes (default `10 MiB`). Counted in `scanStats.skippedTooLarge`.

### `maxNormalizedChars` / `--max-normalized-chars`

Stops scanning once the total stored **normalized code characters** would exceed `n`. When hit, the scan ends early and `scanStats.skippedBudgetMaxNormalizedChars` becomes non-zero.

Used by `--code-spans` and `--report` (text-based detectors).

> With `--strict`, hitting `maxNormalizedChars` is treated as an “incomplete scan” and will fail.

### `maxTokens` / `--max-tokens`

Stops scanning once the total stored **tokens** would exceed `n` (report mode). When hit, the scan ends early and `scanStats.skippedBudgetMaxTokens` becomes non-zero.

> With `--strict`, hitting `maxTokens` is treated as an “incomplete scan” and will fail.

> In `--report` mode, if `maxNormalizedChars` / `maxTokens` are unset, defaults are derived from `maxTotalBytes` to bound memory use.

## Detector thresholds

### `minMatchLen` / `--min-match-len`

Affects:

- `--code-spans` minimum normalized length
- `--report` `codeSpanDuplicates`
- `--report` `lineSpanDuplicates` filtering (prevents tiny line fragments from being treated as duplicates)

Default `50`.

> Must be `>= 1`. Core APIs reject `0` with an `InvalidInput` error.

### `minTokenLen` / `--min-token-len`

Affects token/block detectors in report mode:

- `tokenSpanDuplicates`
- `blockDuplicates`
- `astSubtreeDuplicates`
- `similarBlocksMinhash`
- `similarBlocksSimhash`

Default `50`.

> Must be `>= 1`. Core APIs reject `0` with an `InvalidInput` error.

### `similarityThreshold` / `--similarity-threshold`

Similarity detectors (MinHash/SimHash). Default `0.85` (range `0..1`).

> Core APIs validate this range and reject invalid values.

### `simhashMaxDistance` / `--simhash-max-distance`

SimHash maximum Hamming distance (default `3`, range `0..64`).

> Core APIs validate this range and reject invalid values.

## Output controls (only for `--report`)

### `maxReportItems` / `--max-report-items`

Maximum items per report section (default `200`).

- larger values: more complete, but larger output and higher memory/time
- `0`: outputs an empty report (fast way to “disable report”)

## Cross-root only

### `crossRepoOnly` / `--cross-repo-only`

When `true`, only output groups spanning `>= 2` roots (for both file duplicates and span duplicates).
