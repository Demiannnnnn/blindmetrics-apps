// admin-client/src/lib/attendance.ts — R38 formatting, anomaly mapping and correction validation (TASK-ADMIN-15)

import type {
  AbsenceType,
  AttendanceMark,
  AttendanceState,
  DaySummary,
  MarkType,
  MarkSource,
  CorrectionKind,
  CorrectionStatus,
} from '../api/types';

/**
 * Formats a date in Chilean R38 official format: dd/mm/aa (e.g. 11/10/26).
 */
export function formatR38Date(isoOrDate: string | Date, timezone = 'America/Santiago'): string {
  try {
    const d = typeof isoOrDate === 'string' ? new Date(isoOrDate) : isoOrDate;
    if (isNaN(d.getTime())) return typeof isoOrDate === 'string' ? isoOrDate : '—';
    const formatter = new Intl.DateTimeFormat('es-CL', {
      timeZone: timezone,
      day: '2-digit',
      month: '2-digit',
      year: '2-digit',
    });
    return formatter.format(d).replace(/-/g, '/');
  } catch {
    return typeof isoOrDate === 'string' ? isoOrDate : '—';
  }
}

/**
 * Formats a server day (`YYYY-MM-DD`, already the worker's local date) as dd/mm/aa without any
 * time-zone conversion (parsing it as a UTC instant would show the previous day in Chile).
 */
export function formatR38DayDate(day: string): string {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(day.trim());
  if (!m) return formatR38Date(day);
  return `${m[3]}/${m[2]}/${m[1].slice(2)}`;
}

/**
 * Formats a date in full year: dd/mm/yyyy.
 */
export function formatR38DateFull(isoOrDate: string | Date, timezone = 'America/Santiago'): string {
  try {
    const d = typeof isoOrDate === 'string' ? new Date(isoOrDate) : isoOrDate;
    if (isNaN(d.getTime())) return typeof isoOrDate === 'string' ? isoOrDate : '—';
    const formatter = new Intl.DateTimeFormat('es-CL', {
      timeZone: timezone,
      day: '2-digit',
      month: '2-digit',
      year: 'numeric',
    });
    return formatter.format(d).replace(/-/g, '/');
  } catch {
    return typeof isoOrDate === 'string' ? isoOrDate : '—';
  }
}

/**
 * Formats a time in Chilean R38 second-precision: hh:mm:ss (e.g. 09:00:00).
 */
export function formatR38Time(isoOrDate: string | Date, timezone = 'America/Santiago'): string {
  try {
    const d = typeof isoOrDate === 'string' ? new Date(isoOrDate) : isoOrDate;
    if (isNaN(d.getTime())) return typeof isoOrDate === 'string' ? isoOrDate : '—';
    const formatter = new Intl.DateTimeFormat('es-CL', {
      timeZone: timezone,
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit',
      hour12: false,
    });
    return formatter.format(d);
  } catch {
    return typeof isoOrDate === 'string' ? isoOrDate : '—';
  }
}

/**
 * Formats date and time: dd/mm/aa hh:mm:ss.
 */
export function formatR38DateTime(isoOrDate: string | Date, timezone = 'America/Santiago'): string {
  return `${formatR38Date(isoOrDate, timezone)} ${formatR38Time(isoOrDate, timezone)}`;
}

/**
 * Formats a 64-char hex hash into monospace blocks of 4 chars separated by spaces.
 * e.g. "57c4 cad2 0df4 1e01 ..."
 */
export function formatHashBlocks(hex: string): string {
  if (!hex) return '';
  const clean = hex.trim();
  const chunks: string[] = [];
  for (let i = 0; i < clean.length; i += 4) {
    chunks.push(clean.slice(i, i + 4));
  }
  return chunks.join(' ');
}

// ============================================================================
// State & Type Labels
// ============================================================================

export const ATTENDANCE_STATE_LABELS: Record<
  AttendanceState,
  { label: string; className: string }
> = {
  WORKING: {
    label: 'En jornada',
    className: 'bg-success-subtle text-success border-success/30',
  },
  ON_BREAK: {
    label: 'En colación',
    className: 'bg-warning-subtle text-warning border-warning/30',
  },
  OFF: {
    label: 'Fuera de jornada',
    className: 'bg-surface text-muted border-default',
  },
  // Neutral: also covers rest days; a late start is flagged by the MISSING_START alert (ADMIN-22).
  UNMARKED: {
    label: 'Sin marcar hoy',
    className: 'bg-surface text-muted border-default',
  },
  UNKNOWN: {
    label: 'Desconocido',
    className: 'bg-surface text-muted border-default',
  },
};

export const MARK_TYPE_LABELS: Record<MarkType, string> = {
  WORKDAY_START: 'Inicio de jornada',
  WORKDAY_END: 'Término de jornada',
  BREAK_START: 'Inicio de colación',
  BREAK_END: 'Término de colación',
};

export const MARK_TYPE_SHORT: Record<MarkType, string> = {
  WORKDAY_START: 'Entrada',
  WORKDAY_END: 'Salida',
  BREAK_START: 'Colación inicio',
  BREAK_END: 'Colación fin',
};

export const MARK_SOURCE_LABELS: Record<MarkSource, string> = {
  AGENT: 'App de escritorio (agente)',
  PORTAL: 'Portal web',
  CORRECTION: 'Corrección del empleador',
};

export const CORRECTION_KIND_LABELS: Record<CorrectionKind, string> = {
  MODIFY: 'Modificar marca',
  ADD: 'Agregar marca olvidada',
  FIX: 'Ajuste masivo',
};

export const CORRECTION_STATUS_LABELS: Record<
  CorrectionStatus,
  { label: string; className: string }
> = {
  PENDING: {
    label: 'Pendiente',
    className: 'bg-warning-subtle text-warning border-warning/30',
  },
  ACCEPTED: {
    label: 'Aceptada',
    className: 'bg-success-subtle text-success border-success/30',
  },
  OPPOSED: {
    label: 'Opuesta',
    className: 'bg-danger-subtle text-danger border-danger/30',
  },
  CONSOLIDATED: {
    label: 'Consolidada',
    className: 'bg-info-subtle text-info border-info/30',
  },
  WITHDRAWN: {
    label: 'Retirada',
    className: 'bg-surface text-muted border-default',
  },
};

/** Dashboard KPIs from the live board (RF-ADMIN-22-2). */
export interface BoardKpis {
  working: number;
  unmarked: number;
  /** Unmarked workers whose agreed start passed more than 30 minutes ago (`MISSING_START`). */
  lateStart: number;
}

export function boardKpis(workers: ReadonlyArray<{ state: string; alerts: readonly string[] }>): BoardKpis {
  const kpis: BoardKpis = { working: 0, unmarked: 0, lateStart: 0 };
  for (const w of workers) {
    if (w.state === 'WORKING') kpis.working++;
    if (w.state === 'UNMARKED') {
      kpis.unmarked++;
      if (w.alerts.includes('MISSING_START')) kpis.lateStart++;
    }
  }
  return kpis;
}

// ============================================================================
// Attendance alerts & day anomalies (RF-ADMIN-21-1)
// ============================================================================

/** Severity of an alert / anomaly code; maps to the existing tone classes. */
export type AlertTone = 'danger' | 'warning' | 'info';

export interface AttendanceAlertInfo {
  label: string;
  tone: AlertTone;
  /** One-sentence explanation shown as a tooltip ('' when unknown). */
  hint: string;
}

/**
 * Every code the API emits in `BoardWorker.alerts` (`get_attendance_board`) and
 * `DaySummary.anomalies` (`src/attendance/days.rs`), plus schedule warnings and legacy codes.
 * Never labels an absence as unjustified (contract v3.1).
 */
export const ATTENDANCE_ALERTS: Readonly<Record<string, AttendanceAlertInfo>> = {
  MISSING_START: {
    label: 'Entrada no registrada',
    tone: 'danger',
    hint: 'No hay marca de inicio de jornada en un día con jornada pactada (en el tablero: 30 minutos después de la hora pactada).',
  },
  MISSING_END: {
    label: 'Salida no registrada',
    tone: 'danger',
    hint: 'Hay inicio de jornada pero no hay marca de término.',
  },
  BREAK_WITHOUT_END: {
    label: 'Colación sin término registrado',
    tone: 'warning',
    hint: 'Se marcó el inicio de la colación pero no su término.',
  },
  BREAK_WITHOUT_START: {
    label: 'Término de colación sin inicio',
    tone: 'warning',
    hint: 'Se marcó el término de la colación sin una marca de inicio previa.',
  },
  MARK_ON_REST_DAY: {
    label: 'Marca en día de descanso',
    tone: 'info',
    hint: 'El trabajador marcó un día que su jornada pactada no incluye.',
  },
  OFFLINE_MARK: {
    label: 'Marca sin conexión',
    tone: 'info',
    hint: 'Una marca se registró sin conexión y se envió después; cuenta la hora del equipo (hora efectiva).',
  },
  OFFLINE_MARKS_TODAY: {
    label: 'Marcas sin conexión hoy',
    tone: 'info',
    hint: 'Hoy hay marcas registradas sin conexión; cuenta la hora del equipo (hora efectiva).',
  },
  CORRECTED_MARK: {
    label: 'Día con marca corregida',
    tone: 'info',
    hint: 'El día incluye una marca corregida por el procedimiento de corrección (R38 arts. 39–41).',
  },
  PENDING_CORRECTIONS: {
    label: 'Corrección pendiente',
    tone: 'info',
    hint: 'Hay correcciones propuestas que esperan la respuesta del trabajador (plazo de 48 horas).',
  },
  NO_RECEIPT_EMAIL: {
    label: 'Sin correo para comprobantes',
    tone: 'warning',
    hint: 'El trabajador no tiene correo personal registrado y no recibe el comprobante de sus marcas (R38 art. 8). Agrégalo en Editar trabajador.',
  },
  OUT_OF_ORDER: {
    label: 'Marca fuera de secuencia',
    tone: 'warning',
    hint: 'Las marcas del día no siguen el orden entrada, colación y salida.',
  },
  DEVICE_CLOCK_OFFSET: {
    label: 'Desfase en el reloj del equipo',
    tone: 'warning',
    hint: 'La hora del equipo del trabajador difiere de la del servidor.',
  },
  DAILY_MAX_EXCEEDED: {
    label: 'Supera máximo diario de 10 horas',
    tone: 'warning',
    hint: 'La jornada pactada supera 10 horas efectivas en algún día.',
  },
  WEEKLY_MAX_EXCEEDED: {
    label: 'Supera máximo semanal legal',
    tone: 'warning',
    hint: 'La jornada pactada supera el máximo semanal vigente.',
  },
};

/** Tone → badge classes (existing palette, no new colours). */
export const ALERT_TONE_CLASSES: Record<AlertTone, string> = {
  danger: 'text-danger bg-danger-subtle border-danger/30',
  warning: 'text-warning bg-warning-subtle border-warning/30',
  info: 'text-info bg-info-subtle border-info/30',
};

function humanizeCode(code: string): string {
  const words = code.replace(/_/g, ' ').trim().toLowerCase();
  return words ? words.charAt(0).toUpperCase() + words.slice(1) : 'Aviso';
}

/** Catalog entry for `code`; unknown codes get a readable label, `warning` tone and no hint. */
export function attendanceAlertInfo(code: string): AttendanceAlertInfo {
  if (Object.prototype.hasOwnProperty.call(ATTENDANCE_ALERTS, code)) {
    return ATTENDANCE_ALERTS[code];
  }
  return { label: humanizeCode(code), tone: 'warning', hint: '' };
}

/**
 * Maps anomaly / alert codes to clear Spanish text.
 */
export function describeAnomaly(code: string): string {
  return attendanceAlertInfo(code).label;
}

const TONE_RANK: Record<AlertTone, number> = { danger: 0, warning: 1, info: 2 };

/** Danger first, then warning, then info; stable within the same tone. */
export function sortAlertsBySeverity(codes: readonly string[]): string[] {
  return codes
    .map((code, index) => ({ code, index, rank: TONE_RANK[attendanceAlertInfo(code).tone] }))
    .sort((a, b) => a.rank - b.rank || a.index - b.index)
    .map((entry) => entry.code);
}

// ============================================================================
// Correction Mirror Validation & Error Messages (RF-ADMIN-15-5)
// ============================================================================

export interface CorrectionValidationInput {
  kind: CorrectionKind;
  originalMarkId?: string | null;
  originalMarkTs?: string | null;
  markType: MarkType;
  proposedTs: string;
  reason: string;
  confirmNoHarm: boolean;
  timezone?: string | null;
}

export interface CorrectionValidationResult {
  valid: boolean;
  errors: Record<string, string>;
  reasonCode?: string;
}

/**
 * Extracts YYYY-MM-DD date string for a given Date or ISO string in a specific timezone.
 */
export function getDateStringInTimezone(
  dateOrIso: Date | string,
  timezone = 'America/Santiago',
): string {
  try {
    const d = typeof dateOrIso === 'string' ? new Date(dateOrIso) : dateOrIso;
    if (isNaN(d.getTime())) return '';
    const parts = new Intl.DateTimeFormat('en-US', {
      timeZone: timezone,
      year: 'numeric',
      month: '2-digit',
      day: '2-digit',
    }).formatToParts(d);
    const year = parts.find(p => p.type === 'year')?.value ?? '';
    const month = parts.find(p => p.type === 'month')?.value ?? '';
    const day = parts.find(p => p.type === 'day')?.value ?? '';
    return `${year}-${month}-${day}`;
  } catch {
    return '';
  }
}

/**
 * Validates a proposed correction locally before calling Tauri invoke (RF-ADMIN-15-5).
 * Mirrors server rules:
 * 1. `confirmNoHarm` is required.
 * 2. `reason` length 5-500.
 * 3. MODIFY requires `originalMarkId`.
 * 4. Not same day or future in worker's timezone (`CORRECTION_TOO_EARLY`).
 * 5. Does not harm the worker (`CORRECTION_HARMS_WORKER`):
 *    - Start can only move earlier.
 *    - End can only move later.
 */
export function validateCorrectionForm(
  input: CorrectionValidationInput,
  now = new Date(),
  timezone = 'America/Santiago',
): CorrectionValidationResult {
  const errors: Record<string, string> = {};
  let reasonCode: string | undefined;

  const tz = input.timezone || timezone || 'America/Santiago';

  if (!input.confirmNoHarm) {
    errors.confirmNoHarm = 'Debes confirmar que la corrección no perjudica al trabajador.';
  }

  const reasonTrimmed = input.reason.trim();
  if (reasonTrimmed.length < 5 || reasonTrimmed.length > 500) {
    errors.reason = 'El motivo de la corrección debe tener entre 5 y 500 caracteres.';
  }

  if (input.kind === 'MODIFY' && (!input.originalMarkId || !input.originalMarkId.trim())) {
    errors.originalMarkId = 'Debes seleccionar la marca original que deseas modificar.';
    reasonCode = 'ORIGINAL_MARK_REQUIRED';
  }

  if (!input.proposedTs || !input.proposedTs.trim()) {
    errors.proposedTs = 'Debes indicar la fecha y hora propuesta.';
  } else {
    const proposedDate = new Date(input.proposedTs.trim());
    if (isNaN(proposedDate.getTime())) {
      errors.proposedTs = 'La fecha y hora propuesta no es válida.';
    } else {
      // Check date in worker's timezone: cannot be today or future (only from next business day)
      const proposedDayString = getDateStringInTimezone(proposedDate, tz);
      const todayDayString = getDateStringInTimezone(now, tz);
      if (proposedDayString >= todayDayString) {
        errors.proposedTs =
          'Solo puedes proponer correcciones desde el día hábil siguiente a la jornada corregida.';
        reasonCode = 'CORRECTION_TOO_EARLY';
      }

      // Check harm to worker
      if (input.kind === 'MODIFY' && input.originalMarkTs) {
        const origDate = new Date(input.originalMarkTs.trim());
        if (!isNaN(origDate.getTime())) {
          if (input.markType === 'WORKDAY_START' && proposedDate > origDate) {
            errors.proposedTs =
              'La corrección de inicio de jornada solo puede adelantar la entrada; retrasarla perjudica al trabajador.';
            reasonCode = 'CORRECTION_HARMS_WORKER';
          } else if (input.markType === 'WORKDAY_END' && proposedDate < origDate) {
            errors.proposedTs =
              'La corrección de término de jornada solo puede postergar la salida; adelantarla perjudica al trabajador.';
            reasonCode = 'CORRECTION_HARMS_WORKER';
          }
        }
      }
    }
  }

  return {
    valid: Object.keys(errors).length === 0,
    errors,
    reasonCode,
  };
}

/**
 * Returns human-readable Spanish text for server error reasons in corrections.
 */
export function describeCorrectionError(reason?: string, fallback = 'No se pudo registrar la corrección.'): string {
  switch (reason) {
    case 'CORRECTION_TOO_EARLY':
      return 'Solo puedes proponer correcciones desde el día hábil siguiente a la jornada corregida (R38 art. 41).';
    case 'CORRECTION_HARMS_WORKER':
      return 'La corrección propuesta perjudica al trabajador: la entrada solo puede adelantarse y la salida solo puede postergarse (R38 art. 41).';
    case 'ORIGINAL_MARK_REQUIRED':
      return 'Debes seleccionar la marca original que deseas modificar.';
    case 'AUTOMATED_WORKDAY_MARK_FORBIDDEN':
      return 'No se permite la marcación automatizada del inicio o fin de la jornada (R38 art. 41 d).';
    default:
      return fallback;
  }
}

// ============================================================================
// Effective time of marks (contract v3.1, RF-ADMIN-17-1)
// ============================================================================

type MarkTimes = Pick<AttendanceMark, 'server_ts' | 'device_ts' | 'offline' | 'source'> & {
  effective_ts?: string | null;
};

/**
 * The time that counts for a mark (api-contract §9.1 v3.1). Uses the server's `effective_ts`;
 * when missing, applies the contract rule: `device_ts` if the mark was taken offline or comes from
 * a correction, otherwise `server_ts`. Display/grouping only: the hash uses the canonical fields.
 */
export function effectiveTs(mark: MarkTimes): string {
  if (mark.effective_ts) return mark.effective_ts;
  return mark.offline || mark.source === 'CORRECTION' ? mark.device_ts : mark.server_ts;
}

/** Local date (`YYYY-MM-DD`) of the mark's effective time in the worker's time zone. */
export function effectiveLocalDate(mark: MarkTimes, timezone = 'America/Santiago'): string {
  return getDateStringInTimezone(effectiveTs(mark), timezone);
}

/** Marks sorted by effective time (ascending); the input is not mutated. */
export function sortMarksByEffective<T extends MarkTimes>(marks: T[]): T[] {
  return [...marks].sort(
    (a, b) => new Date(effectiveTs(a)).getTime() - new Date(effectiveTs(b)).getTime(),
  );
}

/**
 * Groups marks by the local date of their effective time in the worker's time zone, each group
 * sorted by effective time. Keys are `YYYY-MM-DD`.
 */
export function groupMarksByEffectiveDay<T extends MarkTimes>(
  marks: T[],
  timezone = 'America/Santiago',
): Record<string, T[]> {
  const groups: Record<string, T[]> = {};
  for (const mark of sortMarksByEffective(marks)) {
    const key = effectiveLocalDate(mark, timezone);
    (groups[key] ??= []).push(mark);
  }
  return groups;
}

/** True when the device clock time differs from the server time (shown as "Hora del equipo"). */
export function deviceTimeDiffers(mark: Pick<AttendanceMark, 'server_ts' | 'device_ts'>): boolean {
  const server = new Date(mark.server_ts).getTime();
  const device = new Date(mark.device_ts).getTime();
  if (isNaN(server) || isNaN(device)) return mark.server_ts !== mark.device_ts;
  return server !== device;
}

// ============================================================================
// Absence qualification (contract v3.1, R38 arts. 27.1 and 28 g, RF-ADMIN-17-3)
// ============================================================================

/** Official abbreviations of R38 art. 28 g, in the order of the resolution, with their meaning. */
export const R38_ABBREVIATIONS: ReadonlyArray<{ code: string; meaning: string }> = [
  { code: 'A.I.', meaning: 'Ausencia injustificada' },
  { code: 'A.J', meaning: 'Ausencia justificada' },
  { code: 'AT', meaning: 'Atraso' },
  { code: 'C.T.', meaning: 'Cambio de turno' },
  { code: 'D.E.J.', meaning: 'Distribución excepcional de jornada' },
  { code: 'H.E.', meaning: 'Horas extraordinarias' },
  { code: 'J.O.', meaning: 'Jornada ordinaria' },
  { code: 'L.M.', meaning: 'Licencia médica' },
  { code: 'P.G.R', meaning: 'Permiso con goce de remuneraciones' },
  { code: 'PREN.', meaning: 'Prenatal' },
  { code: 'P.S.G.R.', meaning: 'Permiso sin goce de remuneraciones' },
  { code: 'POSTN', meaning: 'Postnatal' },
  { code: 'S.A', meaning: 'Salida anticipada' },
  { code: 'VAC', meaning: 'Vacaciones' },
];

export const ABSENCE_NOTE_MAX_CHARS = 300;

export const ABSENCE_KIND_LABELS: Record<AbsenceType, string> = {
  JUSTIFIED: 'Ausencia justificada',
  UNJUSTIFIED: 'Ausencia injustificada',
};

/** Meaning in words of an official abbreviation, or `null` when it is not in the list. */
export function abbreviationMeaning(code: string | null | undefined): string | null {
  if (!code) return null;
  return R38_ABBREVIATIONS.find(a => a.code === code)?.meaning ?? null;
}

/**
 * Abbreviations offered for a qualification: the whole list except the one that contradicts it
 * (`A.I.` is never offered for a justified absence, nor `A.J` for an unjustified one).
 */
export function abbreviationsForKind(kind: AbsenceType): ReadonlyArray<{ code: string; meaning: string }> {
  const contradicting = kind === 'JUSTIFIED' ? 'A.I.' : 'A.J';
  return R38_ABBREVIATIONS.filter(a => a.code !== contradicting);
}

/**
 * Status of a day in words. The system never labels an absence by itself: a workday without
 * marks and without the employer's qualification is "Sin marcas" (api-contract §9.4 v3.1).
 */
export function describeDayStatus(
  day: Pick<DaySummary, 'attended' | 'absence' | 'agreed' | 'marks'>,
): string {
  if (day.attended) return 'Asistió';
  if (day.absence) return ABSENCE_KIND_LABELS[day.absence];
  if (day.marks.length > 0) return 'Sin inicio de jornada';
  return day.agreed ? 'Sin marcas' : 'Día de descanso';
}

/**
 * Whether the employer may qualify (or edit the qualification of) this day: a workday without
 * marks, or a day already qualified, and never a future date in the worker's time zone.
 */
export function canQualifyAbsence(
  day: Pick<DaySummary, 'date' | 'attended' | 'absence' | 'agreed' | 'marks'>,
  now = new Date(),
  timezone = 'America/Santiago',
): boolean {
  if (day.date > getDateStringInTimezone(now, timezone)) return false;
  if (day.absence) return true;
  return !day.attended && day.marks.length === 0 && !!day.agreed;
}

export interface AbsenceFormInput {
  kind: AbsenceType | null;
  abbreviation?: string | null;
  note?: string | null;
  confirmUnjustified: boolean;
}

/**
 * Local mirror of the absence form rules (the Rust command validates again): a kind must be
 * chosen explicitly, the abbreviation must be official and not contradict the kind, the note is
 * at most 300 characters and "Injustificada" needs an explicit confirmation.
 */
export function validateAbsenceForm(input: AbsenceFormInput): Record<string, string> {
  const errors: Record<string, string> = {};
  if (!input.kind) {
    errors.kind = 'Elige si la ausencia es justificada o injustificada.';
  }
  const abbr = input.abbreviation?.trim();
  if (abbr) {
    if (!abbreviationMeaning(abbr)) {
      errors.abbreviation = 'La abreviatura no está en la lista oficial (Res. Ex. N°38, art. 28).';
    } else if (input.kind && !abbreviationsForKind(input.kind).some(a => a.code === abbr)) {
      errors.abbreviation = 'La abreviatura contradice la calificación elegida.';
    }
  }
  if ([...(input.note?.trim() ?? '')].length > ABSENCE_NOTE_MAX_CHARS) {
    errors.note = 'La nota no puede superar los 300 caracteres.';
  }
  if (input.kind === 'UNJUSTIFIED' && !input.confirmUnjustified) {
    errors.confirmUnjustified =
      'Confirma que entiendes que esta calificación aparece en los reportes legales del trabajador.';
  }
  return errors;
}

// ============================================================================
// Timezone conversion for legal corrections (RF-ADMIN-18-3, RF-ADMIN-18-4)
// ============================================================================

interface ZonedParts {
  year: number;
  month: number;
  day: number;
  hour: number;
  minute: number;
  second: number;
}

const formattersCache = new Map<string, Intl.DateTimeFormat>();

function getZonedFormatter(tz: string): Intl.DateTimeFormat {
  let f = formattersCache.get(tz);
  if (!f) {
    f = new Intl.DateTimeFormat('en-US', {
      timeZone: tz,
      year: 'numeric',
      month: '2-digit',
      day: '2-digit',
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit',
      hour12: false,
      hourCycle: 'h23',
    });
    formattersCache.set(tz, f);
  }
  return f;
}

function getZonedParts(utcMs: number, tz: string): ZonedParts {
  const d = new Date(utcMs);
  const formatter = getZonedFormatter(tz);
  const parts = formatter.formatToParts(d);
  let year = 0;
  let month = 0;
  let day = 0;
  let hour = 0;
  let minute = 0;
  let second = 0;
  for (const p of parts) {
    if (p.type === 'year') year = parseInt(p.value, 10);
    else if (p.type === 'month') month = parseInt(p.value, 10);
    else if (p.type === 'day') day = parseInt(p.value, 10);
    else if (p.type === 'hour') hour = parseInt(p.value, 10);
    else if (p.type === 'minute') minute = parseInt(p.value, 10);
    else if (p.type === 'second') second = parseInt(p.value, 10);
  }
  return { year, month, day, hour, minute, second };
}

function getOffsetMs(utcMs: number, tz: string): number {
  const parts = getZonedParts(utcMs, tz);
  const zonedAsUtc = Date.UTC(
    parts.year,
    parts.month - 1,
    parts.day,
    parts.hour,
    parts.minute,
    parts.second,
  );
  return zonedAsUtc - utcMs;
}

function matchesZoned(
  utcMs: number,
  tz: string,
  y: number,
  m: number,
  d: number,
  h: number,
  min: number,
  s: number,
): boolean {
  const p = getZonedParts(utcMs, tz);
  return (
    p.year === y &&
    p.month === m &&
    p.day === d &&
    p.hour === h &&
    p.minute === min &&
    p.second === s
  );
}

/**
 * Converts a wall-clock date and time in a given IANA timezone to an RFC 3339 UTC string
 * with second precision ('...Z') using only the standard `Intl` API (RF-ADMIN-18-3).
 *
 * Rules:
 * - Precision: exact seconds ('...Z').
 * - Nonexistent wall-clock hours (e.g. spring-forward daylight saving gap) throw an Error in es-CL.
 * - Ambiguous wall-clock hours (e.g. fall-back daylight saving overlap) resolve to the first occurrence
 *   chronologically (the earlier UTC instant).
 * - Supported time formats: HH:mm:ss, HH:mm, or H:mm:ss.
 * - Supported date formats: YYYY-MM-DD.
 */
export function zonedLocalToUtcIso(
  date: string,
  time: string,
  timezone = 'America/Santiago',
): string {
  const tz = timezone?.trim() || 'America/Santiago';

  if (!date || !date.trim()) {
    throw new Error('Debes indicar la fecha.');
  }
  if (!time || !time.trim()) {
    throw new Error('Debes indicar la hora.');
  }

  const dateMatch = /^(\d{4})-(\d{1,2})-(\d{1,2})$/.exec(date.trim());
  if (!dateMatch) {
    throw new Error('Formato de fecha inválido. Se espera YYYY-MM-DD.');
  }
  const y = parseInt(dateMatch[1], 10);
  const m = parseInt(dateMatch[2], 10);
  const d = parseInt(dateMatch[3], 10);

  const timeMatch = /^(\d{1,2}):(\d{2})(?::(\d{2}))?$/.exec(time.trim());
  if (!timeMatch) {
    throw new Error('Formato de hora inválido. Se espera HH:mm:ss.');
  }
  const h = parseInt(timeMatch[1], 10);
  const min = parseInt(timeMatch[2], 10);
  const s = timeMatch[3] ? parseInt(timeMatch[3], 10) : 0;

  if (m < 1 || m > 12 || d < 1 || d > 31 || h < 0 || h > 23 || min < 0 || min > 59 || s < 0 || s > 59) {
    throw new Error('Hora o fecha fuera de rango.');
  }

  // Validate calendar date (e.g. 2026-02-30 is invalid)
  const testDate = new Date(Date.UTC(y, m - 1, d));
  if (
    testDate.getUTCFullYear() !== y ||
    testDate.getUTCMonth() !== m - 1 ||
    testDate.getUTCDate() !== d
  ) {
    throw new Error('Fecha del calendario no válida.');
  }

  const targetNaiveMs = Date.UTC(y, m - 1, d, h, min, s);

  // Sample offsets around targetNaiveMs to capture DST transitions
  const testPoints = [
    targetNaiveMs - 24 * 3600 * 1000,
    targetNaiveMs - 12 * 3600 * 1000,
    targetNaiveMs - 4 * 3600 * 1000,
    targetNaiveMs - 3600 * 1000,
    targetNaiveMs,
    targetNaiveMs + 3600 * 1000,
    targetNaiveMs + 4 * 3600 * 1000,
    targetNaiveMs + 12 * 3600 * 1000,
    targetNaiveMs + 24 * 3600 * 1000,
  ];

  const offsets = new Set<number>();
  for (const t of testPoints) {
    offsets.add(getOffsetMs(t, tz));
  }
  for (const off of Array.from(offsets)) {
    offsets.add(getOffsetMs(targetNaiveMs - off, tz));
  }

  const candidateUtcs: number[] = [];
  for (const off of offsets) {
    const cand = targetNaiveMs - off;
    if (matchesZoned(cand, tz, y, m, d, h, min, s)) {
      if (!candidateUtcs.includes(cand)) {
        candidateUtcs.push(cand);
      }
    }
  }

  if (candidateUtcs.length === 0) {
    throw new Error('La hora indicada no existe en la zona horaria debido al cambio de horario de verano.');
  }

  // Sort ascending: in ambiguous fallback times, first occurrence is the earlier UTC instant
  candidateUtcs.sort((a, b) => a - b);
  const chosen = new Date(candidateUtcs[0]);

  return chosen.toISOString().replace(/\.\d{3}Z$/, 'Z');
}

/**
 * Returns a human-readable label in es-CL for a timezone (RF-ADMIN-18-4).
 */
export function formatTimezoneLabel(timezone = 'America/Santiago'): string {
  const tz = timezone?.trim() || 'America/Santiago';
  switch (tz) {
    case 'America/Santiago':
      return 'Hora de Chile continental (America/Santiago)';
    case 'America/Punta_Arenas':
      return 'Hora de Magallanes (America/Punta_Arenas)';
    case 'Pacific/Easter':
      return 'Hora de Rapa Nui (Pacific/Easter)';
    case 'UTC':
      return 'Tiempo Universal Coordinado (UTC)';
    default:
      return tz;
  }
}

