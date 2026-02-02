# 介绍

`dup-code-check` 是一个面向“重复/相似检测”的工具箱：以 **Rust CLI 二进制** 交付；Node.js 仅作为一种安装方式（npm）。

它适合用于：

- 本地重构：快速发现重复热点
- CI 守门：设置扫描预算，并在扫描不完整时失败
- 多仓库/多 root 审计：扫描多个 root，并只输出跨 root 的重复

## 目前能发现什么（MVP）

- **重复文件**（对空白字符归一化后完全一致）
- **疑似重复代码片段**（`--code-spans`，输出行号范围）
- **报告模式**（`--report`，一次扫描输出多种 detector 结果）

## 快速示例

```bash
# 扫描当前目录（默认：重复文件）
dup-code-check .

# 仅报告跨仓库/跨 root 的重复组
dup-code-check --cross-repo-only /repoA /repoB

# 发现疑似重复代码片段（输出行号范围）
dup-code-check --code-spans --cross-repo-only /repoA /repoB

# 输出 JSON（便于接入 CI 或二次处理）
dup-code-check --json --report .
```

## 下一步

- [快速开始](getting-started.zh-CN.md)
- [CLI 使用](cli.zh-CN.md)
- [扫描选项](scan-options.zh-CN.md)
- [CI 集成](ci.zh-CN.md)

