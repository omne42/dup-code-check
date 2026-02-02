---
layout: home

hero:
  name: dup-code-check
  text: 重复/相似检测工具箱
  tagline: 一个面向 CI 的 Rust CLI，用于检测重复文件与疑似重复代码片段（带行号范围）。
  actions:
    - theme: brand
      text: 快速开始
      link: /getting-started.zh-CN
    - theme: alt
      text: CLI 参数
      link: /cli.zh-CN
    - theme: alt
      text: GitHub
      link: https://github.com/omne42/dup-code-check

features:
  - title: 重复文件检测
    details: 对文件内容做空白字符归一化后，检测完全一致的重复文件（支持多 root）。
  - title: 代码片段重复（带行号）
    details: 输出疑似重复代码片段的起止行号范围，便于定位与重构。
  - title: 报告模式
    details: 一次扫描输出多种粒度的重复/相似结果，便于 CI 汇总。
  - title: CI 预算控制
    details: 通过 maxFiles/maxTotalBytes 控制成本，并用 strict 模式显式感知不完整扫描。
---

## 介绍

建议从《[介绍](/introduction.zh-CN)》开始了解 `dup-code-check` 的能力、适用场景与基本用法。

## 面向 LLM 的 `llms.txt`

本站会在 `/llms.txt` 提供一份纯文本的文档合集，方便离线阅读/LLM 上下文注入。

