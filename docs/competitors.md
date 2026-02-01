# Competitors / Similar Tools (Duplicate Code / Clone Detection)

[中文](competitors.zh-CN.md)

Goal: quickly locate mature solutions in the ecosystem and clarify a differentiated direction for “Rust core + npm distribution”.

## Common tools (by adoption / practicality)

- jscpd: text/token duplication detection; multi-language/multi-directory; easy to integrate
- PMD CPD: Java ecosystem “Copy/Paste Detector” (token-level)
- SonarQube Duplication: duplication detection inside a quality platform (multi-language, platform reporting)
- Simian, etc.: general clone detection products (good references for product shape)

## Clone types as a lens

- Type-1: identical or only formatting changes (whitespace/comments) — current MVP aligns here (even stricter: whitespace removal only)
- Type-2: renaming identifiers/literals — typically needs tokenization
- Type-3: small edits/insertions/deletions — needs more complex similarity/windowing/fingerprints/AST
- Type-4: semantic equivalence with different structure — usually beyond “simple duplicate detection”

## Research / large-scale approaches (methodology references)

- SourcererCC: large-scale clone search (index/retrieval)
- Deckard: structured / feature-vector approaches (AST features)
- NiCad: normalization + comparison (emphasizes normalization strategy)

## Comparison dimensions (what to record)

- granularity: file / span (function/block) / cross-file composition
- normalization: whitespace-only / comment removal / tokenization / AST
- clone type coverage: Type-1/2/3
- output: localization (path + range), JSON/SARIF, thresholds/min-block sizes
- integration: CLI, CI, incremental scan, caching, ignore rules
- performance: speed/memory, scaling for large/multi-repo (index vs full compare)

## Suggested differentiation

- Rust for scanning/normalization/fingerprints → speed + portability
- Node for CLI distribution via npm → easy adoption in frontend/fullstack repos
- start with robust cross-repo/file-level duplicates, then expand to span-level clones
