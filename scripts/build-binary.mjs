import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, '..');

function isNodeModulesBinDir(dir) {
  const norm = dir.replace(/\\/g, '/');
  return norm.includes('/node_modules/.bin');
}

function resolveCargoExe() {
  const pathVar = process.env.PATH ?? '';
  const sep = process.platform === 'win32' ? ';' : ':';
  const entries = pathVar.split(sep).filter(Boolean);

  const candidates = process.platform === 'win32' ? ['cargo.exe', 'cargo'] : ['cargo'];
  for (const dir of entries) {
    if (isNodeModulesBinDir(dir)) continue;
    for (const name of candidates) {
      const exe = path.join(dir, name);
      try {
        const st = fs.statSync(exe);
        if (st.isFile()) return exe;
      } catch {
        // ignore
      }
    }
  }
  return null;
}

if (process.env.DUP_CODE_CHECK_SKIP_BUILD === '1') {
  process.stdout.write('Skipping Rust binary build (DUP_CODE_CHECK_SKIP_BUILD=1)\n');
  process.exit(0);
}

const cargoExe = resolveCargoExe();
if (!cargoExe) {
  throw new Error(
    'Rust toolchain is required to build the binary.\n' +
      'This package intentionally does not execute `cargo` from `node_modules/.bin` (supply-chain hardening).\n' +
      'Install Rust (https://rustup.rs) and ensure `cargo` is on PATH, then re-run:\n' +
      '  npm run build\n'
  );
}

try {
  execFileSync(cargoExe, ['build', '--release', '--locked', '-p', 'dup-code-check'], {
    cwd: repoRoot,
    stdio: 'inherit'
  });
} catch (err) {
  if (err && typeof err === 'object' && err.code === 'ENOENT') {
    throw new Error(
      'Rust toolchain is required to build the binary.\n' +
        'Install Rust (https://rustup.rs) and re-run:\n' +
        '  npm run build\n'
    );
  }
  const status =
    err && typeof err === 'object' && 'status' in err && typeof err.status === 'number'
      ? err.status
      : 'unknown';
  throw new Error(
    'Failed to build Rust binary via Cargo.\n' +
      `Exit status: ${status}\n` +
      'Try running this command manually to see full output:\n' +
      '  cargo build --release --locked -p dup-code-check\n'
  );
}

const targetDir = path.join(repoRoot, 'target', 'release');
const builtFile = process.platform === 'win32' ? 'dup-code-check.exe' : 'dup-code-check';
const builtPath = path.join(targetDir, builtFile);
if (!fs.existsSync(builtPath)) {
  throw new Error(`Build succeeded but ${builtPath} was not found`);
}

const outDir = path.join(repoRoot, 'bin');
fs.mkdirSync(outDir, { recursive: true });

const wrapperPath = path.join(outDir, 'dup-code-check.mjs');
if (!fs.existsSync(wrapperPath)) {
  throw new Error(
    `Node wrapper script was not found at ${wrapperPath}.\n` +
      'This repository should contain bin/dup-code-check.mjs.\n' +
      'Restore it and re-run:\n' +
      '  npm run build\n'
  );
}

const outPath = path.join(outDir, builtFile);
fs.copyFileSync(builtPath, outPath);
if (process.platform !== 'win32') {
  fs.chmodSync(outPath, 0o755);
}
process.stdout.write(`Wrote ${outPath}\n`);
