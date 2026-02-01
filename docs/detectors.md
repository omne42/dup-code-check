# 检测器与算法

本页解释 `--report` 里每个 section 在做什么、适合发现什么类型的重复，以及主要的实现思路/局限性。

> 术语提示：常见“克隆类型（Clone Types）”分为 Type-1/2/3/4。它们不是二选一的标准，而是对“相似程度/归一化能力”的一种分类方式。

## 0) 扫描与归一化：所有检测器的共同前提

无论哪种检测器，`code-checker` 都会先做两件事：

1. 收集要扫描的文件路径（默认尊重 `.gitignore`，并忽略常见目录如 `node_modules/`）
2. 读取文件内容并跳过：
   - 超过 `maxFileSize` 的文件
   - 包含 `\\0` 字节的二进制文件
   - 扫描过程中的 `NotFound` / `PermissionDenied` 等异常（会计入 `scanStats`）

详细选项见《[扫描选项](scan-options.md)》。

## 1) fileDuplicates：重复文件（whitespace-insensitive）

### 目标

发现“内容完全一致，但可能仅有空白字符差异”的重复文件。

### 核心思路

- 对文件内容做 *ASCII whitespace* 删除（空格/换行/tab 等）
- 对归一化后的字节序列求指纹并分组
- 同组内再做一次 sample 对比，避免哈希碰撞

### 适用与局限

- 适合：复制粘贴后仅改了缩进/格式的重复文件（Type-1 的一个子集）
- 不适合：变量重命名、插入/删除少量语句（更接近 Type-2/3）

## 2) codeSpanDuplicates：疑似重复代码片段（字符级）

> 对应 CLI 的 `--code-spans`（报告模式下也会包含）。

### 目标

快速发现“疑似重复的代码片段”，并能给出**行号范围**以便人工 review。

### 归一化规则

对文本做字符级归一化：

- 删除换行
- 丢弃所有“符号 + 空白字符”
- 仅保留：字母/数字/下划线（`[A-Za-z0-9_]`）

因此它对“格式/符号差异”更不敏感，但仍然会受“标识符重命名”影响。

### 匹配思路（概念级）

- 在归一化后的字符序列上做指纹（fingerprint）与窗口选择（winnowing）
- 基于候选指纹位置做扩展匹配（maximal match）
- 去重、分组并输出 occurrence（带行号范围）

### 适用与局限

- 适合：跨文件/跨仓库快速定位大段复制粘贴、改格式/改符号的重复片段
- 局限：
  - 不是 AST/token 克隆检测；误报/漏报都可能存在
  - 仅保留 `[A-Za-z0-9_]` 的策略对“语言无关”友好，但会损失语义信息

## 3) lineSpanDuplicates：按行归一化的重复片段

### 目标

发现“多行连续重复”的片段，且对缩进/标点不敏感。

### 归一化规则（每行）

- 每一行仅保留字母/数字/下划线
- 对该序列求 hash 作为“行 token”

检测时在“行 token 序列”上找重复窗口，并用“归一化字符数总和 >= minMatchLen”过滤掉太短的片段。

### 适用与局限

- 适合：成段重复、但每行存在格式/标点差异的情况
- 局限：以“行”为单位，跨行重排/插入/删除对它影响较大

## 4) tokenSpanDuplicates：token 级重复片段

### 目标

更接近 CPD/clone detector 的“token 序列重复”：对空白不敏感，并对一部分 Type-2（如标识符重命名）更稳健。

### Token 化（实现层面的简化规则）

这是一个轻量 tokenizer（不是完整语言解析器），大体规则：

- 关键字（如 `if/for/return/let/class/...`）映射到固定 token
- 标识符统一为 `IDENT`
- 数字统一为 `NUM`
- 字符串统一为 `STR`，并记录字符串起始行号
- 标点符号按字符区分（`{}`, `()`, `;` 等）

然后在 token 序列上使用与 code spans 类似的指纹/窗口策略寻找重复片段。

### 适用与局限

- 适合：同构逻辑但变量名不同的重复（更像 Type-2）
- 局限：
  - tokenizer 是启发式的，多语言通吃但并不“语法正确”
  - 对 Python 等不使用 `{}` 的语言，后续 block/AST 子树相关检测会更弱

## 5) blockDuplicates：`{}` block 级重复

### 目标

发现“花括号块”的完全重复（在 token 层面）。

### 核心思路

- 用 tokenizer 得到 token 序列
- 用 `{` / `}` 构建 block 节点（包含起止 token/行号、层级、子节点）
- 对每个 block 内部 token 切片求 hash 并分组

## 6) astSubtreeDuplicates：基于 `{}` 结构的“AST 子树”近似重复

### 目标

在“块结构”层面更稳健地检测重复：同一块的结构与内容相同，则认为重复。

### 核心思路（简化）

为每个 block 构建一个表示（repr）：

- 子 block 不直接展开 token，而是插入一个 marker + 子 block 的 hash
- block 自身除子块外的 token 会被保留

这相当于用 `{}` 结构构建了一棵树，并做“自底向上”的子树指纹。

### 局限

它不是语言真实 AST，只是 `{}` 结构的近似，因此称为“AST 子树（近似）”。

## 7) similarBlocksMinhash / similarBlocksSimhash：相似块对（近似）

### 目标

发现“不是完全相同，但非常相似”的块对（Type-3 方向的启发式）。

### 输入

- 来自 `{}` block
- 只取较浅层级（实现里对 `depth` 有限制）以控制规模
- 对 block 内 token 进行 shingle（默认 5-gram）

### MinHash（similarBlocksMinhash）

对每个 block 构建 MinHash signature（固定大小），用 LSH（分 band 分桶）生成候选对，再按：

- `score >= similarityThreshold`
- `crossRepoOnly`（如启用）

过滤。

### SimHash（similarBlocksSimhash）

对每个 block 构建 64-bit SimHash，按 band 分桶生成候选对，再按：

- `hamming_distance <= simhashMaxDistance`
- `crossRepoOnly`（如启用）

过滤；输出里会携带 `distance` 字段。

### 适用与局限

- 适合：小范围编辑/插入/删除导致的近似重复提示
- 局限：近似算法存在误报；建议配合 `preview` 与行号人工确认

## 8) 怎样选择检测器？

一个实用的选择顺序：

1. 先跑 `fileDuplicates`（成本低、信号强）
2. 再跑 `codeSpanDuplicates`（快速定位复制粘贴片段）
3. 若要更 Type-2/3：用 `--report` 看 token/block/相似块对

在 CI 中建议：

- 用 `--cross-repo-only` 聚焦“跨 root 复用/复制”的问题
- 用 `--max-report-items` 控制输出规模
- 用 `--strict` 确保扫描完整性（配合 `--stats`）

