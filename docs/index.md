---
layout: home

hero:
  name: dup-code-check
  text: Duplicate detection toolbox
  tagline: A Rust CLI for finding duplicate files and suspected duplicate code spans (built for CI).
  actions:
    - theme: brand
      text: Get started
      link: /getting-started
    - theme: alt
      text: CLI options
      link: /cli
    - theme: alt
      text: GitHub
      link: https://github.com/omne42/dup-code-check

features:
  - title: Fast file duplicates
    details: Detect duplicate files (whitespace-insensitive) within a repo or across multiple roots.
  - title: Code span duplicates with line ranges
    details: Find suspected duplicate code spans and report start/end line numbers for each occurrence.
  - title: Report mode
    details: Run multiple detectors in one pass and output a single structured report.
  - title: CI-friendly budgets
    details: Bound cost with maxFiles/maxTotalBytes and surface incomplete scans with strict mode.
---

## Introduction

Start with the [Introduction](/introduction) to understand what `dup-code-check` does and how to use it.

## LLM bundle (`llms.txt`)

This docs site publishes a plain-text docs bundle, similar to modern docs sites (e.g. the AI SDK):

- `/llms.txt` (EN + 中文)
- `/llms.en.txt` (English only)
- `/llms.zh-CN.txt` (中文 only)

See [LLM Bundle](/llms) for usage examples.
