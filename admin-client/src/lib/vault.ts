// Key vault helpers (TASK-ADMIN-11 = TASK-APP-02). Mirrors telemetry-crypto-core rules:
// Master Password NFC-normalized, ≥ 12 characters (code points), ≤ 1024 UTF-8 bytes.

import type { KeyState } from '../api/types';

export const MIN_MASTER_PASSWORD_CHARS = 12;
export const MAX_MASTER_PASSWORD_BYTES = 1024;
export const NEW_KEY_PHRASE = 'GENERAR NUEVA LLAVE';
export const MAX_BACKUP_FILE_BYTES = 64 * 1024;

export type KeyScreen = 'CREATE' | 'REGISTER' | 'RECOVER' | 'MISMATCH' | 'READY';

export function keyStateScreen(state: KeyState): KeyScreen {
  switch (state) {
    case 'NO_KEY':
      return 'CREATE';
    case 'LOCAL_ONLY':
      return 'REGISTER';
    case 'MISSING_LOCAL':
      return 'RECOVER';
    case 'MISMATCH':
      return 'MISMATCH';
    default:
      return 'READY';
  }
}

/** Problem with a new Master Password, or null when valid. */
export function masterPasswordIssue(password: string, confirmation?: string): string | null {
  const normalized = password.normalize('NFC');
  if ([...normalized].length < MIN_MASTER_PASSWORD_CHARS) {
    return `La Master Password debe tener al menos ${MIN_MASTER_PASSWORD_CHARS} caracteres.`;
  }
  if (new TextEncoder().encode(normalized).length > MAX_MASTER_PASSWORD_BYTES) {
    return 'La Master Password es demasiado larga.';
  }
  if (confirmation !== undefined && confirmation.normalize('NFC') !== normalized) {
    return 'La confirmación no coincide con la Master Password.';
  }
  return null;
}

/** Rough strength hint (length and character classes); never blocks by itself. */
export function passwordStrength(password: string): 'débil' | 'aceptable' | 'fuerte' {
  const length = [...password.normalize('NFC')].length;
  const classes = [/[a-z]/, /[A-Z]/, /\d/, /[^A-Za-z0-9]/].filter(r => r.test(password)).length;
  if (length >= 20 || (length >= 16 && classes >= 3)) return 'fuerte';
  if (length >= MIN_MASTER_PASSWORD_CHARS && classes >= 2) return 'aceptable';
  return 'débil';
}

export function newKeyConfirmationValid(acknowledged: boolean, phrase: string): boolean {
  return acknowledged && phrase.trim() === NEW_KEY_PHRASE;
}

/** `aaa8fff703b50b22…` → `aaa8 fff7 03b5 0b22`. */
export function shortFingerprint(fp: string | null | undefined): string {
  if (!fp) return '—';
  return (fp.slice(0, 16).match(/.{1,4}/g) || []).join(' ');
}

/** Fingerprints are hex and case-insensitive. */
export function sameFingerprint(a: string | null | undefined, b: string | null | undefined): boolean {
  return !!a && !!b && a.trim().toLowerCase() === b.trim().toLowerCase();
}

export function backupFileIssue(file: { name: string; size: number } | null): string | null {
  if (!file) return 'Selecciona un archivo de respaldo.';
  if (file.size === 0 || file.size > MAX_BACKUP_FILE_BYTES) return 'El archivo no es un respaldo válido (.tkb o .vault).';
  if (!/\.(tkb|vault|json)$/i.test(file.name)) return 'Selecciona un archivo .tkb (o un .vault antiguo).';
  return null;
}
