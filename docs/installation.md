# 安装与构建

`code-checker` 通过 Node.js 分发，但核心能力来自 Rust 原生模块（N-API）。**当前版本会在安装阶段从 Rust 源码编译原生模块**，因此需要 Rust 工具链。

## 方式 A：作为 npm 依赖（推荐用于工程接入）

```bash
npm i -D code-checker
```

安装完成后：

```bash
npx code-checker --help
```

> 提示：如果你的环境禁用了 npm scripts（例如 `npm_config_ignore_scripts=true`），`postinstall` 不会构建原生模块；你需要在项目中手动执行 `npm run build`（在 `node_modules/code-checker/` 目录下）或改用源码方式。

## 方式 B：全局安装（适合本机工具）

```bash
npm i -g code-checker
code-checker --help
```

## 方式 C：从源码开发/贡献（推荐用于改代码）

```bash
git clone <repo>
cd code-checker
npm install
npm run build
node bin/code-checker.js --help
```

## 构建原生模块到底做了什么？

`npm run build` 会执行 `scripts/build-native.mjs`，核心步骤是：

1. `cargo build --release -p code-checker`
2. 将产物复制到仓库根目录生成 `code_checker.node`

不同平台产物文件名不同：

- macOS：`libcode_checker.dylib`
- Linux：`libcode_checker.so`
- Windows：`code_checker.dll`

## 常见构建问题

如果你遇到 “Rust toolchain is required…” 或 “Native binding not found”，先看《[排障](troubleshooting.md)》。

