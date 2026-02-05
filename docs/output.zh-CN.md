# 输出与报告

[English](output.md)

`dup-code-check` 支持文本输出与 JSON 输出。文本输出适合人工阅读；JSON 输出适合二次处理与 CI 集成。

## 1) 重复文件（默认模式）

### 文本输出

形如：

- `duplicate groups: <N>`
- 每组：
  - `hash=<...> normalized_len=<...> files=<...>`
  - `- [repoLabel] path`

### JSON 输出（`--json`）

输出为数组，每个元素是：

```ts
interface DuplicateGroup {
  hash: string;          // 16 位 hex 字符串（FNV-1a 64）
  normalizedLen: number; // 去 whitespace 后的字节长度
  files: { repoId: number; repoLabel: string; path: string }[];
}
```

## 2) 疑似重复代码片段（`--code-spans`）

### 文本输出

形如：

- `duplicate code span groups: <N>`
- 每组：
  - `hash=<...> normalized_len=<...> occurrences=<...>`
  - `preview=<...>`
  - `- [repoLabel] path:startLine-endLine`

### JSON 输出（`--json`）

输出为数组，每个元素是：

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

## 3) 扫描统计（`--stats`）

### JSON 模式

当你同时开启 `--json --stats`：

- 默认模式 / `--code-spans`：输出 `{ groups, scanStats }`
- `--report`：输出 `{ report, scanStats }`

`scanStats` 字段：

- `candidateFiles`：候选文件数（收集到的路径数量）
- `scannedFiles`：实际读取并处理的文件数
- `scannedBytes`：实际读取的总字节数
- `gitFastPathFallbacks`：Git 快路径回退次数（尝试使用 Git 快路径但回退到 walker 时为非 0）
- `skippedNotFound`：扫描时遇到 `NotFound`（文件被删/变更）
- `skippedPermissionDenied`：权限不足
- `skippedTooLarge`：超过 `maxFileSize`
- `skippedBinary`：包含 `\\0` 字节的二进制文件
- `skippedOutsideRoot`：路径位于 root 之外或不安全（例如符号链接目标解析到 root 之外；或 Git 快路径遇到不安全路径；为安全起见跳过）
- `skippedRelativizeFailed`：路径无法相对化到提供的 root（不符合预期；可视为 bug 线索）
- `skippedWalkErrors`：遍历错误（walker errors）
- `skippedBudgetMaxFiles`：因 `maxFiles` 预算导致提前结束扫描（非 0 表示触发）
- `skippedBudgetMaxTotalBytes`：因 `maxTotalBytes` 预算跳过的文件数（当某文件会使累计扫描字节数超出预算时被跳过）
- `skippedBudgetMaxNormalizedChars`：因 `maxNormalizedChars` 预算导致提前结束扫描（非 0 表示触发）
- `skippedBudgetMaxTokens`：因 `maxTokens` 预算导致提前结束扫描（报告模式；非 0 表示触发）
- `skippedBucketTruncated`：检测器防爆保护；部分 fingerprint bucket 被截断（可能导致漏报）

### 文本模式

`--stats` 会把统计信息打印到 stderr（stdout 仍输出扫描结果），便于管道处理：

```bash
dup-code-check --stats . >result.txt 2>stats.txt
```

## 4) 严格模式（`--strict`）

`--strict` 用于在 CI 中判断“扫描是否完整”：

- 若出现 `PermissionDenied` / `outside_root` / `relativize_failed` / 遍历错误 / bucket 截断 / 预算限制（`maxFiles` / `maxTotalBytes` / `maxNormalizedChars` / `maxTokens`），退出码为 `1`
- 其他跳过（`NotFound` / `TooLarge` / `Binary`）不会触发失败

当 `--json` 开启且 `--stats` 未开启时，`--strict` 仍会在失败时把统计打印到 stderr，避免你拿不到原因。

## 5) 报告模式（`--report`）

文本输出包含多个 section（顺序如下）：

1. `file duplicates`
2. `code span duplicates`
3. `line span duplicates`
4. `token span duplicates`
5. `block duplicates`
6. `AST subtree duplicates`
7. `similar blocks (minhash)`
8. `similar blocks (simhash)`

JSON 输出为：

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

各 section 的语义/实现思路见《[检测器与算法](detectors.zh-CN.md)》。
