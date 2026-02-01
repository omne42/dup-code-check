# 贡献指南

[English](contributing.md)

欢迎贡献！本页描述最小的协作约定，保证改动可维护、可回溯、可验证。

## 开发准备

```bash
./scripts/bootstrap.sh
```

它会：

- 初始化 git（如果不是仓库）
- 配置 git hooks（`core.hooksPath=githooks`）
- 安装 Node 依赖（`npm install`）

## 改动前建议

1. 先跑一次 gate，确保本地环境正确：

```bash
./scripts/gate.sh
```

2. 明确你在改哪一层：

- Rust 核心能力 → `crates/core`
- CLI 体验 → `crates/cli`

## Commit 约定

### 分支名

允许的分支前缀示例：

- `feat/...`, `fix/...`, `docs/...`, `refactor/...`, `perf/...`, `test/...`, `chore/...`, `build/...`, `ci/...`, `revert/...`

### Commit message

要求 Conventional Commits（示例）：

- `feat(core): add new detector`
- `fix(cli): handle invalid option`
- `docs(readme): add usage examples`

## CHANGELOG 规则

`pre-commit` 会强制：

- 每个 commit 必须更新 `CHANGELOG.md` 的 `[Unreleased]` 部分
- 禁止修改已发布版本的 changelog section（除非显式设置环境变量）

## 提交前检查

建议在提交前执行：

```bash
./scripts/gate.sh
```

或至少：

```bash
cargo test
npm test
```
