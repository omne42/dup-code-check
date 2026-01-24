# code-checker

面向“代码检测”的工具箱：底层 Rust，高层 Node.js（可做 CLI / npm 包）。

## 当前功能（MVP）

- 重复文件检测（忽略空白字符差异：换行 / tab / 空格等）
  - 单仓库：扫描一个 root
  - 多仓库：扫描多个 root（可只输出跨仓库重复）

## 安装（作为 npm 包依赖）

当前版本会在 `postinstall` 阶段从 Rust 源码编译原生模块，因此需要：

- Node.js `>=22`（参考 Codex 项目）
- Rust toolchain `1.92.0`（已通过 `rust-toolchain.toml` 固定，参考 Codex 项目）

## 本地开发

### 1) 构建原生模块

```bash
npm run build
```

生成 `code_checker.node`（N-API 动态库）。

### 2) 运行 CLI

```bash
node bin/code-checker.js --help
node bin/code-checker.js .
node bin/code-checker.js --cross-repo-only /path/to/repoA /path/to/repoB
```

### 3) 运行测试

```bash
cargo test
npm test
```

## 说明

当前的“重复”定义是：对文件内容做 ASCII whitespace 删除后完全一致（Type-1 clone 的一种极简形式）。后续可以扩展为 token/AST 级别的克隆检测，支持 Type-2/Type-3。
