# CLI Usage

[中文](cli.zh-CN.md)

`dup-code-check` is a Rust CLI binary: it parses arguments, calls the Rust core, formats output, and decides exit codes.

## Basic usage

```bash
dup-code-check [options] [root ...]
```

- `root ...`: directories to scan; defaults to current working directory when omitted
- supports `--` to terminate option parsing (useful when a root starts with `-`)

Examples:

```bash
dup-code-check .                 # default: duplicate files
dup-code-check --code-spans .    # suspected duplicate code spans
dup-code-check --report .        # all detectors in one report
dup-code-check -- --repo         # root starts with '-' (use --)
```

## Modes

### 1) Default: duplicate files

```bash
dup-code-check [root ...]
```

Outputs “duplicate file groups” (each group contains 2+ files).

### 2) `--code-spans`: suspected duplicate code spans

```bash
dup-code-check --code-spans [root ...]
```

Outputs “duplicate span groups” (each group contains 2+ occurrences with line ranges).

### 3) `--report`: report mode

```bash
dup-code-check --report [root ...]
```

Runs multiple detectors and outputs a consolidated report (useful for manual review or CI artifacts).

## Output formats

- text (default): human-friendly
- JSON: `--json` for machine-readable output
- stats: `--stats` adds `scanStats` in JSON; prints to stderr in text mode

See [Output & Report](output.md) for a full field reference.

## Flags reference

> Flags apply across default / `--code-spans` / `--report`, but some only affect specific detectors (see [Scan Options](scan-options.md)).

### Behavior switches

- `--localization <en|zh>`: set help/text output language (default `en`; JSON output is unchanged)
- `--report`: run all detectors and output a report
- `--code-spans`: find suspected duplicate code spans (with line ranges)
- `--json`: JSON output
- `--stats`: scan stats (stderr in text; `scanStats` in JSON)
- `--strict`: non-zero exit code if scan was incomplete
- `--cross-repo-only`: only output groups spanning `>=2` roots
- `--no-gitignore`: do not respect `.gitignore` (default: respect)
- `--gitignore`: explicitly enable `.gitignore` (mainly useful in scripts)
- `--follow-symlinks`: follow symlinks (default: off)

### Thresholds & limits

- `--min-match-len <n>`: minimum normalized length for `--code-spans` (default `50`)
- `--min-token-len <n>`: minimum token length for token/block/AST-ish detectors (default `50`)
- `--similarity-threshold <f>`: similarity threshold `0..1` (default `0.85`)
- `--simhash-max-distance <n>`: SimHash max Hamming distance `0..64` (default `3`)
- `--max-report-items <n>`: max items per report section (default `200`)

### Scan budgets

- `--max-files <n>`: stop scanning after reading `n` files (`scanStats.skippedBudgetMaxFiles > 0` indicates the budget was hit)
- `--max-total-bytes <n>`: skip files that would exceed total scanned bytes budget
- `--max-file-size <n>`: skip files larger than `n` bytes (default `10485760` = 10 MiB)

### Ignore rules

- `--ignore-dir <name>`: ignore directory name (repeatable)

### Help

- `-h, --help`: show help
- `-V, --version`: show version

## Exit codes

- `0`: completed successfully (even if some non-fatal skips happened: `NotFound`/`TooLarge`/`Binary`)
- `1`:
  - runtime error (e.g. root does not exist / is not a directory, scan failures)
  - with `--strict`: scan was incomplete due to `PermissionDenied`, `outside_root`, walker errors, or budget abort (`maxFiles`/`maxTotalBytes`)
- `2`: argument parsing error (unknown flags, non-integers for integer flags, etc.)
