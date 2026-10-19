import test from 'node:test';
import assert from 'node:assert/strict';
import {
  formatDateCl,
  formatTimeCl,
  formatDayHeadingCl,
  formatCalendarDate,
  localDateKey,
  shortClock,
  serverNowMs,
  hasClockSkew,
  groupHash,
  shortHash,
  maskEmail,
  nextMarkLabel,
  otherMarks,
  nextMarkAfter,
  markStatusLabel,
  markTypeLabel,
  effectiveTs,
  mergeMarks,
  primaryMark,
  dayStateLabel,
  receiptText,
  selectBanner,
  screenForMode,
  reachedEnd,
  canAcceptTerms,
  hashPrefix,
  pinPolicyMessage,
  newPinMessage,
  sanitizePinInput,
  attendanceErrorMessage,
  remainingDelay,
  rangeForPreset,
  customRangeError,
  daysBetween,
  groupRowsByDay,
  describeAnomaly,
  absenceLabel,
  sortCorrections,
  countdownLabel,
  opposeUntilLabel,
  correctionStatusLabel,
} from '../src/lib/attendance.ts';

const TZ = 'America/Santiago';
const HASH = '3f9a21c08b7e45ad' + '0'.repeat(48);

function serverMark(overrides = {}) {
  return {
    id: 'm-1',
    client_mark_id: 'c-1',
    worker_id: 'w-1',
    type: 'WORKDAY_START',
    server_ts: '2026-10-11T12:02:11Z',
    device_ts: '2026-10-11T12:02:10Z',
    offline: false,
    source: 'AGENT',
    identification: 'DEVICE_PIN',
    workplace: { id: 'wp-1', name: 'Casa matriz', address: 'Av. Apoquindo 1234' },
    location: null,
    hash: HASH,
    prev_hash: 'b'.repeat(64),
    corrected_by: null,
    superseded_by: null,
    effective_ts: '2026-10-11T12:02:11Z',
    ...overrides,
  };
}

function record(overrides = {}) {
  return {
    client_mark_id: 'c-1',
    type: 'WORKDAY_START',
    device_ts: '2026-10-11T12:02:10Z',
    offline: false,
    status: 'CONFIRMED',
    server_mark: null,
    duplicate_of: null,
    rejection_reason: null,
    ...overrides,
  };
}

test('formatDateCl / formatTimeCl: R38 formats in the worker timezone', () => {
  // 12:00:05 UTC = 09:00:05 in Santiago (UTC-3 in October 2026, DST).
  assert.equal(formatDateCl('2026-10-11T12:00:05Z', TZ), '11/10/26');
  assert.equal(formatTimeCl('2026-10-11T12:00:05Z', TZ), '09:00:05');
  // Day boundary: 02:30 UTC on the 12th is still the 11th in Santiago.
  assert.equal(formatDateCl('2026-10-12T02:30:00Z', TZ), '11/10/26');
  assert.equal(formatTimeCl('2026-10-12T02:30:00Z', TZ), '23:30:00');
  assert.equal(formatTimeCl('2026-10-11T03:00:00Z', TZ), '00:00:00');
  assert.equal(formatDateCl('not a date', TZ), '—');
  assert.equal(formatTimeCl('', TZ), '—');
  // Invalid timezone falls back to the device zone instead of throwing.
  assert.match(formatTimeCl('2026-10-11T12:00:05Z', 'Nowhere/Invalid'), /^\d{2}:\d{2}:\d{2}$/);
});

test('formatDayHeadingCl and formatCalendarDate: weekday in words', () => {
  assert.equal(formatDayHeadingCl('2026-10-11T15:00:05Z', TZ), 'Domingo 11/10/26');
  assert.equal(formatDayHeadingCl('2026-10-10T15:00:05Z', TZ), 'Sábado 10/10/26');
  assert.equal(formatCalendarDate('2026-10-09'), 'Viernes 09/10/26');
  assert.equal(formatCalendarDate('bad'), 'bad');
  assert.equal(localDateKey('2026-10-12T02:30:00Z', TZ), '2026-10-11');
  assert.equal(shortClock('09:00:00'), '09:00');
  assert.equal(shortClock('9:30'), '09:30');
  assert.equal(shortClock(null), '');
});

test('server clock and skew notice (ATT-10)', () => {
  assert.equal(serverNowMs(1_000_000, 5), 1_005_000);
  assert.equal(serverNowMs(1_000_000, null), 1_000_000);
  assert.equal(hasClockSkew(60), false);
  assert.equal(hasClockSkew(-60), false);
  assert.equal(hasClockSkew(61), true);
  assert.equal(hasClockSkew(-90), true);
  assert.equal(hasClockSkew(null), false);
});

test('groupHash / shortHash / hashPrefix', () => {
  assert.equal(groupHash('3f9a21c08b7e'), '3f9a 21c0 8b7e');
  assert.equal(groupHash(HASH).split(' ').length, 16);
  assert.equal(groupHash(''), '');
  assert.equal(groupHash(null), '');
  assert.equal(shortHash(HASH), '3f9a 21c0 …');
  assert.equal(shortHash('abcd'), 'abcd');
  assert.equal(hashPrefix(HASH), '3f9a21c08b7e45ad');
});

test('maskEmail', () => {
  assert.equal(maskEmail('juan.perez@gmail.com'), 'j•••@gmail.com');
  assert.equal(maskEmail('  j@acme.cl '), 'j•••@acme.cl');
  assert.equal(maskEmail('ñandú@correo.cl'), 'ñ•••@correo.cl');
  assert.equal(maskEmail('sin-arroba'), null);
  assert.equal(maskEmail('@gmail.com'), null);
  assert.equal(maskEmail('juan@localhost'), null);
  assert.equal(maskEmail(null), null);
});

test('nextMarkLabel, otherMarks and nextMarkAfter', () => {
  assert.equal(nextMarkLabel('WORKDAY_START'), 'Marcar entrada');
  assert.equal(nextMarkLabel('BREAK_START'), 'Iniciar colación');
  assert.equal(nextMarkLabel('BREAK_END'), 'Terminar colación');
  assert.equal(nextMarkLabel('WORKDAY_END'), 'Marcar salida');
  assert.deepEqual(otherMarks('BREAK_START'), ['WORKDAY_START', 'BREAK_END', 'WORKDAY_END']);
  assert.equal(otherMarks('WORKDAY_END').length, 3);
  assert.equal(nextMarkAfter(null), 'WORKDAY_START');
  assert.equal(nextMarkAfter('WORKDAY_START'), 'BREAK_START');
  assert.equal(nextMarkAfter('BREAK_START'), 'BREAK_END');
  assert.equal(nextMarkAfter('BREAK_END'), 'WORKDAY_END');
  assert.equal(nextMarkAfter('WORKDAY_END'), 'WORKDAY_START');
  assert.equal(markTypeLabel('WORKDAY_START'), 'Entrada');
});

test('markStatusLabel', () => {
  assert.deepEqual(markStatusLabel('CONFIRMED'), { label: 'Enviada', tone: 'success' });
  assert.deepEqual(markStatusLabel('PENDING'), { label: 'Pendiente de envío', tone: 'warning' });
  assert.deepEqual(markStatusLabel('HASH_MISMATCH'), { label: 'Hash no coincide', tone: 'danger' });
  assert.equal(
    markStatusLabel('REJECTED', 'OFFLINE_MARK_TOO_OLD').label,
    'Rechazada: la marca sin conexión tenía más de 30 días'
  );
  assert.equal(markStatusLabel('REJECTED', 'OTRO_MOTIVO').label, 'Rechazada: OTRO_MOTIVO');
  assert.equal(markStatusLabel('REJECTED', null).label, 'Rechazada');
});

test('effectiveTs follows contract v3.1 when the server omits it', () => {
  const base = serverMark({ effective_ts: null });
  assert.equal(effectiveTs(base), base.server_ts);
  assert.equal(effectiveTs({ ...base, offline: true }), base.device_ts);
  assert.equal(effectiveTs({ ...base, source: 'CORRECTION' }), base.device_ts);
  assert.equal(effectiveTs({ ...base, effective_ts: '2026-10-11T11:00:00Z' }), '2026-10-11T11:00:00Z');
});

test('mergeMarks: local status wins, server-only marks are sent, pending sorted by time', () => {
  const sent = serverMark();
  const local = [
    record({ server_mark: sent, status: 'HASH_MISMATCH' }),
    record({
      client_mark_id: 'c-2',
      type: 'BREAK_START',
      device_ts: '2026-10-11T16:00:00Z',
      offline: true,
      status: 'PENDING',
    }),
  ];
  const portal = serverMark({
    id: 'm-9',
    client_mark_id: null,
    type: 'BREAK_END',
    source: 'CORRECTION',
    corrected_by: 'corr-1',
    server_ts: '2026-10-11T18:00:00Z',
    device_ts: '2026-10-11T17:00:00Z',
    effective_ts: null,
  });
  const rows = mergeMarks([sent, portal], local);
  assert.equal(rows.length, 3);
  assert.deepEqual(
    rows.map((r) => [r.type, r.status]),
    [
      ['WORKDAY_START', 'HASH_MISMATCH'],
      ['BREAK_START', 'PENDING'],
      ['BREAK_END', 'CONFIRMED'],
    ]
  );
  assert.equal(rows[2].corrected, true);
  assert.equal(rows[2].ts, '2026-10-11T17:00:00Z');
  assert.equal(rows[1].hash, null);
});

test('primaryMark and dayStateLabel: server next_mark online, local journal offline', () => {
  const online = {
    source: 'SERVER',
    server: {
      date: '2026-10-11',
      timezone: TZ,
      schedule_today: null,
      state: 'WORKING',
      next_mark: 'BREAK_START',
      marks: [serverMark()],
      pending_corrections: 0,
      server_time: '2026-10-11T15:00:00Z',
    },
    local_marks: [],
    server_time_offset_secs: 0,
  };
  assert.equal(primaryMark(online), 'BREAK_START');
  assert.equal(dayStateLabel(online, TZ), 'En jornada desde 09:02:11');

  const offline = {
    ...online,
    source: 'LOCAL',
    local_marks: [
      record({ client_mark_id: 'c-2', type: 'BREAK_START', device_ts: '2026-10-11T16:00:00Z', status: 'PENDING' }),
    ],
  };
  assert.equal(primaryMark(offline), 'BREAK_END');
  assert.equal(dayStateLabel(offline, TZ), 'En colación desde 13:00:00');

  const empty = { source: 'LOCAL', server: null, local_marks: [], server_time_offset_secs: null };
  assert.equal(primaryMark(empty), 'WORKDAY_START');
  assert.equal(dayStateLabel(empty, TZ), 'Fuera de jornada');
  assert.equal(primaryMark(null), 'WORKDAY_START');
});

test('receiptText is plain printable text with every field', () => {
  const [row] = mergeMarks([serverMark()], []);
  const text = receiptText(row, TZ, { organization: 'Acme SpA', worker: 'Juan Pérez' });
  assert.match(text, /Tipo: Entrada/);
  assert.match(text, /Fecha: 11\/10\/26/);
  assert.match(text, /Hora: 09:02:11/);
  assert.match(text, /Registrada por el servidor: 11\/10\/26 09:02:11/);
  assert.match(text, /Hash: 3f9a 21c0/);
  assert.match(text, /Hash anterior: bbbb/);
  assert.match(text, /Lugar de trabajo: Casa matriz/);
  assert.match(text, /Estado: Enviada/);
  assert.doesNotMatch(text, /<|>/);
});

test('selectBanner: one banner, spec priority, with its action', () => {
  const base = {
    banners: [],
    pending_marks: 0,
    pending_corrections: 0,
    run_mode: 'COLLECTING',
    last_error: null,
    pending_withdrawal: false,
  };
  assert.equal(selectBanner(base), null);
  assert.equal(selectBanner(null), null);
  const all = {
    ...base,
    banners: ['TELEMETRY_NOTICE', 'NEW_TERMS_VERSION', 'CORRECTION_TO_REVIEW', 'PENDING_MARKS_OFFLINE', 'SERVICE_PROBLEM'],
    pending_marks: 2,
  };
  assert.equal(selectBanner(all).kind, 'SERVICE_PROBLEM');
  assert.equal(selectBanner(all).action, 'DIAGNOSTICS');
  const noProblem = { ...all, banners: all.banners.filter((b) => b !== 'SERVICE_PROBLEM') };
  const pending = selectBanner(noProblem);
  assert.equal(pending.kind, 'PENDING_MARKS_OFFLINE');
  assert.match(pending.message, /2 marcas pendientes de envío/);
  assert.equal(selectBanner({ ...base, banners: ['CORRECTION_TO_REVIEW', 'NEW_TERMS_VERSION'] }).action, 'RECORDS');
  const terms = selectBanner({ ...base, banners: ['NEW_TERMS_VERSION', 'TELEMETRY_NOTICE'] });
  assert.equal(terms.message, 'Hay una nueva versión de los términos. Léela.');
  assert.equal(terms.action, 'TERMS');
  const withdrawal = selectBanner({ ...base, pending_withdrawal: true });
  assert.equal(withdrawal.kind, 'TELEMETRY_NOTICE');
  assert.equal(withdrawal.action, 'TELEMETRY');
  assert.equal(selectBanner({ ...base, banners: ['TELEMETRY_NOTICE'], run_mode: 'HALTED' }).tone, 'danger');
});

test('screenForMode: E1 → T1 → P1 → A', () => {
  assert.equal(screenForMode({ is_enrolled: false, attendance_mode: 'READY' }), 'ENROLLMENT');
  assert.equal(screenForMode({ is_enrolled: true, attendance_mode: 'UNLINKED' }), 'ENROLLMENT');
  assert.equal(screenForMode({ is_enrolled: true, attendance_mode: 'NEEDS_TERMS' }), 'TERMS');
  assert.equal(screenForMode({ is_enrolled: true, attendance_mode: 'NEEDS_PIN' }), 'PIN');
  assert.equal(screenForMode({ is_enrolled: true, attendance_mode: 'READY' }), 'SHELL');
  assert.equal(screenForMode({ is_enrolled: true, attendance_mode: 'EXEMPT' }), 'SHELL');
});

test('He leído y acepto: only after reaching the end of both documents', () => {
  assert.equal(reachedEnd(2000, 0, 600), false);
  assert.equal(reachedEnd(2000, 1390, 600), false);
  assert.equal(reachedEnd(2000, 1395, 600), true);
  assert.equal(reachedEnd(500, 0, 600), true, 'short document visible entirely');
  assert.equal(canAcceptTerms({ terms: false, privacy: false }), false);
  assert.equal(canAcceptTerms({ terms: true, privacy: false }), false);
  assert.equal(canAcceptTerms({ terms: false, privacy: true }), false);
  assert.equal(canAcceptTerms({ terms: true, privacy: true }), true);
  assert.equal(canAcceptTerms({ terms: true, privacy: true }, true), false, 'disabled while sending');
});

test('PIN mirror validation (informative; Rust applies the rule)', () => {
  assert.equal(pinPolicyMessage('482916'), null);
  assert.equal(pinPolicyMessage('48291673'), null);
  assert.equal(pinPolicyMessage(''), 'Ingresa un PIN de 6 a 8 dígitos.');
  assert.equal(pinPolicyMessage('12a456'), 'El PIN solo puede tener números.');
  assert.equal(pinPolicyMessage('48291'), 'El PIN debe tener entre 6 y 8 dígitos.');
  assert.equal(pinPolicyMessage('482916735'), 'El PIN debe tener entre 6 y 8 dígitos.');
  assert.equal(pinPolicyMessage('111111'), 'No uses el mismo número repetido, como 111111.');
  assert.equal(pinPolicyMessage('123456'), 'No uses números repetidos ni seguidos como 123456.');
  assert.equal(pinPolicyMessage('876543'), 'No uses números repetidos ni seguidos como 123456.');
  assert.equal(newPinMessage('482916', '482917'), 'Los PIN no coinciden.');
  assert.equal(newPinMessage('482916', '482916'), null);
  assert.equal(sanitizePinInput('48 29-16abc7890'), '48291678');
});

test('attendanceErrorMessage and remainingDelay', () => {
  assert.equal(
    attendanceErrorMessage({ code: 'PIN_INCORRECT', message: 'x', retry_after_secs: 4 }),
    'PIN incorrecto. Espera 4 s para reintentar.'
  );
  assert.equal(attendanceErrorMessage({ code: 'PIN_INCORRECT', message: 'x', retry_after_secs: 0 }), 'PIN incorrecto.');
  assert.equal(
    attendanceErrorMessage({ code: 'TERMS_OUTDATED', message: 'x' }),
    'Hay una versión más nueva de esta app. Actualízala para continuar.'
  );
  assert.equal(attendanceErrorMessage({ code: 'OTHER', message: 'Mensaje del servicio.' }), 'Mensaje del servicio.');
  assert.equal(remainingDelay(4, 10_000, 10_000), 4);
  assert.equal(remainingDelay(4, 10_000, 11_500), 3);
  assert.equal(remainingDelay(4, 10_000, 20_000), 0);
});

test('rangeForPreset (weeks start on Monday) and custom range validation', () => {
  // 2026-10-15 is a Thursday.
  assert.deepEqual(rangeForPreset('THIS_WEEK', '2026-10-15'), { from: '2026-10-12', to: '2026-10-15' });
  assert.deepEqual(rangeForPreset('LAST_WEEK', '2026-10-15'), { from: '2026-10-05', to: '2026-10-11' });
  assert.deepEqual(rangeForPreset('THIS_MONTH', '2026-10-15'), { from: '2026-10-01', to: '2026-10-15' });
  assert.deepEqual(rangeForPreset('LAST_MONTH', '2026-10-15'), { from: '2026-09-01', to: '2026-09-30' });
  assert.deepEqual(rangeForPreset('LAST_MONTH', '2026-01-10'), { from: '2025-12-01', to: '2025-12-31' });
  // Sunday belongs to the week that started on the previous Monday.
  assert.deepEqual(rangeForPreset('THIS_WEEK', '2026-10-18'), { from: '2026-10-12', to: '2026-10-18' });
  assert.equal(rangeForPreset('THIS_WEEK', 'bad'), null);
  assert.equal(daysBetween('2026-10-01', '2026-10-31'), 30);

  assert.equal(customRangeError({ from: '2026-10-01', to: '2026-10-15' }, '2026-10-15'), null);
  assert.equal(customRangeError({ from: '2021-10-15', to: '2021-12-01' }, '2026-10-15'), null);
  assert.equal(
    customRangeError({ from: '2021-10-14', to: '2021-12-01' }, '2026-10-15'),
    'Puedes consultar hasta 5 años hacia atrás.'
  );
  assert.equal(
    customRangeError({ from: '2026-10-10', to: '2026-10-01' }, '2026-10-15'),
    'La fecha inicial debe ser anterior a la final.'
  );
  assert.equal(
    customRangeError({ from: '2026-10-10', to: '2026-10-16' }, '2026-10-15'),
    'La fecha final no puede ser posterior a hoy.'
  );
  assert.equal(customRangeError({ from: '', to: '2026-10-01' }, '2026-10-15'), 'Elige una fecha inicial y una final.');
  assert.equal(customRangeError({ from: '2026-02-30', to: '2026-03-01' }, '2026-10-15'), 'Elige una fecha inicial y una final.');
});

test('groupRowsByDay uses the effective date in the worker zone, newest day first', () => {
  const rows = mergeMarks(
    [
      serverMark({ id: 'a', client_mark_id: null, effective_ts: '2026-10-12T02:30:00Z' }),
      serverMark({ id: 'b', client_mark_id: null, effective_ts: '2026-10-12T12:00:00Z' }),
      serverMark({ id: 'c', client_mark_id: null, effective_ts: '2026-10-11T12:00:00Z' }),
    ],
    []
  );
  const days = groupRowsByDay(rows, TZ);
  assert.deepEqual(
    days.map((d) => [d.date, d.rows.map((r) => r.key)]),
    [
      ['2026-10-12', ['b']],
      ['2026-10-11', ['c', 'a']],
    ]
  );
});

test('day summary words: anomalies and absences (ATT-09, v3.1)', () => {
  assert.equal(describeAnomaly('BREAK_WITHOUT_END'), 'Colación sin término');
  assert.equal(describeAnomaly('SOMETHING_NEW'), 'Something new');
  assert.equal(absenceLabel('JUSTIFIED', false, true), 'Ausencia justificada');
  assert.equal(absenceLabel('UNJUSTIFIED', false, true), 'Ausencia injustificada');
  assert.equal(absenceLabel(null, false, true), 'Sin marcas');
  assert.equal(absenceLabel(null, true, true), null);
  assert.equal(absenceLabel(null, false, false), null);
});

test('corrections: pending first, countdown and deadline label', () => {
  const mk = (id, status, extra = {}) => ({
    id,
    worker_id: null,
    kind: 'MODIFY',
    original_mark: null,
    type: 'WORKDAY_START',
    proposed_ts: '2026-10-10T12:00:00Z',
    reason: 'Ajuste',
    status,
    created_at: '2026-10-11T12:00:00Z',
    notified_at: null,
    respond_before: null,
    responded_at: null,
    resulting_mark_id: null,
    ...extra,
  });
  const { pending, history } = sortCorrections([
    mk('h1', 'OPPOSED', { responded_at: '2026-10-11T13:00:00Z' }),
    mk('p2', 'PENDING', { respond_before: '2026-10-14T12:00:00Z' }),
    mk('p1', 'PENDING', { respond_before: '2026-10-13T12:00:00Z' }),
    mk('h2', 'CONSOLIDATED', { responded_at: '2026-10-12T13:00:00Z' }),
  ]);
  assert.deepEqual(pending.map((c) => c.id), ['p1', 'p2']);
  assert.deepEqual(history.map((c) => c.id), ['h2', 'h1']);

  const now = Date.parse('2026-10-12T10:00:00Z');
  assert.equal(countdownLabel('2026-10-13T13:30:00Z', now), 'Quedan 1 d 3 h');
  assert.equal(countdownLabel('2026-10-12T12:15:00Z', now), 'Quedan 2 h 15 min');
  assert.equal(countdownLabel('2026-10-12T10:00:20Z', now), 'Quedan 1 min');
  assert.equal(countdownLabel('2026-10-12T09:00:00Z', now), 'Plazo vencido');
  assert.equal(countdownLabel(null, now), null);
  assert.equal(opposeUntilLabel('2026-10-12T17:30:00Z', TZ), 'Puedes oponerte hasta el 12/10/26 14:30');
  assert.equal(correctionStatusLabel('OPPOSED').label, 'Te opusiste: se mantiene tu marca original');
});
