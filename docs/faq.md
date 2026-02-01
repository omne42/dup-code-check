# FAQ

[中文](faq.zh-CN.md)

## Q: What languages are supported?

Current detectors are mostly heuristic-based on text/characters/tokens/brace-blocks, so they don’t have hard language dependencies:

- duplicate files: any text
- code spans / line spans: any text (but more useful for code-like content)
- token/block/“AST subtree”: more effective for `{}`-heavy languages (JS/TS/Java/C/C++/Rust, etc.)

## Q: Is this a complete clone detector? Does it cover Type-2/3?

It’s closer to an “engineering-friendly toolbox” that spans from strong signals to heuristics:

- file duplicates: strong signal, low cost
- token spans / blocks: more robust for some Type-2 cases
- MinHash/SimHash: similarity hints towards Type-3

It’s not a full academic clone detector and does not build a real language AST (current “AST subtree” is `{}`-structure approximation).

## Q: Why Rust + Node.js?

- Rust: scanning/normalization/fingerprinting is faster and more memory-controllable
- Node.js: CLI distribution and ecosystem integration is convenient (especially in frontend/fullstack repos)

## Q: Why does npm install require Rust? Can we avoid compiling?

The current npm package builds the native binary from source during installation, so Rust is required.

If/when we ship prebuilt artifacts, Rust could become optional for consumers.

## Q: What is the `hash` in results? Can it collide?

`hash` is a 64-bit fingerprint of normalized content (printed as a 16-char hex string).

The implementation does a sample comparison within a hash bucket to reduce collision risk, but in theory extremely rare collisions are still possible. If you need zero-collision guarantees, treat these as candidates and perform a secondary verification.
