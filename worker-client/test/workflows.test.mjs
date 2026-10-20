import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, existsSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const repoRoot = join(here, '..', '..');
const ciWorkflowPath = join(repoRoot, '.github', 'workflows', 'ci.yml');
const releaseWorkflowPath = join(repoRoot, '.github', 'workflows', 'release.yml');

function validateWorkflowSyntax(filePath) {
  assert.ok(existsSync(filePath), `Workflow file must exist: ${filePath}`);
  const content = readFileSync(filePath, 'utf8');

  // Must not contain tab indentation (YAML standard)
  assert.ok(!content.includes('\t'), `${filePath} must not contain tabs`);

  // Must not have unclosed GitHub Actions expressions ${{ ... }}
  const openCount = (content.match(/\$\{\{/g) || []).length;
  const closeCount = (content.match(/\}\}/g) || []).length;
  assert.equal(
    openCount,
    closeCount,
    `${filePath} has mismatched expression braces: ${openCount} open vs ${closeCount} close`
  );

  // Must declare permissions
  assert.match(content, /permissions:\s*\n/m, `${filePath} must explicitly declare permissions`);

  return content;
}

test('CI workflow (.github/workflows/ci.yml) syntax and permissions', () => {
  const content = validateWorkflowSyntax(ciWorkflowPath);
  assert.match(content, /permissions:\s*\n\s+contents:\s*read/m);
});

test('CI workflow triggers on pull_request, main, develop-AGENTS and workflow_dispatch', () => {
  const content = readFileSync(ciWorkflowPath, 'utf8');
  assert.match(content, /pull_request:/m);
  assert.match(content, /push:\s*\n\s+branches:\s*\n\s+- main\s*\n\s+- develop-AGENTS/m);
  assert.match(content, /workflow_dispatch:/m);
});

test('CI workflow executes all 5 AGENTS.md §9 quality gate commands', () => {
  const content = readFileSync(ciWorkflowPath, 'utf8');
  const requiredCommands = [
    'cargo fmt --all --check',
    'cargo clippy --workspace --all-targets -- -D warnings',
    'cargo test --workspace',
    'cd admin-client && npm ci && npm run build && npm test',
    'cd worker-client && npm ci && npm run build && npm test',
  ];

  for (const cmd of requiredCommands) {
    assert.ok(content.includes(cmd), `CI workflow must execute command: ${cmd}`);
  }
});

test('Release workflow (.github/workflows/release.yml) syntax and permissions', () => {
  const content = validateWorkflowSyntax(releaseWorkflowPath);
  assert.match(content, /permissions:\s*\n\s+contents:\s*read/m);
  assert.match(content, /permissions:\s*\n\s+contents:\s*write/m);
});

test('Release workflow triggers on v* tags and workflow_dispatch', () => {
  const content = readFileSync(releaseWorkflowPath, 'utf8');
  assert.match(content, /tags:\s*\n\s+- ['"]?v\*/m);
  assert.match(content, /workflow_dispatch:/m);
  assert.match(content, /inputs:\s*\n\s+tag:/m);
});

test('Release workflow matrix covers macOS, Windows, and Ubuntu for both apps', () => {
  const content = readFileSync(releaseWorkflowPath, 'utf8');
  assert.match(content, /macos-latest/m);
  assert.match(content, /windows-latest/m);
  assert.match(content, /ubuntu-24\.04/m);
  assert.match(content, /aarch64-apple-darwin/m);
  assert.match(content, /x86_64-apple-darwin/m);
  assert.match(content, /x86_64-pc-windows-msvc/m);
  assert.match(content, /x86_64-unknown-linux-gnu/m);

  assert.ok(content.includes('admin-client'), 'Release workflow must build admin-client');
  assert.ok(content.includes('worker-client'), 'Release workflow must build worker-client');
});

test('Release workflow invokes npm run tauri build on both apps', () => {
  const content = readFileSync(releaseWorkflowPath, 'utf8');
  assert.match(content, /cd admin-client[\s\S]*?npm ci[\s\S]*?npm run tauri build/m);
  assert.match(content, /cd worker-client[\s\S]*?npm ci[\s\S]*?npm run tauri build/m);
});

test('Release workflow generates SHA256SUMS.txt and creates a draft release', () => {
  const content = readFileSync(releaseWorkflowPath, 'utf8');
  assert.ok(content.includes('SHA256SUMS.txt'), 'Workflow must generate SHA256SUMS.txt');
  assert.ok(content.includes('sha256sum'), 'Workflow must use sha256sum');
  assert.match(content, /draft:\s*true|--draft/m, 'Workflow must publish release as draft');
});

test('Release and CI workflows do not contain hardcoded secrets or fake API URLs', () => {
  const ciContent = readFileSync(ciWorkflowPath, 'utf8');
  const releaseContent = readFileSync(releaseWorkflowPath, 'utf8');

  for (const [name, text] of [['ci.yml', ciContent], ['release.yml', releaseContent]]) {
    assert.ok(!text.includes('BEGIN PRIVATE KEY'), `${name} must not contain embedded private keys`);
    assert.ok(!text.includes('BEGIN CERTIFICATE'), `${name} must not contain embedded certificates`);
    assert.ok(!text.includes('api.telemetry.example.com'), `${name} must not contain fake example URLs`);
  }
});
