'use strict';

const fs = require('node:fs');
const path = require('node:path');

function loadNativeBinding() {
  const candidates = [
    path.join(__dirname, 'code_checker.node'),
    path.join(__dirname, 'index.node'),
    path.join(__dirname, 'dist', 'code_checker.node'),
    path.join(__dirname, 'dist', 'index.node')
  ];

  for (const candidate of candidates) {
    if (fs.existsSync(candidate)) {
      return require(candidate);
    }
  }

  const searched = candidates.map((p) => `- ${p}`).join('\n');
  throw new Error(
    `Native binding not found.\n` +
      `Build it first:\n` +
      `  npm run build\n` +
      `Searched:\n${searched}\n`
  );
}

const native = loadNativeBinding();

module.exports = {
  findDuplicateFiles: native.findDuplicateFiles
};

