import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import process from 'node:process';

import pkg from '../index.js';

const { findDuplicateFiles } = pkg;

const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'code-checker-smoke-'));
const repoA = path.join(tmp, 'repoA');
const repoB = path.join(tmp, 'repoB');
fs.mkdirSync(repoA, { recursive: true });
fs.mkdirSync(repoB, { recursive: true });

fs.writeFileSync(path.join(repoA, 'a.txt'), 'a b\nc');
fs.writeFileSync(path.join(repoA, 'b.txt'), 'ab\tc');
fs.writeFileSync(path.join(repoB, 'c.txt'), 'ab c');
fs.writeFileSync(path.join(repoB, 'd.txt'), 'different');

const groups = findDuplicateFiles([repoA, repoB], { cross_repo_only: true });
if (groups.length !== 1 || groups[0].files.length !== 3) {
  process.stderr.write(`Unexpected result: ${JSON.stringify(groups, null, 2)}\n`);
  process.exitCode = 1;
} else {
  process.stdout.write('smoke ok\n');
}
