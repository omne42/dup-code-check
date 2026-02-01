# Detectors & Algorithms

[中文](detectors.zh-CN.md)

This page explains what each `--report` section does, what kinds of duplicates it is good at, and the main implementation idea / limitations.

> Terminology: “clone types” are often categorized as Type-1/2/3/4. They are a way to describe similarity levels (not a strict binary standard).

## 0) Scanning & normalization (common prerequisites)

All detectors share the same upfront work:

1. Collect candidate file paths (respects `.gitignore` by default and skips common dirs like `node_modules/`)
2. Read file contents and skip:
   - files larger than `maxFileSize`
   - binary files containing `\\0`
   - runtime anomalies like `NotFound` / `PermissionDenied` (counted in `scanStats`)

See [Scan Options](scan-options.md) for details.

## 1) `fileDuplicates`: duplicate files (whitespace-insensitive)

### Goal

Find files that are identical after removing ASCII whitespace.

### Core idea

- remove ASCII whitespace (space/newline/tab, etc.)
- hash normalized bytes and group
- compare a sample within a group to reduce hash-collision risk

### Good for / not good for

- good: copy-pasted files with only formatting/indentation changes (a subset of Type-1)
- not good: identifier renames or small insertions/deletions (closer to Type-2/3)

## 2) `codeSpanDuplicates`: suspected duplicate code spans (character-level)

> Corresponds to the CLI `--code-spans` (also included in report mode).

### Goal

Quickly find suspected duplicate code snippets and report **line ranges** for manual review.

### Normalization

Character-level normalization:

- remove newlines
- drop all symbols + whitespace
- keep only `[A-Za-z0-9_]`

This is robust to formatting/punctuation differences, but still sensitive to identifier renames.

### Matching idea (conceptual)

- fingerprint + window selection (winnowing) over the normalized character stream
- extend candidate matches (maximal match)
- de-duplicate, group, and output occurrences with line ranges

### Good for / limitations

- good: quickly locate large copy/paste spans across files/repos
- limitations:
  - not an AST/token clone detector; false positives/negatives are possible
  - language-agnostic normalization loses semantic information

## 3) `lineSpanDuplicates`: line-normalized duplicate spans

### Goal

Detect multi-line duplicated spans while being insensitive to indentation/punctuation.

### Normalization (per line)

- keep only `[A-Za-z0-9_]` per line
- hash the sequence as a “line token”

Then detect duplicated windows over the line-token sequence, and filter using “sum of normalized char lengths >= minMatchLen”.

### Good for / limitations

- good: repeated blocks with per-line formatting/punctuation differences
- limitations: line-based, so reordering/insertion/deletion across lines impacts results more

## 4) `tokenSpanDuplicates`: token-level duplicate spans

### Goal

Closer to CPD-style detection: whitespace-insensitive and more robust for some Type-2 changes (like identifier renames).

### Tokenization (simplified)

A lightweight tokenizer (heuristic, not a full parser):

- keywords (`if/for/return/let/class/...`) → fixed tokens
- identifiers → `IDENT`
- numbers → `NUM`
- strings → `STR` (and records the start line for multi-line strings)
- punctuation kept as-is (`{}`, `()`, `;`, ...)

Then it applies a similar fingerprint/window strategy to find duplicated token spans.

### Good for / limitations

- good: structurally similar logic with renamed variables (Type-2-ish)
- limitations:
  - heuristic tokenizer: multi-language friendly but not “syntax-correct”
  - block/AST-ish detectors are weaker for languages without `{}` (e.g. Python)

## 5) `blockDuplicates`: `{}` block-level duplicates

### Goal

Detect fully duplicated brace blocks (at token level).

### Core idea

- tokenize
- build block nodes using `{` / `}` (token/line ranges, nesting, children)
- hash token slices per block and group

## 6) `astSubtreeDuplicates`: `{}`-structure “AST subtree” approximate duplicates

### Goal

More robust at the block-structure level: if a block’s structure + contents match, consider it a duplicate.

### Core idea (simplified)

Build a representation for each block:

- child blocks are replaced by a marker + child hash
- tokens outside child blocks are kept

This forms a tree based on `{}` nesting and fingerprints subtrees bottom-up.

### Limitation

This is not a real language AST; it’s an approximation based on brace structure.

## 7) `similarBlocksMinhash` / `similarBlocksSimhash`: similar block pairs

### Goal

Find highly similar (but not identical) block pairs (heuristics towards Type-3).

### Input

- derived from `{}` blocks
- uses only shallow depths (depth is limited to control scale)
- shingles over block token stream (default 5-grams)

### MinHash (`similarBlocksMinhash`)

- build MinHash signatures
- generate candidate pairs via LSH (banding/bucketing)
- filter by `score >= similarityThreshold` (and `crossRepoOnly` when enabled)

### SimHash (`similarBlocksSimhash`)

- build 64-bit SimHash per block
- generate candidates via banding/bucketing
- filter by `hamming_distance <= simhashMaxDistance` (and `crossRepoOnly` when enabled)
- output includes `distance`

### Good for / limitations

- good: hints for small edits/insertions/deletions in otherwise similar blocks
- limitations: approximate methods can produce false positives; verify via `preview` and line ranges

## 8) How to choose detectors?

A practical order:

1. start with `fileDuplicates` (cheap and high-signal)
2. then `codeSpanDuplicates` (fast localization for copy/paste spans)
3. for Type-2/3-ish signals: use `--report` (token/block/similar pairs)

In CI:

- use `--cross-repo-only` to focus on cross-root reuse/copy
- use `--max-report-items` to limit output size
- use `--strict` + `--stats` to enforce scan completeness
