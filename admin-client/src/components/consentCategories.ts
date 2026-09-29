import type { ConsentDescriptions, ConsentFlags, ConsentRecord } from '../api/types';

export type ConsentKey = keyof ConsentFlags;

export interface ConsentCategory {
  key: ConsentKey;
  label: string;
  /** Editable starting text for the description shown verbatim to the worker (C1). */
  template: string;
}

// Order and wording follow docs/admin-app-spec.md §W2 and the agent C1 screen.
export const CONSENT_CATEGORIES: ConsentCategory[] = [
  {
    key: 'system_activity',
    label: 'Aplicaciones en uso',
    template:
      'Registramos qué aplicaciones usas y el uso de CPU/memoria de tu equipo durante el horario laboral.',
  },
  {
    key: 'network_activity',
    label: 'Navegación web (solo dominios)',
    template:
      'Registramos los dominios de internet que visitas (p. ej. github.com), nunca direcciones completas ni el contenido de las páginas.',
  },
  {
    key: 'productivity_basic',
    label: 'Actividad de teclado y mouse (conteos)',
    template: 'Contamos cuántas teclas y clics haces por minuto, nunca lo que escribes.',
  },
  {
    key: 'productivity_screenshots',
    label: 'Capturas de pantalla',
    template:
      'Tomamos una captura de pantalla cada ~15 minutos durante el horario laboral. Se te notifica cada vez.',
  },
  {
    key: 'file_activity',
    label: 'Actividad de archivos (metadatos)',
    template:
      'Registramos cuándo se crean, modifican o eliminan archivos en la carpeta de trabajo designada, nunca su contenido.',
  },
  {
    key: 'location',
    label: 'Ubicación (solo FIELD)',
    template: 'Registramos tu ubicación solo durante el horario laboral, para rutas de terreno.',
  },
];

export const DESCRIPTION_MIN = 10;
export const DESCRIPTION_MAX = 500;

// DP-08: nothing is collected unless the admin explicitly enables it.
export const NO_CONSENT: ConsentFlags = {
  system_activity: false,
  network_activity: false,
  productivity_basic: false,
  productivity_screenshots: false,
  file_activity: false,
  location: false,
};

export const NO_DESCRIPTIONS: ConsentDescriptions = {
  system_activity: null,
  network_activity: null,
  productivity_basic: null,
  productivity_screenshots: null,
  file_activity: null,
  location: null,
};

/** Returns an error message per enabled category whose description is out of range. */
export function descriptionErrors(
  flags: ConsentFlags,
  descriptions: ConsentDescriptions,
): Partial<Record<ConsentKey, string>> {
  const errors: Partial<Record<ConsentKey, string>> = {};
  for (const { key } of CONSENT_CATEGORIES) {
    if (!flags[key]) continue;
    const len = (descriptions[key] || '').trim().length;
    if (len < DESCRIPTION_MIN || len > DESCRIPTION_MAX) {
      errors[key] = `La descripción debe tener entre ${DESCRIPTION_MIN} y ${DESCRIPTION_MAX} caracteres.`;
    }
  }
  return errors;
}

/** Keeps only descriptions of enabled categories, trimmed (server rejects the rest). */
export function normalizeDescriptions(
  flags: ConsentFlags,
  descriptions: ConsentDescriptions,
): ConsentDescriptions {
  const out = { ...NO_DESCRIPTIONS };
  for (const { key } of CONSENT_CATEGORIES) {
    out[key] = flags[key] ? (descriptions[key] || '').trim() : null;
  }
  return out;
}

const HH_MM = /^([01]\d|2[0-3]):[0-5]\d(:[0-5]\d)?$/;

/** Client-side mirror of the Rust validation (RF-ADMIN-03 §4): HH:MM and start < end. */
export function workHoursError(start: string, end: string): string | null {
  if (!HH_MM.test(start) || !HH_MM.test(end)) return 'El horario debe tener formato HH:MM.';
  if (start.slice(0, 5) >= end.slice(0, 5)) return 'La hora de inicio debe ser anterior a la de término.';
  return null;
}

/** IANA zone list from the runtime when available (searchable datalist). */
export function ianaTimezones(): string[] {
  const intl = Intl as unknown as { supportedValuesOf?: (k: string) => string[] };
  try {
    return intl.supportedValuesOf ? intl.supportedValuesOf('timeZone') : [];
  } catch {
    return [];
  }
}

export function isValidTimezone(tz: string): boolean {
  if (!tz.trim()) return false;
  try {
    new Intl.DateTimeFormat('en-US', { timeZone: tz.trim() });
    return true;
  } catch {
    return false;
  }
}

function csvCell(v: string | number | null): string {
  const s = v === null ? '' : String(v);
  return /[",\n]/.test(s) ? `"${s.replace(/"/g, '""')}"` : s;
}

/** RF-ADMIN-08-8: consent evidence export for legal requests. */
export function consentRecordsToCsv(records: ConsentRecord[]): string {
  const header = ['created_at', 'consent_version', 'decision', 'ip_address', 'user_agent', 'manifest_hash'];
  const rows = records.map(r =>
    [r.created_at, r.consent_version, r.decision, r.ip_address, r.user_agent, r.manifest_hash].map(csvCell).join(','),
  );
  return [header.join(','), ...rows].join('\n');
}
