import test from 'node:test';
import assert from 'node:assert/strict';

import {
  timeToMinutes,
  calculateIntervalMinutes,
  calculateDayMinutes,
  calculateDayHours,
  calculateWeeklyTotalMinutes,
  calculateWeeklyTotalHours,
  getWeeklyLegalMaxHours,
  calculateDisconnectionMinutes,
  calculateDisconnectionHours,
  validateDisconnection,
  getScheduleWarnings,
  getDefaultWeekDays,
  formatMinutes,
  LOCATION_AT_MARK_DESKTOP_NOTICE,
} from '../src/lib/schedule.ts';

test('schedule: timeToMinutes parses valid times and rejects invalid', () => {
  assert.equal(timeToMinutes('09:00'), 540);
  assert.equal(timeToMinutes('18:30'), 1110);
  assert.equal(timeToMinutes('00:00'), 0);
  assert.equal(timeToMinutes('23:59'), 1439);
  assert.equal(timeToMinutes('09:00:00'), 540);

  assert.ok(isNaN(timeToMinutes('')));
  assert.ok(isNaN(timeToMinutes('25:00')));
  assert.ok(isNaN(timeToMinutes('12:60')));
  assert.ok(isNaN(timeToMinutes('abc')));
});

test('schedule: calculateDayHours subtracts meal break when break_counts_as_work is false', () => {
  const day = {
    weekday: 1,
    start: '09:00',
    end: '18:00', // 9 hours span
    break_start: '13:00',
    break_end: '14:00', // 1 hour meal break
  };

  // Not imputed: 9h - 1h = 8h
  assert.equal(calculateDayHours(day, false), 8);
  assert.equal(calculateDayMinutes(day, false), 480);

  // Imputed: full 9h counts as work
  assert.equal(calculateDayHours(day, true), 9);
  assert.equal(calculateDayMinutes(day, true), 540);
});

test('schedule: calculateDayHours without meal break returns full span', () => {
  const day = {
    weekday: 2,
    start: '08:00',
    end: '16:30', // 8.5 hours
  };
  assert.equal(calculateDayHours(day, false), 8.5);
  assert.equal(calculateDayHours(day, true), 8.5);
});

test('schedule: calculateWeeklyTotalHours sums all working days', () => {
  const defaultDays = getDefaultWeekDays();
  // 5 days x 8 hours = 40 hours
  assert.equal(calculateWeeklyTotalHours(defaultDays, false), 40);
  // If lunch counts as work: 5 days x 9 hours = 45 hours
  assert.equal(calculateWeeklyTotalHours(defaultDays, true), 45);
});

test('schedule: weekly legal max depends on date (ATT-11)', () => {
  // Until 2026-04-25: 44 hours
  assert.equal(getWeeklyLegalMaxHours('2024-05-01'), 44);
  assert.equal(getWeeklyLegalMaxHours('2026-04-25'), 44);

  // From 2026-04-26 to 2028-04-25: 42 hours
  assert.equal(getWeeklyLegalMaxHours('2026-04-26'), 42);
  assert.equal(getWeeklyLegalMaxHours('2026-10-14'), 42);
  assert.equal(getWeeklyLegalMaxHours('2028-04-25'), 42);

  // From 2028-04-26: 40 hours
  assert.equal(getWeeklyLegalMaxHours('2028-04-26'), 40);
  assert.equal(getWeeklyLegalMaxHours('2029-01-01'), 40);
});

test('schedule: disconnection >= 12 h crossing midnight (CT art. 152 quáter J)', () => {
  // 20:00 to 08:00 = 4h before midnight + 8h after = 12h
  assert.equal(calculateDisconnectionMinutes('20:00', '08:00'), 720);
  assert.equal(calculateDisconnectionHours('20:00', '08:00'), 12);
  const v1 = validateDisconnection({ start: '20:00', end: '08:00' });
  assert.equal(v1.valid, true);
  assert.equal(v1.hours, 12);

  // 21:00 to 08:00 = 3h + 8h = 11h (< 12h -> invalid)
  assert.equal(calculateDisconnectionHours('21:00', '08:00'), 11);
  const v2 = validateDisconnection({ start: '21:00', end: '08:00' });
  assert.equal(v2.valid, false);
  assert.ok(v2.error?.includes('12 horas'));

  // Missing disconnection
  const v3 = validateDisconnection(null);
  assert.equal(v3.valid, false);
});

test('schedule: warnings generated for excessive daily and weekly hours', () => {
  // Normal 40h schedule has no warnings
  const normalDays = getDefaultWeekDays();
  const wNormal = getScheduleWarnings(normalDays, false, '2026-10-14');
  assert.deepEqual(wNormal, []);

  // 11h in one day (> 10h daily limit)
  const heavyDay = {
    weekday: 1,
    start: '07:00',
    end: '19:00', // 12h span - 1h break = 11h
    break_start: '13:00',
    break_end: '14:00',
  };
  const wDay = getScheduleWarnings([heavyDay], false, '2026-10-14');
  assert.equal(wDay.length, 1);
  assert.ok(wDay[0].includes('máximo diario'));

  // 5 days x 9h = 45h (exceeds 42h limit on 2026-10-14)
  const heavyWeek = [1, 2, 3, 4, 5].map(weekday => ({
    weekday,
    start: '08:00',
    end: '18:00', // 10h span - 1h break = 9h x 5 = 45h
    break_start: '13:00',
    break_end: '14:00',
  }));
  const wWeek = getScheduleWarnings(heavyWeek, false, '2026-10-14');
  assert.equal(wWeek.length, 1);
  assert.ok(wWeek[0].includes('máximo legal de 42 horas'));
});

test('schedule: formatMinutes formats hours and minutes cleanly', () => {
  assert.equal(formatMinutes(480), '8 h');
  assert.equal(formatMinutes(510), '8 h 30 min');
  assert.equal(formatMinutes(0), '0 h');
});

test('schedule: location-at-mark toggle tells the employer the desktop app sends no location (AGENT-15)', () => {
  assert.match(LOCATION_AT_MARK_DESKTOP_NOTICE, /no puede obtener la ubicación/);
  assert.match(LOCATION_AT_MARK_DESKTOP_NOTICE, /sin ubicación/);
});
