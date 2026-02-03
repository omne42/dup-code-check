# Installation & Build

[中文](installation.zh-CN.md)

`dup-code-check` is a Rust CLI binary. Node.js is only used as an installation option (npm). The npm package builds the binary from source when needed (Cargo), so Rust is required.

By default, project installs build on first run. Global installs may build during `postinstall` to avoid permission issues.

> Security note: building runs a native build (Cargo), which may execute dependency build scripts. Depending on how you install, this may run during `postinstall` (e.g. global installs) or on first run. Use `--ignore-scripts` / `npm_config_ignore_scripts=true` if you need to avoid running install scripts.

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

> Tip: to build during install (project dependency), set `DUP_CODE_CHECK_BUILD_ON_INSTALL=1`.
>
> Tip: to skip builds (install + first run), set `DUP_CODE_CHECK_SKIP_BUILD=1` and build manually later.

## Option C: Global npm install (local tooling)

```bash
npm i -g dup-code-check
dup-code-check --help
```

> Tip: global installs work best with install scripts enabled (`postinstall` builds the binary).

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
