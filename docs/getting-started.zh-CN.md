# 快速开始

[English](getting-started.md)

本页目标：用最少步骤跑通一次扫描，并看懂输出长什么样。

## 0) 前置条件

- Rust toolchain `1.92.0`（仓库用 `rust-toolchain.toml` 固定）
- （可选）Node.js `>= 22`（如果你想用 npm 安装/构建）
- （可选）Git：默认会尊重 `.gitignore`，并在可用时通过 `git ls-files` 加速文件收集

## 1) 从源码跑 CLI（推荐：最稳定）

在仓库根目录：

```bash
cargo build --release -p dup-code-check
./target/release/dup-code-check --help
```

你也可以通过 npm 构建并运行：

```bash
npm install
npm run build
./bin/dup-code-check --help
```

你也可以用 `npx`（会执行 `postinstall` 编译二进制）：

```bash
npx dup-code-check --help
```

> 安全提示：npm 安装会执行 `postinstall`（Cargo 原生构建），并可能运行依赖的 build script。如果你需要避免执行安装脚本，请使用 `--ignore-scripts` / `npm_config_ignore_scripts=true`（见《[安装与构建](installation.zh-CN.md)》）。

## 2) 扫描一个目录：重复文件（默认）

```bash
dup-code-check .
```

输出示例（文本模式）大致是：

- `duplicate groups: N`
- 每个 group 一行 `hash=... normalized_len=... files=...`
- 然后列出 `- [repoLabel] path`

这里的 `normalized_len` 是“去 whitespace 后”的字节长度（不是原文件大小）。

## 3) 扫描多个目录：只看跨 root 的重复

当你要比较多个仓库/多个目录时：

```bash
dup-code-check --cross-repo-only /repoA /repoB
```

`--cross-repo-only` 会过滤掉“只在同一个 root 内出现”的重复组。

## 4) 扫描疑似重复代码片段（输出行号范围）

```bash
dup-code-check --code-spans --cross-repo-only /repoA /repoB
```

这会输出：

- `duplicate code span groups: N`
- 每组包含 `preview=...`
- 每个 occurrence 以 `path:startLine-endLine` 的形式定位

> 这是一个“轻量级”的重复片段检测：它不会做 AST 解析，只做字符归一化与指纹/匹配。

## 5) 输出 JSON（用于二次处理/CI）

```bash
dup-code-check --json --stats --strict .
```

- `--json`：结构化输出（机器可读）
- `--stats`：包含扫描统计信息（JSON 中会附带 `scanStats`；文本模式下会打印到 stderr）
- `--strict`：如果扫描过程中出现“致命跳过”（例如权限错误、遍历错误、预算打断），退出码为非 0

更完整的输出字段说明见《[输出与报告](output.zh-CN.md)》。
