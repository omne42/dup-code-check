#!/usr/bin/env node
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

const binDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(binDir, '..');
const binaryName = process.platform === 'win32' ? 'dup-code-check.exe' : 'dup-code-check';
const binaryPath = path.join(binDir, binaryName);

function buildBinary() {
  if (process.env.DUP_CODE_CHECK_SKIP_BUILD === '1') {
    process.stderr.write(
      'dup-code-check: build is disabled (DUP_CODE_CHECK_SKIP_BUILD=1)\n' +
        `Binary not found at ${binaryPath}\n`
    );
    process.exit(1);
  }

  const buildScript = path.join(repoRoot, 'scripts', 'build-binary.mjs');
  const build = spawnSync(process.execPath, [buildScript], { cwd: repoRoot, stdio: 'inherit' });
  if (build.error) {
    process.stderr.write(`dup-code-check: failed to spawn build script\n${build.error}\n`);
    process.exit(1);
  }
  if (build.status !== 0) {
    process.exit(build.status ?? 1);
  }
}

if (!fs.existsSync(binaryPath)) {
  buildBinary();
  if (!fs.existsSync(binaryPath)) {
    process.stderr.write(
      `dup-code-check: binary not found at ${binaryPath}\n` +
        'Build reported success but the binary is still missing.\n'
    );
    process.exit(1);
  }
}

const res = spawnSync(binaryPath, process.argv.slice(2), { stdio: 'inherit' });
if (res.error) {
  process.stderr.write(`dup-code-check: failed to spawn ${binaryPath}\n${res.error}\n`);
  process.exit(1);
}
process.exit(res.status ?? 1);
