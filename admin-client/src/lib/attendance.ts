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
  UNMARKED: {
    label: 'Sin marcar hoy',
    className: 'bg-danger-subtle text-danger border-danger/30',
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

/**
 * Maps anomaly / alert codes to clear Spanish text.
 */
export function describeAnomaly(code: string): string {
  switch (code) {
    case 'BREAK_WITHOUT_END':
      return 'Colación sin término registrado';
    case 'MISSING_START':
      return 'Entrada no registrada';
    case 'MISSING_END':
      return 'Salida no registrada';
    case 'OUT_OF_ORDER':
      return 'Marca fuera de secuencia';
    case 'DEVICE_CLOCK_OFFSET':
      return 'Desfase en el reloj del equipo';
    case 'DAILY_MAX_EXCEEDED':
      return 'Supera máximo diario de 10 horas';
    case 'WEEKLY_MAX_EXCEEDED':
      return 'Supera máximo semanal legal';
    default:
      return code.replace(/_/g, ' ');
  }
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
