import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

const cliName = process.platform === 'win32' ? 'dup-code-check.exe' : 'dup-code-check';
const cliPath = path.join(repoRoot, 'bin', cliName);

function buildIfNeeded() {
  if (fs.existsSync(cliPath)) {
    const probe = spawnSync(cliPath, ['--help'], { encoding: 'utf8' });
    if (probe.status === 0) return;
  }
  const build = spawnSync(process.execPath, [path.join(repoRoot, 'scripts', 'build-binary.mjs')], {
    encoding: 'utf8',
    cwd: repoRoot
  });
  if (build.status !== 0) {
    process.stderr.write(
      `Failed to build CLI.\nstatus=${build.status}\nstdout:\n${build.stdout}\nstderr:\n${build.stderr}\n`
    );
    process.exit(1);
  }
  if (!fs.existsSync(cliPath)) {
    process.stderr.write(`Build reported success but ${cliPath} was not found\n`);
    process.exit(1);
  }
}

function runCli(args) {
  return spawnSync(cliPath, args, { encoding: 'utf8' });
}

function runCliJson(args) {
  const res = runCli(['--json', ...args]);
  if (res.status !== 0) {
    process.stderr.write(
      `CLI failed.\nstatus=${res.status}\nstdout:\n${res.stdout}\nstderr:\n${res.stderr}\n`
    );
    process.exit(1);
  }
  try {
    return JSON.parse(res.stdout);
  } catch (err) {
    process.stderr.write(`Failed to parse JSON.\nstdout:\n${res.stdout}\nerror: ${err}\n`);
    process.exit(1);
  }
}

function expectExitCode(name, args, code) {
  const res = runCli(args);
  if (res.status !== code) {
    process.stderr.write(
      `Expected ${name} exit code ${code}, got ${res.status}\nstdout:\n${res.stdout}\nstderr:\n${res.stderr}\n`
    );
    process.exit(1);
  }
}

buildIfNeeded();

const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'dup-code-check-smoke-'));
const repoA = path.join(tmp, 'repoA');
const repoB = path.join(tmp, 'repoB');
fs.mkdirSync(repoA, { recursive: true });
fs.mkdirSync(repoB, { recursive: true });

fs.writeFileSync(path.join(repoA, 'a.txt'), 'a b\nc');
fs.writeFileSync(path.join(repoA, 'b.txt'), 'ab\tc');
fs.writeFileSync(path.join(repoB, 'c.txt'), 'ab c');
fs.writeFileSync(path.join(repoB, 'd.txt'), 'different');

const groups = runCliJson(['--cross-repo-only', repoA, repoB]);
if (groups.length !== 1 || groups[0].files.length !== 3) {
  process.stderr.write(`Unexpected result: ${JSON.stringify(groups, null, 2)}\n`);
  process.exit(1);
}

const withStats = runCliJson(['--stats', '--cross-repo-only', repoA, repoB]);
if (
  !withStats ||
  !Array.isArray(withStats.groups) ||
  withStats.groups.length !== 1 ||
  typeof withStats.scanStats?.scannedFiles !== 'number'
) {
  process.stderr.write(
    `Unexpected result (with stats): ${JSON.stringify(withStats, null, 2)}\n`
  );
  process.exit(1);
}

const limited = runCliJson([
  '--stats',
  '--cross-repo-only',
  '--max-files',
  '1',
  repoA,
  repoB,
]);
if (
  !limited ||
  typeof limited.scanStats?.scannedFiles !== 'number' ||
  limited.scanStats.scannedFiles !== 1 ||
  limited.scanStats.skippedBudgetMaxFiles !== 3
) {
  process.stderr.write(
    `Unexpected result (maxFiles): ${JSON.stringify(limited, null, 2)}\n`
  );
  process.exit(1);
}

const bigSize = 10 * 1024 * 1024 + 1;
const big = Buffer.alloc(bigSize, 'a');
fs.writeFileSync(path.join(repoA, 'big_a.txt'), big);
fs.writeFileSync(path.join(repoB, 'big_b.txt'), big);

const groupsWithBig = runCliJson(['--cross-repo-only', repoA, repoB]);
if (groupsWithBig.length !== 1) {
  process.stderr.write(`Unexpected result: ${JSON.stringify(groupsWithBig, null, 2)}\n`);
  process.exit(1);
}

expectExitCode('bad --max-file-size', ['--max-file-size', '1.5', repoA], 2);

const dashRepo = path.join(tmp, '-repo');
fs.mkdirSync(dashRepo, { recursive: true });
fs.writeFileSync(path.join(dashRepo, 'a.txt'), 'a b\nc');
fs.writeFileSync(path.join(dashRepo, 'b.txt'), 'ab\tc');
const dashParsed = runCliJson(['--', dashRepo]);
if (!Array.isArray(dashParsed) || dashParsed.length !== 1 || dashParsed[0].files.length !== 2) {
  process.stderr.write(`Unexpected -- output: ${JSON.stringify(dashParsed, null, 2)}\n`);
  process.exit(1);
}

const ignoreRepo = path.join(tmp, 'ignoreRepo');
fs.mkdirSync(ignoreRepo, { recursive: true });
fs.writeFileSync(path.join(ignoreRepo, '.gitignore'), 'ignored.txt\n');
fs.writeFileSync(path.join(ignoreRepo, 'a.txt'), 'same content');
fs.writeFileSync(path.join(ignoreRepo, 'ignored.txt'), 'same content');

const defaultGitignore = runCliJson([ignoreRepo]);
if (!Array.isArray(defaultGitignore) || defaultGitignore.length !== 0) {
  process.stderr.write(
    `Unexpected result (default gitignore): ${JSON.stringify(defaultGitignore, null, 2)}\n`
  );
  process.exit(1);
}

const noGitignore = runCliJson(['--no-gitignore', ignoreRepo]);
if (!Array.isArray(noGitignore) || noGitignore.length !== 1 || noGitignore[0].files.length !== 2) {
  process.stderr.write(
    `Unexpected result (--no-gitignore): ${JSON.stringify(noGitignore, null, 2)}\n`
  );
  process.exit(1);
}

expectExitCode('similarityThreshold NaN', ['--similarity-threshold', 'NaN', repoA], 2);
expectExitCode('minMatchLen 1.5', ['--min-match-len', '1.5', repoA], 2);
expectExitCode('minMatchLen -1', ['--min-match-len', '-1', repoA], 2);
expectExitCode('minTokenLen 0', ['--min-token-len', '0', repoA], 2);
expectExitCode('simhashMaxDistance 1.5', ['--simhash-max-distance', '1.5', repoA], 2);
expectExitCode('simhashMaxDistance -1', ['--simhash-max-distance', '-1', repoA], 2);
expectExitCode('maxReportItems -1', ['--max-report-items', '-1', repoA], 2);

const snippet = 'abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789';
fs.writeFileSync(path.join(repoA, 'spanA.txt'), `////\nP${snippet}Q\n`);
fs.writeFileSync(path.join(repoB, 'spanB.txt'), `####\nR${snippet}S\n`);

const spans = runCliJson(['--code-spans', '--cross-repo-only', repoA, repoB]);
if (
  spans.length !== 1 ||
  spans[0].normalizedLen !== snippet.length ||
  spans[0].occurrences.length !== 2 ||
  spans[0].occurrences.some((o) => o.startLine !== 2 || o.endLine !== 2)
) {
  process.stderr.write(`Unexpected result: ${JSON.stringify(spans, null, 2)}\n`);
  process.exit(1);
}

const report = runCliJson([
  '--report',
  '--cross-repo-only',
  '--min-match-len',
  '50',
  '--min-token-len',
  '5',
  '--similarity-threshold',
  '0.8',
  repoA,
  repoB,
]);
if (
  !report ||
  report.fileDuplicates?.length !== 1 ||
  report.codeSpanDuplicates?.length !== 1 ||
  !Array.isArray(report.lineSpanDuplicates) ||
  !Array.isArray(report.tokenSpanDuplicates)
) {
  process.stderr.write(`Unexpected result: ${JSON.stringify(report, null, 2)}\n`);
  process.exit(1);
}

const reportWithStats = runCliJson([
  '--report',
  '--stats',
  '--cross-repo-only',
  '--min-match-len',
  '50',
  '--min-token-len',
  '5',
  '--similarity-threshold',
  '0.8',
  repoA,
  repoB,
]);
if (
  !reportWithStats ||
  !reportWithStats.report ||
  !reportWithStats.scanStats ||
  typeof reportWithStats.scanStats.scannedFiles !== 'number'
) {
  process.stderr.write(
    `Unexpected result (reportWithStats): ${JSON.stringify(reportWithStats, null, 2)}\n`
  );
  process.exit(1);
}

process.stdout.write('smoke ok\n');
