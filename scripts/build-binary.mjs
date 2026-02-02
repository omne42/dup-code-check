import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(__dirname, '..');

if (process.env.DUP_CODE_CHECK_SKIP_BUILD === '1') {
  process.stdout.write('Skipping Rust binary build (DUP_CODE_CHECK_SKIP_BUILD=1)\n');
  process.exit(0);
}

try {
  execFileSync('cargo', ['build', '--release', '--locked', '-p', 'dup-code-check'], {
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
  throw err;
}

const targetDir = path.join(repoRoot, 'target', 'release');
const builtFile = process.platform === 'win32' ? 'dup-code-check.exe' : 'dup-code-check';
const builtPath = path.join(targetDir, builtFile);
if (!fs.existsSync(builtPath)) {
  throw new Error(`Build succeeded but ${builtPath} was not found`);
}

const outDir = path.join(repoRoot, 'bin');
fs.mkdirSync(outDir, { recursive: true });

const outPath = path.join(outDir, builtFile);
fs.copyFileSync(builtPath, outPath);
if (process.platform !== 'win32') {
  fs.chmodSync(outPath, 0o755);
}
process.stdout.write(`Wrote ${outPath}\n`);
