import test from 'node:test';
import assert from 'node:assert/strict';

import {
  computeDv,
  cleanRut,
  formatRut,
  parseRut,
  validateRut,
  formatRutLive,
} from '../src/lib/rut.ts';

test('rut: computeDv matches Chilean modulo 11 algorithm', () => {
  assert.equal(computeDv(12345678), '5');
  assert.equal(computeDv(1111111), '4');
  assert.equal(computeDv(11111111), '1');
  assert.equal(computeDv(76123456), '0');
  assert.equal(computeDv(12345679), '3');
  assert.equal(computeDv(1000000), '9');
  assert.equal(computeDv(2000000), '7');
  assert.equal(computeDv(6000000), 'K');
  assert.equal(computeDv(10000013), 'K');
  assert.equal(computeDv(0), '0');
});

test('rut: parseRut accepts valid formats from CRYPTO-04 vectors', () => {
  // Standard format with dots and hyphen
  const r1 = parseRut('12.345.678-5');
  assert.equal(r1.body, 12345678);
  assert.equal(r1.dv, '5');
  assert.equal(r1.formatted, '12.345.678-5');
  assert.equal(r1.bodyDigits, '12345678');
  assert.equal(validateRut('12.345.678-5'), true);

  // Without dots, with hyphen
  const r2 = parseRut('12345678-5');
  assert.equal(r2.formatted, '12.345.678-5');
  assert.equal(validateRut('12345678-5'), true);

  // Without dots or hyphen
  const r3 = parseRut('123456785');
  assert.equal(r3.formatted, '12.345.678-5');
  assert.equal(validateRut('123456785'), true);

  // 7-digit body
  const r4 = parseRut('1.111.111-4');
  assert.equal(r4.body, 1111111);
  assert.equal(r4.formatted, '1.111.111-4');
  assert.equal(r4.bodyDigits, '1111111');
  assert.equal(validateRut('1.111.111-4'), true);

  // 8-digit body with 1
  const r5 = parseRut('11.111.111-1');
  assert.equal(r5.formatted, '11.111.111-1');
  assert.equal(validateRut('11.111.111-1'), true);

  // Company RUT with DV 0
  const r6 = parseRut('76.123.456-0');
  assert.equal(r6.body, 76123456);
  assert.equal(r6.dv, '0');
  assert.equal(r6.formatted, '76.123.456-0');
  assert.equal(validateRut('76.123.456-0'), true);

  // Lowercase 'k' normalized to 'K'
  const r7 = parseRut('6.000.000-k');
  assert.equal(r7.dv, 'K');
  assert.equal(r7.formatted, '6.000.000-K');
  assert.equal(validateRut('6.000.000-k'), true);
});

test('rut: parseRut and validateRut reject invalid check digit', () => {
  assert.throws(() => parseRut('12.345.678-K'), /Dígito verificador inválido/);
  assert.equal(validateRut('12.345.678-K'), false);

  assert.throws(() => parseRut('12.345.678-4'), /Dígito verificador inválido/);
  assert.equal(validateRut('12.345.678-4'), false);
});

test('rut: parseRut rejects empty, invalid characters and out of range', () => {
  assert.throws(() => parseRut(''), /El RUT no puede estar vacío/);
  assert.throws(() => parseRut('   '), /El RUT no puede estar vacío/);
  assert.throws(() => parseRut('12.345.abc-5'), /solo debe contener dígitos/);
  assert.throws(() => parseRut('12.345.678-Z'), /debe ser un número o K/);
  assert.throws(() => parseRut('999.999-1'), /fuera del rango/);
  assert.throws(() => parseRut('100.000.000-4'), /fuera del rango/);
});

test('rut: formatRut canonicalizes input', () => {
  assert.equal(formatRut('12345678-5'), '12.345.678-5');
  assert.equal(formatRut('76123456-0'), '76.123.456-0');
  assert.equal(formatRut('11111114'), '1.111.111-4');
  assert.equal(formatRut(12345678, '5'), '12.345.678-5');
  // Invalid format returns cleaned/uppercased string safely
  assert.equal(formatRut('invalid'), 'INVALID');
});

test('rut: formatRutLive handles incremental typing', () => {
  assert.equal(formatRutLive('1'), '1');
  assert.equal(formatRutLive('12'), '1-2');
  assert.equal(formatRutLive('123456785'), '12.345.678-5');
  assert.equal(formatRutLive('761234560'), '76.123.456-0');
  assert.equal(formatRutLive('6000000k'), '6.000.000-K');
});
