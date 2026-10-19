// admin-client/test/correction-timezone.test.mjs — Timezone conversion & round-trip tests (TASK-ADMIN-18)

import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  zonedLocalToUtcIso,
  formatTimezoneLabel,
  getDateStringInTimezone,
  formatR38Time,
} from '../src/lib/attendance.ts';

test('zonedLocalToUtcIso: America/Santiago in winter (UTC-4) and summer (UTC-3)', () => {
  // Winter: June 15, 2026 is standard time (UTC-4)
  // Local 09:00:00 -> 13:00:00Z UTC
  const winterUtc = zonedLocalToUtcIso('2026-06-15', '09:00:00', 'America/Santiago');
  assert.equal(winterUtc, '2026-06-15T13:00:00Z');

  // Summer: October 11, 2026 is daylight saving time (UTC-3)
  // Local 09:00:00 -> 12:00:00Z UTC
  const summerUtc = zonedLocalToUtcIso('2026-10-11', '09:00:00', 'America/Santiago');
  assert.equal(summerUtc, '2026-10-11T12:00:00Z');
});

test('zonedLocalToUtcIso: day of DST change forward (spring forward) rejects nonexistent hour with clear es-CL error', () => {
  // In Chile, DST begins on the first Saturday/Sunday of September (2026-09-06 at 00:00).
  // Clocks jump from 24:00 (00:00) to 01:00:00.
  // 00:30:00 on 2026-09-06 does not exist.
  assert.throws(
    () => {
      zonedLocalToUtcIso('2026-09-06', '00:30:00', 'America/Santiago');
    },
    (err) => {
      assert.ok(err instanceof Error);
      assert.match(err.message, /no existe.*cambio de horario de verano/i);
      return true;
    },
  );

  assert.throws(
    () => {
      zonedLocalToUtcIso('2026-09-06', '00:00:00', 'America/Santiago');
    },
    (err) => {
      assert.ok(err instanceof Error);
      assert.match(err.message, /no existe.*cambio de horario de verano/i);
      return true;
    },
  );

  // But 01:00:00 is valid (first instant of the new daylight time) -> 04:00:00Z (UTC-3)
  const validAfterSpring = zonedLocalToUtcIso('2026-09-06', '01:00:00', 'America/Santiago');
  assert.equal(validAfterSpring, '2026-09-06T04:00:00Z');
});

test('zonedLocalToUtcIso: day of DST change backward (fall back) resolves ambiguous hour to first occurrence', () => {
  // In Chile, DST ends on the first Saturday of April (2026-04-04).
  // Clocks turn back at 24:00 (00:00 of 2026-04-05) to 23:00:00 of 2026-04-04.
  // 23:00:00 on 2026-04-04 happens twice:
  // 1st occurrence: 2026-04-05T02:00:00Z (UTC-3 daylight saving time)
  // 2nd occurrence: 2026-04-05T03:00:00Z (UTC-4 standard winter time)
  // Per RF-ADMIN-18-3, ambiguous hours must resolve to the first occurrence (earlier UTC instant).
  const ambiguousFirst = zonedLocalToUtcIso('2026-04-04', '23:00:00', 'America/Santiago');
  assert.equal(ambiguousFirst, '2026-04-05T02:00:00Z');

  const ambiguousFirst30 = zonedLocalToUtcIso('2026-04-04', '23:30:00', 'America/Santiago');
  assert.equal(ambiguousFirst30, '2026-04-05T02:30:00Z');
});

test('zonedLocalToUtcIso: UTC timezone', () => {
  const utc = zonedLocalToUtcIso('2026-10-11', '12:00:00', 'UTC');
  assert.equal(utc, '2026-10-11T12:00:00Z');
});

test('zonedLocalToUtcIso: half-hour timezone (Asia/Kolkata, UTC+5:30)', () => {
  // 12:00:00 local in Kolkata -> 06:30:00Z UTC
  const kolkata = zonedLocalToUtcIso('2026-10-11', '12:00:00', 'Asia/Kolkata');
  assert.equal(kolkata, '2026-10-11T06:30:00Z');
});

test('zonedLocalToUtcIso: midnight crossing (23:30 local in America/Santiago -> next day in UTC)', () => {
  // In summer (UTC-3), 23:30:00 local on 2026-10-11 is 02:30:00Z on 2026-10-12
  const crossing = zonedLocalToUtcIso('2026-10-11', '23:30:00', 'America/Santiago');
  assert.equal(crossing, '2026-10-12T02:30:00Z');
});

test('zonedLocalToUtcIso: round-trip fidelity with getDateStringInTimezone and formatR38Time', () => {
  const testTimestamps = [
    '2026-10-11T12:00:00Z',
    '2026-10-11T12:05:08Z',
    '2026-06-15T13:45:22Z',
    '2026-01-01T00:00:00Z',
    '2026-12-31T23:59:59Z',
    '2026-07-20T04:12:30Z',
  ];

  const testZones = [
    'America/Santiago',
    'UTC',
    'Asia/Kolkata',
    'America/Punta_Arenas',
    'Pacific/Easter',
  ];

  for (const ts of testTimestamps) {
    for (const tz of testZones) {
      const localDate = getDateStringInTimezone(ts, tz);
      const localTime = formatR38Time(ts, tz);
      const roundtrip = zonedLocalToUtcIso(localDate, localTime, tz);
      assert.equal(
        roundtrip,
        ts,
        `Round-trip mismatch for ${ts} in timezone ${tz}: got ${roundtrip}`,
      );
    }
  }
});

test('zonedLocalToUtcIso: validation of invalid inputs', () => {
  // Empty inputs
  assert.throws(() => zonedLocalToUtcIso('', '09:00:00'), /indicar la fecha/);
  assert.throws(() => zonedLocalToUtcIso('2026-10-11', ''), /indicar la hora/);

  // Invalid date format
  assert.throws(() => zonedLocalToUtcIso('11-10-2026', '09:00:00'), /Formato de fecha inválido/);

  // Invalid time format
  assert.throws(() => zonedLocalToUtcIso('2026-10-11', 'abc'), /Formato de hora inválido/);

  // Out of range hours/minutes/seconds
  assert.throws(() => zonedLocalToUtcIso('2026-10-11', '25:00:00'), /Hora o fecha fuera de rango/);
  assert.throws(() => zonedLocalToUtcIso('2026-10-11', '09:65:00'), /Hora o fecha fuera de rango/);
  assert.throws(() => zonedLocalToUtcIso('2026-10-11', '09:00:99'), /Hora o fecha fuera de rango/);

  // Invalid calendar date (e.g. Feb 30)
  assert.throws(() => zonedLocalToUtcIso('2026-02-30', '09:00:00'), /Fecha del calendario no válida/);
});

test('formatTimezoneLabel: returns proper es-CL human readable names', () => {
  assert.equal(
    formatTimezoneLabel('America/Santiago'),
    'Hora de Chile continental (America/Santiago)',
  );
  assert.equal(
    formatTimezoneLabel('America/Punta_Arenas'),
    'Hora de Magallanes (America/Punta_Arenas)',
  );
  assert.equal(
    formatTimezoneLabel('Pacific/Easter'),
    'Hora de Rapa Nui (Pacific/Easter)',
  );
  assert.equal(
    formatTimezoneLabel('UTC'),
    'Tiempo Universal Coordinado (UTC)',
  );
  assert.equal(
    formatTimezoneLabel('Asia/Kolkata'),
    'Asia/Kolkata',
  );
});
