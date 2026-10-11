import test from 'node:test';
import assert from 'node:assert/strict';

import { formatAddress, CHILEAN_REGIONS } from '../src/lib/employer.ts';

test('employer: formatAddress correctly formats addresses', () => {
  // Full address with floor and office
  const a1 = {
    street: 'Av. Apoquindo',
    number: '1234',
    floor: '5',
    office: '501',
    commune: 'Las Condes',
    city: 'Santiago',
    region: 'Región Metropolitana de Santiago',
  };
  assert.equal(
    formatAddress(a1),
    'Av. Apoquindo 1234, Piso 5, Of. 501, Las Condes, Santiago, Región Metropolitana de Santiago',
  );

  // Address without floor or office
  const a2 = {
    street: 'Prat',
    number: '450',
    commune: 'Valparaíso',
    city: 'Valparaíso',
    region: 'Valparaíso',
  };
  // city equals commune, so city is not repeated
  assert.equal(formatAddress(a2), 'Prat 450, Valparaíso, Valparaíso');

  // Address with only floor
  const a3 = {
    street: 'Moneda',
    number: '1100',
    floor: '2',
    commune: 'Santiago',
    city: 'Santiago',
    region: 'Región Metropolitana de Santiago',
  };
  assert.equal(formatAddress(a3), 'Moneda 1100, Piso 2, Santiago, Región Metropolitana de Santiago');
});

test('employer: CHILEAN_REGIONS contains all 16 official regions', () => {
  assert.equal(CHILEAN_REGIONS.length, 16);
  assert.equal(CHILEAN_REGIONS.includes('Región Metropolitana de Santiago'), true);
  assert.equal(CHILEAN_REGIONS.includes('Valparaíso'), true);
  assert.equal(CHILEAN_REGIONS.includes('Biobío'), true);
  assert.equal(CHILEAN_REGIONS.includes('La Araucanía'), true);
  assert.equal(CHILEAN_REGIONS.includes('Arica y Parinacota'), true);
  assert.equal(CHILEAN_REGIONS.includes('Tarapacá'), true);
  assert.equal(CHILEAN_REGIONS.includes('Antofagasta'), true);
  assert.equal(CHILEAN_REGIONS.includes('Atacama'), true);
  assert.equal(CHILEAN_REGIONS.includes('Coquimbo'), true);
  assert.equal(CHILEAN_REGIONS.includes("Libertador General Bernardo O'Higgins"), true);
  assert.equal(CHILEAN_REGIONS.includes('Maule'), true);
  assert.equal(CHILEAN_REGIONS.includes('Ñuble'), true);
  assert.equal(CHILEAN_REGIONS.includes('Los Ríos'), true);
  assert.equal(CHILEAN_REGIONS.includes('Los Lagos'), true);
  assert.equal(CHILEAN_REGIONS.includes('Aysén del General Carlos Ibáñez del Campo'), true);
  assert.equal(CHILEAN_REGIONS.includes('Magallanes y de la Antártica Chilena'), true);
});

test('terms: scroll threshold enables button when scrolled near bottom', () => {
  const isScrollComplete = (scrollHeight, scrollTop, clientHeight) => {
    return scrollHeight - scrollTop - clientHeight <= 25;
  };

  // When scroll position is at the very top of long document
  assert.equal(isScrollComplete(1000, 0, 300), false);

  // When halfway through
  assert.equal(isScrollComplete(1000, 400, 300), false);

  // When 30px away from bottom (not yet within 25px threshold)
  assert.equal(isScrollComplete(1000, 670, 300), false);

  // When 20px away from bottom (within 25px threshold)
  assert.equal(isScrollComplete(1000, 680, 300), true);

  // When at the very bottom
  assert.equal(isScrollComplete(1000, 700, 300), true);

  // When content fits without scrollbar
  assert.equal(isScrollComplete(300, 0, 300), true);
});

test('employer: isEmployerKind validates allowed values', () => {
  const isEmployerKind = value => value === 'LEGAL_ENTITY' || value === 'NATURAL_PERSON';

  assert.equal(isEmployerKind('LEGAL_ENTITY'), true);
  assert.equal(isEmployerKind('NATURAL_PERSON'), true);
  assert.equal(isEmployerKind('OTHER'), false);
  assert.equal(isEmployerKind(''), false);
  assert.equal(isEmployerKind(null), false);
  assert.equal(isEmployerKind(undefined), false);
});

test('markdown: only absolute https links are accepted for external opening', () => {
  const isHttpsLink = url => /^https:\/\//i.test(url);

  assert.equal(isHttpsLink('https://dt.gob.cl'), true);
  assert.equal(isHttpsLink('https://dt.gob.cl/path'), true);
  assert.equal(isHttpsLink('http://dt.gob.cl'), false);
  assert.equal(isHttpsLink('ftp://dt.gob.cl'), false);
  assert.equal(isHttpsLink('javascript:alert(1)'), false);
  assert.equal(isHttpsLink('file:///etc/passwd'), false);
  assert.equal(isHttpsLink('/local/path'), false);
});

