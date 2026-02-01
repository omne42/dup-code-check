import fs from 'node:fs';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const binDir = path.join(repoRoot, 'bin');

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

// Ensure npm will create the bin shim on install. The real binary is built during
// `postinstall` and overwrites this stub.
fs.mkdirSync(binDir, { recursive: true });
const stubPath = path.join(binDir, 'dup-code-check');
const stub = `#!/usr/bin/env sh
echo "dup-code-check: binary not built (postinstall did not run or failed)" >&2
echo "Try: npm run build" >&2
exit 1
`;
fs.writeFileSync(stubPath, stub, { mode: 0o755 });
process.stdout.write(`Wrote stub ${stubPath}\n`);
