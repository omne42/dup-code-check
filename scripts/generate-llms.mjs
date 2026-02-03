import fs from 'node:fs';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const docsRoot = path.join(repoRoot, 'docs');
const publicDir = path.join(docsRoot, 'public');

const EXCLUDED_DIRS = new Set(['_book', '.vitepress', 'public']);

function isMarkdownFile(p) {
  return p.endsWith('.md');
}

function isReadmeFile(p) {
  const base = path.basename(p).toLowerCase();
  return base === 'readme.md' || (base.startsWith('readme.') && base.endsWith('.md'));
}

function linkToRelDocs(link) {
  if (link === '/') return 'index.md';
  const cleaned = link.split(/[?#]/)[0];
  const trimmed = cleaned.replace(/^\/+/, '').replace(/\/+$/, '');
  if (!trimmed) return null;
  if (trimmed.includes('://')) return null;
  return `${trimmed}.md`;
}

function stripFrontmatter(markdown) {
  const match = markdown.match(/^(?:\uFEFF)?---\r?\n[\s\S]*?\r?\n---\r?\n/);
  if (!match) return markdown;
  return markdown.slice(match[0].length);
}

function extractBalanced(source, startIndex, openChar, closeChar) {
  let depth = 0;
  let inString = null;
  let escape = false;
  let inLineComment = false;
  let inBlockComment = false;

  for (let i = startIndex; i < source.length; i += 1) {
    const ch = source[i];
    const next = source[i + 1] ?? '';

    if (inLineComment) {
      if (ch === '\n') inLineComment = false;
      continue;
    }
    if (inBlockComment) {
      if (ch === '*' && next === '/') {
        inBlockComment = false;
        i += 1;
      }
      continue;
    }
    if (inString) {
      if (escape) {
        escape = false;
        continue;
      }
      if (ch === '\\') {
        escape = true;
        continue;
      }
      if (ch === inString) {
        inString = null;
      }
      continue;
    }

    if (ch === '/' && next === '/') {
      inLineComment = true;
      i += 1;
      continue;
    }
    if (ch === '/' && next === '*') {
      inBlockComment = true;
      i += 1;
      continue;
    }
    if (ch === "'" || ch === '"' || ch === '`') {
      inString = ch;
      continue;
    }

    if (ch === openChar) {
      depth += 1;
      continue;
    }
    if (ch === closeChar) {
      depth -= 1;
      if (depth === 0) {
        return source.slice(startIndex, i + 1);
      }
      continue;
    }
  }

  return null;
}

function extractSidebarArrayFromVitepressConfig(text) {
  const idx = text.indexOf('sidebar');
  if (idx === -1) return null;

  const colon = text.indexOf(':', idx);
  if (colon === -1) return null;

  const start = text.indexOf('[', colon);
  if (start === -1) return null;

  return extractBalanced(text, start, '[', ']');
}

function loadDocOrderFromVitepressConfig() {
  const configPath = path.join(docsRoot, '.vitepress', 'config.mts');
  try {
    const text = fs.readFileSync(configPath, 'utf8');
    const sidebar = extractSidebarArrayFromVitepressConfig(text);
    if (!sidebar) return null;

    const english = [];
    const zh = [];
    const seenEn = new Set();
    const seenZh = new Set();

    function add(relDocs) {
      if (isZh(relDocs)) {
        if (seenZh.has(relDocs)) return;
        seenZh.add(relDocs);
        zh.push(relDocs);
      } else {
        if (seenEn.has(relDocs)) return;
        seenEn.add(relDocs);
        english.push(relDocs);
      }
    }

    if (fs.existsSync(path.join(docsRoot, 'index.md'))) add('index.md');
    if (fs.existsSync(path.join(docsRoot, 'index.zh-CN.md'))) add('index.zh-CN.md');

    const re = /link:\s*['"]([^'"]+)['"]/g;
    for (const m of sidebar.matchAll(re)) {
      const link = m[1];
      if (typeof link !== 'string' || !link.startsWith('/')) continue;
      const relDocs = linkToRelDocs(link);
      if (!relDocs) continue;
      add(relDocs);
    }

    const out = [...english, ...zh];
    return out.length > 0 ? out : null;
  } catch {
    return null;
  }
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

function header(bundleLabel, lang) {
  let out = '';
  if (lang === 'zh') {
    out += '# dup-code-check 文档（llms 合集）\n';
    out += '\n';
    out += '本文件为自动生成的纯文本文档合集，用于离线阅读或 LLM 上下文注入。\n';
    out += '\n';
    out += `合集: ${bundleLabel}\n`;
    out += `生成时间: ${new Date().toISOString()}\n`;
    out += '\n';
    out += '提示词模板:\n';
    out += '\n';
    out += '```text\n';
    out += '文档:\n';
    out += '{把文档内容粘贴到这里}\n';
    out += '---\n';
    out += '基于上述文档，回答下面的问题：\n';
    out += '{你的问题}\n';
    out += '```\n';
    out += '\n';
  } else {
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
  }
  return out;
}

if (!fs.existsSync(docsRoot)) {
  process.stderr.write(
    `docs root not found: ${docsRoot}\n` +
      'This script is intended to run from the git repository.\n' +
      'If you installed dup-code-check from npm, the docs site is not shipped in the package.\n'
  );
  process.exit(1);
}

fs.mkdirSync(publicDir, { recursive: true });

const docOrder = loadDocOrderFromVitepressConfig();
const strict = process.env.LLMS_STRICT === '1';
if (!docOrder) {
  const msg =
    'llms: warning: failed to derive ordering from VitePress config; falling back to lexicographic order.\n' +
    'llms: set LLMS_STRICT=1 to treat this as an error.\n';
  process.stderr.write(msg);
  if (strict) process.exit(1);
}
const orderIndex = new Map();
if (docOrder) {
  for (let i = 0; i < docOrder.length; i += 1) {
    orderIndex.set(docOrder[i], i);
  }
}

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

    const orderA = orderIndex.get(a.relDocs) ?? Number.POSITIVE_INFINITY;
    const orderB = orderIndex.get(b.relDocs) ?? Number.POSITIVE_INFINITY;
    const order = orderA - orderB;
    if (order !== 0) return order;

    return a.rel.localeCompare(b.rel);
  });

if (docOrder) {
  const known = new Set(files.map((f) => f.relDocs));

  const missing = docOrder.filter((p) => !known.has(p));
  if (missing.length > 0) {
    process.stderr.write('llms: warning: docs sidebar references missing pages:\n');
    for (const p of missing) {
      process.stderr.write(`- ${p}\n`);
    }
    if (strict) process.exit(1);
  }

  const unknown = files.filter((f) => !orderIndex.has(f.relDocs)).map((f) => f.relDocs);
  if (unknown.length > 0) {
    process.stderr.write('llms: warning: docs pages not referenced in VitePress sidebar:\n');
    for (const p of unknown) {
      process.stderr.write(`- ${p}\n`);
    }
    process.stderr.write(
      'llms: those pages will be appended after ordered pages; add them to the sidebar to control ordering.\n'
    );
    if (strict) process.exit(1);
  }
}

const outputs = [
  {
    filename: 'llms.txt',
    bundleLabel: 'Combined (EN + 中文)',
    lang: 'en',
    filter: () => true,
  },
  {
    filename: 'llms.en.txt',
    bundleLabel: 'English only',
    lang: 'en',
    filter: (relDocs) => !isZh(relDocs),
  },
  {
    filename: 'llms.zh-CN.txt',
    bundleLabel: '中文 only',
    lang: 'zh',
    filter: (relDocs) => isZh(relDocs),
  },
];

for (const o of outputs) {
  const outPath = path.join(publicDir, o.filename);
  let out = header(o.bundleLabel, o.lang);

  for (const file of files) {
    if (!o.filter(file.relDocs)) continue;
    const content = stripFrontmatter(fs.readFileSync(file.abs, 'utf8'));
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
