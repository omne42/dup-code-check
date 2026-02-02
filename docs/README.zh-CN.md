# dup-code-check 文档

[English](README.md)

`dup-code-check` 是一个面向“重复/相似检测”的工具箱：以 **Rust 二进制** 交付；Node.js 仅作为一种安装方式（npm）。

当前重点在“重复”相关能力：

- **重复文件检测（Type-1 clone 的一个子集）**：对文件内容做 *ASCII whitespace*（空格/换行/tab 等）删除后完全一致即视为重复。
- **疑似重复代码片段检测（Code spans）**：对内容去除“符号 + 空白字符”，仅保留字母/数字/下划线；若存在连续 `>= minMatchLen`（默认 50）字符相同的片段，则输出可定位的行号范围。
- **报告模式（Report）**：一次扫描输出多种粒度的重复与相似度结果（文件 / 片段 / 行 / token / block / “AST 子树”近似，以及 MinHash/SimHash 相似块对）。

> 目标用户：需要在本地/CI 中快速发现 **跨目录/跨仓库** 的重复（或者疑似重复）并能定位到文件与行号范围。

## 你可以从这里开始

- 想先跑起来：看《[快速开始](getting-started.zh-CN.md)》
- 想接入 CI：看《[CI 集成](ci.zh-CN.md)》
- 想理解各检测器差异：看《[检测器与算法](detectors.zh-CN.md)》

## 快速示例

```bash
# 扫描当前目录（默认：重复文件）
dup-code-check .

# 仅报告跨仓库/跨 root 的重复组
dup-code-check --cross-repo-only /repoA /repoB

# 发现疑似重复代码片段（输出行号范围）
dup-code-check --code-spans --cross-repo-only /repoA /repoB

# 输出 JSON（便于做二次处理/接入 CI 报告）
dup-code-check --json --report .
```

## 文档导航

- 首页：`docs/index.md` / `docs/index.zh-CN.md`
- 介绍：`docs/introduction.md` / `docs/introduction.zh-CN.md`

## 本地预览文档

如果你只是想预览文档（并不想/不能安装 Rust），可以在安装依赖时跳过 scripts：

```bash
npm install --ignore-scripts
npm run docs:serve
```

## 面向 LLM 的 `llms.txt`

在执行 `docs:build` / `docs:serve` 时，会生成 `/llms.txt`（纯文本的文档合集）。
