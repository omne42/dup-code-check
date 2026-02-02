# CLI 使用

[English](cli.md)

`dup-code-check` 是一个 Rust 二进制程序：负责参数解析、调用 Rust 核心、格式化输出与退出码策略。

## 基本用法

```bash
dup-code-check [options] [root ...]
```

- `root ...`：要扫描的目录列表；不传时默认是当前工作目录
- 支持 `--` 结束参数解析（当 root 以 `-` 开头时很有用）

示例：

```bash
dup-code-check .                 # 默认：重复文件检测
dup-code-check --code-spans .    # 疑似重复代码片段
dup-code-check --report .        # 一次输出多种检测结果
dup-code-check -- --repo         # root 以 '-' 开头时用 -- 终止 option 解析
```

## 三种运行模式

### 1) 默认模式：重复文件检测

```bash
dup-code-check [root ...]
```

输出为“重复文件组”（每组包含 2 个及以上文件）。

### 2) `--code-spans`：疑似重复代码片段

```bash
dup-code-check --code-spans [root ...]
```

输出为“疑似重复片段组”（每组包含 2 个及以上 occurrence，带行号范围）。

### 3) `--report`：报告模式

```bash
dup-code-check --report [root ...]
```

一次扫描输出多种粒度的结果，适合做人工 review 或接入 CI 产物。

## 输出格式

- 文本（默认）：面向人类阅读
- JSON：`--json` 输出结构化数据
- 统计：`--stats` 在 JSON 中附带 `scanStats`；在文本模式下打印到 stderr

更完整的字段说明见《[输出与报告](output.zh-CN.md)》。

## 参数一览

> 以下参数同时适用于 “默认/`--code-spans`/`--report`”，但有些参数只会影响特定检测器（见《[扫描选项](scan-options.zh-CN.md)》）。

### 行为开关

- `--localization <en|zh>`：切换帮助/文本输出语言（默认 `en`；JSON 输出不变）
- `--report`：运行全部检测器并输出报告
- `--code-spans`：发现疑似重复代码片段（输出行号范围）
- `--json`：输出 JSON（机器可读）
- `--stats`：输出扫描统计（文本模式写 stderr；JSON 模式附带 `scanStats`）
- `--strict`：若扫描不完整（出现“致命跳过”）则退出码非 0
- `--cross-repo-only`：仅输出跨 `>=2` 个 root 的重复组
- `--no-gitignore`：不尊重 `.gitignore`（默认会尊重）
- `--gitignore`：显式启用 `.gitignore`（默认已启用；主要用于脚本里和 `--no-gitignore` 做开关）
- `--follow-symlinks`：跟随符号链接（默认关闭）

### 阈值/上限

- `--min-match-len <n>`：`--code-spans` 的最小归一化长度（默认 `50`）
- `--min-token-len <n>`：token/block/“AST 子树”等检测的最小 token 长度（默认 `50`）
- `--similarity-threshold <f>`：相似度阈值 `0..1`（默认 `0.85`）
- `--simhash-max-distance <n>`：SimHash 最大汉明距离 `0..64`（默认 `3`）
- `--max-report-items <n>`：每个报告 section 最多输出条目数（默认 `200`）

### 扫描预算（Budget）

- `--max-files <n>`：读取 `n` 个文件后停止扫描（`scanStats.skippedBudgetMaxFiles > 0` 表示触发了预算）
- `--max-total-bytes <n>`：跳过会导致“累计扫描字节数”超过 `n` 的文件
- `--max-file-size <n>`：跳过大于 `n` 字节的文件（默认 `10485760`，即 10 MiB）

### 忽略规则

- `--ignore-dir <name>`：忽略目录名（可重复）

### 帮助

- `-h, --help`：显示帮助

## 退出码（Exit Codes）

- `0`：正常完成（即使跳过了 “NotFound/TooLarge/Binary”等非致命情况）
- `1`：
  - 运行期错误（例如 root 不存在/不是目录、扫描过程异常）
  - 启用 `--strict` 且出现“致命跳过”：`PermissionDenied` / 遍历错误 / 被预算中断（`maxFiles` / `maxTotalBytes`）
- `2`：参数解析错误（未知参数、非整数的整数参数等）
