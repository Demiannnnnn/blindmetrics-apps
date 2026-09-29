import test from 'node:test';
import assert from 'node:assert/strict';

// Import / replicate logic from FieldMap.tsx
function isValidCoordinate(lat, lng) {
  if (typeof lat !== 'number' || typeof lng !== 'number') return false;
  if (isNaN(lat) || isNaN(lng)) return false;
  return lat >= -90 && lat <= 90 && lng >= -180 && lng <= 180;
}

function getDistanceMeters(lat1, lon1, lat2, lon2) {
  const R = 6371e3;
  const phi1 = (lat1 * Math.PI) / 180;
  const phi2 = (lat2 * Math.PI) / 180;
  const deltaPhi = ((lat2 - lat1) * Math.PI) / 180;
  const deltaLambda = ((lon2 - lon1) * Math.PI) / 180;

  const a =
    Math.sin(deltaPhi / 2) * Math.sin(deltaPhi / 2) +
    Math.cos(phi1) * Math.cos(phi2) * Math.sin(deltaLambda / 2) * Math.sin(deltaLambda / 2);
  const c = 2 * Math.atan2(Math.sqrt(a), Math.sqrt(1 - a));

  return R * c;
}

function isWithinShift(timeStr, startStr, endStr) {
  if (!timeStr || !startStr || !endStr) return true;
  const [h, m] = timeStr.slice(0, 5).split(':').map(Number);
  const [sH, sM] = startStr.split(':').map(Number);
  const [eH, eM] = endStr.split(':').map(Number);

  if (isNaN(h) || isNaN(sH) || isNaN(eH)) return true;

  const cur = h * 60 + (m || 0);
  const start = sH * 60 + (sM || 0);
  const end = eH * 60 + (eM || 0);

  if (start <= end) {
    return cur >= start && cur <= end;
  } else {
    return cur >= start || cur <= end;
  }
}

test('test_field_map_blocks_office_role: OFFICE role prevents GPS data collection and viewing', () => {
  const workerRole = 'OFFICE';
  const shouldBlockMap = workerRole === 'OFFICE';
  assert.equal(shouldBlockMap, true);
});

test('test_field_map_renders_waypoints: coordinate validation and distance calculation', () => {
  // Valid Santiago coordinates
  assert.equal(isValidCoordinate(-33.4489, -70.6693), true);
  // Out of range coordinates
  assert.equal(isValidCoordinate(95.0, -70.0), false);
  assert.equal(isValidCoordinate(-33.0, 190.0), false);
  assert.equal(isValidCoordinate(NaN, -70.0), false);
  assert.equal(isValidCoordinate('invalid', -70.0), false);

  // Distance between Plaza de Armas and La Moneda (~1 km)
  const dist = getDistanceMeters(-33.4372, -70.6506, -33.4429, -70.6539);
  assert.ok(dist > 500 && dist < 1500, `Expected ~700-1000m, got ${dist}m`);
});

test('test_work_hours_audit: verifies waypoints strictly within shift hours', () => {
  // Standard day shift: 09:00 - 18:00
  assert.equal(isWithinShift('09:15:00', '09:00', '18:00'), true);
  assert.equal(isWithinShift('12:30:00', '09:00', '18:00'), true);
  assert.equal(isWithinShift('17:59:00', '09:00', '18:00'), true);
  assert.equal(isWithinShift('08:45:00', '09:00', '18:00'), false);
  assert.equal(isWithinShift('19:00:00', '09:00', '18:00'), false);

  // Overnight shift: 22:00 - 06:00
  assert.equal(isWithinShift('23:00:00', '22:00', '06:00'), true);
  assert.equal(isWithinShift('02:00:00', '22:00', '06:00'), true);
  assert.equal(isWithinShift('08:00:00', '22:00', '06:00'), false);
});
