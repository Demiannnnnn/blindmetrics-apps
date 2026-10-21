import { test } from 'node:test';
import assert from 'node:assert/strict';
import { journeyTypeLabel, nextScheduleHeading, scheduleFacts, scheduleWeekRows } from '../src/lib/attendance.ts';

const base = {
  version: 2,
  journey_type: 'FIXED',
  days: [
    { weekday: 1, start: '09:00', end: '18:00', break_start: '13:00', break_end: '14:00' },
    { weekday: 5, start: '22:00:00', end: '06:00:00' },
  ],
  break_counts_as_work: false,
  disconnection: null,
  location_at_mark: false,
  effective_from: '2026-10-01',
  requested_by: 'EMPLOYER',
  note: null,
};

test('schedule: seven rows Monday to Sunday, rest days and overnight shifts (AGENT-16)', () => {
  const rows = scheduleWeekRows(base);
  assert.equal(rows.length, 7);
  assert.deepEqual(rows[0], { weekday: 1, name: 'Lunes', hours: '09:00 – 18:00', breakText: 'Colación 13:00 – 14:00', works: true });
  assert.equal(rows[1].hours, 'Descanso');
  assert.equal(rows[1].works, false);
  assert.equal(rows[4].hours, '22:00 – 06:00 (termina al día siguiente)');
  assert.equal(rows[4].breakText, '');
  assert.equal(rows[6].name, 'Domingo');
});

test('schedule: facts in words', () => {
  assert.deepEqual(scheduleFacts(base), [
    'Jornada fija · vigente desde el Jueves 01/10/26',
    'La colación no se cuenta como tiempo trabajado.',
  ]);
  const remote = {
    ...base,
    journey_type: 'EXCEPTIONAL',
    exceptional_resolution: { number: '123', date: '2026-01-15' },
    break_counts_as_work: true,
    disconnection: { start: '20:00', end: '08:00' },
    requested_by: 'WORKER',
    note: '  Acordado por anexo ',
  };
  const facts = scheduleFacts(remote);
  assert.equal(facts[0], 'Jornada excepcional · vigente desde el Jueves 01/10/26');
  assert.match(facts[1], /resolución N° 123 del Jueves 15\/01\/26/);
  assert.equal(facts[2], 'La colación se cuenta como tiempo trabajado.');
  assert.match(facts[3], /^Desconexión: de 20:00 a 08:00\./);
  assert.equal(facts[4], 'Esta jornada fue solicitada por ti.');
  assert.equal(facts[5], 'Nota: Acordado por anexo');
});

test('schedule: labels and next version heading', () => {
  assert.equal(journeyTypeLabel('PART_TIME'), 'Jornada parcial');
  assert.equal(journeyTypeLabel('NEW_KIND'), 'NEW_KIND');
  assert.equal(journeyTypeLabel('toString'), 'toString');
  assert.equal(nextScheduleHeading({ ...base, effective_from: '2026-11-01' }), 'Desde el Domingo 01/11/26 tu jornada cambia');
});
