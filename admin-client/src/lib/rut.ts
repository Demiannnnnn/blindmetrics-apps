/**
 * Chilean RUN / RUT parser, validator, and canonical formatter.
 *
 * Direct TypeScript mirror of `telemetry_crypto_core::rut` (RF-ADMIN-13-5, SEC-20, ATT-03).
 * Enforces modulo 11 check digit verification and canonical representation `12.345.678-5`.
 */

export const MIN_RUT_BODY = 1_000_000;
export const MAX_RUT_BODY = 99_999_999;

export interface ParsedRut {
  body: number;
  dv: string;
  formatted: string;
  bodyDigits: string;
}

/**
 * Computes the Chilean modulo 11 check digit for a given RUT body number.
 * Returns '0'..'9' or 'K'.
 */
export function computeDv(body: number): string {
  let sum = 0;
  let multiplier = 2;
  let temp = Math.floor(body);

  while (temp > 0) {
    const digit = temp % 10;
    sum += digit * multiplier;
    temp = Math.floor(temp / 10);
    multiplier = multiplier === 7 ? 2 : multiplier + 1;
  }

  const remainder = sum % 11;
  const dv = 11 - remainder;

  if (dv === 11) return '0';
  if (dv === 10) return 'K';
  return String(dv);
}

/**
 * Strips formatting dots, spaces, and hyphens from a RUT string.
 */
export function cleanRut(input: string): string {
  return input.replace(/[.\s-]/g, '').toUpperCase();
}

/**
 * Formats a valid RUT body and check digit into canonical representation `12.345.678-5`.
 * Also accepts a single string argument and parses/canonicalizes it.
 */
export function formatRut(bodyOrString: number | string, dv?: string): string {
  if (typeof bodyOrString === 'number' && dv !== undefined) {
    const s = String(bodyOrString);
    const len = s.length;
    const upperDv = dv.toUpperCase();

    if (len === 8) {
      return `${s.slice(0, 2)}.${s.slice(2, 5)}.${s.slice(5, 8)}-${upperDv}`;
    }
    if (len === 7) {
      return `${s.slice(0, 1)}.${s.slice(1, 4)}.${s.slice(4, 7)}-${upperDv}`;
    }
    return `${s}-${upperDv}`;
  }

  try {
    return parseRut(String(bodyOrString)).formatted;
  } catch {
    return cleanRut(String(bodyOrString));
  }
}

/**
 * Parses and validates a Chilean RUT / RUN from a string.
 *
 * Accepts:
 * - Formatted: "12.345.678-5"
 * - Without dots: "12345678-5"
 * - Raw digits: "123456785"
 * - Lowercase or uppercase check digit 'k' / 'K'
 *
 * Throws an Error if invalid.
 */
export function parseRut(input: string): ParsedRut {
  const trimmed = input.trim();
  if (!trimmed) {
    throw new Error('El RUT no puede estar vacío.');
  }

  let bodyStr: string;
  let dvChar: string;

  if (trimmed.includes('-')) {
    const parts = trimmed.split('-');
    if (parts.length !== 2) {
      throw new Error('Formato de RUT inválido.');
    }
    const dClean = parts[1].trim();
    if (dClean.length !== 1) {
      throw new Error('El dígito verificador debe ser de un carácter.');
    }
    bodyStr = parts[0];
    dvChar = dClean.toUpperCase();
  } else {
    const dotsRemoved = trimmed.replace(/\./g, '');
    if (dotsRemoved.length < 2) {
      throw new Error('El RUT es demasiado corto.');
    }
    dvChar = dotsRemoved.slice(-1).toUpperCase();
    bodyStr = dotsRemoved.slice(0, -1);
  }

  const cleanBody = bodyStr.replace(/\./g, '');
  if (!/^\d+$/.test(cleanBody)) {
    throw new Error('El cuerpo del RUT solo debe contener dígitos.');
  }

  const bodyNum = parseInt(cleanBody, 10);
  if (bodyNum < MIN_RUT_BODY || bodyNum > MAX_RUT_BODY) {
    throw new Error(
      `El RUT ${bodyNum} está fuera del rango válido (1.000.000 a 99.999.999).`
    );
  }

  if (!/^[0-9K]$/.test(dvChar)) {
    throw new Error('El dígito verificador debe ser un número o K.');
  }

  const expectedDv = computeDv(bodyNum);
  if (dvChar !== expectedDv) {
    throw new Error(
      `Dígito verificador inválido: se esperaba ${expectedDv}, se recibió ${dvChar}.`
    );
  }

  return {
    body: bodyNum,
    dv: dvChar,
    formatted: formatRut(bodyNum, dvChar),
    bodyDigits: String(bodyNum),
  };
}

/**
 * Checks if a string is a valid Chilean RUT.
 */
export function validateRut(input: string): boolean {
  try {
    parseRut(input);
    return true;
  } catch {
    return false;
  }
}

/**
 * Helper for live input formatting.
 * Returns formatted RUT if parseable, or cleaned input as user types.
 */
export function formatRutLive(input: string): string {
  try {
    return parseRut(input).formatted;
  } catch {
    // If not fully valid yet, format best-effort with dots and hyphen
    const cleaned = cleanRut(input);
    if (cleaned.length <= 1) return cleaned;
    const dv = cleaned.slice(-1);
    const body = cleaned.slice(0, -1);
    if (!/^\d+$/.test(body)) return input;
    const bodyNum = parseInt(body, 10);
    return formatRut(bodyNum, dv);
  }
}
