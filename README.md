# code-checker

面向“代码检测”的工具箱：底层 Rust，高层 Node.js（可做 CLI / npm 包）。

## 文档

更完整的文档（GitBook 风格目录）在：

- `docs/README.md`
- `docs/SUMMARY.md`

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
node bin/code-checker.js --code-spans --cross-repo-only /path/to/repoA /path/to/repoB
node bin/code-checker.js --report --cross-repo-only /path/to/repoA /path/to/repoB
node bin/code-checker.js --max-file-size 20971520 .
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
