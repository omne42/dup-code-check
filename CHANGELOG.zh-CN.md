# 更新日志

[English](CHANGELOG.md)

本文件记录本项目的所有重要变更。

格式基于 *Keep a Changelog*，并遵循 *Semantic Versioning*。

## [Unreleased]

### Added
- 初始脚手架。
- Rust CLI 二进制：`dup-code-check`。
- CLI 国际化：`--localization <en|zh>`（默认：`en`）。
- 扫描预算：`maxFiles` / `maxTotalBytes`（CLI：`--max-files` / `--max-total-bytes`）。
- 扫描统计 + 严格模式（`--stats`, `--strict`）。
- CLI：`--gitignore` 显式启用 `.gitignore` 过滤（默认开启；主要用于脚本）。
- GitHub Actions：CI（Linux/macOS/Windows）、文档（GitHub Pages）与发布（GitHub Release + npm publish）。
- 文档：中英双语（EN/ZH），并提供互相跳转链接。

### Changed
- 项目重命名：`dup-check` → `dup-code-check`。
- 开发：released changelog 编辑的 override 环境变量重命名为 `DUP_CODE_CHECK_ALLOW_CHANGELOG_RELEASE_EDIT`。
- Rust：重构 core/report 与 CLI，拆分为更小的模块（无行为变化）。
- Rust：通过共享 helper 去重“扫描读取/跳过”的重复逻辑。
- 默认跳过大于 10 MiB 的文件（`DEFAULT_MAX_FILE_SIZE_BYTES`）。
- fallback scanner 现在通过 `ignore` crate 尊重嵌套 `.gitignore` 规则。
- CLI 整数参数拒绝非整数（例如 `--max-file-size 1.5`）。
- `--report` 通过避免大对象中间 clone 降低内存占用。
- 无效 root 现在会尽早失败，而不是输出空结果。
- 文档：在 `docs/` 下增加 GitBook 风格文档。
- 文档 workflow：当仓库为私有时跳过 GitHub Pages 部署。
- 文档 workflow：部署文档时自动启用 GitHub Pages（GitHub Actions）。
- 发布 workflow：在 tag 发布时执行 crates.io 发布（未配置 token 时自动跳过）。
- 发布 workflow：避免在 `if:` 中引用 `secrets.*`，以规避 workflow 校验问题。
- 发布 workflow：对 CLI 的 `cargo publish` 增加重试，以容忍 crates.io index 的同步延迟。
- 仓库链接：更新 GitHub owner / Pages 地址（`omne42`）。
- 元数据：license 仅使用 MIT（不再是双协议）。
- 扫描流程：改为流式遍历文件，降低峰值内存（避免预先收集完整文件列表）。

### Fixed
- 扫描时容忍 `NotFound`（例如扫描过程中文件被删除）。
- 避免 `git check-ignore` 集成中的 panic；失败时会 fallback。
- 当前缀剥离失败时避免在结果里泄漏绝对路径。
- `--follow-symlinks` 现在通过使用 walker path 更可靠。
- 启用 `--follow-symlinks` 时，跳过解析后位于 root 之外的符号链接目录。
- token 检测器现在会记录多行字符串 token 的起始行号。
- CLI 支持 `--` 终止参数解析（允许 root 以 `-` 开头）。
- 扫描会跳过 `PermissionDenied` 和 walker traversal errors，而不是直接中止。
- CLI 现在会捕获运行期扫描失败并以退出码 1 退出。
- 移除 unstable rustfmt 配置，避免 stable toolchain 警告。
- `--max-report-items` 现在在所有报告 section 中一致生效，并优先保留更大的 group。
- CLI：`--max-files` 现在会拒绝超过 `usize` 上限的值，而不是截断。
- CLI 支持 `--no-gitignore` 关闭 `.gitignore` 过滤。
- 当前缀剥离失败时，输出使用 `<external:...>/name`，以避免泄漏绝对路径同时保持可区分性。
- `git ls-files` 输出中若出现不安全的相对路径（绝对路径、`..` 等），将跳过而不是尝试读取。
- 扫描预算：启用 `maxFiles` / `maxTotalBytes` 时仍保持 Git 快路径（加速 CI 扫描）。
- 文档：澄清 `maxFiles` 行为与 `skippedBudgetMaxFiles` 字段语义。
