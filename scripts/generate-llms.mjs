import fs from 'node:fs';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const docsRoot = path.join(repoRoot, 'docs');
const publicDir = path.join(docsRoot, 'public');

const EXCLUDED_DIRS = new Set(['_book', '.vitepress', 'public']);

const DOC_ORDER = [
  'index.md',
  'introduction.md',
  'llms.md',
  'getting-started.md',
  'installation.md',
  'cli.md',
  'scan-options.md',
  'detectors.md',
  'output.md',
  'ci.md',
  'performance.md',
  'architecture.md',
  'development.md',
  'contributing.md',
  'troubleshooting.md',
  'faq.md',
  'roadmap.md',
  'competitors.md',
];

function isMarkdownFile(p) {
  return p.endsWith('.md');
}

function isReadmeFile(p) {
  const base = path.basename(p).toLowerCase();
  return base === 'readme.md' || (base.startsWith('readme.') && base.endsWith('.md'));
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

function relDocsPath(abs) {
  return path.relative(docsRoot, abs).replace(/\\/g, '/');
}

function isZh(relDocs) {
  return relDocs.endsWith('.zh-CN.md');
}

function orderKey(relDocs) {
  const base = relDocs.replace(/\.zh-CN\.md$/, '.md');
  const idx = DOC_ORDER.indexOf(base);
  return idx === -1 ? Number.POSITIVE_INFINITY : idx;
}

function header(bundleLabel) {
  let out = '';
  out += '# dup-code-check docs (llms bundle)\n';
  out += '\n';
  out +=
    'This file is an automatically generated, plain-text bundle of the documentation.\n' +
    'It is intended for LLM context ingestion and offline reading.\n';
  out += '\n';
  out += `Bundle: ${bundleLabel}\n`;
  out += `Generated: ${new Date().toISOString()}\n`;
  out += '\n';
  out += 'Example prompt format:\n';
  out += '\n';
  out += '```text\n';
  out += 'Documentation:\n';
  out += '{paste documentation here}\n';
  out += '---\n';
  out += 'Based on the above documentation, answer the following:\n';
  out += '{your question}\n';
  out += '```\n';
  out += '\n';
  return out;
}

if (!fs.existsSync(docsRoot)) {
  process.stderr.write(`docs root not found: ${docsRoot}\n`);
  process.exit(1);
}

fs.mkdirSync(publicDir, { recursive: true });

const files = walkDir(docsRoot)
  .filter((p) => !isReadmeFile(p))
  .map((abs) => ({
    abs,
    rel: relDocPath(abs),
    relDocs: relDocsPath(abs),
  }))
  .sort((a, b) => {
    const lang = Number(isZh(a.relDocs)) - Number(isZh(b.relDocs));
    if (lang !== 0) return lang;

    const order = orderKey(a.relDocs) - orderKey(b.relDocs);
    if (order !== 0) return order;

    return a.rel.localeCompare(b.rel);
  });

const outputs = [
  {
    filename: 'llms.txt',
    bundleLabel: 'Combined (EN + 中文)',
    filter: () => true,
  },
  {
    filename: 'llms.en.txt',
    bundleLabel: 'English only',
    filter: (relDocs) => !isZh(relDocs),
  },
  {
    filename: 'llms.zh-CN.txt',
    bundleLabel: '中文 only',
    filter: (relDocs) => isZh(relDocs),
  },
];

for (const o of outputs) {
  const outPath = path.join(publicDir, o.filename);
  let out = header(o.bundleLabel);

  for (const file of files) {
    if (!o.filter(file.relDocs)) continue;
    const content = fs.readFileSync(file.abs, 'utf8');
    out += '\n';
    out += '---\n';
    out += `source: ${file.rel}\n`;
    out += '---\n';
    out += '\n';
    out += content.trimEnd();
    out += '\n';
  }

  fs.writeFileSync(outPath, out, 'utf8');
  process.stdout.write(`Wrote ${relDocPath(outPath)}\n`);
}
