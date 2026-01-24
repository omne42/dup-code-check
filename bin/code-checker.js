#!/usr/bin/env node
'use strict';

const path = require('node:path');
const process = require('node:process');

const { findDuplicateFiles } = require('..');

function printHelp() {
  process.stdout.write(
    [
      'code-checker (duplicate files, whitespace-insensitive)',
      '',
      'Usage:',
      '  code-checker [options] [root ...]',
      '',
      'Options:',
      '  --json                  Output JSON',
      '  --cross-repo-only       Only report groups spanning >= 2 roots',
      '  --max-file-size <n>     Skip files larger than n bytes (u32)',
      '  --ignore-dir <name>     Add an ignored directory name (repeatable)',
      '  --follow-symlinks       Follow symlinks (default: off)',
      '  -h, --help              Show help',
      '',
      'Examples:',
      '  code-checker .',
      '  code-checker --cross-repo-only /repoA /repoB',
      '  code-checker --ignore-dir vendor --ignore-dir .venv .',
      ''
    ].join('\n')
  );
}

function parseArgs(argv) {
  const roots = [];
  const ignoreDirs = [];
  let json = false;
  let crossRepoOnly = false;
  let followSymlinks = false;
  let maxFileSize;

  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];
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
      if (!Number.isFinite(value) || value < 0 || value > 0xffffffff) {
        throw new Error('--max-file-size must be 0..4294967295');
      }
      maxFileSize = value;
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
    roots: roots.length ? roots : [process.cwd()],
    options: {
      ignore_dirs: ignoreDirs.length ? ignoreDirs : undefined,
      max_file_size: maxFileSize,
      cross_repo_only: crossRepoOnly,
      follow_symlinks: followSymlinks
    }
  };
}

function formatText(groups) {
  const lines = [];
  lines.push(`duplicate groups: ${groups.length}`);

  for (const group of groups) {
    lines.push('');
    lines.push(`hash=${group.hash} normalized_len=${group.normalized_len} files=${group.files.length}`);
    for (const file of group.files) {
      lines.push(`- [${file.repo_label}] ${file.path}`);
    }
  }

  lines.push('');
  return lines.join('\n');
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
  const groups = findDuplicateFiles(roots, parsed.options);

  if (parsed.json) {
    process.stdout.write(`${JSON.stringify(groups, null, 2)}\n`);
    return;
  }

  process.stdout.write(formatText(groups));
}

main();

