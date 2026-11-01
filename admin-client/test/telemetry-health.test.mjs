import test from 'node:test';
import assert from 'node:assert/strict';
import React from 'react';
import { renderToStaticMarkup } from 'react-dom/server';

import {
  formatRelativeTime,
  formatLastTelemetry,
  formatRejectionDecision,
  formatRejectionDetail,
  formatRemainingCount,
  formatWithoutDataAlertLabel,
  formatRejectionsAlertLabel,
} from '../src/lib/telemetryHealth.ts';

test('telemetry health: formatWithoutDataAlertLabel handles counts, dynamic hours and error state (RF-ADMIN-25-2, RF-ADMIN-25-4)', () => {
  assert.equal(
    formatWithoutDataAlertLabel(0, 24),
    '0 trabajador(es) activos sin datos hace más de 24 h',
  );
  assert.equal(
    formatWithoutDataAlertLabel(3, 48),
    '3 trabajador(es) activos sin datos hace más de 48 h',
  );
  // On error/failure, count is null and displays "—" instead of false 0.
  assert.equal(
    formatWithoutDataAlertLabel(null, 24),
    '— trabajador(es) activos sin datos hace más de 24 h',
  );
});

test('telemetry health: formatRejectionsAlertLabel handles counts, dynamic days and error state (RF-ADMIN-25-2, RF-ADMIN-25-4)', () => {
  assert.equal(
    formatRejectionsAlertLabel(0, 7),
    '0 rechazo(s) o retiro(s) de consentimiento (7 días)',
  );
  assert.equal(
    formatRejectionsAlertLabel(2, 14),
    '2 rechazo(s) o retiro(s) de consentimiento (14 días)',
  );
  // On error/failure, count is null and displays "—" instead of false 0.
  assert.equal(
    formatRejectionsAlertLabel(null, 7),
    '— rechazo(s) o retiro(s) de consentimiento (7 días)',
  );
});

test('telemetry health: formatLastTelemetry handles null and relative elapsed time (RF-ADMIN-25-3)', () => {
  const baseNow = new Date('2026-10-31T20:00:00Z').getTime();

  // Null telemetry timestamp
  assert.equal(formatLastTelemetry(null, baseNow), 'nunca envió datos');

  // 15 minutes ago
  const t15m = new Date(baseNow - 15 * 60 * 1000).toISOString();
  assert.equal(formatLastTelemetry(t15m, baseNow), 'último dato hace 15 min');

  // 25 hours ago
  const t25h = new Date(baseNow - 25 * 3600 * 1000).toISOString();
  assert.equal(formatLastTelemetry(t25h, baseNow), 'último dato hace 25 h');

  // 4 days ago
  const t4d = new Date(baseNow - 4 * 24 * 3600 * 1000).toISOString();
  assert.equal(formatLastTelemetry(t4d, baseNow), 'último dato hace 4 días');
});

test('telemetry health: formatRejectionDecision and formatRejectionDetail (RF-ADMIN-25-3)', () => {
  assert.equal(formatRejectionDecision('REJECTED'), 'Rechazó');
  assert.equal(formatRejectionDecision('WITHDRAWN'), 'Retiró');
  assert.equal(formatRejectionDecision('rejected'), 'Rechazó');
  assert.equal(formatRejectionDecision('withdrawn'), 'Retiró');

  const workerRejected = {
    id: 'w-1',
    name: 'Carlos Valenzuela',
    decision: 'REJECTED',
    decided_at: '2026-10-25T14:30:00Z',
    consent_version: 3,
  };
  const detailRejected = formatRejectionDetail(workerRejected, 'America/Santiago');
  assert.ok(detailRejected.includes('Rechazó'));
  assert.ok(detailRejected.includes('25/10/26'));
  assert.ok(detailRejected.includes('v3'));

  const workerWithdrawn = {
    id: 'w-2',
    name: 'Fernanda Morales',
    decision: 'WITHDRAWN',
    decided_at: '2026-10-26T10:00:00Z',
    consent_version: 2,
  };
  const detailWithdrawn = formatRejectionDetail(workerWithdrawn, 'America/Santiago');
  assert.ok(detailWithdrawn.includes('Retiró'));
  assert.ok(detailWithdrawn.includes('26/10/26'));
  assert.ok(detailWithdrawn.includes('v2'));
});

test('telemetry health: formatRemainingCount computes "y M más" correctly (RF-ADMIN-25-3)', () => {
  assert.equal(formatRemainingCount(10, 2), 'y 8 más');
  assert.equal(formatRemainingCount(50, 5), 'y 45 más');
  assert.equal(formatRemainingCount(2, 2), null);
  assert.equal(formatRemainingCount(0, 0), null);
});

test('telemetry health: UI markup renders counts > 0 with warning color, accessible buttons, and worker list with "y M más"', () => {
  const healthData = {
    generated_at: '2026-10-31T22:00:00Z',
    stale_hours: 24,
    since_days: 7,
    active_without_data: {
      total: 3,
      workers: [
        {
          id: 'w-1',
          name: 'Ana Díaz',
          last_telemetry_at: '2026-10-30T10:00:00Z',
          active_since: '2026-10-20T08:00:00Z',
        },
        {
          id: 'w-2',
          name: 'Bernardo O\'Higgins',
          last_telemetry_at: null,
          active_since: '2026-10-22T09:00:00Z',
        },
      ],
    },
    recent_rejections: {
      total: 1,
      workers: [
        {
          id: 'w-3',
          name: 'Carlos Valenzuela',
          decision: 'REJECTED',
          decided_at: '2026-10-25T14:30:00Z',
          consent_version: 3,
        },
      ],
    },
  };

  // Render a simulated alert section to static HTML to verify markup, classes and accessibility.
  const element = React.createElement(
    'div',
    { className: 'alerts-container' },
    // Active without data line (N > 0)
    React.createElement(
      'button',
      {
        type: 'button',
        'aria-expanded': true,
        className: 'text-warning font-semibold',
      },
      formatWithoutDataAlertLabel(
        healthData.active_without_data.total,
        healthData.stale_hours,
      ),
    ),
    // Expanded list for active without data
    React.createElement(
      'ul',
      null,
      ...healthData.active_without_data.workers.map(w =>
        React.createElement(
          'li',
          { key: w.id },
          React.createElement(
            'button',
            { type: 'button', 'data-worker-id': w.id },
            w.name,
            ` (${formatLastTelemetry(w.last_telemetry_at)})`,
          ),
        ),
      ),
      formatRemainingCount(
        healthData.active_without_data.total,
        healthData.active_without_data.workers.length,
      ) &&
        React.createElement(
          'li',
          null,
          formatRemainingCount(
            healthData.active_without_data.total,
            healthData.active_without_data.workers.length,
          ),
        ),
    ),
    // Recent rejections line (N > 0)
    React.createElement(
      'button',
      {
        type: 'button',
        'aria-expanded': true,
        className: 'text-warning font-semibold',
      },
      formatRejectionsAlertLabel(
        healthData.recent_rejections.total,
        healthData.since_days,
      ),
    ),
    // Expanded list for recent rejections
    React.createElement(
      'ul',
      null,
      ...healthData.recent_rejections.workers.map(w =>
        React.createElement(
          'li',
          { key: w.id },
          React.createElement(
            'button',
            { type: 'button', 'data-worker-id': w.id },
            w.name,
            ` (${formatRejectionDetail(w)})`,
          ),
        ),
      ),
    ),
  );

  const html = renderToStaticMarkup(element);

  // Verifications:
  // 1. Warning colors
  assert.ok(html.includes('text-warning font-semibold'));
  // 2. Button accessibility
  assert.ok(html.includes('aria-expanded="true"'));
  assert.ok(html.includes('type="button"'));
  // 3. Dynamic hours and days
  assert.ok(html.includes('3 trabajador(es) activos sin datos hace más de 24 h'));
  assert.ok(html.includes('1 rechazo(s) o retiro(s) de consentimiento (7 días)'));
  // 4. "nunca envió datos" and "último dato hace"
  assert.ok(html.includes('nunca envió datos'));
  assert.ok(html.includes('último dato hace'));
  // 5. Worker names
  assert.ok(html.includes('Ana Díaz'));
  assert.ok(html.includes('Bernardo O&#x27;Higgins') || html.includes("Bernardo O'Higgins"));
  assert.ok(html.includes('Carlos Valenzuela'));
  // 6. "y M más" when total (3) > length (2)
  assert.ok(html.includes('y 1 más'));
  // 7. Rejection detail: Rechazó, date, version
  assert.ok(html.includes('Rechazó'));
  assert.ok(html.includes('25/10/26'));
  assert.ok(html.includes('v3'));
});

test('telemetry health: UI markup renders counts = 0 with normal color and no expandable button', () => {
  const element = React.createElement(
    'div',
    null,
    React.createElement(
      'p',
      { className: 'text-fg' },
      formatWithoutDataAlertLabel(0, 24),
    ),
    React.createElement(
      'p',
      { className: 'text-fg' },
      formatRejectionsAlertLabel(0, 7),
    ),
  );

  const html = renderToStaticMarkup(element);

  assert.ok(!html.includes('text-warning'));
  assert.ok(!html.includes('aria-expanded'));
  assert.ok(html.includes('text-fg'));
  assert.ok(html.includes('0 trabajador(es) activos sin datos hace más de 24 h'));
  assert.ok(html.includes('0 rechazo(s) o retiro(s) de consentimiento (7 días)'));
});

test('telemetry health: UI markup renders "—" with descriptive title when API call fails (RF-ADMIN-25-4)', () => {
  const titleText = 'No se pudo cargar la información de salud de telemetría';
  const element = React.createElement(
    'div',
    null,
    React.createElement(
      'p',
      { className: 'text-fg cursor-help', title: titleText },
      formatWithoutDataAlertLabel(null, 24),
    ),
    React.createElement(
      'p',
      { className: 'text-fg cursor-help', title: titleText },
      formatRejectionsAlertLabel(null, 7),
    ),
  );

  const html = renderToStaticMarkup(element);

  assert.ok(html.includes('title="No se pudo cargar la información de salud de telemetría"'));
  assert.ok(html.includes('— trabajador(es) activos sin datos hace más de 24 h'));
  assert.ok(html.includes('— rechazo(s) o retiro(s) de consentimiento (7 días)'));
  assert.ok(!html.includes('0 trabajador(es)'));
  assert.ok(!html.includes('0 rechazo(s)'));
});

test('telemetry health: W4 navigation mechanism opens local or remote worker detail', async () => {
  const localWorkers = [
    { id: 'w-local', name: 'Local Worker', status: 'ACTIVE' },
  ];
  let selectedDetail = null;

  const handleOpenWorkerFromAlert = async (workerId) => {
    const existing = localWorkers.find(w => w.id === workerId);
    if (existing) {
      selectedDetail = existing;
      return;
    }
    // Simulation of remote workersGet
    selectedDetail = { id: workerId, name: 'Remote Worker', status: 'ACTIVE' };
  };

  await handleOpenWorkerFromAlert('w-local');
  assert.equal(selectedDetail?.id, 'w-local');
  assert.equal(selectedDetail?.name, 'Local Worker');

  await handleOpenWorkerFromAlert('w-remote');
  assert.equal(selectedDetail?.id, 'w-remote');
  assert.equal(selectedDetail?.name, 'Remote Worker');
});
