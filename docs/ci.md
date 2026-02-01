# CI 集成

本页给出一些可直接落地的 CI 用法（重点：稳定输出、可控成本、明确退出码）。

## 推荐命令模板

### 1) 最小成本：只做重复文件

```bash
code-checker --json --stats --strict .
```

适合做“低成本守门”：如果扫描遇到权限/遍历错误或被预算中断，会直接失败；否则输出 JSON 供后续处理。

### 2) 完整报告：一次跑全套检测器

```bash
code-checker --json --stats --strict --report .
```

配合 `--max-report-items` 控制输出规模：

```bash
code-checker --json --stats --strict --report --max-report-items 100 .
```

### 3) 多仓库/多目录：只看跨 root 重复

```bash
code-checker --json --stats --strict --report --cross-repo-only /repoA /repoB
```

## 输出落盘与产物归档

推荐把结果与统计分开保存（避免 stdout/stderr 混在一起）：

```bash
code-checker --json --stats --report . >code-checker.result.json 2>code-checker.stats.txt
```

> 文本模式下 `--stats` 打印到 stderr；JSON 模式下当 `--stats` 开启会把 `scanStats` 合并进 stdout 的 JSON。

## 如何“让 CI 失败”？

`code-checker` 的失败条件主要有两类：

1. 运行期错误：参数错误、root 不存在/不是目录、扫描异常等
2. `--strict` 触发：扫描不完整（权限/遍历错误/预算中断）

如果你还希望在“发现重复”时失败，可以在 CI 的下一步对 JSON 做检查，例如：

- `fileDuplicates.length > 0` → fail
- `codeSpanDuplicates.length > 0` → fail

（这一步属于“策略层”，建议由你的团队按实际容忍度来定义。）

## 扫描成本控制建议

在大仓库中建议至少开启其中一项：

- `--ignore-dir`（忽略构建产物/依赖目录）
- `--max-file-size`（跳过超大文件）
- `--max-files` / `--max-total-bytes`（设置预算，避免 CI 过慢）

并谨慎使用：

- `--follow-symlinks`（可能扩大扫描范围）

## 退出码速查

详见《[CLI 使用](cli.md)》的退出码章节。

