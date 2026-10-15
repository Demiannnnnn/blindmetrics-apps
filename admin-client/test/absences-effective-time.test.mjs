import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  effectiveTs,
  effectiveLocalDate,
  sortMarksByEffective,
  groupMarksByEffectiveDay,
  deviceTimeDiffers,
  R38_ABBREVIATIONS,
  abbreviationMeaning,
  abbreviationsForKind,
  describeDayStatus,
  canQualifyAbsence,
  validateAbsenceForm,
  ABSENCE_KIND_LABELS,
  formatR38DayDate,
} from '../src/lib/attendance.ts';

const TZ = 'America/Santiago';

function mark(over) {
  return {
    id: over.id ?? 'm',
    server_ts: '2026-10-11T12:00:05Z',
    device_ts: '2026-10-11T12:00:04Z',
    offline: false,
    source: 'AGENT',
    ...over,
  };
}

test('effective_ts: an online mark counts at server time', () => {
  assert.equal(effectiveTs(mark({})), '2026-10-11T12:00:05Z');
});

test('effective_ts: an offline mark counts at device time', () => {
  assert.equal(
    effectiveTs(mark({ offline: true, device_ts: '2026-10-11T08:00:00Z', server_ts: '2026-10-11T15:00:00Z' })),
    '2026-10-11T08:00:00Z',
  );
});

test('effective_ts: a correction mark counts at the proposed (device) time', () => {
  assert.equal(
    effectiveTs(mark({ source: 'CORRECTION', device_ts: '2026-10-05T11:30:00Z', server_ts: '2026-10-08T12:00:00Z' })),
    '2026-10-05T11:30:00Z',
  );
});

test('effective_ts: the server value wins when present', () => {
  assert.equal(
    effectiveTs(mark({ offline: true, effective_ts: '2026-10-11T09:09:09Z' })),
    '2026-10-11T09:09:09Z',
  );
  // null falls back to the contract rule
  assert.equal(effectiveTs(mark({ effective_ts: null })), '2026-10-11T12:00:05Z');
});

test('effective_ts: grouping by local day near midnight in America/Santiago (UTC-3 in October)', () => {
  // Offline end at 23:59:59 local on the 11th, received by the server at 00:00:05 on the 12th.
  const lateOffline = mark({
    id: 'late-offline',
    type: 'WORKDAY_END',
    offline: true,
    device_ts: '2026-10-12T02:59:59Z',
    server_ts: '2026-10-12T03:00:05Z',
  });
  // Online mark one second after local midnight belongs to the 12th.
  const afterMidnight = mark({
    id: 'after-midnight',
    server_ts: '2026-10-12T03:00:01Z',
    device_ts: '2026-10-12T03:00:00Z',
  });
  // Online mark at 23:30 local on the 11th (UTC already the 12th).
  const lateOnline = mark({
    id: 'late-online',
    server_ts: '2026-10-12T02:30:00Z',
    device_ts: '2026-10-12T02:30:00Z',
  });

  assert.equal(effectiveLocalDate(lateOffline, TZ), '2026-10-11');
  assert.equal(effectiveLocalDate(afterMidnight, TZ), '2026-10-12');
  assert.equal(effectiveLocalDate(lateOnline, TZ), '2026-10-11');

  const groups = groupMarksByEffectiveDay([afterMidnight, lateOffline, lateOnline], TZ);
  assert.deepEqual(Object.keys(groups).sort(), ['2026-10-11', '2026-10-12']);
  assert.deepEqual(groups['2026-10-11'].map(m => m.id), ['late-online', 'late-offline']);
  assert.deepEqual(groups['2026-10-12'].map(m => m.id), ['after-midnight']);
});

test('effective_ts: grouping uses the winter offset (UTC-4) too', () => {
  const winter = mark({ server_ts: '2026-07-01T03:30:00Z', device_ts: '2026-07-01T03:30:00Z' });
  assert.equal(effectiveLocalDate(winter, TZ), '2026-06-30');
  const winterAfter = mark({ server_ts: '2026-07-01T04:00:00Z', device_ts: '2026-07-01T04:00:00Z' });
  assert.equal(effectiveLocalDate(winterAfter, TZ), '2026-07-01');
});

test('effective_ts: sorting by effective time does not mutate the input', () => {
  const a = mark({ id: 'a', offline: true, device_ts: '2026-10-11T09:00:00Z', server_ts: '2026-10-11T18:00:00Z' });
  const b = mark({ id: 'b', server_ts: '2026-10-11T12:00:00Z' });
  const input = [b, a];
  assert.deepEqual(sortMarksByEffective(input).map(m => m.id), ['a', 'b']);
  assert.deepEqual(input.map(m => m.id), ['b', 'a']);
});

test('effective_ts: device time is flagged only when it differs from server time', () => {
  assert.equal(deviceTimeDiffers(mark({})), true);
  assert.equal(
    deviceTimeDiffers(mark({ device_ts: '2026-10-11T12:00:05Z', server_ts: '2026-10-11T12:00:05.000Z' })),
    false,
  );
});

test('absences: official R38 art. 28 abbreviations in order with their meaning', () => {
  assert.deepEqual(
    R38_ABBREVIATIONS.map(a => a.code),
    ['A.I.', 'A.J', 'AT', 'C.T.', 'D.E.J.', 'H.E.', 'J.O.', 'L.M.', 'P.G.R', 'PREN.', 'P.S.G.R.', 'POSTN', 'S.A', 'VAC'],
  );
  assert.equal(abbreviationMeaning('L.M.'), 'Licencia médica');
  assert.equal(abbreviationMeaning('D.E.J.'), 'Distribución excepcional de jornada');
  assert.equal(abbreviationMeaning('P.S.G.R.'), 'Permiso sin goce de remuneraciones');
  assert.equal(abbreviationMeaning('LM'), null);
  for (const a of R38_ABBREVIATIONS) assert.ok(a.meaning.length > 3, a.code);
});

test('absences: contradicting abbreviation is not offered for the kind', () => {
  assert.ok(!abbreviationsForKind('JUSTIFIED').some(a => a.code === 'A.I.'));
  assert.ok(abbreviationsForKind('JUSTIFIED').some(a => a.code === 'A.J'));
  assert.ok(!abbreviationsForKind('UNJUSTIFIED').some(a => a.code === 'A.J'));
  assert.ok(abbreviationsForKind('UNJUSTIFIED').some(a => a.code === 'A.I.'));
  assert.equal(abbreviationsForKind('JUSTIFIED').length, 13);
});

test('absences: the system never labels an unqualified absence by itself', () => {
  const agreed = { start: '09:00:00', end: '18:00:00', break_counts_as_work: false };
  assert.equal(describeDayStatus({ attended: false, absence: null, agreed, marks: [] }), 'Sin marcas');
  assert.equal(describeDayStatus({ attended: false, absence: undefined, agreed, marks: [] }), 'Sin marcas');
  assert.equal(describeDayStatus({ attended: false, absence: null, agreed: null, marks: [] }), 'Día de descanso');
  assert.equal(describeDayStatus({ attended: true, absence: null, agreed, marks: [mark({})] }), 'Asistió');
  assert.equal(
    describeDayStatus({ attended: false, absence: 'UNJUSTIFIED', agreed, marks: [] }),
    ABSENCE_KIND_LABELS.UNJUSTIFIED,
  );
  assert.equal(describeDayStatus({ attended: false, absence: 'JUSTIFIED', agreed, marks: [] }), 'Ausencia justificada');
});

test('absences: qualification is offered only for workdays without marks or already qualified, never future', () => {
  const now = new Date('2026-10-15T03:30:00Z'); // 00:30 on the 15th in Santiago
  const agreed = { start: '09:00:00', end: '18:00:00', break_counts_as_work: false };
  const base = { date: '2026-10-09', attended: false, absence: null, agreed, marks: [] };
  assert.equal(canQualifyAbsence(base, now, TZ), true);
  assert.equal(canQualifyAbsence({ ...base, date: '2026-10-15' }, now, TZ), true);
  assert.equal(canQualifyAbsence({ ...base, date: '2026-10-16' }, now, TZ), false);
  assert.equal(canQualifyAbsence({ ...base, agreed: null }, now, TZ), false);
  assert.equal(canQualifyAbsence({ ...base, marks: [mark({})] }, now, TZ), false);
  assert.equal(canQualifyAbsence({ ...base, attended: true, marks: [mark({})] }, now, TZ), false);
  assert.equal(canQualifyAbsence({ ...base, agreed: null, absence: 'JUSTIFIED' }, now, TZ), true);
  // 23:00 on the 14th in Santiago: the 15th is still future there.
  assert.equal(canQualifyAbsence({ ...base, date: '2026-10-15' }, new Date('2026-10-15T02:00:00Z'), TZ), false);
});

test('absences: form validation mirrors the command rules', () => {
  assert.ok(validateAbsenceForm({ kind: null, confirmUnjustified: false }).kind);
  assert.deepEqual(
    validateAbsenceForm({ kind: 'JUSTIFIED', abbreviation: 'L.M.', note: 'Licencia', confirmUnjustified: false }),
    {},
  );
  assert.ok(validateAbsenceForm({ kind: 'JUSTIFIED', abbreviation: 'XX', confirmUnjustified: false }).abbreviation);
  assert.ok(validateAbsenceForm({ kind: 'JUSTIFIED', abbreviation: 'A.I.', confirmUnjustified: false }).abbreviation);
  assert.ok(validateAbsenceForm({ kind: 'JUSTIFIED', note: 'x'.repeat(301), confirmUnjustified: false }).note);
  assert.deepEqual(validateAbsenceForm({ kind: 'JUSTIFIED', note: 'ñ'.repeat(300), confirmUnjustified: false }), {});
  // "Injustificada" needs an explicit confirmation.
  assert.ok(validateAbsenceForm({ kind: 'UNJUSTIFIED', confirmUnjustified: false }).confirmUnjustified);
  assert.deepEqual(validateAbsenceForm({ kind: 'UNJUSTIFIED', abbreviation: 'A.I.', confirmUnjustified: true }), {});
});

test('effective_ts: a server day is shown as that same local date (no UTC shift)', () => {
  assert.equal(formatR38DayDate('2026-10-09'), '09/10/26');
  assert.equal(formatR38DayDate('2026-01-01'), '01/01/26');
});
