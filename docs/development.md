# 开发指南

本页面向想修改 `dup-code-check` 的贡献者：如何构建、测试、理解目录结构。

## 目录结构

- `crates/core`：Rust 核心（扫描、归一化、检测器、report 生成）
- `crates/cli`：Rust CLI（二进制入口）
- `bin/`：构建后的二进制输出目录（`bin/dup-code-check`）
- `scripts/`：构建/验证脚本

## 构建

```bash
npm install
npm run build
```

构建产物会写到 `bin/dup-code-check`。

## 运行

```bash
./bin/dup-code-check --help
./bin/dup-code-check .
```

## 测试与验证

### Rust 测试

```bash
cargo test
```

### CLI smoke

```bash
npm test
```

`npm test` 会运行 `scripts/smoke.mjs`，它会在临时目录构造一些小文件来验证：

- 重复文件扫描基本正确
- `--max-file-size` 等参数校验正确
- `--` 终止参数解析可用
- `.gitignore` 默认生效，`--no-gitignore` 可关闭

### 统一 gate（本地/钩子使用）

```bash
./scripts/gate.sh
```

它会按项目标记自动执行：

- `cargo fmt --check`
- `cargo check`
- `npm run check`

## Git hooks（可选但推荐）

```bash
./scripts/bootstrap.sh
```

会配置 `core.hooksPath=githooks`，并启用：

- `pre-commit`：要求每个 commit 同步更新 `CHANGELOG.md`，并运行 gate
- `commit-msg`：强制 Conventional Commits 格式 + 分支名前缀策略
