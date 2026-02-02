#!/usr/bin/env node
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

const binDir = path.dirname(fileURLToPath(import.meta.url));
const binaryName = process.platform === 'win32' ? 'dup-code-check.exe' : 'dup-code-check';
const binaryPath = path.join(binDir, binaryName);

if (!fs.existsSync(binaryPath)) {
  process.stderr.write(
    `dup-code-check: binary not found at ${binaryPath}\n` +
      'This package builds the Rust binary during postinstall.\n' +
      'If you disabled install scripts, re-install with scripts enabled, or run:\n' +
      '  npm run build\n'
  );
  process.exit(1);
}

const res = spawnSync(binaryPath, process.argv.slice(2), { stdio: 'inherit' });
if (res.error) {
  process.stderr.write(`dup-code-check: failed to spawn ${binaryPath}\n${res.error}\n`);
  process.exit(1);
}
process.exit(res.status ?? 1);
