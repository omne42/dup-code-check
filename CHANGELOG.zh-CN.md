# 更新日志

[English](CHANGELOG.md)

本文件记录本项目的所有重要变更。

格式基于 *Keep a Changelog*，并遵循 *Semantic Versioning*。

## [Unreleased]

### Added
- 初始脚手架。
- Rust CLI 二进制：`dup-code-check`。
- CLI 国际化：`--localization <en|zh>`（默认：`en`）。
- CLI：`--version` / `-V`。
- 扫描预算：`maxFiles` / `maxTotalBytes`（CLI：`--max-files` / `--max-total-bytes`）。
- 扫描统计 + 严格模式（`--stats`, `--strict`）。
- CLI：`--gitignore` 显式启用 `.gitignore` 过滤（默认开启；主要用于脚本）。
- npm：`bin/dup-code-check.mjs` 启动脚本（通过 npm 跨平台运行 `dup-code-check`）。
- Core：支持通过 `DUP_CODE_CHECK_GIT_BIN` 覆盖 `git` 可执行文件路径。
- GitHub Actions：CI（Linux/macOS/Windows）、文档（GitHub Pages）与发布（GitHub Release + npm publish）。
- 文档：中英双语（EN/ZH），并提供互相跳转链接。
- 文档：新增 `/llms.en.txt` + `/llms.zh-CN.txt` 合集，并补充 `LLMs` 文档页。
- 测试：增加安全相对路径校验与“读取过程中”预算/文件大小上限的回归覆盖。

### Changed
- 项目重命名：`dup-check` → `dup-code-check`。
- 开发：released changelog 编辑的 override 环境变量重命名为 `DUP_CODE_CHECK_ALLOW_CHANGELOG_RELEASE_EDIT`。
- Rust：重构 core/report 与 CLI，拆分为更小的模块（无行为变化）。
- Rust：通过共享 helper 去重“扫描读取/跳过”的重复逻辑。
- 测试：增加扫描预算场景下 Git 快路径的回归覆盖。
- 默认跳过大于 10 MiB 的文件（`DEFAULT_MAX_FILE_SIZE_BYTES`）。
- fallback scanner 现在通过 `ignore` crate 尊重嵌套 `.gitignore` 规则。
- CLI 整数参数拒绝非整数（例如 `--max-file-size 1.5`）。
- `--report` 通过避免大对象中间 clone 降低内存占用。
- 无效 root 现在会尽早失败，而不是输出空结果。
- 文档：从 Honkit 迁移到 VitePress（并提供 `llms.txt`）。
- 文档：优化 `llms.txt` 生成顺序，并补充可直接复用的提示词模板。
- 文档：`llms` 合集中排除 `README*.md` 页面。
- 文档：导航栏“中文”入口现在指向中文首页。
- npm：对齐 npm 生态（移除 pnpm 相关提示），并避免在 npm 包中发布 `docs/`。
- 开发：用 `registry.npmjs.org` 重新生成 `package-lock.json`（避免 `npmmirror`）。
- 文档：`llms` 合集的顺序改为从 VitePress sidebar 配置派生（并对缺失/未挂载页面给出 warning）。
- 文档：`llms.zh-CN.txt` 的 header 改为中文。
- 文档：`llms` 合集中移除 VitePress frontmatter（降低噪音）。
- 文档：README 改为指向线上文档，并说明 npm 包不包含 `docs/`。
- 文档：支持 `LLMS_STRICT=1`，用于在顺序/sidebar 不一致时让 `docs:llms` 失败。
- 文档 workflow：当仓库为私有时跳过 GitHub Pages 部署。
- 文档 workflow：部署文档时自动启用 GitHub Pages（GitHub Actions）。
- 发布 workflow：在 tag 发布时执行 crates.io 发布（未配置 token 时自动跳过）。
- 发布 workflow：避免在 `if:` 中引用 `secrets.*`，以规避 workflow 校验问题。
- 发布 workflow：对 CLI 的 `cargo publish` 增加重试，以容忍 crates.io index 的同步延迟。
- 仓库链接：更新 GitHub owner / Pages 地址（`omne42`）。
- 元数据：license 仅使用 MIT（不再是双协议）。
- 扫描预算：`maxFiles` 达到上限后会停止扫描（`skippedBudgetMaxFiles` 会变为非 0）。
- Node 安装：全局安装时 `postinstall` 会构建 Rust 二进制；作为工程依赖安装时默认在首次运行时构建（或设置 `DUP_CODE_CHECK_BUILD_ON_INSTALL=1`）。
- 扫描流程：Git 快路径对 `git ls-files` 做流式遍历（避免收集完整列表；`maxFiles` 可提前停止）。
- CLI：澄清 `--strict` 语义（仅在“致命跳过”：权限/遍历错误/预算中断/bucket 截断时返回非 0），并增加 smoke 覆盖。
- CLI：当出现“致命跳过”时，在 stderr 输出一次警告（即使启用了 `--stats`；仅在需要时提示重新运行 `--stats`）。
- Rust：通过共享内部 helper 去重 code-span（winnowing）与 file-duplicates 分组逻辑，避免漂移。
- Core：收紧 `DUP_CODE_CHECK_GIT_BIN` 覆盖校验（仅允许绝对路径；且要求文件存在）。
- Core：进一步收紧 `DUP_CODE_CHECK_GIT_BIN` 覆盖校验（不允许 symlink；Unix 下必须可执行且不可被其他用户写入）。
- Core：只有在 `DUP_CODE_CHECK_ALLOW_CUSTOM_GIT=1` 时才会启用 `DUP_CODE_CHECK_GIT_BIN`（显式 opt-in）。
- Report：默认设置 `maxTotalBytes` 预算（256 MiB）以限制内存占用；可用 `--max-total-bytes` 覆盖。
- 文档：在 `--help` 与 README 中说明 `--report` 模式默认 `--max-total-bytes` 预算。
- CLI：root 路径使用 `canonicalize()`（失败则报错），降低符号链接歧义。
- 文档：补充安全提示——npm `postinstall` 会触发原生构建（Cargo），并可能运行依赖的 build script。
- 文档：在《快速开始》中补齐同样的 `postinstall` 安全提示。
- 扫描统计：新增 `skippedBucketTruncated`，用于标记检测器 fingerprint bucket 被截断（防爆保护）。
- Core：重复文件分组不再保存完整归一化样本，显著降低大仓库内存占用。
- Core：将 scan 模块拆分为更小的文件（无行为变化）。
- CLI：将 `skippedBucketTruncated` 视为“扫描不完整”（致命跳过），从而影响 warning/`--strict` 退出码。
- CLI：`--strict` 现在会把 `outside_root` 视为“扫描不完整”（遍历跳过），从而退出非 0。
- 扫描：Windows 下 `ignoreDirs` 按 ASCII 做大小写不敏感匹配。
- Tokenizer：仅在行首（允许前置空白）把 `#` 视为注释。
- CI：docs build 默认启用 `LLMS_STRICT=1`。
- CI：固定 Rust toolchain 为 `1.92.0`（与 `rust-toolchain.toml` 对齐）。
- 扫描：Git 流式枚举失败时回退到 walker（避免提前中止/重复扫描）。
- CLI：root 路径直接使用 `canonicalize()` 解析（保留 symlink 语义）。
- Report：不再为 preview 保存全量文本，按需从文件生成 preview，降低内存占用。
- Rust：重构 winnowing 检测器 API，把参数收敛为一个 struct（无行为变化）。
- 归一化：code-span 与 line-span 检测器现在仅保留 ASCII 的“词字符”（`[A-Za-z0-9_]`），与文档一致。
- Report：在 `--report` 扫描/分词过程中避免额外的 `String` 分配，降低内存与拷贝开销。
- 扫描：保持 `RepoFile` 更轻量，避免为每个候选文件 clone repo root/label（减少分配）。
- 扫描：Git 快路径移除冗余的 `git check-ignore` 步骤（开销更低；结果不变）。
- 扫描统计：新增 `gitFastPathFallbacks`，用于标记 Git 快路径回退到 walker 的次数。
- Report：相似块检测器的 `splitmix64` helper 去重（无行为变化）。
- CLI：JSON 输出 scanStats 时避免 clone（`--json --stats`）。

### Fixed
- 扫描时容忍 `NotFound`（例如扫描过程中文件被删除）。
- 当前缀剥离失败时避免在结果里泄漏绝对路径。
- 避免 winnowing 在异常空窗口情况下可能触发的 panic。
- `--follow-symlinks` 现在通过使用 walker path 更可靠。
- 启用 `--follow-symlinks` 时，跳过解析后位于 root 之外的符号链接目录。
- 启用 `--follow-symlinks` 时，读取文件增加针对 symlink/TOCTOU 竞态的防护。
- token 检测器现在会记录多行字符串 token 的起始行号。
- CLI 支持 `--` 终止参数解析（允许 root 以 `-` 开头）。
- CLI：当同时指定 `--report` 与 `--code-spans` 时将报错。
- CLI：`--cross-repo-only` 现在要求至少 2 个 root，否则会报错。
- 扫描会跳过 `PermissionDenied` 和 walker traversal errors，而不是直接中止。
- 扫描现在会把单文件读取 I/O 错误计为跳过，而不是中止整个扫描。
- CLI 现在会捕获运行期扫描失败并以退出码 1 退出。
- 移除 unstable rustfmt 配置，避免 stable toolchain 警告。
- `--max-report-items` 现在在所有报告 section 中一致生效，并优先保留更大的 group。
- CLI：`--max-files` 现在会拒绝超过 `usize` 上限的值，而不是截断。
- CLI 支持 `--no-gitignore` 关闭 `.gitignore` 过滤。
- 当前缀剥离失败时，输出使用 `<external:...>/name`，以避免泄漏绝对路径同时保持可区分性。
- `git ls-files` 输出中若出现不安全的相对路径（绝对路径、`..` 等），将跳过而不是尝试读取。
- 扫描预算：启用 `maxFiles` / `maxTotalBytes` 时仍保持 Git 快路径（加速 CI 扫描）。
- npm 安装：通过 Node wrapper 启动平台二进制，使 Windows 上的 `dup-code-check` 可用。
- 文档：补充 `maxFiles` 停止行为与 `skippedBudgetMaxFiles` 字段语义。
- CLI：`--version` 现在会尊重 `--`（`--` 之后的 `--version` 会被当作 root 而不是参数）。
- 扫描流程：流式模式下 `git ls-files` 非 0 退出时回退到 walker（fail closed）。
- 扫描流程：流式模式遇到 `git ls-files` 输出非 UTF-8 路径时回退到 walker（包括扫描已开始的情况）。
- npm 构建：当 `bin/dup-code-check.mjs` wrapper 缺失时，提前失败并输出更清晰的错误信息。
- npm 构建：Cargo 构建失败时输出更友好的诊断信息。
- npm 构建：查找 `cargo` 时会排除 `node_modules/.bin`（供应链加固）。
- npm 构建：Unix 下避免执行 PATH 中不可执行或 world-writable 的 `cargo` 二进制（加固）。
- Node smoke：在决定是否需要重建时额外验证 wrapper 可执行 `--version`。
- Node smoke：探测源码 mtime 时不再跟随 symlink 目录，避免异常 worktree 下递归/循环。
- npm 包：包含 `rust-toolchain.toml`，使安装时使用固定的 Rust toolchain。
- CLI：本地化 `Number.MAX_SAFE_INTEGER` 相关整数参数错误信息。
- CLI：致命跳过 warning 会输出原因摘要与可操作建议（未启用 `--stats` 时会提示使用 `--stats` 查看完整统计）。
- CLI：致命跳过 warning 现在会包含与 JSON `scanStats` 对齐的 key（camelCase），并附带 snake_case 别名。
- Core：为降低哈希碰撞导致的重复文件误分组风险，fingerprint 额外加入 prefix/suffix 样本。
- Core：在输出重复文件组前会重新校验去空白后的文件内容，避免哈希碰撞或文件变化导致误报。
- 扫描预算：二进制文件不会再绕过 `maxFiles` / `maxTotalBytes`，且二进制检测会避免读完整文件。
- 扫描预算：在读取过程中也会强制 `maxTotalBytes` / `maxFileSize`，避免扫描期间文件增长导致预算超出。
- Report：截断包含非 ASCII 字符的 preview 时不再 panic。
- 测试：当运行环境下 `chmod 000` 无法触发 `PermissionDenied`（例如 root）时，`PermissionDenied` 扫描测试会跳过，避免误报失败。
