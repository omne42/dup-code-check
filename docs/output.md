# Output & Report

[中文](output.zh-CN.md)

`dup-code-check` supports both text output and JSON output. Text is for humans; JSON is for post-processing and CI integration.

## 1) Duplicate files (default mode)

### Text

You’ll see:

- `duplicate groups: <N>`
- for each group:
  - `hash=<...> normalized_len=<...> files=<...>`
  - `- [repoLabel] path`

### JSON (`--json`)

JSON output is an array, each element:

```ts
interface DuplicateGroup {
  hash: string;          // 16 hex chars (FNV-1a 64)
  normalizedLen: number; // byte length after ASCII whitespace removal
  files: { repoId: number; repoLabel: string; path: string }[];
}
```

## 2) Suspected duplicate code spans (`--code-spans`)

### Text

- `duplicate code span groups: <N>`
- per group:
  - `hash=<...> normalized_len=<...> occurrences=<...>`
  - `preview=<...>`
  - `- [repoLabel] path:startLine-endLine`

### JSON (`--json`)

JSON output is an array, each element:

```ts
interface DuplicateSpanGroup {
  hash: string;
  normalizedLen: number;
  preview: string;
  occurrences: {
    repoId: number;
    repoLabel: string;
    path: string;
    startLine: number;
    endLine: number;
  }[];
}
```

## 3) Scan stats (`--stats`)

### JSON mode

With `--json --stats`:

- default / `--code-spans`: `{ groups, scanStats }`
- `--report`: `{ report, scanStats }`

`scanStats` fields include:

- `candidateFiles`, `scannedFiles`, `scannedBytes`
- `gitFastPathFallbacks`: non-zero when the scan attempted the Git fast path and had to fall back to the filesystem walker
- `skippedNotFound`, `skippedPermissionDenied`, `skippedTooLarge`, `skippedBinary`, `skippedOutsideRoot`, `skippedWalkErrors`
- `skippedBudgetMaxFiles`: non-zero when the scan stopped early due to the `maxFiles` budget
- `skippedBudgetMaxTotalBytes`: skipped due to `maxTotalBytes` (reading would exceed the total bytes budget)
- `skippedBucketTruncated`: detector guardrail; fingerprint buckets were truncated to cap worst-case cost (results may miss some matches)

### Text mode

In text mode, `--stats` prints stats to stderr while keeping results on stdout:

```bash
dup-code-check --stats . >result.txt 2>stats.txt
```

## 4) Strict mode (`--strict`)

`--strict` is intended for CI and answers “was the scan complete?”:

- exits `1` on `PermissionDenied`, traversal errors, or budget abort (`maxFiles` / `maxTotalBytes`)
- does **not** fail on `NotFound`, `TooLarge`, `Binary`, or `BucketTruncated`

When `--json` is enabled and `--stats` is not, `--strict` still prints stats to stderr on failure (so you can see why).

## 5) Report mode (`--report`)

Text output contains multiple sections (in this order):

1. `file duplicates`
2. `code span duplicates`
3. `line span duplicates`
4. `token span duplicates`
5. `block duplicates`
6. `AST subtree duplicates`
7. `similar blocks (minhash)`
8. `similar blocks (simhash)`

JSON output:

```ts
interface DuplicationReport {
  fileDuplicates: DuplicateGroup[];
  codeSpanDuplicates: DuplicateSpanGroup[];
  lineSpanDuplicates: DuplicateSpanGroup[];
  tokenSpanDuplicates: DuplicateSpanGroup[];
  blockDuplicates: DuplicateSpanGroup[];
  astSubtreeDuplicates: DuplicateSpanGroup[];
  similarBlocksMinhash: SimilarityPair[];
  similarBlocksSimhash: SimilarityPair[];
}
```

For the meaning/implementation ideas of each section, see [Detectors & Algorithms](detectors.md).
