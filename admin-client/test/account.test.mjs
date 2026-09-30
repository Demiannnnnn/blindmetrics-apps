import test from 'node:test';
import assert from 'node:assert/strict';

// Real modules (Node >= 22.18 strips TypeScript types natively).
import { describeError, describeLoginError, errorIs, retryAfterSeconds } from '../src/lib/errors.ts';
import {
  displayUrl,
  isBlocked,
  isSafePortalUrl,
  lockScreenCopy,
  shouldShowTrialBanner,
  trialBannerText,
} from '../src/lib/subscription.ts';
import {
  backupFileIssue,
  keyStateScreen,
  masterPasswordIssue,
  NEW_KEY_PHRASE,
  newKeyConfirmationValid,
  passwordStrength,
  sameFingerprint,
  shortFingerprint,
} from '../src/lib/vault.ts';

test('errors: reason wins over code, local Spanish messages are kept', () => {
  const revokeFirst = { code: 'CONFLICT', message: 'Worker already completed', reason: 'ENROLLMENT_ALREADY_COMPLETED' };
  assert.match(describeError(revokeFirst), /primero revoca el actual/);
  assert.equal(errorIs(revokeFirst, 'ENROLLMENT_ALREADY_COMPLETED'), true);
  assert.equal(errorIs(revokeFirst, 'CONFLICT'), true);
  assert.equal(describeError({ code: 'VALIDATION_ERROR', message: 'La confirmación no coincide.' }), 'La confirmación no coincide.');
  assert.match(describeError({ code: 'BACKUP_KEY_MISMATCH', message: 'Este respaldo corresponde a la llave abcd…' }), /abcd/);
  assert.equal(describeError(undefined, 'fallback'), 'fallback');
  assert.equal(describeError('texto plano'), 'texto plano');
});

test('errors: rate limits expose the wait', () => {
  const err = { code: 'RATE_LIMITED', reason: 'ENROLLMENT_LOCKED', details: { retry_after_seconds: 900 } };
  assert.equal(retryAfterSeconds(err), 900);
  assert.match(describeError({ code: 'RATE_LIMITED', details: { retry_after_seconds: 120 } }), /2 min/);
});

test('errors: failed password login does not reveal which part was wrong', () => {
  const msg = describeLoginError({ code: 'UNAUTHORIZED', message: 'Invalid email or password' });
  assert.match(msg, /Correo o contraseña incorrectos/);
  assert.match(describeLoginError({ code: 'UNAUTHORIZED', reason: 'DEVICE_REVOKED' }), /revocado/);
});

test('subscription: lock screen copy by status and reason (TASK-APP-01)', () => {
  assert.equal(lockScreenCopy('SUSPENDED', 'TRIAL_EXPIRED').title, '⚠️ Período de prueba terminado');
  assert.equal(lockScreenCopy('SUSPENDED', 'SUBSCRIPTION_EXPIRED').body, 'Tu suscripción ha sido suspendida por falta de pago.');
  assert.equal(lockScreenCopy('SUSPENDED', 'SUBSCRIPTION_SUSPENDED').title, '⚠️ Suscripción suspendida');
  assert.equal(lockScreenCopy('CANCELLED', 'SUBSCRIPTION_CANCELLED').title, '⛔ Suscripción cancelada');
  assert.equal(isBlocked({ effective_status: 'SUSPENDED' }), true);
  assert.equal(isBlocked({ effective_status: 'CANCELLED' }), true);
  assert.equal(isBlocked({ effective_status: 'TRIAL' }), false);
  assert.equal(isBlocked(null), false);
});

test('subscription: portal url shown without scheme and validated', () => {
  assert.equal(displayUrl('https://app.tudominio.cl/facturacion/'), 'app.tudominio.cl/facturacion');
  assert.equal(isSafePortalUrl('https://app.tudominio.cl/dashboard/facturacion'), true);
  assert.equal(isSafePortalUrl('http://app.tudominio.cl/f'), false);
  assert.equal(isSafePortalUrl('https://user:pw@app.tudominio.cl/f'), false);
  assert.equal(isSafePortalUrl('javascript:alert(1)'), false);
  assert.equal(isSafePortalUrl(null), false);
});

test('subscription: trial banner only in the last 7 days', () => {
  assert.equal(shouldShowTrialBanner({ effective_status: 'TRIAL', trial_days_left: 7 }), true);
  assert.equal(shouldShowTrialBanner({ effective_status: 'TRIAL', trial_days_left: 8 }), false);
  assert.equal(shouldShowTrialBanner({ effective_status: 'ACTIVE', trial_days_left: 2 }), false);
  assert.equal(trialBannerText(1), 'Te queda 1 día de prueba.');
  assert.equal(trialBannerText(0), 'Tu período de prueba termina hoy.');
});

test('vault: Master Password rules mirror telemetry-crypto-core', () => {
  assert.notEqual(masterPasswordIssue('corta'), null);
  assert.equal(masterPasswordIssue('exactly-12ch'), null);
  assert.equal(masterPasswordIssue('ñññññññññññ1'), null, 'counts characters, not bytes');
  assert.notEqual(masterPasswordIssue('a'.repeat(1025)), null);
  assert.notEqual(masterPasswordIssue('exactly-12ch', 'exactly-12cH'), null, 'case matters');
  // NFC: composed and decomposed ñ are the same password.
  assert.equal(masterPasswordIssue('contraseña-segura', 'contraseña-segura'), null);
  assert.equal(passwordStrength('abc'), 'débil');
  assert.equal(passwordStrength('correct horse battery staple'), 'fuerte');
});

test('vault: key screens and destructive confirmation (TASK-APP-02)', () => {
  assert.equal(keyStateScreen('NO_KEY'), 'CREATE');
  assert.equal(keyStateScreen('MISSING_LOCAL'), 'RECOVER');
  assert.equal(keyStateScreen('MISMATCH'), 'MISMATCH');
  assert.equal(keyStateScreen('LOCAL_ONLY'), 'REGISTER');
  assert.equal(keyStateScreen('OK'), 'READY');
  assert.equal(newKeyConfirmationValid(true, NEW_KEY_PHRASE), true);
  assert.equal(newKeyConfirmationValid(true, ' GENERAR NUEVA LLAVE '), true);
  assert.equal(newKeyConfirmationValid(false, NEW_KEY_PHRASE), false);
  assert.equal(newKeyConfirmationValid(true, 'generar nueva llave'), false);
});

test('vault: fingerprints and backup files', () => {
  assert.equal(shortFingerprint('aaa8fff703b50b2297f4f6e13508f724'), 'aaa8 fff7 03b5 0b22');
  assert.equal(shortFingerprint(null), '—');
  assert.equal(sameFingerprint('ABCD', 'abcd'), true);
  assert.equal(sameFingerprint(null, 'abcd'), false);
  assert.equal(backupFileIssue({ name: 'telemetry_key_acme_2026-09-30.tkb', size: 600 }), null);
  assert.equal(backupFileIssue({ name: 'old.vault', size: 300 }), null);
  assert.notEqual(backupFileIssue({ name: 'foto.png', size: 300 }), null);
  assert.notEqual(backupFileIssue({ name: 'big.tkb', size: 70 * 1024 }), null);
  assert.notEqual(backupFileIssue(null), null);
});
