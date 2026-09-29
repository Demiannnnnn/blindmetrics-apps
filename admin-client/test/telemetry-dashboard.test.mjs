import test from 'node:test';
import assert from 'node:assert/strict';

// Helper replication for node test environment (matching tauri-bridge logic)
function sanitizeHtmlText(str) {
  return str
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
    .replace(/'/g, '&#039;');
}

function validateFqdnPrivacy(fqdn) {
  const hasForbiddenChars = /[\/?:#@]/.test(fqdn);
  let clean = fqdn.trim().toLowerCase();
  clean = clean.replace(/^[a-z]+:\/\//, '');
  clean = clean.split('@').pop() || clean;
  clean = clean.split('/')[0];
  clean = clean.split('?')[0];
  clean = clean.split('#')[0];
  clean = clean.split(':')[0];
  return {
    isValid: !hasForbiddenChars && clean === fqdn.trim().toLowerCase(),
    sanitized: clean,
  };
}

function getTimeRangeIso(filter) {
  const now = new Date();
  if (filter.preset === 'today') {
    const start = new Date(now.getFullYear(), now.getMonth(), now.getDate(), 0, 0, 0);
    return { from: start.toISOString(), to: now.toISOString() };
  } else if (filter.preset === 'yesterday') {
    const start = new Date(now.getFullYear(), now.getMonth(), now.getDate() - 1, 0, 0, 0);
    const end = new Date(now.getFullYear(), now.getMonth(), now.getDate() - 1, 23, 59, 59, 999);
    return { from: start.toISOString(), to: end.toISOString() };
  } else if (filter.preset === '7days') {
    const start = new Date(now.getTime() - 7 * 24 * 60 * 60 * 1000);
    return { from: start.toISOString(), to: now.toISOString() };
  } else {
    return { from: filter.from, to: filter.to };
  }
}

test('validateFqdnPrivacy: canonical FQDN is valid and untouched', () => {
  const r1 = validateFqdnPrivacy('github.com');
  assert.equal(r1.isValid, true);
  assert.equal(r1.sanitized, 'github.com');

  const r2 = validateFqdnPrivacy('api.subdomain.service.co.uk');
  assert.equal(r2.isValid, true);
  assert.equal(r2.sanitized, 'api.subdomain.service.co.uk');
});

test('validateFqdnPrivacy: paths, queries, ports and credentials are flagged and stripped', () => {
  const pathTest = validateFqdnPrivacy('github.com/owner/repo');
  assert.equal(pathTest.isValid, false);
  assert.equal(pathTest.sanitized, 'github.com');

  const queryTest = validateFqdnPrivacy('google.com/search?q=confidential+data');
  assert.equal(queryTest.isValid, false);
  assert.equal(queryTest.sanitized, 'google.com');

  const portTest = validateFqdnPrivacy('internal.corp:8443');
  assert.equal(portTest.isValid, false);
  assert.equal(portTest.sanitized, 'internal.corp');

  const credsTest = validateFqdnPrivacy('user:password@corp.internal');
  assert.equal(credsTest.isValid, false);
  assert.equal(credsTest.sanitized, 'corp.internal');
});

test('getTimeRangeIso: calculates correct date boundaries', () => {
  const today = getTimeRangeIso({ preset: 'today' });
  assert.ok(today.from);
  assert.ok(today.to);
  assert.ok(new Date(today.from) <= new Date(today.to));

  const yesterday = getTimeRangeIso({ preset: 'yesterday' });
  assert.ok(yesterday.from);
  assert.ok(yesterday.to);
  assert.ok(new Date(yesterday.from) < new Date(today.from));

  const sevenDays = getTimeRangeIso({ preset: '7days' });
  const diffHours = (new Date(sevenDays.to) - new Date(sevenDays.from)) / (1000 * 60 * 60);
  assert.ok(diffHours >= 167 && diffHours <= 169); // ~168 hours in 7 days
});

test('sanitizeHtmlText: prevents script injection in window titles and filenames', () => {
  const malicious = '<script>alert("XSS")</script>';
  const sanitized = sanitizeHtmlText(malicious);
  assert.equal(sanitized, '&lt;script&gt;alert(&quot;XSS&quot;)&lt;/script&gt;');
  assert.ok(!sanitized.includes('<script>'));
});

test('screenshot_blur_toggle logic test', () => {
  let privacyBlur = true;
  const getImgClass = (blur) => `w-full h-full object-cover transition ${blur ? 'blur-md select-none' : ''}`;

  assert.ok(getImgClass(privacyBlur).includes('blur-md'));
  privacyBlur = !privacyBlur;
  assert.ok(!getImgClass(privacyBlur).includes('blur-md'));
});
