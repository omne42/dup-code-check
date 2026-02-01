# FAQ

## Q: `dup-code-check` 支持哪些语言？

当前检测器主要基于“文本/字符/token/花括号块”启发式，因此对语言没有硬依赖：

- 重复文件：任何文本都可以
- code spans / line spans：任何文本都可以（但更偏代码形态）
- token/block/“AST 子树”：对 `{}` 结构明显的语言更有效（JS/TS/Java/C/C++/Rust 等）

## Q: 这是一个完整的克隆检测器吗？覆盖 Type-2/3 吗？

它更像一个“可工程落地的工具箱”，覆盖从强信号到启发式的多层检测：

- file duplicates：强信号、低成本
- token spans / blocks：对一部分 Type-2 更稳健
- MinHash/SimHash：提供 Type-3 方向的“相似提示”

但它不是学术意义上的完整 clone detector，也不会做语言级 AST 解析（当前是 `{}` 结构近似）。

## Q: 为什么选择 Rust + Node.js？

- Rust：扫描/归一化/指纹计算在性能和内存上更可控
- Node.js：CLI 交付与生态集成更方便（在前端/全栈工程中落地成本低）

## Q: 为什么安装时需要 Rust？能不能不编译？

当前版本会在安装阶段从源码构建原生模块，因此需要 Rust。

未来如果引入预编译产物（prebuild），可以将“安装依赖 Rust”变成可选。

## Q: 结果里的 `hash` 是什么？会冲突吗？

`hash` 是对归一化内容的 64-bit 指纹（以 16 位 hex 字符串输出）。

实现会在同一 hash bucket 内做 sample 对比以降低碰撞风险，但理论上仍可能存在极低概率的冲突。若你的场景对零碰撞有强需求，建议把结果当作候选，再做二次验证。
