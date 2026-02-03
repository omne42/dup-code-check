# dup-code-check

[English](README.md)

面向“重复/相似检测”的工具箱：以 **Rust 二进制程序** 交付；Node.js 仅作为一种安装方式（npm）。

## 文档

在线文档（VitePress）：

- https://omne42.github.io/dup-code-check/

文档源文件在仓库 `docs/` 目录（npm 包不包含 `docs/`）。

本地预览（需 clone 仓库）：

```bash
npm install --ignore-scripts
npm run docs:serve
```

## 当前功能（MVP）

- 重复文件检测（忽略空白字符差异：换行 / tab / 空格等）
  - 单仓库：扫描一个 root
  - 多仓库：扫描多个 root（可只输出跨仓库重复）
- 疑似重复代码片段检测
  - 对文件内容去除“符号 + 空白字符”（仅保留字母/数字/下划线）
  - 连续 `>= 50` 个字符相同视为疑似重复，报告真实行号范围
- 重复检测报告（`--report`）
  - 行级 / token 级 / block 级 / “AST 子树”级（基于 `{}` 结构）重复
  - 近似重复（MinHash / SimHash）
- 扫描默认会跳过 `.gitignore` 命中的文件

## 安装

### 方式 A：直接使用 Rust 二进制（推荐）

在仓库根目录：

```bash
cargo build --release -p dup-code-check
./target/release/dup-code-check --help
```

或者安装到本机（开发期常用）：

```bash
cargo install --path . --bin dup-code-check
dup-code-check --help
```

### 方式 B：通过 npm 安装（二进制从源码编译）

当前版本会在 `postinstall` 阶段从 Rust 源码编译二进制，因此需要：

- Node.js `>=22`（参考 Codex 项目）
- Rust toolchain `1.92.0`（已通过 `rust-toolchain.toml` 固定，参考 Codex 项目）

安全提示：npm 安装会执行 `postinstall`，会触发原生构建（Cargo），并可能运行依赖的 build script。若需要避免执行安装脚本，请使用 `npm_config_ignore_scripts=true`。

如果你希望避免在安装时执行脚本，可以用 `npm_config_ignore_scripts=true` 安装后手动构建：

```bash
npm_config_ignore_scripts=true npm i -D dup-code-check
npm run build
```

## 本地开发

### 1) 构建二进制

```bash
npm run build
```

生成 `bin/dup-code-check`。

### 2) 运行 CLI

```bash
./bin/dup-code-check --help
./bin/dup-code-check .
./bin/dup-code-check --cross-repo-only /path/to/repoA /path/to/repoB
./bin/dup-code-check --code-spans --cross-repo-only /path/to/repoA /path/to/repoB
./bin/dup-code-check --report --cross-repo-only /path/to/repoA /path/to/repoB
./bin/dup-code-check --max-file-size 20971520 .
```

### 3) 运行测试

```bash
cargo test
npm test
```

## 说明

当前包含两类检测：

- 重复文件：对文件内容做 ASCII whitespace 删除后完全一致（Type-1 clone 的一种极简形式）
- 代码片段：对内容去除“符号 + 空白字符”后，存在连续 `>= 50` 字符相同的片段，输出行号范围

默认会跳过大于 10 MiB（10485760 bytes）的文件；可用 `--max-file-size` 调整（Rust 侧常量：`DEFAULT_MAX_FILE_SIZE_BYTES`）。

后续可以扩展为 token/AST 级别的克隆检测，支持 Type-2/Type-3。
