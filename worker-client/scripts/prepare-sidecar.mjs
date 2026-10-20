// Builds the worker service (`telemetry-agent`, crate `agent`) in release mode and copies it to
// `src-tauri/binaries/telemetry-agent-<target-triple>[.exe]`, the name Tauri's `bundle.externalBin`
// requires (RF-AGENT-11-2). Run through `npm run sidecar` (also part of `beforeBuildCommand`).
import { spawnSync } from 'node:child_process';
import { copyFileSync, chmodSync, mkdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const repoRoot = join(here, '..', '..');
const binDir = join(here, '..', 'src-tauri', 'binaries');

function run(cmd, args) {
  const res = spawnSync(cmd, args, { cwd: repoRoot, stdio: ['ignore', 'pipe', 'inherit'], encoding: 'utf8' });
  if (res.status !== 0) {
    console.error(`${cmd} ${args.join(' ')} failed`);
    process.exit(res.status ?? 1);
  }
  return res.stdout;
}

const hostTriple = /^host: (.+)$/m.exec(run('rustc', ['-vV']))?.[1];
const triple = process.env.TAURI_ENV_TARGET_TRIPLE || hostTriple;
if (!triple) {
  console.error('Could not determine the target triple');
  process.exit(1);
}
const cross = triple !== hostTriple;
const exe = triple.includes('windows') ? '.exe' : '';

const isDebug = process.env.TAURI_ENV_DEBUG === 'true' || process.argv.includes('--debug');
const profile = isDebug ? 'debug' : 'release';

const buildArgs = ['build', '-p', 'agent'];
if (!isDebug) {
  buildArgs.splice(1, 0, '--release');
}
if (cross) buildArgs.push('--target', triple);
console.log(`Building telemetry-agent (${profile}) for ${triple}...`);
// RF-INT-07-3: In release mode pass process.env as-is so service/build.rs enforces
// explicit https:// TELEMETRY_API_URL (SEC-10) and checks legal placeholders (LEG-06).
const buildEnv = isDebug
  ? { ...process.env, TELEMETRY_API_URL: process.env.TELEMETRY_API_URL || 'http://localhost:8080/api/v1' }
  : process.env;
const res = spawnSync('cargo', buildArgs, { cwd: repoRoot, stdio: 'inherit', env: buildEnv });
if (res.status !== 0) {
  console.error(`cargo ${buildArgs.join(' ')} failed with exit status ${res.status}`);
  process.exit(res.status ?? 1);
}

const built = join(repoRoot, 'target', ...(cross ? [triple] : []), profile, `telemetry-agent${exe}`);
const dest = join(binDir, `telemetry-agent-${triple}${exe}`);
mkdirSync(binDir, { recursive: true });
copyFileSync(built, dest);
if (!exe) chmodSync(dest, 0o755);
console.log(`Sidecar ready: ${dest}`);
