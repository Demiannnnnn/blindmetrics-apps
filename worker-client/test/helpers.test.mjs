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
