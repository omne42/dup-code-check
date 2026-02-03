# 安装与构建

[English](installation.md)

`dup-code-check` 是一个 Rust 二进制程序。Node.js 仅作为一种安装方式（npm）。npm 包会在需要时从源码编译二进制（Cargo），因此需要 Rust 工具链。

默认情况下，作为工程依赖安装会在**首次运行**时编译；全局安装为了避免权限问题，可能会在 `postinstall` 阶段编译。

> 安全提示：编译过程会触发原生构建（Cargo），并可能运行依赖的 build script。根据安装方式不同，这一步可能发生在 `postinstall`（例如全局安装）或首次运行时。如果你需要避免执行安装脚本，请使用 `--ignore-scripts` / `npm_config_ignore_scripts=true`。

## 方式 A：直接使用 Rust（推荐）

在仓库根目录：

```bash
cargo build --release -p dup-code-check
./target/release/dup-code-check --help
```

或安装到本机：

```bash
cargo install --path . --bin dup-code-check
dup-code-check --help
```

## 方式 B：作为 npm 依赖（工程接入）

```bash
npm i -D dup-code-check
```

安装完成后：

```bash
npx dup-code-check --help
```

> 提示：如果你希望在安装阶段就构建（工程依赖），可设置 `DUP_CODE_CHECK_BUILD_ON_INSTALL=1`。
>
> 提示：如果你想禁用构建（安装 + 首次运行），可设置 `DUP_CODE_CHECK_SKIP_BUILD=1`，之后再手动构建。

## 方式 C：全局安装（适合本机工具）

```bash
npm i -g dup-code-check
dup-code-check --help
```

> 提示：全局安装建议保持 npm scripts 开启（`postinstall` 会构建二进制）。

## 方式 D：从源码开发/贡献（推荐用于改代码）

```bash
git clone <repo>
cd dup-code-check
npm install
npm run build
./bin/dup-code-check --help
```

## `npm run build` 到底做了什么？

`npm run build` 会执行 `scripts/build-binary.mjs`，核心步骤是：

1. `cargo build --release -p dup-code-check`
2. 将产物复制到 `bin/` 下生成可执行文件

不同平台产物文件名不同：

- macOS / Linux：`bin/dup-code-check`
- Windows：`bin/dup-code-check.exe`

## 常见构建问题

如果你遇到 “Rust toolchain is required…”，先看《[排障](troubleshooting.zh-CN.md)》。
