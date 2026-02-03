import { spawnSync } from 'node:child_process';
import path from 'node:path';
import process from 'node:process';
import { fileURLToPath } from 'node:url';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

function isGlobalInstall() {
  return (
    process.env.npm_config_global === 'true' ||
    process.env.npm_config_location === 'global'
  );
}

if (process.env.DUP_CODE_CHECK_SKIP_BUILD === '1') {
  process.stdout.write('Skipping Rust binary build (DUP_CODE_CHECK_SKIP_BUILD=1)\n');
  process.exit(0);
}

const buildOnInstall = process.env.DUP_CODE_CHECK_BUILD_ON_INSTALL === '1' || isGlobalInstall();
if (!buildOnInstall) {
  process.stdout.write(
    'Skipping Rust binary build during postinstall (will build on first run).\n' +
      'To build during install: DUP_CODE_CHECK_BUILD_ON_INSTALL=1 npm install\n' +
      'To skip all builds: DUP_CODE_CHECK_SKIP_BUILD=1 npm install\n'
  );
  process.exit(0);
}

const buildScript = path.join(repoRoot, 'scripts', 'build-binary.mjs');
const res = spawnSync(process.execPath, [buildScript], { cwd: repoRoot, stdio: 'inherit' });
if (res.error) {
  process.stderr.write(`postinstall: failed to run build script\n${res.error}\n`);
  process.exit(1);
}
process.exit(res.status ?? 1);

