# 架构速览

[English](architecture.md)

## 分层

- `crates/core`：纯 Rust 核心逻辑（扫描文件、归一化、计算指纹、输出结果）
- `crates/cli`：Rust CLI（二进制入口，参数解析/输出/退出码）

## 数据流（从 CLI 到结果）

1. `dup-code-check` 解析 CLI 参数 → 组装 `roots + options`
2. Rust 核心：
   - 收集文件路径（默认尊重 `.gitignore`，并忽略常见依赖/产物目录）
   - 读取文件内容，跳过超大/二进制文件
   - 执行检测器并返回结果（必要时附带 `ScanStats`）
3. CLI 侧格式化输出（文本或 JSON），并根据 `--strict`/stats 设置退出码

## 核心抽象（概念层）

- `roots`：多个扫描 root（可跨仓库/跨目录）
- `ScanOptions`：控制 ignore、预算、阈值、输出规模等
- `ScanStats`：扫描统计与“扫描完整性”的依据（CI/strict 模式常用）

相关文档：

- 选项详解：《[扫描选项](scan-options.zh-CN.md)》
- 检测器详解：《[检测器与算法](detectors.zh-CN.md)》

## 构建方式（本地）

- `npm run build`：
  - `cargo build --release -p dup-code-check`
  - 将产物复制为 `bin/dup-code-check`

## 关键实现位置（方便读代码）

- CLI：`crates/cli/src/main.rs`
  - 参数解析：`parse_args()`
  - 退出码策略：`--strict` + `ScanStats`
- Rust 核心：`crates/core/src/lib.rs`
  - 文件收集：`collect_repo_files*`
  - 重复文件：`find_duplicate_files*`
  - code spans：`find_duplicate_code_spans*`
  - report：`generate_duplication_report*`

## 可扩展点

扩展一个新 detector 的推荐路径：

1. 在 `crates/core` 增加新的检测逻辑（最好带单测）
2. 若要对外暴露：
   - 在 `crates/cli` 增加 CLI 参数/输出
3. 更新文档（`docs/`）与 `CHANGELOG.md`

## 扩展思路

- 在 `crates/core` 增加新的检测能力（例如：片段级克隆检测、规则扫描、AST 分析等）
- 在 `crates/cli` 增加子命令或参数，形成统一的 CLI 入口
