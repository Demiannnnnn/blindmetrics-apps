import { test } from 'node:test';
import assert from 'node:assert/strict';
import { ATTENDANCE_STATE_LABELS, boardKpis } from '../src/lib/attendance.ts';

test('board kpis: counts working, unmarked and late starts (ADMIN-22)', () => {
  const kpis = boardKpis([
    { state: 'WORKING', alerts: [] },
    { state: 'ON_BREAK', alerts: ['BREAK_WITHOUT_END'] },
    { state: 'UNMARKED', alerts: ['MISSING_START'] },
    { state: 'UNMARKED', alerts: [] },
    { state: 'OFF', alerts: ['MISSING_START'] },
  ]);
  assert.deepEqual(kpis, { working: 1, unmarked: 2, lateStart: 1 });
  assert.deepEqual(boardKpis([]), { working: 0, unmarked: 0, lateStart: 0 });
});

test('board state: "Sin marcar hoy" is neutral (rest days), never red', () => {
  assert.equal(ATTENDANCE_STATE_LABELS.UNMARKED.label, 'Sin marcar hoy');
  assert.doesNotMatch(ATTENDANCE_STATE_LABELS.UNMARKED.className, /danger/);
});
