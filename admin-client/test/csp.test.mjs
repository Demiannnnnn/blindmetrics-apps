// admin-client/test/csp.test.mjs — Strict CSP and bundle eval/Function verification (TASK-ADMIN-18)

import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const projectRoot = path.resolve(__dirname, '..');

test('CSP: tauri.conf.json enforces strict production CSP without unsafe-inline or unsafe-eval (RF-ADMIN-18-1)', () => {
  const confPath = path.join(projectRoot, 'src-tauri', 'tauri.conf.json');
  assert.ok(fs.existsSync(confPath), 'tauri.conf.json exists');

  const raw = fs.readFileSync(confPath, 'utf8');
  const conf = JSON.parse(raw);

  const security = conf.app?.security;
  assert.ok(security, 'app.security configuration exists');

  const csp = security.csp;
  assert.ok(typeof csp === 'string' && csp.length > 0, 'security.csp is defined and non-empty');

  // Parse directives
  const directives = Object.fromEntries(
    csp
      .split(';')
      .map((d) => d.trim())
      .filter(Boolean)
      .map((d) => {
        const [name, ...values] = d.split(/\s+/);
        return [name.toLowerCase(), values.map((v) => v.toLowerCase())];
      }),
  );

  // default-src must be 'self' (SEC-06)
  assert.ok(directives['default-src'], "default-src directive must be present");
  assert.ok(
    directives['default-src'].includes("'self'"),
    "default-src must include 'self'",
  );

  // script-src must have 'self' and NEVER contain 'unsafe-inline' or 'unsafe-eval'
  assert.ok(directives['script-src'], "script-src directive must be present");
  assert.ok(
    directives['script-src'].includes("'self'"),
    "script-src must include 'self'",
  );
  assert.equal(
    directives['script-src'].includes("'unsafe-inline'"),
    false,
    "production script-src must NEVER contain 'unsafe-inline'",
  );
  assert.equal(
    directives['script-src'].includes("'unsafe-eval'"),
    false,
    "production script-src must NEVER contain 'unsafe-eval'",
  );

  // No connect-src to external API origins (SEC-06: API communication is Rust-only via IPC)
  if (directives['connect-src']) {
    for (const source of directives['connect-src']) {
      assert.ok(
        source === "'self'" || source === "'none'",
        `connect-src must not point to external origins: found ${source}`,
      );
    }
  }

  // If devCsp is configured, verify it is present for dev while production csp remains strict
  if (security.devCsp) {
    assert.equal(typeof security.devCsp, 'string');
  }
});

test('Bundle: dist/**/*.js contains no eval() or new Function() (RF-ADMIN-18-2)', () => {
  const distDir = path.join(projectRoot, 'dist');
  assert.ok(
    fs.existsSync(distDir),
    'dist directory exists (run npm run build before tests)',
  );

  const jsFiles = [];
  function scanDir(dir) {
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
      const fullPath = path.join(dir, entry.name);
      if (entry.isDirectory()) {
        scanDir(fullPath);
      } else if (entry.isFile() && entry.name.endsWith('.js')) {
        jsFiles.push(fullPath);
      }
    }
  }
  scanDir(distDir);

  assert.ok(
    jsFiles.length > 0,
    'At least one JavaScript bundle must be present in dist/',
  );

  const evalRegex = /\beval\s*\(/;
  const newFunctionRegex = /new\s+Function\s*\(/;

  for (const file of jsFiles) {
    const code = fs.readFileSync(file, 'utf8');
    const relPath = path.relative(projectRoot, file);

    const hasEval = evalRegex.test(code);
    assert.equal(
      hasEval,
      false,
      `Forbidden eval() found in production bundle: ${relPath}`,
    );

    const hasNewFunction = newFunctionRegex.test(code);
    assert.equal(
      hasNewFunction,
      false,
      `Forbidden new Function() found in production bundle: ${relPath}`,
    );
  }
});
