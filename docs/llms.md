# LLM Bundle (`llms.txt`)

[中文](llms.zh-CN.md)

This docs site publishes a plain-text bundle of the documentation (inspired by modern docs sites like the AI SDK).

## Files

- `/llms.txt`: combined (EN + 中文)
- `/llms.en.txt`: English only
- `/llms.zh-CN.txt`: 中文 only

These files are generated automatically during `npm run docs:serve` / `npm run docs:build`.

## Example usage

1. Open one of the files above and copy its contents.
2. Use a prompt like:

```text
Documentation:
{paste llms.txt here}
---
Based on the above documentation, answer the following:
{your question}
```

## Build locally

```bash
npm install --ignore-scripts
npm run docs:serve
```
