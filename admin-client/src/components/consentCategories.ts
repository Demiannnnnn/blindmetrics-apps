import type {
  ConsentDescriptions,
  ConsentFlags,
  ConsentRecord,
  WorkerStatus,
  TelemetryStatus,
  WorkMode,
} from '../api/types';

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

/** Labels for evidence decisions (granular consent + legacy package rows). */
export const DECISION_LABELS: Record<string, string> = {
  ACCEPTED: 'Aceptó',
  REJECTED: 'Rechazó',
  WITHDRAWN: 'Retiró',
  NARROWED: 'Reducción del admin',
  ACCEPT: 'Aceptó',
  REJECT: 'Rechazó',
  WITHDRAW: 'Retiró',
};

export function decisionLabel(decision: string): string {
  return DECISION_LABELS[decision] ?? decision;
}

/**
 * Categories accepted in an evidence row (RF-ADMIN-09-7). `accepted_consent_flags = null`
 * means "all requested" for ACCEPTED rows written before granular consent and "none" for
 * rejections.
 */
export function acceptedCategoriesOf(record: ConsentRecord): ConsentKey[] {
  const decision = record.decision;
  const flags =
    record.accepted_consent_flags ??
    (decision === 'ACCEPTED' || decision === 'ACCEPT' ? record.consent_flags ?? null : null);
  if (!flags) return [];
  return CONSENT_CATEGORIES.filter(c => flags[c.key]).map(c => c.key);
}

export function categoryLabel(key: ConsentKey): string {
  return CONSENT_CATEGORIES.find(c => c.key === key)?.label ?? key;
}

/** "Aceptó n de m categorías solicitadas" (RF-ADMIN-09-8). */
export function consentSummary(effective: ConsentFlags, requested: ConsentFlags): { accepted: number; requested: number } {
  let accepted = 0;
  let requestedCount = 0;
  for (const { key } of CONSENT_CATEGORIES) {
    if (requested[key]) {
      requestedCount += 1;
      if (effective[key]) accepted += 1;
    }
  }
  return { accepted, requested: requestedCount };
}

/** CSV cell: quotes when needed and neutralizes spreadsheet formulas (=, +, -, @, tab, CR). */
export function csvCell(v: string | number | null | undefined): string {
  let s = v === null || v === undefined ? '' : String(v);
  if (/^[=+\-@\t\r]/.test(s)) s = `'${s}`;
  return /[",\n\r']/.test(s) ? `"${s.replace(/"/g, '""')}"` : s;
}

/** RF-ADMIN-08-8 / RF-ADMIN-09-7: consent evidence export for legal requests. */
export function consentRecordsToCsv(records: ConsentRecord[]): string {
  const header = [
    'created_at',
    'consent_version',
    'decision',
    'accepted_categories',
    'ip_address',
    'user_agent',
    'app_version',
    'manifest_hash',
  ];
  const rows = records.map(r =>
    [
      r.created_at,
      r.consent_version,
      r.decision,
      acceptedCategoriesOf(r).join(' '),
      r.ip_address,
      r.user_agent,
      r.app_version ?? null,
      r.manifest_hash,
    ]
      .map(csvCell)
      .join(','),
  );
  return [header.join(','), ...rows].join('\n');
}

export const STATUS_BADGES: Record<WorkerStatus, { label: string; className: string }> = {
  ACTIVE: { label: 'Activo', className: 'bg-success-subtle text-success border-default' },
  PENDING_TERMS: {
    label: 'Esperando términos',
    className: 'bg-info-subtle text-info border-default',
  },
  PENDING_ENROLLMENT: { label: 'Invitado', className: 'bg-surface-subtle text-fg border-strong' },
  REVOKED: { label: 'Revocado', className: 'bg-danger-subtle text-danger border-default' },
};

export function getTelemetryBadge(
  telemetryStatus: TelemetryStatus,
  consentFlags: ConsentFlags,
  requestedFlags: ConsentFlags,
): { label: string; className: string } {
  const summary = consentSummary(consentFlags, requestedFlags);
  switch (telemetryStatus) {
    case 'NOT_REQUESTED':
      return { label: 'No solicitada', className: 'bg-surface text-muted border-default' };
    case 'PENDING_DECISION':
      return { label: 'Pendiente', className: 'bg-info-subtle text-info border-default' };
    case 'ACCEPTED':
      return {
        label: `${summary.accepted}/${summary.requested} aceptadas`,
        className: 'bg-success-subtle text-success border-default',
      };
    case 'DECLINED':
      return { label: 'Rechazada', className: 'bg-danger-subtle text-danger border-default' };
    case 'PAUSED':
      return { label: 'Pausada', className: 'bg-warning-subtle text-warning border-default' };
    default:
      return { label: 'No solicitada', className: 'bg-surface text-muted border-default' };
  }
}

/** Verifies Chilean RUN check digit (modulo 11). */
export function isValidRut(input: string): boolean {
  const clean = input.replace(/[.\s-]/g, '').toUpperCase();
  if (clean.length < 2) return false;
  const dv = clean.slice(-1);
  const body = clean.slice(0, -1);
  if (!/^\d+$/.test(body)) return false;
  const num = parseInt(body, 10);
  if (num < 1_000_000 || num > 99_999_999) return false;
  let sum = 0, mult = 2, temp = num;
  while (temp > 0) {
    sum += (temp % 10) * mult;
    temp = Math.floor(temp / 10);
    mult = mult === 7 ? 2 : mult + 1;
  }
  const rem = 11 - (sum % 11);
  const expected = rem === 11 ? '0' : rem === 10 ? 'K' : String(rem);
  return dv === expected;
}

export interface WorkerIdentityInput {
  name: string;
  run: string;
  personalEmail: string;
  jobTitle: string;
}

export function validateWorkerIdentity(input: WorkerIdentityInput): string | null {
  const name = input.name.trim();
  if (!name || name.length < 2 || name.length > 255) {
    return 'El nombre completo debe tener entre 2 y 255 caracteres.';
  }

  const run = input.run.trim();
  if (!run) {
    return 'El RUN del trabajador es obligatorio.';
  }
  if (!isValidRut(run)) {
    return 'El RUN ingresado es inválido (módulo 11).';
  }

  const email = input.personalEmail.trim();
  if (!email || !/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email)) {
    return 'Se requiere un correo personal válido para el envío de comprobantes.';
  }

  const jobTitle = input.jobTitle.trim();
  if (!jobTitle || jobTitle.length < 2 || jobTitle.length > 100) {
    return 'El cargo debe tener entre 2 y 100 caracteres.';
  }

  return null;
}

export function validateWorkerWorkplaceAndMode(workplaceId: string, workMode: string): string | null {
  if (!workplaceId || !workplaceId.trim()) {
    return 'Selecciona un lugar de trabajo.';
  }
  if (!['ON_SITE', 'REMOTE', 'HYBRID'].includes(workMode)) {
    return 'Modalidad de trabajo inválida.';
  }
  return null;
}

export function validateWorkerTelemetry(
  workMode: WorkMode,
  role: 'OFFICE' | 'FIELD',
  consentFlags: ConsentFlags,
  locationCause?: string | null,
): string | null {
  // DP-05: screenshots not allowed for REMOTE or HYBRID
  if (workMode !== 'ON_SITE' && consentFlags.productivity_screenshots) {
    return 'Las capturas de pantalla están deshabilitadas para teletrabajo o modalidad mixta (DP-05, R38 art. 55 a).';
  }

  // DP-04: location requires FIELD role
  if (consentFlags.location && role !== 'FIELD') {
    return 'La telemetría de ubicación solo está permitida para rol en terreno (FIELD) (DP-04).';
  }

  // DP-04: location requires location_cause between 20 and 500 chars
  if (consentFlags.location) {
    const cause = (locationCause || '').trim();
    if (!cause || cause.length < 20 || cause.length > 500) {
      return 'La solicitud de ubicación requiere declarar una causa de seguridad o legal de entre 20 y 500 caracteres (DP-04).';
    }
  }

  return null;
}

export function validateEnrollmentTtl(ttlHours: number): string | null {
  if (isNaN(ttlHours) || ttlHours < 1 || ttlHours > 72) {
    return 'El código de vinculación debe tener una vigencia de entre 1 y 72 horas.';
  }
  return null;
}

