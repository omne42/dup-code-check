# 性能与可扩展性

`code-checker` 的默认目标是：在本地与 CI 中快速扫描工程目录，给出可定位的重复/相似结果。

## 扫描阶段：I/O 与文件收集

### 文件收集策略

默认会尊重 `.gitignore`，并在满足条件时优先走 Git 路径收集文件列表：

- `respectGitignore=true`
- `followSymlinks=false`
- root 是 Git 仓库且系统上可用 `git`

当 Git 路径不可用时，会退回到基于 walker 的遍历方式。

### I/O 成本控制

你可以用以下选项控制扫描成本：

- `maxFileSize`：跳过超大文件（默认 10 MiB）
- `maxFiles`：扫描文件数量预算
- `maxTotalBytes`：扫描字节数预算
- `ignoreDirs`：跳过依赖/构建目录（默认已包含常见目录）

## 检测阶段：算法复杂度直觉

不同检测器的成本差异很大：

- `fileDuplicates`：低成本（线性扫描 + 分组）
- `codeSpanDuplicates` / `tokenSpanDuplicates`：中等成本（指纹/窗口/候选匹配）
- `similarBlocks*`：成本更高（候选对生成 + 相似度计算），但实现上限制了 block 深度并有阈值过滤

如果你只想“先把最强信号跑出来”，建议先只跑重复文件，再逐步升级到 `--report`。

## 大仓库建议（经验法则）

1. 优先把扫描 root 控制到“你真正关心的目录”
2. 对依赖/产物目录明确加 `--ignore-dir`
3. CI 中尽量设预算（`--max-total-bytes` 或 `--max-files`），并用 `--strict` 显式感知“预算导致的不完整扫描”
4. 需要更快时：
   - 先尝试关闭 `--report`
   - 再尝试提高阈值（`--min-token-len` / `--min-match-len`）

