# CI Integration

[中文](ci.zh-CN.md)

This page provides CI-ready recipes with stable output, controlled cost, and explicit exit code behavior.

## Recommended command templates

### 1) Lowest cost: duplicate files only

```bash
dup-code-check --json --stats --strict .
```

Good for a low-cost gate: permission/traversal errors or budget aborts will fail; otherwise it outputs JSON for downstream checks.

### 2) Full report: run all detectors

```bash
dup-code-check --json --stats --strict --report .
```

Control output size with `--max-report-items`:

```bash
dup-code-check --json --stats --strict --report --max-report-items 100 .
```

### 3) Multiple repos/roots: only cross-root duplicates

```bash
dup-code-check --json --stats --strict --report --cross-repo-only /repoA /repoB
```

## Persist outputs as CI artifacts

Save results and stats separately (avoid mixing stdout/stderr):

```bash
dup-code-check --json --stats --report . >dup-code-check.result.json 2>dup-code-check.stats.txt
```

> In text mode, `--stats` prints to stderr. In JSON mode, `--stats` merges `scanStats` into stdout JSON.

## How to fail the CI?

`dup-code-check` fails mainly in two cases:

1. runtime errors: invalid args, root does not exist / is not a directory, scan failures
2. `--strict`: scan was incomplete (permission / traversal / budget abort)

If you also want to fail when duplicates are found, add a separate step to check the JSON output, e.g.:

- `fileDuplicates.length > 0` → fail
- `codeSpanDuplicates.length > 0` → fail

(This is policy; define thresholds based on your team’s tolerance.)

## Cost control tips

In large repos, consider enabling at least one of:

- `--ignore-dir` (skip dependencies/build outputs)
- `--max-file-size` (skip huge files)
- `--max-files` / `--max-total-bytes` (set budgets)

Use carefully:

- `--follow-symlinks` (may expand scan scope)

## Exit code quick reference

See the “Exit codes” section in [CLI Usage](cli.md).
