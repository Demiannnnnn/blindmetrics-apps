import test from 'node:test';
import assert from 'node:assert/strict';
import {
  normalizeCode,
  formatSafetyCode,
  getCategoryStateDetails,
  countSelectedCategories,
  getRunModeBanner,
  formatUptime,
  formatBytes,
  qrImageFileIssue,
  isEnrollmentTampered,
  isTamperedError,
  extractCodeFromQrPayload,
} from '../src/lib/helpers.ts';

test('normalizeCode: strips invalid characters and groups into XXXX-XXXX-XXXX', () => {
  assert.equal(normalizeCode('a68k2pzspc4h'), 'A68K-2PZS-PC4H');
  assert.equal(normalizeCode('a68k-2pzs-pc4h'), 'A68K-2PZS-PC4H');
  assert.equal(normalizeCode('  a68k 2pzs pc4h '), 'A68K-2PZS-PC4H');
  assert.equal(normalizeCode('a68k!2pzs@pc4h#extra'), 'A68K-2PZS-PC4H');
  assert.equal(normalizeCode(''), '');
  assert.equal(normalizeCode('abc'), 'ABC');
  assert.equal(normalizeCode('abcd12'), 'ABCD-12');
});

test('formatSafetyCode: groups 12 decimal digits into 4-digit blocks', () => {
  assert.equal(formatSafetyCode('123456789012'), '1234 5678 9012');
  assert.equal(formatSafetyCode('1234 5678 9012'), '1234 5678 9012');
  assert.equal(formatSafetyCode('1234567890129999'), '1234 5678 9012');
  assert.equal(formatSafetyCode('1234'), '1234');
  assert.equal(formatSafetyCode('12345678'), '1234 5678');
});

test('getCategoryStateDetails: maps CategoryState correctly', () => {
  assert.deepEqual(getCategoryStateDetails('ACTIVE'), { label: 'Activa', tone: 'active' });
  assert.deepEqual(getCategoryStateDetails('NOT_ACCEPTED'), { label: 'No aceptada', tone: 'muted' });
  assert.deepEqual(getCategoryStateDetails('NOT_REQUESTED'), { label: 'No solicitada', tone: 'muted' });
  assert.deepEqual(getCategoryStateDetails('SLEEPING'), { label: 'Durmiendo', tone: 'warning' });
  assert.deepEqual(getCategoryStateDetails('UNAVAILABLE'), { label: 'No disponible', tone: 'danger' });
});

test('countSelectedCategories: accurately counts true flags', () => {
  assert.equal(countSelectedCategories(null), 0);
  assert.equal(countSelectedCategories(undefined), 0);
  assert.equal(
    countSelectedCategories({
      system_activity: false,
      network_activity: false,
      productivity_basic: false,
      productivity_screenshots: false,
      file_activity: false,
      location: false,
    }),
    0
  );
  assert.equal(
    countSelectedCategories({
      system_activity: true,
      network_activity: true,
      productivity_basic: false,
      productivity_screenshots: false,
      file_activity: true,
      location: false,
    }),
    3
  );
  assert.equal(
    countSelectedCategories({
      system_activity: true,
      network_activity: true,
      productivity_basic: true,
      productivity_screenshots: true,
      file_activity: true,
      location: true,
    }),
    6
  );
});

test('getRunModeBanner: provides appropriate banners according to run_mode', () => {
  assert.equal(getRunModeBanner('COLLECTING'), null);
  assert.equal(getRunModeBanner('UNENROLLED'), null);

  const paused = getRunModeBanner('PAUSED');
  assert.notEqual(paused, null);
  assert.equal(paused?.title, 'Telemetría en pausa');
  assert.equal(paused?.tone, 'warning');

  const suspended = getRunModeBanner('EMPLOYER_SUSPENDED');
  assert.notEqual(suspended, null);
  assert.equal(suspended?.title, 'Suscripción suspendida');
  assert.equal(suspended?.tone, 'warning');

  const halted = getRunModeBanner('HALTED', 'Clave corrupta');
  assert.notEqual(halted, null);
  assert.equal(halted?.title, 'Telemetría detenida por seguridad');
  assert.equal(halted?.message, 'Clave corrupta');
  assert.equal(halted?.tone, 'danger');

  const awaiting = getRunModeBanner('AWAITING_CONSENT');
  assert.notEqual(awaiting, null);
  assert.equal(awaiting?.title, 'Solicitud de telemetría pendiente');
  assert.equal(awaiting?.tone, 'info');
});

test('formatUptime and formatBytes format appropriately', () => {
  assert.equal(formatUptime(30), '30 s');
  assert.equal(formatUptime(120), '2 m');
  assert.equal(formatUptime(3660), '1 h 1 m');
  assert.equal(formatUptime(90000), '1 d 1 h');

  assert.equal(formatBytes(500), '500 B');
  assert.equal(formatBytes(2048), '2.0 KB');
  assert.equal(formatBytes(5 * 1024 * 1024), '5.0 MB');
});

test('qrImageFileIssue: accepts valid PNG, JPEG, WebP under 10 MiB', () => {
  assert.equal(qrImageFileIssue({ size: 1024, type: 'image/png', name: 'qr.png' }), null);
  assert.equal(qrImageFileIssue({ size: 5000, type: 'image/jpeg', name: 'qr.jpg' }), null);
  assert.equal(qrImageFileIssue({ size: 2000, type: 'image/webp', name: 'qr.webp' }), null);
  assert.equal(qrImageFileIssue({ size: 10 * 1024 * 1024, type: 'image/png' }), null);
});

test('qrImageFileIssue: rejects empty files, oversized files and unsupported types', () => {
  // Empty
  const empty = qrImageFileIssue({ size: 0, type: 'image/png', name: 'qr.png' });
  assert.notEqual(empty, null);
  assert.match(empty || '', /vacío/i);

  // Oversized (> 10 MiB)
  const oversized = qrImageFileIssue({ size: 10 * 1024 * 1024 + 1, type: 'image/png', name: 'qr.png' });
  assert.notEqual(oversized, null);
  assert.match(oversized || '', /10 MiB/i);

  // Unsupported MIME / extension
  const svg = qrImageFileIssue({ size: 1024, type: 'image/svg+xml', name: 'code.svg' });
  assert.notEqual(svg, null);
  assert.match(svg || '', /no soportado/i);

  const pdf = qrImageFileIssue({ size: 1024, type: 'application/pdf', name: 'doc.pdf' });
  assert.notEqual(pdf, null);
  assert.match(pdf || '', /no soportado/i);

  const txt = qrImageFileIssue({ size: 1024, type: 'text/plain', name: 'info.txt' });
  assert.notEqual(txt, null);
  assert.match(txt || '', /no soportado/i);
});

test('isEnrollmentTampered: detects ENROLLMENT_TAMPERED code to trigger tamper alert E3', () => {
  assert.equal(isEnrollmentTampered({ code: 'ENROLLMENT_TAMPERED' }), true);
  assert.equal(isTamperedError({ code: 'ENROLLMENT_TAMPERED' }), true);
  assert.equal(isEnrollmentTampered({ code: 'ENROLLMENT_ERROR' }), false);
  assert.equal(isEnrollmentTampered({ code: 'API_ERROR' }), false);
  assert.equal(isEnrollmentTampered(null), false);
  assert.equal(isEnrollmentTampered(undefined), false);
  assert.equal(isEnrollmentTampered('ENROLLMENT_TAMPERED'), false);
});

test('extractCodeFromQrPayload: extracts normalized 12-char code from JSON or base64 payload', () => {
  const jsonPayload = JSON.stringify({
    t: 'telemetry-enroll',
    v: 1,
    code: 'a68k-2pzs-pc4h',
    commitment: 'a'.repeat(64),
  });
  assert.equal(extractCodeFromQrPayload(jsonPayload), 'A68K-2PZS-PC4H');

  const b64Payload = Buffer.from(jsonPayload).toString('base64');
  assert.equal(extractCodeFromQrPayload(b64Payload), 'A68K-2PZS-PC4H');

  assert.equal(extractCodeFromQrPayload('invalid json'), null);
  assert.equal(extractCodeFromQrPayload(JSON.stringify({ t: 'other' })), null);
});
