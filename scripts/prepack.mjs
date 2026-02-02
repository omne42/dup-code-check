import fs from 'node:fs';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const binDir = path.join(repoRoot, 'bin');

const wrapperPath = path.join(binDir, 'dup-code-check.mjs');

const candidates = [
  path.join(binDir, 'dup-code-check'),
  path.join(binDir, 'dup-code-check.exe'),
];

for (const candidate of candidates) {
  try {
    fs.rmSync(candidate);
    process.stdout.write(`Removed ${candidate}\n`);
  } catch (err) {
    if (err && typeof err === 'object' && err.code === 'ENOENT') {
      continue;
    }
    throw err;
  }
}

if (!fs.existsSync(wrapperPath)) {
  throw new Error(`Missing wrapper script: ${wrapperPath}`);
}
process.stdout.write(`Verified wrapper ${wrapperPath}\n`);
