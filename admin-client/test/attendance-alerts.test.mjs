import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  ATTENDANCE_ALERTS,
  ALERT_TONE_CLASSES,
  attendanceAlertInfo,
  describeAnomaly,
  sortAlertsBySeverity,
} from '../src/lib/attendance.ts';

// Codes emitted by telemetry-api develop-AGENTS (get_attendance_board, src/attendance/days.rs).
const BOARD_ALERTS = ['MISSING_START', 'MISSING_END', 'BREAK_WITHOUT_END', 'OFFLINE_MARKS_TODAY', 'PENDING_CORRECTIONS'];
const DAY_ANOMALIES = [
  'MISSING_START',
  'MISSING_END',
  'BREAK_WITHOUT_END',
  'BREAK_WITHOUT_START',
  'MARK_ON_REST_DAY',
  'OFFLINE_MARK',
  'CORRECTED_MARK',
  'NO_RECEIPT_EMAIL',
];

test('attendance alerts: every API code has a label, a known tone and a hint', () => {
  for (const code of [...BOARD_ALERTS, ...DAY_ANOMALIES]) {
    const info = ATTENDANCE_ALERTS[code];
    assert.ok(info, `missing catalog entry for ${code}`);
    assert.ok(info.label.length > 0 && !info.label.includes('_'), code);
    assert.ok(info.tone in ALERT_TONE_CLASSES, code);
    assert.ok(info.hint.length > 10, code);
    assert.equal(attendanceAlertInfo(code), info);
  }
});

test('attendance alerts: existing labels are unchanged', () => {
  assert.equal(describeAnomaly('BREAK_WITHOUT_END'), 'Colación sin término registrado');
  assert.equal(describeAnomaly('MISSING_START'), 'Entrada no registrada');
  assert.equal(describeAnomaly('MISSING_END'), 'Salida no registrada');
  assert.equal(describeAnomaly('DAILY_MAX_EXCEEDED'), 'Supera máximo diario de 10 horas');
});

test('attendance alerts: tones reflect severity', () => {
  assert.equal(attendanceAlertInfo('MISSING_START').tone, 'danger');
  assert.equal(attendanceAlertInfo('PENDING_CORRECTIONS').tone, 'info');
  assert.equal(attendanceAlertInfo('OFFLINE_MARKS_TODAY').tone, 'info');
  assert.equal(attendanceAlertInfo('NO_RECEIPT_EMAIL').tone, 'warning');
  assert.match(attendanceAlertInfo('NO_RECEIPT_EMAIL').hint, /Editar trabajador/);
});

test('attendance alerts: unknown code gets a readable label, warning tone and no hint', () => {
  assert.deepEqual(attendanceAlertInfo('SOMETHING_NEW'), { label: 'Something new', tone: 'warning', hint: '' });
  assert.equal(describeAnomaly('toString'), 'Tostring');
  assert.equal(describeAnomaly(''), 'Aviso');
});

test('attendance alerts: sorted by severity, stable within a tone', () => {
  const sorted = sortAlertsBySeverity(['PENDING_CORRECTIONS', 'BREAK_WITHOUT_END', 'OFFLINE_MARKS_TODAY', 'MISSING_END', 'MISSING_START']);
  assert.deepEqual(sorted, ['MISSING_END', 'MISSING_START', 'BREAK_WITHOUT_END', 'PENDING_CORRECTIONS', 'OFFLINE_MARKS_TODAY']);
  const input = ['OFFLINE_MARK'];
  sortAlertsBySeverity(input);
  assert.deepEqual(input, ['OFFLINE_MARK']);
});

test('attendance alerts: the system never labels anything unjustified (v3.1)', () => {
  for (const info of Object.values(ATTENDANCE_ALERTS)) {
    assert.doesNotMatch(`${info.label} ${info.hint}`, /injustificad/i);
  }
});
