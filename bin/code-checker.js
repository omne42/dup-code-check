#!/usr/bin/env node
'use strict';

const path = require('node:path');
const process = require('node:process');

const DEFAULT_MAX_FILE_SIZE_BYTES = 10 * 1024 * 1024;

let cachedApi;
function getApi() {
  if (!cachedApi) {
    cachedApi = require('..');
  }
  return cachedApi;
}

function printHelp() {
  process.stdout.write(
    [
      'code-checker (duplicate files / suspected duplicate code spans)',
      '',
      'Usage:',
      '  code-checker [options] [root ...]',
      '',
      'Options:',
      '  --report                Run all detectors and output a report',
      '  --code-spans            Find suspected duplicate code spans',
      '  --json                  Output JSON',
      '  --cross-repo-only       Only report groups spanning >= 2 roots',
      '  --min-match-len <n>     Code spans: minimum normalized length (default: 50)',
      '  --min-token-len <n>     Token-based: minimum token length (default: 50)',
      '  --similarity-threshold <f>  Similarity: 0..1 (default: 0.85)',
      '  --simhash-max-distance <n>  SimHash: max Hamming distance (default: 3)',
      '  --max-report-items <n>  Limit items per report section (default: 200)',
      `  --max-file-size <n>     Skip files larger than n bytes (default: ${DEFAULT_MAX_FILE_SIZE_BYTES})`,
      '  --ignore-dir <name>     Add an ignored directory name (repeatable)',
      '  --follow-symlinks       Follow symlinks (default: off)',
      '  -h, --help              Show help',
      '',
      'Examples:',
      '  code-checker .',
      '  code-checker --cross-repo-only /repoA /repoB',
      '  code-checker --code-spans --cross-repo-only /repoA /repoB',
      '  code-checker --report --cross-repo-only /repoA /repoB',
      '  code-checker --ignore-dir vendor --ignore-dir .venv .',
      ''
    ].join('\n')
  );
}

function parseArgs(argv) {
  const roots = [];
  const ignoreDirs = [];
  let report = false;
  let codeSpans = false;
  let json = false;
  let crossRepoOnly = false;
  let followSymlinks = false;
  let maxFileSize;
  let minMatchLen;
  let minTokenLen;
  let similarityThreshold;
  let simhashMaxDistance;
  let maxReportItems;

  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];
    if (arg === '--report') {
      report = true;
      continue;
    }
    if (arg === '--code-spans') {
      codeSpans = true;
      continue;
    }
    if (arg === '--json') {
      json = true;
      continue;
    }
    if (arg === '--cross-repo-only') {
      crossRepoOnly = true;
      continue;
    }
    if (arg === '--follow-symlinks') {
      followSymlinks = true;
      continue;
    }
    if (arg === '--max-file-size') {
      const raw = argv[++i];
      if (!raw) throw new Error('--max-file-size requires a value');
      const value = Number(raw);
      if (!Number.isSafeInteger(value) || value < 0) {
        throw new Error(
          `--max-file-size must be an integer 0..${Number.MAX_SAFE_INTEGER}`
        );
      }
      maxFileSize = value;
      continue;
    }
    if (arg === '--min-match-len') {
      const raw = argv[++i];
      if (!raw) throw new Error('--min-match-len requires a value');
      const value = Number(raw);
      if (!Number.isSafeInteger(value) || value <= 0 || value > 0xffffffff) {
        throw new Error('--min-match-len must be 1..4294967295');
      }
      minMatchLen = value;
      continue;
    }
    if (arg === '--min-token-len') {
      const raw = argv[++i];
      if (!raw) throw new Error('--min-token-len requires a value');
      const value = Number(raw);
      if (!Number.isSafeInteger(value) || value <= 0 || value > 0xffffffff) {
        throw new Error('--min-token-len must be 1..4294967295');
      }
      minTokenLen = value;
      continue;
    }
    if (arg === '--similarity-threshold') {
      const raw = argv[++i];
      if (!raw) throw new Error('--similarity-threshold requires a value');
      const value = Number(raw);
      if (!Number.isFinite(value) || value < 0 || value > 1) {
        throw new Error('--similarity-threshold must be 0..1');
      }
      similarityThreshold = value;
      continue;
    }
    if (arg === '--simhash-max-distance') {
      const raw = argv[++i];
      if (!raw) throw new Error('--simhash-max-distance requires a value');
      const value = Number(raw);
      if (!Number.isSafeInteger(value) || value < 0 || value > 64) {
        throw new Error('--simhash-max-distance must be 0..64');
      }
      simhashMaxDistance = value;
      continue;
    }
    if (arg === '--max-report-items') {
      const raw = argv[++i];
      if (!raw) throw new Error('--max-report-items requires a value');
      const value = Number(raw);
      if (!Number.isSafeInteger(value) || value < 0 || value > 0xffffffff) {
        throw new Error('--max-report-items must be 0..4294967295');
      }
      maxReportItems = value;
      continue;
    }
    if (arg === '--ignore-dir') {
      const value = argv[++i];
      if (!value) throw new Error('--ignore-dir requires a value');
      ignoreDirs.push(value);
      continue;
    }
    if (arg === '-h' || arg === '--help') {
      return { help: true };
    }
    if (arg.startsWith('-')) {
      throw new Error(`Unknown option: ${arg}`);
    }
    roots.push(arg);
  }

  return {
    help: false,
    json,
    report,
    codeSpans,
    roots: roots.length ? roots : [process.cwd()],
    options: {
      ignoreDirs: ignoreDirs.length ? ignoreDirs : undefined,
      maxFileSize: maxFileSize,
      minMatchLen: minMatchLen,
      minTokenLen: minTokenLen,
      similarityThreshold: similarityThreshold,
      simhashMaxDistance: simhashMaxDistance,
      maxReportItems: maxReportItems,
      crossRepoOnly: crossRepoOnly,
      followSymlinks: followSymlinks
    }
  };
}

function formatText(groups) {
  const lines = [];
  lines.push(`duplicate groups: ${groups.length}`);

  for (const group of groups) {
    lines.push('');
    lines.push(
      `hash=${group.hash} normalized_len=${group.normalizedLen} files=${group.files.length}`
    );
    for (const file of group.files) {
      lines.push(`- [${file.repoLabel}] ${file.path}`);
    }
  }

  lines.push('');
  return lines.join('\n');
}

function formatTextCodeSpans(groups) {
  const lines = [];
  lines.push(`duplicate code span groups: ${groups.length}`);

  for (const group of groups) {
    lines.push('');
    lines.push(
      `hash=${group.hash} normalized_len=${group.normalizedLen} occurrences=${group.occurrences.length}`
    );
    lines.push(`preview=${group.preview}`);
    for (const occ of group.occurrences) {
      lines.push(
        `- [${occ.repoLabel}] ${occ.path}:${occ.startLine}-${occ.endLine}`
      );
    }
  }

  lines.push('');
  return lines.join('\n');
}

function formatTextSimilarPairs(pairs) {
  const lines = [];
  lines.push(`similar pairs: ${pairs.length}`);
  for (const pair of pairs) {
    const distance =
      pair.distance === null || pair.distance === undefined
        ? ''
        : ` distance=${pair.distance}`;
    lines.push(`score=${pair.score}${distance}`);
    lines.push(
      `- A [${pair.a.repoLabel}] ${pair.a.path}:${pair.a.startLine}-${pair.a.endLine}`
    );
    lines.push(
      `- B [${pair.b.repoLabel}] ${pair.b.path}:${pair.b.startLine}-${pair.b.endLine}`
    );
  }
  lines.push('');
  return lines.join('\n');
}

function formatTextReport(report) {
  const parts = [];
  parts.push('== file duplicates ==');
  parts.push(formatText(report.fileDuplicates).trimEnd());
  parts.push('');
  parts.push('== code span duplicates ==');
  parts.push(formatTextCodeSpans(report.codeSpanDuplicates).trimEnd());
  parts.push('');
  parts.push('== line span duplicates ==');
  parts.push(formatTextCodeSpans(report.lineSpanDuplicates).trimEnd());
  parts.push('');
  parts.push('== token span duplicates ==');
  parts.push(formatTextCodeSpans(report.tokenSpanDuplicates).trimEnd());
  parts.push('');
  parts.push('== block duplicates ==');
  parts.push(formatTextCodeSpans(report.blockDuplicates).trimEnd());
  parts.push('');
  parts.push('== AST subtree duplicates ==');
  parts.push(formatTextCodeSpans(report.astSubtreeDuplicates).trimEnd());
  parts.push('');
  parts.push('== similar blocks (minhash) ==');
  parts.push(formatTextSimilarPairs(report.similarBlocksMinhash).trimEnd());
  parts.push('');
  parts.push('== similar blocks (simhash) ==');
  parts.push(formatTextSimilarPairs(report.similarBlocksSimhash).trimEnd());
  parts.push('');
  return parts.join('\n');
}

function main() {
  let parsed;
  try {
    parsed = parseArgs(process.argv.slice(2));
  } catch (err) {
    process.stderr.write(`Error: ${err.message}\n\n`);
    printHelp();
    process.exitCode = 2;
    return;
  }

  if (parsed.help) {
    printHelp();
    return;
  }

  const roots = parsed.roots.map((p) => path.resolve(p));
  const { findDuplicateFiles, findDuplicateCodeSpans, generateDuplicationReport } =
    getApi();
  if (parsed.report) {
    const report = generateDuplicationReport(roots, parsed.options);
    if (parsed.json) {
      process.stdout.write(`${JSON.stringify(report, null, 2)}\n`);
      return;
    }
    process.stdout.write(formatTextReport(report));
    return;
  }

  const groups = parsed.codeSpans
    ? findDuplicateCodeSpans(roots, parsed.options)
    : findDuplicateFiles(roots, parsed.options);

  if (parsed.json) {
    process.stdout.write(`${JSON.stringify(groups, null, 2)}\n`);
    return;
  }

  process.stdout.write(
    parsed.codeSpans ? formatTextCodeSpans(groups) : formatText(groups)
  );
}

main();
