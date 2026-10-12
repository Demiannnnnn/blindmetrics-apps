import test from 'node:test';
import assert from 'node:assert/strict';

// Imports the real module (Node >= 22.18 strips TypeScript types natively).
import {
  acceptedCategoriesOf,
  consentSummary,
  csvCell,
  decisionLabel,
  consentRecordsToCsv,
  descriptionErrors,
  normalizeDescriptions,
  workHoursError,
  isValidTimezone,
  NO_CONSENT,
  NO_DESCRIPTIONS,
  STATUS_BADGES,
  getTelemetryBadge,
  validateWorkerIdentity,
  validateWorkerWorkplaceAndMode,
  validateWorkerTelemetry,
  validateEnrollmentTtl,
} from '../src/components/consentCategories.ts';

test('workers: malformed work hours are rejected before any IPC call', () => {
  assert.notEqual(workHoursError('25:99', '18:00'), null);
  assert.notEqual(workHoursError('9:00', '18:00'), null);
  assert.notEqual(workHoursError('18:00', '09:00'), null);
  assert.notEqual(workHoursError('09:00', '09:00'), null);
  assert.equal(workHoursError('09:00', '18:00'), null);
  assert.equal(workHoursError('09:00:00', '18:00:00'), null);
});

test('workers: timezone must be a real IANA zone', () => {
  assert.equal(isValidTimezone('America/Santiago'), true);
  assert.equal(isValidTimezone('Mars/Olympus'), false);
  assert.equal(isValidTimezone(''), false);
});

test('workers: consent defaults collect nothing (DP-08)', () => {
  assert.ok(Object.values(NO_CONSENT).every(v => v === false));
});

test('workers: enabled category requires a 10-500 char description', () => {
  const flags = { ...NO_CONSENT, system_activity: true };
  assert.ok(descriptionErrors(flags, { ...NO_DESCRIPTIONS, system_activity: 'corto' }).system_activity);
  assert.deepEqual(descriptionErrors(flags, { ...NO_DESCRIPTIONS, system_activity: 'Aplicaciones en uso' }), {});
  assert.ok(descriptionErrors(flags, { ...NO_DESCRIPTIONS, system_activity: 'x'.repeat(501) }).system_activity);
});

test('workers: descriptions of disabled categories are dropped', () => {
  const flags = { ...NO_CONSENT, network_activity: true };
  const out = normalizeDescriptions(flags, {
    ...NO_DESCRIPTIONS,
    network_activity: '  Dominios visitados  ',
    location: 'no debería enviarse',
  });
  assert.equal(out.network_activity, 'Dominios visitados');
  assert.equal(out.location, null);
});

test('workers: consent evidence CSV escapes quotes, commas and nulls', () => {
  const csv = consentRecordsToCsv([
    {
      id: 'r1',
      worker_id: 'w1',
      consent_version: 2,
      decision: 'ACCEPT',
      manifest_hash: 'ab'.repeat(32),
      ip_address: null,
      user_agent: 'Agent "x", v1',
      created_at: '2026-09-29T10:00:00Z',
    },
  ]);
  const [header, row] = csv.split('\n');
  assert.equal(
    header,
    'created_at,consent_version,decision,accepted_categories,ip_address,user_agent,app_version,manifest_hash',
  );
  assert.equal(row, `2026-09-29T10:00:00Z,2,ACCEPT,,,"Agent ""x"", v1",,${'ab'.repeat(32)}`);
});

test('workers: granular consent evidence labels and accepted categories (ADMIN-09)', () => {
  const requested = { ...NO_CONSENT, system_activity: true, network_activity: true, file_activity: true };
  const chosen = { ...NO_CONSENT, system_activity: true };
  assert.equal(decisionLabel('ACCEPTED'), 'Aceptó');
  assert.equal(decisionLabel('NARROWED'), 'Reducción del admin');
  assert.deepEqual(
    acceptedCategoriesOf({ decision: 'ACCEPTED', consent_flags: requested, accepted_consent_flags: chosen }),
    ['system_activity'],
  );
  // Rows written before granular consent: null = everything requested was accepted.
  assert.deepEqual(
    acceptedCategoriesOf({ decision: 'ACCEPTED', consent_flags: requested, accepted_consent_flags: null }),
    ['system_activity', 'network_activity', 'file_activity'],
  );
  assert.deepEqual(acceptedCategoriesOf({ decision: 'REJECTED', consent_flags: requested, accepted_consent_flags: null }), []);
  assert.deepEqual(consentSummary(chosen, requested), { accepted: 1, requested: 3 });
});

test('workers: evidence CSV neutralizes spreadsheet formulas', () => {
  assert.equal(csvCell('=HYPERLINK("x")'), `"'=HYPERLINK(""x"")"`);
  assert.equal(csvCell('+1'), `"'+1"`);
  assert.equal(csvCell('@SUM(A1)'), `"'@SUM(A1)"`);
  assert.equal(csvCell('telemetry-agent/0.3.0'), 'telemetry-agent/0.3.0');
  const csv = consentRecordsToCsv([
    {
      id: 'r', worker_id: 'w', consent_version: 2, decision: 'ACCEPTED',
      consent_flags: { ...NO_CONSENT, system_activity: true },
      accepted_consent_flags: { ...NO_CONSENT, system_activity: true },
      manifest_hash: 'ab', ip_address: '203.0.113.7', user_agent: '=cmd', app_version: '0.3.0',
      created_at: '2026-09-30T10:00:00Z',
    },
  ]);
  const [header, row] = csv.split('\n');
  assert.equal(header, 'created_at,consent_version,decision,accepted_categories,ip_address,user_agent,app_version,manifest_hash');
  assert.match(row, /system_activity/);
  assert.match(row, /"'=cmd"/);
});

test('workers: status badges map correctly to v3 specifications (RF-ADMIN-14-1)', () => {
  assert.equal(STATUS_BADGES.ACTIVE.label, 'Activo');
  assert.equal(STATUS_BADGES.PENDING_TERMS.label, 'Esperando términos');
  assert.equal(STATUS_BADGES.PENDING_ENROLLMENT.label, 'Invitado');
  assert.equal(STATUS_BADGES.REVOKED.label, 'Revocado');
});

test('workers: telemetry badges reflect state and accepted count (RF-ADMIN-14-1)', () => {
  const empty = { ...NO_CONSENT };
  const requested3 = { ...NO_CONSENT, system_activity: true, network_activity: true, productivity_basic: true };
  const accepted2 = { ...NO_CONSENT, system_activity: true, network_activity: true };

  assert.equal(getTelemetryBadge('NOT_REQUESTED', empty, empty).label, 'No solicitada');
  assert.equal(getTelemetryBadge('PENDING_DECISION', empty, requested3).label, 'Pendiente');
  assert.equal(getTelemetryBadge('ACCEPTED', accepted2, requested3).label, '2/3 aceptadas');
  assert.equal(getTelemetryBadge('DECLINED', empty, requested3).label, 'Rechazada');
  assert.equal(getTelemetryBadge('PAUSED', accepted2, requested3).label, 'Pausada');
});

test('workers: W2 identity validation requires valid RUN, email, and job title', () => {
  // Valid
  assert.equal(
    validateWorkerIdentity({
      name: 'Juan Pérez',
      run: '12.345.678-5',
      personalEmail: 'juan.perez@example.com',
      jobTitle: 'Analista de Operaciones',
    }),
    null,
  );

  // Invalid RUN check digit
  assert.match(
    validateWorkerIdentity({
      name: 'Juan Pérez',
      run: '12.345.678-9',
      personalEmail: 'juan.perez@example.com',
      jobTitle: 'Analista',
    }),
    /RUN ingresado es inválido/,
  );

  // Invalid email
  assert.match(
    validateWorkerIdentity({
      name: 'Juan Pérez',
      run: '12.345.678-5',
      personalEmail: 'not-an-email',
      jobTitle: 'Analista',
    }),
    /correo personal válido/,
  );

  // Short job title
  assert.match(
    validateWorkerIdentity({
      name: 'Juan Pérez',
      run: '12.345.678-5',
      personalEmail: 'juan.perez@example.com',
      jobTitle: 'A',
    }),
    /cargo debe tener entre 2 y 100/,
  );
});

test('workers: W2 telemetry restrictions enforce DP-04, DP-05 and allow all-false', () => {

  // All false allowed (attendance-only worker, DP-08)
  assert.equal(validateWorkerTelemetry('REMOTE', 'OFFICE', NO_CONSENT, null), null);

  // DP-05: screenshots forbidden for REMOTE and HYBRID
  const screenshotFlags = { ...NO_CONSENT, productivity_screenshots: true };
  assert.match(
    validateWorkerTelemetry('REMOTE', 'OFFICE', screenshotFlags, null),
    /capturas de pantalla están deshabilitadas para teletrabajo/i,
  );
  assert.match(
    validateWorkerTelemetry('HYBRID', 'OFFICE', screenshotFlags, null),
    /capturas de pantalla están deshabilitadas para teletrabajo/i,
  );
  // DP-05: screenshots allowed for ON_SITE
  assert.equal(validateWorkerTelemetry('ON_SITE', 'OFFICE', screenshotFlags, null), null);

  // DP-04: location requires FIELD role
  const locationFlags = { ...NO_CONSENT, location: true };
  assert.match(
    validateWorkerTelemetry('ON_SITE', 'OFFICE', locationFlags, 'Supervisión en terreno de rutas'),
    /solo está permitida para rol en terreno \(FIELD\)/i,
  );

  // DP-04: location requires location_cause between 20 and 500 chars
  assert.match(
    validateWorkerTelemetry('ON_SITE', 'FIELD', locationFlags, 'Muy corta'),
    /entre 20 y 500 caracteres/i,
  );
  assert.equal(
    validateWorkerTelemetry(
      'ON_SITE',
      'FIELD',
      locationFlags,
      'Monitoreo de seguridad de técnicos en cuadrillas de terreno durante horario laboral',
    ),
    null,
  );

  // TTL validation: 1 to 72 hours
  assert.equal(validateEnrollmentTtl(24), null);
  assert.match(validateEnrollmentTtl(0), /entre 1 y 72 horas/);
  assert.match(validateEnrollmentTtl(73), /entre 1 y 72 horas/);
});

