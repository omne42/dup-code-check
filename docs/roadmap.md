# Roadmap

[中文](roadmap.zh-CN.md)

This page records directional ideas to align priorities in open-source collaboration (not a promise).

## Short term (stability & usability)

- richer output formats (e.g. SARIF for quality/security platforms)
- a more friendly report JSON schema (versioning + backward compatibility policy)
- stronger ignore rules (e.g. `.dup-code-checkignore` or similar)
- finer-grained CLI subcommands (run one detector / output one section)

## Mid term (capability expansion)

- better Type-2/Type-3 detection: more robust tokenization, windowing, denoising
- incremental scanning: cache file/block fingerprints and scan only changes
- finer granularity: function/class-level duplicate localization

## Long term (deep analysis)

- real language AST (Tree-sitter, etc.) and structured clone detection
- large-scale indexing/retrieval (SourcererCC-style indexing)
