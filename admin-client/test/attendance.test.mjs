import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  formatR38Date,
  formatR38DateFull,
  formatR38Time,
  formatHashBlocks,
  ATTENDANCE_STATE_LABELS,
  MARK_TYPE_LABELS,
  CORRECTION_STATUS_LABELS,
  describeAnomaly,
  validateCorrectionForm,
  describeCorrectionError,
} from '../src/lib/attendance.ts';

test('attendance: formatR38Date outputs dd/mm/aa format', () => {
  const iso = '2026-10-11T12:00:00Z';
  const formatted = formatR38Date(iso, 'America/Santiago');
  // 11/10/26 (in Chilean timezone UTC-3 it's 11/10/26 09:00:00)
  assert.match(formatted, /^\d{2}\/\d{2}\/\d{2}$/);
  assert.equal(formatted, '11/10/26');
});

test('attendance: formatR38DateFull outputs dd/mm/yyyy format', () => {
  const iso = '2026-10-11T12:00:00Z';
  const formatted = formatR38DateFull(iso, 'America/Santiago');
  assert.equal(formatted, '11/10/2026');
});

test('attendance: formatR38Time outputs hh:mm:ss second precision', () => {
  const iso = '2026-10-11T12:05:08Z';
  const formatted = formatR38Time(iso, 'UTC');
  assert.equal(formatted, '12:05:08');
});

test('attendance: formatHashBlocks splits 64-char hex in blocks of 4', () => {
  const hash = '57c4cad20df41e012451c2841647daef7691fd333b1aaef5afb7fdad7666784e';
  const blocked = formatHashBlocks(hash);
  assert.equal(blocked.split(' ').length, 16);
  assert.equal(blocked.startsWith('57c4 cad2 0df4'), true);
  assert.equal(blocked.endsWith('784e'), true);
});

test('attendance: labels cover all states, mark types, and anomalies', () => {
  assert.equal(ATTENDANCE_STATE_LABELS.WORKING.label, 'En jornada');
  assert.equal(ATTENDANCE_STATE_LABELS.ON_BREAK.label, 'En colación');
  assert.equal(ATTENDANCE_STATE_LABELS.OFF.label, 'Fuera de jornada');
  assert.equal(ATTENDANCE_STATE_LABELS.UNMARKED.label, 'Sin marcar hoy');

  assert.equal(MARK_TYPE_LABELS.WORKDAY_START, 'Inicio de jornada');
  assert.equal(MARK_TYPE_LABELS.WORKDAY_END, 'Término de jornada');

  assert.equal(CORRECTION_STATUS_LABELS.PENDING.label, 'Pendiente');
  assert.equal(CORRECTION_STATUS_LABELS.ACCEPTED.label, 'Aceptada');
  assert.equal(CORRECTION_STATUS_LABELS.OPPOSED.label, 'Opuesta');
  assert.equal(CORRECTION_STATUS_LABELS.CONSOLIDATED.label, 'Consolidada');
  assert.equal(CORRECTION_STATUS_LABELS.WITHDRAWN.label, 'Retirada');

  assert.equal(describeAnomaly('BREAK_WITHOUT_END'), 'Colación sin término registrado');
  assert.equal(describeAnomaly('MISSING_START'), 'Entrada no registrada');
});

test('attendance: validateCorrectionForm checks confirmation and reason length', () => {
  const fixedNow = new Date('2026-10-12T12:00:00Z');

  // Missing confirmation
  const unconfirmed = validateCorrectionForm(
    {
      kind: 'ADD',
      markType: 'WORKDAY_START',
      proposedTs: '2026-10-10T08:00:00Z',
      reason: 'Marca olvidada por corte de energía',
      confirmNoHarm: false,
    },
    fixedNow,
  );
  assert.equal(unconfirmed.valid, false);
  assert.ok(unconfirmed.errors.confirmNoHarm);

  // Short reason
  const shortReason = validateCorrectionForm(
    {
      kind: 'ADD',
      markType: 'WORKDAY_START',
      proposedTs: '2026-10-10T08:00:00Z',
      reason: 'hola',
      confirmNoHarm: true,
    },
    fixedNow,
  );
  assert.equal(shortReason.valid, false);
  assert.ok(shortReason.errors.reason);
});

test('attendance: validateCorrectionForm rejects same day or future corrections (CORRECTION_TOO_EARLY)', () => {
  const fixedNow = new Date('2026-10-12T12:00:00Z');

  // Same day
  const sameDay = validateCorrectionForm(
    {
      kind: 'ADD',
      markType: 'WORKDAY_START',
      proposedTs: '2026-10-12T08:00:00Z',
      reason: 'Marca olvidada por corte de energía',
      confirmNoHarm: true,
    },
    fixedNow,
  );
  assert.equal(sameDay.valid, false);
  assert.equal(sameDay.reasonCode, 'CORRECTION_TOO_EARLY');

  // Future day
  const futureDay = validateCorrectionForm(
    {
      kind: 'ADD',
      markType: 'WORKDAY_START',
      proposedTs: '2026-10-15T08:00:00Z',
      reason: 'Marca olvidada por corte de energía',
      confirmNoHarm: true,
    },
    fixedNow,
  );
  assert.equal(futureDay.valid, false);
  assert.equal(futureDay.reasonCode, 'CORRECTION_TOO_EARLY');
});

test('attendance: validateCorrectionForm rejects harm to worker (CORRECTION_HARMS_WORKER)', () => {
  const fixedNow = new Date('2026-10-12T12:00:00Z');

  // Workday start: delayed start harms worker
  const delayedStart = validateCorrectionForm(
    {
      kind: 'MODIFY',
      originalMarkId: '01927d3b-65e1-7000-8000-000000000001',
      originalMarkTs: '2026-10-09T08:50:00Z',
      markType: 'WORKDAY_START',
      proposedTs: '2026-10-09T09:15:00Z', // later than 08:50
      reason: 'Se propone ingreso más tarde',
      confirmNoHarm: true,
    },
    fixedNow,
  );
  assert.equal(delayedStart.valid, false);
  assert.equal(delayedStart.reasonCode, 'CORRECTION_HARMS_WORKER');

  // Workday end: advanced end harms worker
  const advancedEnd = validateCorrectionForm(
    {
      kind: 'MODIFY',
      originalMarkId: '01927d3b-65e1-7000-8000-000000000002',
      originalMarkTs: '2026-10-09T18:00:00Z',
      markType: 'WORKDAY_END',
      proposedTs: '2026-10-09T17:30:00Z', // earlier than 18:00
      reason: 'Se propone salida antes',
      confirmNoHarm: true,
    },
    fixedNow,
  );
  assert.equal(advancedEnd.valid, false);
  assert.equal(advancedEnd.reasonCode, 'CORRECTION_HARMS_WORKER');
});

test('attendance: validateCorrectionForm accepts valid non-harming corrections', () => {
  const fixedNow = new Date('2026-10-12T12:00:00Z');

  // Workday start: earlier start eliminates delay (does not harm worker)
  const earlierStart = validateCorrectionForm(
    {
      kind: 'MODIFY',
      originalMarkId: '01927d3b-65e1-7000-8000-000000000001',
      originalMarkTs: '2026-10-09T09:15:00Z',
      markType: 'WORKDAY_START',
      proposedTs: '2026-10-09T08:55:00Z', // earlier than 09:15
      reason: 'Elimina atraso autorizado por jefatura',
      confirmNoHarm: true,
    },
    fixedNow,
  );
  assert.equal(earlierStart.valid, true);

  // Workday end: later end recognises overtime (does not harm worker)
  const laterEnd = validateCorrectionForm(
    {
      kind: 'MODIFY',
      originalMarkId: '01927d3b-65e1-7000-8000-000000000002',
      originalMarkTs: '2026-10-09T18:00:00Z',
      markType: 'WORKDAY_END',
      proposedTs: '2026-10-09T19:00:00Z', // later than 18:00
      reason: 'Reconocimiento de sobretiempo',
      confirmNoHarm: true,
    },
    fixedNow,
  );
  assert.equal(laterEnd.valid, true);
});

test('attendance: describeCorrectionError maps all R38 reasons', () => {
  assert.ok(describeCorrectionError('CORRECTION_TOO_EARLY').includes('siguiente'));
  assert.ok(describeCorrectionError('CORRECTION_HARMS_WORKER').includes('perjudica'));
  assert.ok(describeCorrectionError('ORIGINAL_MARK_REQUIRED').includes('original'));
  assert.ok(describeCorrectionError('AUTOMATED_WORKDAY_MARK_FORBIDDEN').includes('automatizada'));
});
