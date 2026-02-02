import fs from 'node:fs';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const docsRoot = path.join(repoRoot, 'docs');
const publicDir = path.join(docsRoot, 'public');
const outPath = path.join(publicDir, 'llms.txt');

const EXCLUDED_DIRS = new Set(['_book', '.vitepress', 'public']);

function isMarkdownFile(p) {
  return p.endsWith('.md');
}

function walkDir(dir) {
  const out = [];
  for (const ent of fs.readdirSync(dir, { withFileTypes: true })) {
    const abs = path.join(dir, ent.name);
    if (ent.isDirectory()) {
      if (EXCLUDED_DIRS.has(ent.name)) continue;
      out.push(...walkDir(abs));
      continue;
    }
    if (!ent.isFile()) continue;
    if (!isMarkdownFile(ent.name)) continue;
    out.push(abs);
  }
  return out;
}

function relDocPath(abs) {
  return path.relative(repoRoot, abs).replace(/\\/g, '/');
}

if (!fs.existsSync(docsRoot)) {
  process.stderr.write(`docs root not found: ${docsRoot}\n`);
  process.exit(1);
}

fs.mkdirSync(publicDir, { recursive: true });

const files = walkDir(docsRoot)
  .filter((p) => path.basename(p).toLowerCase() !== 'readme.md')
  .sort((a, b) => relDocPath(a).localeCompare(relDocPath(b)));

let out = '';
out += '# dup-code-check docs (llms.txt)\n';
out += '\n';
out +=
  'This file is an automatically generated, plain-text bundle of the documentation.\n' +
  'It is intended for LLM context ingestion and offline reading.\n';
out += '\n';
out += `Generated: ${new Date().toISOString()}\n`;
out += '\n';

for (const abs of files) {
  const rel = relDocPath(abs);
  const content = fs.readFileSync(abs, 'utf8');
  out += '\n';
  out += '---\n';
  out += `source: ${rel}\n`;
  out += '---\n';
  out += '\n';
  out += content.trimEnd();
  out += '\n';
}

fs.writeFileSync(outPath, out, 'utf8');
process.stdout.write(`Wrote ${relDocPath(outPath)}\n`);

