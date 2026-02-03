# 扫描选项（ScanOptions）

[English](scan-options.md)

CLI 与 Node.js API 共享同一套扫描选项；CLI 参数会被转换为 `ScanOptions` 传入原生模块。

> 默认值以 Rust 核心的 `ScanOptions::default()` 为准；CLI 的 `--help` 里也会展示部分默认值。

## 目录与 ignore 规则

### `ignoreDirs` / `--ignore-dir`

忽略特定“目录名”（按 path segment 匹配），常用于跳过依赖目录或构建产物目录。

默认包含（节选）：

- `.git`, `node_modules`, `target`, `dist`, `build`, `out`, `.next`, `.turbo`, `.cache`

CLI 中可多次传入：

```bash
dup-code-check --ignore-dir vendor --ignore-dir .venv .
```

### `respectGitignore` / `--no-gitignore`

默认 `true`，会尊重 `.gitignore` 规则（并在可用时使用 `git` 命令加速文件收集）。

关闭：

```bash
dup-code-check --no-gitignore .
```

重新启用（默认已启用；主要用于脚本组合）：

```bash
dup-code-check --gitignore .
```

注意：

- 即使关闭 `.gitignore`，`ignoreDirs` 仍然生效
- 在 Git 仓库内会遵循 `.gitignore`、`.git/info/exclude` 与全局忽略规则

### `followSymlinks` / `--follow-symlinks`

默认 `false`（不跟随符号链接）。开启后会跟随 symlink 目录/文件进行扫描：

```bash
dup-code-check --follow-symlinks .
```

> 在包含大量 symlink 的 monorepo/构建目录中，建议谨慎开启，以免扫描范围爆炸或产生循环。

## 扫描预算（Budget）

预算用于控制扫描成本，适合在 CI 中做“快速守门”。

### `maxFiles` / `--max-files`

读取并处理 `n` 个文件后停止扫描；达到上限后会提前结束扫描，`scanStats.skippedBudgetMaxFiles` 会变为非 0。

> `--strict` 模式下，触发 `maxFiles` 会被视为“扫描不完整”，从而退出非 0。

### `maxTotalBytes` / `--max-total-bytes`

累计扫描字节数预算：当某个文件会导致 `scannedBytes + fileSize > maxTotalBytes` 时，该文件会被跳过，并在 `scanStats.skippedBudgetMaxTotalBytes` 中体现。

> 这与 `maxFiles` 不同：`maxFiles` 达到上限后会停止扫描；`maxTotalBytes` 会继续扫描，但可能跳过很多文件。

### `maxFileSize` / `--max-file-size`

跳过大于 `n` 字节的文件（默认 `10 MiB`）。被跳过的文件会计入 `scanStats.skippedTooLarge`。

## 检测阈值

### `minMatchLen` / `--min-match-len`

影响：

- `--code-spans`（疑似重复代码片段）的最小归一化长度
- 报告模式中的 `codeSpanDuplicates`
- 报告模式中的 `lineSpanDuplicates` 会以“字符长度预算”做过滤（避免把很短的行片段当成重复）

默认 `50`。

### `minTokenLen` / `--min-token-len`

影响报告模式中基于 token/block 的检测器：

- `tokenSpanDuplicates`
- `blockDuplicates`
- `astSubtreeDuplicates`
- `similarBlocksMinhash`
- `similarBlocksSimhash`

默认 `50`。

### `similarityThreshold` / `--similarity-threshold`

影响相似度检测器（MinHash/SimHash）。默认 `0.85`（范围 `0..1`）。

### `simhashMaxDistance` / `--simhash-max-distance`

影响 SimHash：最大允许的汉明距离（默认 `3`，范围 `0..64`）。

## 输出控制（仅 `--report`）

### `maxReportItems` / `--max-report-items`

每个报告 section 最多输出多少条结果（默认 `200`）。

- 数值越大：越全面，但输出更长、内存/时间开销更高
- 设置为 `0`：直接输出空报告（快速“禁用 report”）

## 仅跨 root 输出

### `crossRepoOnly` / `--cross-repo-only`

若为 `true`，仅输出跨 `>=2` 个 root 的重复组（无论是文件重复还是片段重复）。
