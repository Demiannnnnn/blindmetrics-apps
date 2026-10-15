import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  REPORT_MENU,
  DAILY_REPORT_INFO,
  initialReportForm,
  buildReportFilters,
  validateReportFilters,
  subtractMonths,
  maxRangeMonths,
  isAlteredCell,
  cellText,
  cellAccessibleText,
  hasAlteredCells,
  workplaceLabel,
  filterWorkerOptions,
  sortWorkerOptions,
  uniqueJobTitles,
  savedToastText,
} from '../src/lib/reports.ts';

const W1 = '01927d3b-65e1-7000-8000-0000000000aa';
const W2 = '01927d3b-65e1-7000-8000-0000000000ab';
const WP = '01927d3b-65e1-7000-8000-0000000000b1';
const HASH = '57c4cad20df41e012451c2841647daef7691fd333b1aaef5afb7fdad7666784e';

test('reports: menu has the six R38 reports in order with exact names', () => {
  assert.deepEqual(
    REPORT_MENU.map(r => r.name),
    [
      'Reporte de asistencia',
      'Reporte de jornada diaria',
      'Reporte de domingos y/o días festivos',
      'Reporte de modificaciones y/o alteraciones de turnos',
      'Reporte diario',
      'Reporte de incidentes técnicos',
    ],
  );
  assert.deepEqual(
    REPORT_MENU.map(r => r.id),
    ['attendance', 'daily-hours', 'sundays-holidays', 'shift-changes', 'daily', 'incidents'],
  );
  assert.equal(DAILY_REPORT_INFO, 'Se envía automáticamente por correo cada día al empleador');
});

test('reports: default form builds a last-week query for all workers', () => {
  const filters = buildReportFilters(initialReportForm());
  assert.deepEqual(filters, { worker_ids: [], period: 'last_week' });
  assert.equal(validateReportFilters('attendance', filters), null);
});

test('reports: every filter combines and is normalized', () => {
  const filters = buildReportFilters({
    selectedWorkerIds: [W2, W1.toUpperCase(), W2],
    search: 'Pérez',
    mode: 'range',
    period: 'last_week',
    from: '2026-09-01',
    to: ' 2026-09-30 ',
    journeyType: 'SHIFTS',
    workplaceId: WP,
    jobTitle: '  Vendedor ',
    hash: HASH.toUpperCase(),
  });
  assert.deepEqual(filters, {
    worker_ids: [W2, W1],
    from: '2026-09-01',
    to: '2026-09-30',
    journey_type: 'SHIFTS',
    workplace_id: WP,
    job_title: 'Vendedor',
    hash: HASH,
  });
  // Search goes to the server only when nobody is selected.
  assert.equal(filters.search, undefined);
  assert.equal(validateReportFilters('daily-hours', filters), null);
});

test('reports: search text is sent when no worker is selected', () => {
  const filters = buildReportFilters({ ...initialReportForm(), search: ' 12.345.678-5 ' });
  assert.equal(filters.search, '12.345.678-5');
  assert.deepEqual(filters.worker_ids, []);
});

test('reports: period and range are mutually exclusive and the range is complete', () => {
  assert.match(
    validateReportFilters('attendance', { worker_ids: [], period: 'last_month', from: '2026-01-01' }),
    /no ambos/,
  );
  assert.match(validateReportFilters('attendance', { worker_ids: [] }), /período/);
  assert.match(validateReportFilters('attendance', { worker_ids: [], from: '2026-01-01' }), /dos fechas/);
  assert.match(
    validateReportFilters('attendance', { worker_ids: [], from: '2026-02-30', to: '2026-03-01' }),
    /no son válidas/,
  );
  assert.match(
    validateReportFilters('attendance', { worker_ids: [], from: '2026-03-02', to: '2026-03-01' }),
    /posterior/,
  );
});

test('reports: range limit is 5 years, 24 months for Sundays/holidays', () => {
  assert.equal(maxRangeMonths('attendance'), 60);
  assert.equal(maxRangeMonths('sundays-holidays'), 24);
  const ok5 = { worker_ids: [], from: '2021-10-15', to: '2026-10-15' };
  const over5 = { worker_ids: [], from: '2021-10-14', to: '2026-10-15' };
  assert.equal(validateReportFilters('attendance', ok5), null);
  assert.match(validateReportFilters('attendance', over5), /5 años/);
  assert.match(validateReportFilters('sundays-holidays', ok5), /24 meses/);
  assert.equal(
    validateReportFilters('sundays-holidays', { worker_ids: [], from: '2024-10-15', to: '2026-10-15' }),
    null,
  );
  assert.match(
    validateReportFilters('sundays-holidays', { worker_ids: [], from: '2024-10-14', to: '2026-10-15' }),
    /24 meses/,
  );
});

test('reports: subtractMonths clamps to month end like chrono', () => {
  assert.equal(subtractMonths('2026-03-31', 1), '2026-02-28');
  assert.equal(subtractMonths('2028-03-31', 1), '2028-02-29');
  assert.equal(subtractMonths('2026-10-15', 60), '2021-10-15');
  assert.equal(subtractMonths('2026-01-10', 13), '2024-12-10');
  assert.equal(subtractMonths('bad', 1), null);
});

test('reports: ids, hash and text length are validated', () => {
  const base = { worker_ids: [], period: 'last_week' };
  assert.match(validateReportFilters('attendance', { ...base, worker_ids: ['x'] }), /identificador/);
  assert.match(validateReportFilters('attendance', { ...base, workplace_id: '../x' }), /lugar/);
  assert.match(validateReportFilters('attendance', { ...base, hash: 'abc' }), /64/);
  assert.match(validateReportFilters('attendance', { ...base, job_title: 'x'.repeat(101) }), /100/);
  const many = Array.from({ length: 201 }, (_, i) => `01927d3b-65e1-7000-8000-${i.toString(16).padStart(12, '0')}`);
  assert.match(validateReportFilters('attendance', { ...base, worker_ids: many }), /200/);
});

test('reports: cells render the server text verbatim', () => {
  assert.equal(cellText('+00:45:25'), '+00:45:25');
  assert.equal(cellText('-00:12:30'), '-00:12:30');
  assert.equal(cellText({ v: '08:02:00', altered: true }), '08:02:00');
  assert.equal(cellText(null), '');
  assert.equal(cellText(3), '3');
  assert.equal(isAlteredCell({ v: '08:02:00', altered: true }), true);
  assert.equal(isAlteredCell({ v: '08:02:00', altered: false }), false);
  assert.equal(isAlteredCell('08:02:00'), false);
});

test('reports: altered cells have accessible text', () => {
  assert.equal(cellAccessibleText({ v: '08:02:00', altered: true }), '08:02:00 (valor alterado)');
  assert.equal(cellAccessibleText('Sí'), 'Sí');
  assert.equal(cellAccessibleText(''), 'vacío');
  assert.equal(cellAccessibleText(null), 'vacío');
  assert.equal(hasAlteredCells([['a', 'b'], ['c', { v: 'd', altered: true }]]), true);
  assert.equal(hasAlteredCells([['a', null]]), false);
});

test('reports: workplace header label', () => {
  assert.equal(workplaceLabel('Casa matriz'), 'Casa matriz');
  assert.equal(workplaceLabel({ name: 'Sucursal', address: 'Huérfanos 1234' }), 'Sucursal — Huérfanos 1234');
  assert.equal(workplaceLabel({ name: 'Sucursal' }), 'Sucursal');
  assert.equal(workplaceLabel(null), '');
});

test('reports: worker picker sorts alphabetically and searches by name or RUN', () => {
  const options = [
    { id: W1, name: 'Óscar Muñoz', run: '12.345.678-5', jobTitle: 'Vendedor' },
    { id: W2, name: 'ana Soto', run: '9.876.543-3', jobTitle: 'Cajera' },
    { id: WP, name: 'Bruno Díaz', run: '7.654.321-K', jobTitle: 'Vendedor' },
  ];
  assert.deepEqual(sortWorkerOptions(options).map(o => o.name), ['ana Soto', 'Bruno Díaz', 'Óscar Muñoz']);
  assert.deepEqual(filterWorkerOptions(options, 'oscar').map(o => o.id), [W1]);
  assert.deepEqual(filterWorkerOptions(options, '12345678').map(o => o.id), [W1]);
  assert.deepEqual(filterWorkerOptions(options, '7.654.321-k').map(o => o.id), [WP]);
  assert.equal(filterWorkerOptions(options, '').length, 3);
  assert.deepEqual(uniqueJobTitles(options), ['Cajera', 'Vendedor']);
});

test('reports: saved toast text', () => {
  assert.equal(savedToastText('/home/a/Descargas/r.pdf'), 'Guardado en /home/a/Descargas/r.pdf');
});

test('reports: lock screen notice keeps the R38/BILL-04 wording', async () => {
  const { LOCK_ATTENDANCE_NOTICE } = await import('../src/lib/subscription.ts');
  assert.equal(
    LOCK_ATTENDANCE_NOTICE,
    'Tus trabajadores siguen pudiendo marcar asistencia. La telemetría está detenida.',
  );
});
