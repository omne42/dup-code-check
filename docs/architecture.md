# 架构速览

## 分层

- `crates/core`：纯 Rust 核心逻辑（扫描文件、归一化、计算指纹、输出结果）
- `crates/napi`：N-API 绑定层（把 Rust API 暴露给 Node）
- `index.js` / `bin/code-checker.js`：Node 侧加载原生模块并提供 CLI

## 构建方式（本地）

- `npm run build`：
  - `cargo build --release -p code-checker`
  - 将产物复制为仓库根目录的 `code_checker.node`

## 扩展思路

- 在 `crates/core` 增加新的检测能力（例如：片段级克隆检测、规则扫描、AST 分析等）
- 在 `crates/napi` 增加对应的导出函数与参数类型
- 在 `bin/code-checker.js` 增加子命令或参数，形成统一的 CLI 入口

