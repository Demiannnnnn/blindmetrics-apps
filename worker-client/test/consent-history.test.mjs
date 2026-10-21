import test from 'node:test';
import assert from 'node:assert/strict';
import { consentDecisionLabel, consentRecordSummary, consentRequestedSummary } from '../src/lib/helpers.ts';

const requested = { system_activity: true, network_activity: true, location: false };

test('consent history: decisions in words (AGENT-17)', () => {
  assert.equal(consentDecisionLabel('ACCEPTED'), 'Aceptaste');
  assert.equal(consentDecisionLabel('WITHDRAWN'), 'Retiraste tu consentimiento');
  assert.equal(consentDecisionLabel('NARROWED'), 'Tu empleador redujo lo que solicita');
  assert.equal(consentDecisionLabel('SOMETHING_NEW'), 'SOMETHING_NEW');
  assert.equal(consentDecisionLabel('toString'), 'toString');
});

test('consent history: active categories per decision', () => {
  assert.equal(
    consentRecordSummary({ decision: 'ACCEPTED', consent_flags: requested, accepted_consent_flags: { system_activity: true, network_activity: false } }),
    'Activas: Actividad del sistema.'
  );
  // Rows written before granular consent: null = everything requested.
  assert.equal(
    consentRecordSummary({ decision: 'ACCEPTED', consent_flags: requested, accepted_consent_flags: null }),
    'Activas: Actividad del sistema, Actividad de red (FQDN).'
  );
  assert.equal(consentRecordSummary({ decision: 'REJECTED', consent_flags: requested, accepted_consent_flags: null }), 'Ninguna categoría activa.');
  assert.equal(
    consentRecordSummary({ decision: 'WITHDRAWN', consent_flags: requested, accepted_consent_flags: { system_activity: false } }),
    'Ninguna categoría activa.'
  );
  assert.equal(
    consentRecordSummary({ decision: 'NARROWED', consent_flags: requested, accepted_consent_flags: { network_activity: true, new_cat: true } }),
    'Activas: Actividad de red (FQDN), new_cat.'
  );
});

test('consent history: requested categories', () => {
  assert.equal(consentRequestedSummary(requested), 'Solicitadas: Actividad del sistema, Actividad de red (FQDN).');
  assert.equal(consentRequestedSummary({}), 'Sin categorías solicitadas.');
});
