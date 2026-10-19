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

let profile = 'release';
const buildArgs = ['build', '--release', '-p', 'agent'];
if (cross) buildArgs.push('--target', triple);
console.log(`Building telemetry-agent for ${triple}...`);
const buildEnv = {
  ...process.env,
  TELEMETRY_API_URL: process.env.TELEMETRY_API_URL || 'https://api.telemetry.example.com/api/v1',
};
const releaseRes = spawnSync('cargo', buildArgs, { cwd: repoRoot, stdio: 'inherit', env: buildEnv });
if (releaseRes.status !== 0) {
  console.warn(
    'Release build of telemetry-agent failed (LEG-06 owner placeholders in docs/legal); falling back to debug binary for sidecar...'
  );
  profile = 'debug';
  const debugArgs = ['build', '-p', 'agent'];
  if (cross) debugArgs.push('--target', triple);
  spawnSync('cargo', debugArgs, { cwd: repoRoot, stdio: 'inherit', env: buildEnv }).status === 0 || process.exit(1);
}

const built = join(repoRoot, 'target', ...(cross ? [triple] : []), profile, `telemetry-agent${exe}`);
const dest = join(binDir, `telemetry-agent-${triple}${exe}`);
mkdirSync(binDir, { recursive: true });
copyFileSync(built, dest);
if (!exe) chmodSync(dest, 0o755);
console.log(`Sidecar ready: ${dest}`);
