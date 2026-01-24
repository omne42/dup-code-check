import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, '..');

try {
  execFileSync('cargo', ['build', '--release', '-p', 'code-checker'], {
    cwd: repoRoot,
    stdio: 'inherit'
  });
} catch (err) {
  if (err && typeof err === 'object' && err.code === 'ENOENT') {
    throw new Error(
      'Rust toolchain is required to build the native module.\n' +
        'Install Rust (https://rustup.rs) and re-run:\n' +
        '  npm run build\n'
    );
  }
  throw err;
}

const targetDir = path.join(repoRoot, 'target', 'release');
const libFile =
  process.platform === 'win32'
    ? 'code_checker.dll'
    : process.platform === 'darwin'
      ? 'libcode_checker.dylib'
      : 'libcode_checker.so';

const builtPath = path.join(targetDir, libFile);
if (!fs.existsSync(builtPath)) {
  throw new Error(`Build succeeded but ${builtPath} was not found`);
}

const outPath = path.join(repoRoot, 'code_checker.node');
fs.copyFileSync(builtPath, outPath);
process.stdout.write(`Wrote ${outPath}\n`);
