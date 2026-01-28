import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

import pkg from '../index.js';

const { findDuplicateFiles } = pkg;
const { findDuplicateCodeSpans } = pkg;
const { generateDuplicationReport } = pkg;

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'code-checker-smoke-'));
const repoA = path.join(tmp, 'repoA');
const repoB = path.join(tmp, 'repoB');
fs.mkdirSync(repoA, { recursive: true });
fs.mkdirSync(repoB, { recursive: true });

fs.writeFileSync(path.join(repoA, 'a.txt'), 'a b\nc');
fs.writeFileSync(path.join(repoA, 'b.txt'), 'ab\tc');
fs.writeFileSync(path.join(repoB, 'c.txt'), 'ab c');
fs.writeFileSync(path.join(repoB, 'd.txt'), 'different');

const groups = findDuplicateFiles([repoA, repoB], { crossRepoOnly: true });
if (groups.length !== 1 || groups[0].files.length !== 3) {
  process.stderr.write(`Unexpected result: ${JSON.stringify(groups, null, 2)}\n`);
  process.exitCode = 1;
} else {
  const bigSize = 10 * 1024 * 1024 + 1;
  const big = Buffer.alloc(bigSize, 'a');
  fs.writeFileSync(path.join(repoA, 'big_a.txt'), big);
  fs.writeFileSync(path.join(repoB, 'big_b.txt'), big);

  const groupsWithBig = findDuplicateFiles([repoA, repoB], { crossRepoOnly: true });
  if (groupsWithBig.length !== 1) {
    process.stderr.write(
      `Unexpected result: ${JSON.stringify(groupsWithBig, null, 2)}\n`
    );
    process.exitCode = 1;
	  } else {
	    const badCli = spawnSync(
	      process.execPath,
	      [path.join(repoRoot, 'bin', 'code-checker.js'), '--max-file-size', '1.5', repoA],
	      { encoding: 'utf8' }
	    );
	    if (badCli.status !== 2) {
	      process.stderr.write(
	        `Unexpected CLI exit code: ${badCli.status}\nstdout:\n${badCli.stdout}\nstderr:\n${badCli.stderr}\n`
	      );
	      process.exitCode = 1;
	    }

	    function expectThrow(name, fn) {
	      let threw = false;
	      try {
	        fn();
	      } catch {
	        threw = true;
	      }
	      if (!threw) {
	        process.stderr.write(`Expected error: ${name}\n`);
	        process.exitCode = 1;
	      }
	    }

	    expectThrow('similarityThreshold NaN', () =>
	      findDuplicateFiles([repoA], { similarityThreshold: Number.NaN })
	    );
	    expectThrow('minMatchLen 1.5', () =>
	      findDuplicateFiles([repoA], { minMatchLen: 1.5 })
	    );
	    expectThrow('minMatchLen -1', () =>
	      findDuplicateFiles([repoA], { minMatchLen: -1 })
	    );
	    expectThrow('minTokenLen 0', () =>
	      findDuplicateFiles([repoA], { minTokenLen: 0 })
	    );
	    expectThrow('simhashMaxDistance 1.5', () =>
	      findDuplicateFiles([repoA], { simhashMaxDistance: 1.5 })
	    );
	    expectThrow('simhashMaxDistance -1', () =>
	      findDuplicateFiles([repoA], { simhashMaxDistance: -1 })
	    );
	    expectThrow('maxReportItems -1', () =>
	      findDuplicateFiles([repoA], { maxReportItems: -1 })
	    );
	  }

	  const snippet = 'abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789';
	  fs.writeFileSync(path.join(repoA, 'spanA.txt'), `////\nP${snippet}Q\n`);
	  fs.writeFileSync(path.join(repoB, 'spanB.txt'), `####\nR${snippet}S\n`);

  const spans = findDuplicateCodeSpans([repoA, repoB], {
    crossRepoOnly: true,
    minMatchLen: 50
  });
  if (
    spans.length !== 1 ||
    spans[0].normalizedLen !== snippet.length ||
    spans[0].occurrences.length !== 2 ||
    spans[0].occurrences.some((o) => o.startLine !== 2 || o.endLine !== 2)
  ) {
    process.stderr.write(`Unexpected result: ${JSON.stringify(spans, null, 2)}\n`);
    process.exitCode = 1;
  } else {
    const report = generateDuplicationReport([repoA, repoB], {
      crossRepoOnly: true,
      minMatchLen: 50,
      minTokenLen: 5,
      similarityThreshold: 0.8
    });
    if (
      !report ||
      report.fileDuplicates?.length !== 1 ||
      report.codeSpanDuplicates?.length !== 1 ||
      !Array.isArray(report.lineSpanDuplicates) ||
      !Array.isArray(report.tokenSpanDuplicates)
    ) {
      process.stderr.write(`Unexpected result: ${JSON.stringify(report, null, 2)}\n`);
      process.exitCode = 1;
    } else {
      process.stdout.write('smoke ok\n');
    }
  }
}
