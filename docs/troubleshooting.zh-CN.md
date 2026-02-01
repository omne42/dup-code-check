# 排障

[English](troubleshooting.md)

本页覆盖常见构建/运行问题，以及如何快速定位原因。

## 1) Rust toolchain 缺失 / `cargo` 找不到

现象：

- 安装依赖或运行 `npm run build` 时提示需要 Rust toolchain
- 报错包含 `ENOENT` / `cargo: not found`

处理：

1. 安装 rustup：`https://rustup.rs`
2. 安装并使用仓库固定的工具链：

```bash
rustup toolchain install 1.92.0
rustup override set 1.92.0
```

然后重新执行：

```bash
npm run build
```

## 2) 二进制不存在 / 无法执行

现象：

- 运行 `dup-code-check` 提示 “command not found”
- 或运行 `./bin/dup-code-check` 报 “No such file or directory”

原因：二进制尚未构建（或放置位置不对）。

处理：

```bash
npm run build
```

或直接用 Rust 构建：

```bash
cargo build --release -p dup-code-check
```

并确认存在 `bin/dup-code-check`（Windows 为 `bin/dup-code-check.exe`）。

## 3) CLI 参数报错（退出码 2）

现象：命令行提示 `Unknown option` 或 `--xxx must be an integer ...`。

原因：CLI 对部分参数做了严格校验（例如整数参数拒绝 `1.5`）。

处理：

- 用 `--help` 查看正确用法
- 对整数参数传入整数（字节数、条数等）

## 4) 扫描不完整导致 CI 失败（`--strict`）

现象：退出码为 `1`，并且 stderr 打印了 scan stats，包含：

- `permission_denied`
- `walk_errors`
- `budget_max_files` / `budget_max_total_bytes`

处理建议：

- 权限问题：调整扫描 root（避免扫系统目录/受限目录），或在 CI 中提升权限
- 遍历错误：确认文件系统稳定性（容器挂载、并发写入等）
- 预算中断：增大 `--max-files` / `--max-total-bytes`，或缩小 root/加 `--ignore-dir`

## 5) `.gitignore` 行为与预期不一致

默认会尊重 `.gitignore`。如果你希望完全扫描（包括被忽略的文件），使用：

```bash
dup-code-check --no-gitignore .
```

## 6) Windows 构建问题

如果你在 Windows 上从源码构建失败，通常需要：

- 安装 Visual Studio Build Tools（包含 C/C++ 工具链）
- 确保 Rust toolchain 与 Node 版本满足要求

由于环境差异较大，建议优先在 CI 中用容器/固定镜像构建，或使用 WSL。
