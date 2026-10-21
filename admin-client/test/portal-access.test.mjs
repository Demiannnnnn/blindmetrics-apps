import test from 'node:test';
import assert from 'node:assert/strict';

// Imports the real module (Node >= 22.18 strips TypeScript types natively).
import {
  PORTAL_ACCESS_DAYS,
  formatPortalAccessUntil,
  normalizePortalAccessEmail,
  portalAccessEmailError,
  portalAccessErrorMessage,
  portalAccessState,
} from '../src/lib/portalAccess.ts';

test('portal access: lasts 90 days (ATTENDANCE.md §11.5)', () => {
  assert.equal(PORTAL_ACCESS_DAYS, 90);
});

test('portal access: email validation mirrors the Rust core', () => {
  assert.equal(portalAccessEmailError('ex.ana@correo.cl'), null);
  assert.equal(portalAccessEmailError('  ex.ana@correo.cl  '), null);
  for (const bad of ['', '   ', 'sin-arroba', 'a@b', '@correo.cl', 'con espacio@correo.cl', 'a@@correo.cl', 'a@.cl', 'a@correo.']) {
    assert.notEqual(portalAccessEmailError(bad), null, bad);
  }
  assert.notEqual(portalAccessEmailError(`${'a'.repeat(250)}@x.cl`), null);
});

test('portal access: email is trimmed and lower-cased before sending', () => {
  assert.equal(normalizePortalAccessEmail('  Ex.Ana@Correo.CL '), 'ex.ana@correo.cl');
});

test('portal access: enabled only while the deadline is in the future', () => {
  const now = new Date('2026-10-21T12:00:00Z');
  assert.deepEqual(portalAccessState(null, now), { enabled: false, until: null });
  assert.deepEqual(portalAccessState(undefined, now), { enabled: false, until: null });
  assert.deepEqual(portalAccessState('no-es-fecha', now), { enabled: false, until: null });
  const future = portalAccessState('2027-01-19T15:00:00Z', now);
  assert.equal(future.enabled, true);
  assert.equal(future.until?.toISOString(), '2027-01-19T15:00:00.000Z');
  assert.equal(portalAccessState('2026-10-21T12:00:00Z', now).enabled, false);
  assert.equal(portalAccessState('2026-10-01T00:00:00Z', now).enabled, false);
});

test('portal access: deadline shown in the worker time zone (es-CL)', () => {
  // Summer time in Chile (UTC-3) on 2027-01-19.
  assert.equal(formatPortalAccessUntil('2027-01-19T15:00:00Z', 'America/Santiago'), '19/01/2027 12:00');
  // Crossing midnight changes the shown day.
  assert.equal(formatPortalAccessUntil('2027-01-20T02:30:00Z', 'America/Santiago'), '19/01/2027 23:30');
  assert.equal(formatPortalAccessUntil('2027-01-20T02:30:00Z', 'UTC'), '20/01/2027 02:30');
  // Missing or unknown zone falls back to America/Santiago.
  assert.equal(formatPortalAccessUntil('2027-01-19T15:00:00Z', null), '19/01/2027 12:00');
  assert.equal(formatPortalAccessUntil('2027-01-19T15:00:00Z', 'Nada/Zona'), '19/01/2027 12:00');
  assert.equal(formatPortalAccessUntil('basura', 'America/Santiago'), 'basura');
});

test('portal access: Spanish messages for 400 / 404 / 409', () => {
  assert.match(portalAccessErrorMessage({ code: 'VALIDATION_ERROR', message: 'Worker is not REVOKED' }), /ex-trabajadores/);
  assert.match(portalAccessErrorMessage({ code: 'NOT_FOUND', message: 'x' }), /no existe/);
  assert.match(portalAccessErrorMessage({ code: 'CONFLICT', message: 'x' }), /ya lo usa otro trabajador/);
  assert.equal(portalAccessErrorMessage({ code: 'NETWORK_ERROR' }), null);
  assert.equal(portalAccessErrorMessage('texto'), null);
  assert.equal(portalAccessErrorMessage(null), null);
});
