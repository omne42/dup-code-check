# Performance & Scaling

[中文](performance.zh-CN.md)

The default goal of `dup-code-check` is to scan repos quickly in local dev and CI, and output duplicates/similarity results with actionable file/line locations.

## Scanning: I/O and file collection

### File collection strategy

By default it respects `.gitignore` and, when possible, uses Git to speed up file enumeration:

- `respectGitignore=true`
- `followSymlinks=false`
- the root is a Git repo and `git` is available

If Git-based collection is not available, it falls back to a walker-based traversal.

### Controlling I/O cost

Use these options to control scan cost:

- `maxFileSize`: skip huge files (default 10 MiB)
- `maxFiles`: file-count budget
- `maxTotalBytes`: total-bytes budget
- `ignoreDirs`: skip dependency/build directories (defaults include common ones)

## Detection: rough complexity intuition

Detectors vary significantly in cost:

- `fileDuplicates`: low cost (linear scan + grouping)
- `codeSpanDuplicates` / `tokenSpanDuplicates`: medium cost (fingerprints/windows/candidate matching)
- `similarBlocks*`: higher cost (candidate generation + similarity computation), but depth is limited and thresholds filter aggressively

If you want “highest signal first”, start with duplicate files, then move to `--report` only when needed.

## Large repo tips (rules of thumb)

1. keep scan roots tight (only the directories you care about)
2. add explicit `--ignore-dir` for dependencies/build outputs
3. in CI, set budgets (`--max-total-bytes` or `--max-files`) and use `--strict` to surface incomplete scans
4. if it’s too slow:
   - first try disabling `--report`
   - then raise thresholds (`--min-token-len` / `--min-match-len`)
