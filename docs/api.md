# Node.js API

`code-checker` 也可以作为 npm 包被其他 Node.js 工程调用。它导出的 API 与 CLI 是同一套能力。

## 导出函数

在 Node.js 中：

```js
const {
  findDuplicateFiles,
  findDuplicateFilesWithStats,
  findDuplicateCodeSpans,
  findDuplicateCodeSpansWithStats,
  generateDuplicationReport,
  generateDuplicationReportWithStats
} = require('code-checker');
```

所有函数都是**同步调用**（会阻塞事件循环直到扫描完成），适合在 CLI/CI 工具、脚本中使用。

## 基本示例

### 重复文件

```js
const { findDuplicateFiles } = require('code-checker');

const groups = findDuplicateFiles(['/repoA', '/repoB'], { crossRepoOnly: true });
console.log(groups);
```

### 疑似重复代码片段（行号范围）

```js
const { findDuplicateCodeSpans } = require('code-checker');

const groups = findDuplicateCodeSpans(['/repoA', '/repoB'], {
  crossRepoOnly: true,
  minMatchLen: 80
});
console.log(groups);
```

### 报告模式（多检测器一次输出）

```js
const { generateDuplicationReport } = require('code-checker');

const report = generateDuplicationReport(['/repoA', '/repoB'], {
  crossRepoOnly: true,
  maxReportItems: 200
});
console.log(report.tokenSpanDuplicates);
```

## 选项类型（ScanOptions）

TypeScript 类型定义位于 `index.d.ts`，核心字段包括：

- `ignoreDirs?: string[]`
- `maxFileSize?: number`
- `maxFiles?: number`
- `maxTotalBytes?: number`
- `minMatchLen?: number`
- `minTokenLen?: number`
- `similarityThreshold?: number`
- `simhashMaxDistance?: number`
- `maxReportItems?: number`
- `respectGitignore?: boolean`
- `crossRepoOnly?: boolean`
- `followSymlinks?: boolean`

每个字段的语义/默认值以及与 CLI 参数的对应关系见《[扫描选项](scan-options.md)》。

## WithStats 版本

带 `WithStats` 的函数会返回：

- 结果本体（`groups` 或 `report`）
- `scanStats`（扫描统计，便于在 CI 里判断“扫描是否完整”或做性能观测）

例如：

```js
const { findDuplicateFilesWithStats } = require('code-checker');

const { groups, scanStats } = findDuplicateFilesWithStats(['/repoA'], { maxFiles: 1000 });
console.log(scanStats);
```

## 参数校验与错误

- `roots` 不能为空
- 数值选项会拒绝 `NaN`、小数、越界值（例如 `minMatchLen: 1.5`）
- 发生错误时会抛出异常（可在上层捕获并处理）

