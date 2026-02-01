# Installation & Build

[中文](installation.zh-CN.md)

`dup-code-check` is a Rust CLI binary. Node.js is only used as an installation option (npm). The current npm package builds the binary from source during install, so Rust is required.

## Option A: Rust (recommended)

From the repo root:

```bash
cargo build --release -p dup-code-check
./target/release/dup-code-check --help
```

Or install locally:

```bash
cargo install --path . --bin dup-code-check
dup-code-check --help
```

## Option B: Install via npm (project dependency)

```bash
npm i -D dup-code-check
```

After installation:

```bash
npx dup-code-check --help
```

> Tip: if your environment disables npm scripts (e.g. `npm_config_ignore_scripts=true`), `postinstall` won’t build the binary. You can run `npm run build` inside `node_modules/dup-code-check/`, or use the Rust option.

## Option C: Global npm install (local tooling)

```bash
npm i -g dup-code-check
dup-code-check --help
```

## Option D: Contribute / develop from source

```bash
git clone git@github.com:omne42/dup-code-check.git
cd dup-code-check
npm install
npm run build
./bin/dup-code-check --help
```

## What does `npm run build` do?

`npm run build` executes `scripts/build-binary.mjs`:

1. `cargo build --release -p dup-code-check`
2. copy the output into `bin/`

Output binary name:

- macOS / Linux: `bin/dup-code-check`
- Windows: `bin/dup-code-check.exe`

## Common build issues

If you see “Rust toolchain is required…”, check [Troubleshooting](troubleshooting.md).
