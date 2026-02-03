# 面向 LLM 的文档包（`llms.txt`）

[English](llms.md)

本站会提供一份“纯文本的文档合集”（参考一些现代文档站点的做法，比如 AI SDK 的 `llms.txt`），用于离线阅读或作为 LLM 上下文注入。

## 文件列表

- `/llms.txt`：合集（EN + 中文）
- `/llms.en.txt`：仅英文
- `/llms.zh-CN.txt`：仅中文

这些文件会在执行 `npm run docs:serve` / `npm run docs:build` 时自动生成。

## 用法示例

1. 打开上述文件之一并复制内容
2. 使用类似下面的提示词：

```text
Documentation:
{粘贴 llms.txt 内容到这里}
---
基于上述文档，回答下面的问题：
{你的问题}
```

## 本地预览文档

```bash
npm install --ignore-scripts
npm run docs:serve
```
