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
  resolveWorkerFromAlert,
  TelemetryHealthAlerts,
  renderTelemetryHealthAlerts,
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

const sampleHealthData = {
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
        name: "Bernardo O'Higgins",
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

test('telemetry health: TelemetryHealthAlerts production component renders counts > 0 in collapsed state (RF-ADMIN-25-2, RF-ADMIN-25-3)', () => {
  const element = React.createElement(TelemetryHealthAlerts, {
    telemetryHealth: sampleHealthData,
    expandedWithoutData: false,
    expandedRejections: false,
  });

  const html = renderToStaticMarkup(element);

  // Verifications for collapsed state:
  assert.ok(html.includes('text-warning font-semibold'), 'must use warning color');
  assert.ok(html.includes('aria-expanded="false"'), 'both buttons must have aria-expanded="false"');
  assert.ok(html.includes('3 trabajador(es) activos sin datos hace más de 24 h'));
  assert.ok(html.includes('1 rechazo(s) o retiro(s) de consentimiento (7 días)'));
  // Lists must NOT be rendered when collapsed
  assert.ok(!html.includes('<ul'), 'worker lists should not be rendered when collapsed');
  assert.ok(!html.includes('Ana Díaz'), 'worker names should not be visible when collapsed');
});

test('telemetry health: TelemetryHealthAlerts production component renders counts > 0 in expanded state with worker list and "y M más" (RF-ADMIN-25-2, RF-ADMIN-25-3)', () => {
  const element = renderTelemetryHealthAlerts({
    telemetryHealth: sampleHealthData,
    expandedWithoutData: true,
    expandedRejections: true,
  });

  const html = renderToStaticMarkup(element);

  // Verifications for expanded state:
  assert.ok(html.includes('text-warning font-semibold'), 'must use warning color');
  assert.ok(html.includes('aria-expanded="true"'), 'buttons must have aria-expanded="true"');
  assert.ok(html.includes('type="button"'), 'must use accessible buttons');
  // Dynamic hours and days
  assert.ok(html.includes('3 trabajador(es) activos sin datos hace más de 24 h'));
  assert.ok(html.includes('1 rechazo(s) o retiro(s) de consentimiento (7 días)'));
  // Worker names and status
  assert.ok(html.includes('Ana Díaz'));
  assert.ok(html.includes('Bernardo O&#x27;Higgins') || html.includes("Bernardo O'Higgins"));
  assert.ok(html.includes('Carlos Valenzuela'));
  assert.ok(html.includes('nunca envió datos'));
  assert.ok(html.includes('último dato hace'));
  // "y M más"
  assert.ok(html.includes('y 1 más'));
  // Rejection detail
  assert.ok(html.includes('Rechazó'));
  assert.ok(html.includes('25/10/26'));
  assert.ok(html.includes('v3'));
});

test('telemetry health: TelemetryHealthAlerts renders counts = 0 with normal color and no expandable button (RF-ADMIN-25-2)', () => {
  const zeroHealthData = {
    generated_at: '2026-10-31T22:00:00Z',
    stale_hours: 24,
    since_days: 7,
    active_without_data: { total: 0, workers: [] },
    recent_rejections: { total: 0, workers: [] },
  };

  const element = React.createElement(TelemetryHealthAlerts, {
    telemetryHealth: zeroHealthData,
  });

  const html = renderToStaticMarkup(element);

  assert.ok(!html.includes('text-warning'), 'must not use warning color when count is 0');
  assert.ok(!html.includes('aria-expanded'), 'must not render expandable button when count is 0');
  assert.ok(html.includes('text-fg'), 'must use normal text color');
  assert.ok(html.includes('0 trabajador(es) activos sin datos hace más de 24 h'));
  assert.ok(html.includes('0 rechazo(s) o retiro(s) de consentimiento (7 días)'));
});

test('telemetry health: TelemetryHealthAlerts renders "—" with descriptive title when API call fails (RF-ADMIN-25-4)', () => {
  const element = React.createElement(TelemetryHealthAlerts, {
    telemetryHealth: null,
  });

  const html = renderToStaticMarkup(element);

  assert.ok(html.includes('title="No se pudo cargar la información de salud de telemetría"'));
  assert.ok(html.includes('— trabajador(es) activos sin datos hace más de 24 h'));
  assert.ok(html.includes('— rechazo(s) o retiro(s) de consentimiento (7 días)'));
  assert.ok(!html.includes('0 trabajador(es)'), 'must not display false 0');
  assert.ok(!html.includes('0 rechazo(s)'), 'must not display false 0');
  assert.ok(!html.includes('text-warning'), 'must not use warning color on error');
});

test('telemetry health: resolveWorkerFromAlert resolves local worker directly and fetches remote worker if absent (RF-ADMIN-25-3)', async () => {
  const localWorkers = [
    { id: 'w-local-1', name: 'Ana Díaz Local', status: 'ACTIVE' },
  ];
  let remoteFetchCalls = 0;
  const fetchRemote = async (id) => {
    remoteFetchCalls++;
    if (id === 'w-remote-2') {
      return { id: 'w-remote-2', name: 'Carlos Valenzuela Remote', status: 'ACTIVE' };
    }
    throw new Error('Worker not found');
  };

  // 1. Local resolution: found in localWorkers, remote fetch NOT invoked
  const localResult = await resolveWorkerFromAlert('w-local-1', localWorkers, fetchRemote);
  assert.equal(localResult?.id, 'w-local-1');
  assert.equal(localResult?.name, 'Ana Díaz Local');
  assert.equal(remoteFetchCalls, 0, 'remote fetcher must not be called when worker exists locally');

  // 2. Remote resolution: missing from localWorkers, calls remote fetcher
  const remoteResult = await resolveWorkerFromAlert('w-remote-2', localWorkers, fetchRemote);
  assert.equal(remoteResult?.id, 'w-remote-2');
  assert.equal(remoteResult?.name, 'Carlos Valenzuela Remote');
  assert.equal(remoteFetchCalls, 1, 'remote fetcher must be called when worker is absent locally');

  // 3. Graceful error handling: remote fetcher fails, returns null
  const missingResult = await resolveWorkerFromAlert('w-nonexistent', localWorkers, fetchRemote);
  assert.equal(missingResult, null, 'returns null when remote fetcher fails');
  assert.equal(remoteFetchCalls, 2);

  // 4. Missing remote fetcher: returns null without throwing
  const noFetcherResult = await resolveWorkerFromAlert('w-absent', localWorkers);
  assert.equal(noFetcherResult, null, 'returns null if fetchRemote is omitted');
});
