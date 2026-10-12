// admin-client/src/lib/schedule.ts
// Pure schedule calculations, validation and legal compliance checks (ATT-11,
// CT art. 152 quáter J, R38). No React, no IPC: unit-tested with `node --test`.
//
// These totals only drive the live hints of the agreed-schedule editor; the server
// remains the authority for every legal value and answers with `warnings` (ATT-09, ATT-11).

import type {
  DisconnectionWindow,
  JourneyType,
  ScheduleDay,
  ScheduleInput,
  WorkMode,
} from '../api/types';

export const WEEKDAY_NAMES: Record<number, string> = {
  1: 'Lunes',
  2: 'Martes',
  3: 'Miércoles',
  4: 'Jueves',
  5: 'Viernes',
  6: 'Sábado',
  7: 'Domingo',
};

export const JOURNEY_TYPE_LABELS: Record<JourneyType, string> = {
  FIXED: 'Jornada ordinaria fija',
  SHIFTS: 'Jornada por turnos',
  CYCLES: 'Jornada en ciclos',
  BIWEEKLY: 'Jornada bisemanal',
  EXCEPTIONAL: 'Jornada excepcional (resolución DT)',
  PART_TIME: 'Jornada parcial',
};

/** Daily ordinary maximum (hours, ATT-11). */
export const DAILY_MAX_HOURS = 10;
/** Minimum continuous disconnection for REMOTE/HYBRID workers (hours). */
export const MIN_DISCONNECTION_HOURS = 12;

const HHMM = /^([01]\d|2[0-3]):[0-5]\d$/;
const ISO_DATE = /^\d{4}-\d{2}-\d{2}$/;

/** Strict `HH:MM` (what `ScheduleInput` carries). */
export function isValidHHMM(t: string | null | undefined): boolean {
  return !!t && HHMM.test(t.trim());
}

/** Real calendar date `YYYY-MM-DD`. */
export function isValidIsoDate(d: string | null | undefined): boolean {
  if (!d || !ISO_DATE.test(d.trim())) return false;
  const [y, m, day] = d.trim().split('-').map(Number);
  const dt = new Date(Date.UTC(y, m - 1, day));
  return dt.getUTCFullYear() === y && dt.getUTCMonth() === m - 1 && dt.getUTCDate() === day;
}

/**
 * Converts `HH:MM` or `HH:MM:SS` into minutes from midnight (0..1439); NaN when invalid.
 */
export function timeToMinutes(t: string | null | undefined): number {
  if (!t) return NaN;
  const parts = t.trim().split(':');
  if (parts.length < 2 || parts.length > 3) return NaN;
  if (!parts.every(p => /^\d{1,2}$/.test(p))) return NaN;
  const h = Number(parts[0]);
  const m = Number(parts[1]);
  if (h > 23 || m > 59) return NaN;
  return h * 60 + m;
}

/**
 * Minutes between `start` and `end`; when `end < start` the interval crosses midnight
 * (22:00 → 06:00 = 8 h).
 */
export function calculateIntervalMinutes(start: string, end: string): number {
  const s = timeToMinutes(start);
  const e = timeToMinutes(end);
  if (isNaN(s) || isNaN(e)) return 0;
  return e >= s ? e - s : 24 * 60 - s + e;
}

/**
 * Working minutes of one day: end − start, minus the meal break unless it is imputed to
 * the working day (`break_counts_as_work`).
 */
export function calculateDayMinutes(day: ScheduleDay, breakCountsAsWork: boolean): number {
  const span = calculateIntervalMinutes(day.start, day.end);
  if (span === 0) return 0;
  if (day.break_start && day.break_end && !breakCountsAsWork) {
    const breakSpan = calculateIntervalMinutes(day.break_start, day.break_end);
    return Math.max(0, span - breakSpan);
  }
  return span;
}

export function calculateDayHours(day: ScheduleDay, breakCountsAsWork: boolean): number {
  return calculateDayMinutes(day, breakCountsAsWork) / 60;
}

export function calculateWeeklyTotalMinutes(days: ScheduleDay[], breakCountsAsWork: boolean): number {
  return days.reduce((acc, day) => acc + calculateDayMinutes(day, breakCountsAsWork), 0);
}

export function calculateWeeklyTotalHours(days: ScheduleDay[], breakCountsAsWork: boolean): number {
  return calculateWeeklyTotalMinutes(days, breakCountsAsWork) / 60;
}

/** `8 h`, `8 h 30 min`. */
export function formatMinutes(minutes: number): string {
  const h = Math.floor(minutes / 60);
  const m = Math.round(minutes % 60);
  if (m === 0) return `${h} h`;
  return `${h} h ${m} min`;
}

/**
 * Legal weekly maximum in force on a date (ATT-11): 44 h until 2026-04-25, 42 h from
 * 2026-04-26, 40 h from 2028-04-26. Without a date, today's.
 */
export function getWeeklyLegalMaxHours(effectiveDate?: string | Date): number {
  let dateStr: string;
  if (!effectiveDate) {
    dateStr = new Date().toISOString().slice(0, 10);
  } else if (typeof effectiveDate === 'string') {
    dateStr = effectiveDate.slice(0, 10);
  } else {
    dateStr = effectiveDate.toISOString().slice(0, 10);
  }
  if (dateStr <= '2026-04-25') return 44;
  if (dateStr < '2028-04-26') return 42;
  return 40;
}

/** Disconnection window length in minutes (it may cross midnight). */
export function calculateDisconnectionMinutes(start: string, end: string): number {
  return calculateIntervalMinutes(start, end);
}

export function calculateDisconnectionHours(start: string, end: string): number {
  return calculateDisconnectionMinutes(start, end) / 60;
}

/** CT art. 152 quáter J: ≥ 12 continuous hours for REMOTE/HYBRID workers. */
export function validateDisconnection(
  disconnection: DisconnectionWindow | null | undefined,
): { valid: boolean; hours: number; error?: string } {
  if (
    !disconnection ||
    !isValidHHMM(disconnection.start) ||
    !isValidHHMM(disconnection.end)
  ) {
    return {
      valid: false,
      hours: 0,
      error: 'La franja de desconexión es obligatoria para teletrabajo o jornada mixta.',
    };
  }
  const hours = calculateDisconnectionHours(disconnection.start, disconnection.end);
  if (hours < MIN_DISCONNECTION_HOURS) {
    return {
      valid: false,
      hours,
      error: `La desconexión debe ser de al menos 12 horas continuas (actual: ${formatMinutes(hours * 60)}).`,
    };
  }
  return { valid: true, hours };
}

/**
 * Non-blocking legal warnings of an agreed schedule (ATT-11, R38 art. 45.2): more than
 * 10 h in a day, or more than the weekly maximum in force on `effectiveDate`.
 */
export function getScheduleWarnings(
  days: ScheduleDay[],
  breakCountsAsWork: boolean,
  effectiveDate?: string,
): string[] {
  const warnings: string[] = [];
  const maxWeekly = getWeeklyLegalMaxHours(effectiveDate);

  for (const day of days) {
    const hours = calculateDayHours(day, breakCountsAsWork);
    if (hours > DAILY_MAX_HOURS) {
      const dayName = WEEKDAY_NAMES[day.weekday] || `Día ${day.weekday}`;
      warnings.push(
        `Supera el máximo diario ordinario de ${DAILY_MAX_HOURS} horas en ${dayName} (${hours.toFixed(1)} h).`,
      );
    }
  }

  const weeklyHours = calculateWeeklyTotalHours(days, breakCountsAsWork);
  if (weeklyHours > maxWeekly) {
    warnings.push(
      `Supera el máximo legal de ${maxWeekly} horas semanales (total: ${weeklyHours.toFixed(1)} h).`,
    );
  }
  return warnings;
}

/** User-facing text of the server's `warnings` codes (contract v3 §11). */
export function serverWarningLabel(code: string): string {
  switch (code) {
    case 'DAILY_MAX_EXCEEDED':
      return `La jornada supera el máximo diario ordinario de ${DAILY_MAX_HOURS} horas.`;
    case 'WEEKLY_MAX_EXCEEDED':
      return 'La jornada supera el máximo semanal legal vigente.';
    default:
      return `Advertencia del servidor: ${code}`;
  }
}

/**
 * Template of the explicit "Aplicar lunes a viernes 09:00–18:00 con colación 13:00–14:00"
 * action. It is never applied by default (RF-ADMIN-14, no invented schedules).
 */
export function getDefaultWeekDays(): ScheduleDay[] {
  return [1, 2, 3, 4, 5].map(weekday => ({
    weekday,
    start: '09:00',
    end: '18:00',
    break_start: '13:00',
    break_end: '14:00',
  }));
}

/** One row of the Monday–Sunday editor grid. */
export interface WeekGridRow {
  weekday: number;
  works: boolean;
  start: string;
  end: string;
  break_start: string;
  break_end: string;
}

/** Empty grid: nobody works any day until the admin fills it. */
export function emptyWeekGrid(): WeekGridRow[] {
  return [1, 2, 3, 4, 5, 6, 7].map(weekday => ({
    weekday,
    works: false,
    start: '',
    end: '',
    break_start: '',
    break_end: '',
  }));
}

/** Loads schedule days into the editor grid. */
export function daysToGrid(days: ScheduleDay[]): WeekGridRow[] {
  return emptyWeekGrid().map(row => {
    const d = days.find(x => x.weekday === row.weekday);
    if (!d) return row;
    return {
      weekday: row.weekday,
      works: true,
      start: d.start.slice(0, 5),
      end: d.end.slice(0, 5),
      break_start: d.break_start ? d.break_start.slice(0, 5) : '',
      break_end: d.break_end ? d.break_end.slice(0, 5) : '',
    };
  });
}

/** Working rows of the grid as API days (no break keys when the break is empty). */
export function gridToDays(grid: WeekGridRow[]): ScheduleDay[] {
  return grid
    .filter(r => r.works)
    .map(r => {
      const day: ScheduleDay = { weekday: r.weekday, start: r.start.trim(), end: r.end.trim() };
      if (r.break_start.trim() || r.break_end.trim()) {
        day.break_start = r.break_start.trim();
        day.break_end = r.break_end.trim();
      }
      return day;
    });
}

/**
 * Mirror of the Rust `validate_schedule` for live messages (the Rust command stays the
 * authority). Exceeding legal maxima is **not** an error (ATT-11): see `getScheduleWarnings`.
 * Returns human readable errors; empty when valid.
 */
export function validateScheduleInput(
  schedule: Pick<
    ScheduleInput,
    | 'journey_type'
    | 'exceptional_resolution'
    | 'days'
    | 'disconnection'
    | 'effective_from'
    | 'requested_by'
  >,
  workMode: WorkMode,
): string[] {
  const errors: string[] = [];

  if (schedule.journey_type === 'EXCEPTIONAL') {
    const res = schedule.exceptional_resolution;
    if (!res || !res.number.trim()) {
      errors.push('La jornada excepcional requiere el número de resolución de la DT.');
    }
    if (!res || !isValidIsoDate(res.date)) {
      errors.push('La jornada excepcional requiere la fecha de la resolución (AAAA-MM-DD).');
    }
  }

  if (schedule.days.length === 0) {
    errors.push('Marca al menos un día de trabajo.');
  }

  const seen = new Set<number>();
  for (const day of schedule.days) {
    const name = WEEKDAY_NAMES[day.weekday] ?? `Día ${day.weekday}`;
    if (day.weekday < 1 || day.weekday > 7 || seen.has(day.weekday)) {
      errors.push(`Día de la semana inválido o repetido: ${day.weekday}.`);
      continue;
    }
    seen.add(day.weekday);
    if (!isValidHHMM(day.start) || !isValidHHMM(day.end)) {
      errors.push(`${name}: ingresa entrada y salida en formato HH:MM.`);
      continue;
    }
    const s = timeToMinutes(day.start);
    const e = timeToMinutes(day.end);
    if (s >= e) {
      errors.push(`${name}: la entrada debe ser anterior a la salida.`);
      continue;
    }
    const hasBs = !!day.break_start;
    const hasBe = !!day.break_end;
    if (hasBs !== hasBe) {
      errors.push(`${name}: la colación requiere inicio y término.`);
    } else if (hasBs && hasBe) {
      if (!isValidHHMM(day.break_start) || !isValidHHMM(day.break_end)) {
        errors.push(`${name}: la colación debe tener formato HH:MM.`);
      } else {
        const bs = timeToMinutes(day.break_start);
        const be = timeToMinutes(day.break_end);
        if (bs < s || be > e || bs >= be) {
          errors.push(`${name}: la colación debe quedar dentro de la jornada.`);
        }
      }
    }
  }

  // REMOTE/HYBRID require a window; an ON_SITE one is optional but must be valid if present.
  if (workMode !== 'ON_SITE' || schedule.disconnection) {
    const v = validateDisconnection(schedule.disconnection);
    if (!v.valid && v.error) errors.push(v.error);
  }

  if (!isValidIsoDate(schedule.effective_from)) {
    errors.push('Indica la fecha desde la que rige la jornada (AAAA-MM-DD).');
  }
  if (schedule.requested_by !== 'EMPLOYER' && schedule.requested_by !== 'WORKER') {
    errors.push('Indica quién solicita la jornada.');
  }
  return errors;
}
