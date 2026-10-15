// admin-client/src/lib/reports.ts — R38 legal reports: menu, art. 25 filters and cell rendering
// (TASK-ADMIN-16). Pure module (no React, no IPC): unit-tested with `node --test`.
//
// The server computes every report (ATT-09, R38 art. 28 a). Nothing here reorders, rounds or
// recalculates a value: cells are shown with the exact text the server sent. The range checks
// mirror the Rust validation (`commands/reports.rs`) only to give inline feedback.

import type {
  JourneyType,
  ReportCell,
  ReportFilters,
  ReportKind,
  ReportPeriod,
  ReportWorkplace,
} from '../api/types';

/** Menu entry id: the five server reports plus the emailed daily report. */
export type ReportMenuId = ReportKind | 'daily';

export interface ReportMenuEntry {
  id: ReportMenuId;
  /** Exact R38 art. 27 name. */
  name: string;
}

/** The six reports in the R38 art. 27 order and with its exact names (RF-ADMIN-16-2). */
export const REPORT_MENU: readonly ReportMenuEntry[] = [
  { id: 'attendance', name: 'Reporte de asistencia' },
  { id: 'daily-hours', name: 'Reporte de jornada diaria' },
  { id: 'sundays-holidays', name: 'Reporte de domingos y/o días festivos' },
  { id: 'shift-changes', name: 'Reporte de modificaciones y/o alteraciones de turnos' },
  { id: 'daily', name: 'Reporte diario' },
  { id: 'incidents', name: 'Reporte de incidentes técnicos' },
];

export const DAILY_REPORT_INFO = 'Se envía automáticamente por correo cada día al empleador';

export const PERIOD_LABELS: Record<ReportPeriod, string> = {
  last_week: 'Última semana',
  last_fortnight: 'Última quincena',
  last_month: 'Mes anterior',
};

/** Range limit per report in months (R38 arts. 25 and 27.3). */
export const MAX_RANGE_MONTHS = 60;
export const MAX_RANGE_MONTHS_SUNDAYS = 24;
export const MAX_SELECTED_WORKERS = 200;
export const MAX_TEXT_FILTER_CHARS = 100;

export function maxRangeMonths(report: ReportKind): number {
  return report === 'sundays-holidays' ? MAX_RANGE_MONTHS_SUNDAYS : MAX_RANGE_MONTHS;
}

export function rangeLimitLabel(report: ReportKind): string {
  return report === 'sundays-holidays' ? '24 meses' : '5 años';
}

/** Form state of the filters panel (all filters visible at once, any order). */
export interface ReportFormState {
  selectedWorkerIds: string[];
  /** Name or RUN typed in the worker search box. */
  search: string;
  mode: 'period' | 'range';
  period: ReportPeriod;
  from: string;
  to: string;
  journeyType: JourneyType | '';
  workplaceId: string;
  jobTitle: string;
  hash: string;
}

export function initialReportForm(): ReportFormState {
  return {
    selectedWorkerIds: [],
    search: '',
    mode: 'period',
    period: 'last_week',
    from: '',
    to: '',
    journeyType: '',
    workplaceId: '',
    jobTitle: '',
    hash: '',
  };
}

/**
 * Builds the `ReportFilters` sent to `attendance_report` / `attendance_export_report`.
 * Selected workers win over the search text: the search box narrows the list, and is only sent
 * to the server (name or RUN, `search`) when nobody is selected — so workers missing from the
 * local list (e.g. revoked) can still be found.
 */
export function buildReportFilters(state: ReportFormState): ReportFilters {
  const ids: string[] = [];
  for (const id of state.selectedWorkerIds) {
    const clean = id.trim().toLowerCase();
    if (clean && !ids.includes(clean)) ids.push(clean);
  }
  const filters: ReportFilters = { worker_ids: ids };
  const search = state.search.trim();
  if (ids.length === 0 && search) filters.search = search;
  if (state.mode === 'period') {
    filters.period = state.period;
  } else {
    if (state.from.trim()) filters.from = state.from.trim();
    if (state.to.trim()) filters.to = state.to.trim();
  }
  if (state.journeyType) filters.journey_type = state.journeyType;
  if (state.workplaceId.trim()) filters.workplace_id = state.workplaceId.trim();
  const jobTitle = state.jobTitle.trim();
  if (jobTitle) filters.job_title = jobTitle;
  const hash = state.hash.trim().toLowerCase();
  if (hash) filters.hash = hash;
  return filters;
}

const DATE_RE = /^(\d{4})-(\d{2})-(\d{2})$/;

function parseDate(value: string): { y: number; m: number; d: number } | null {
  const match = DATE_RE.exec(value);
  if (!match) return null;
  const y = Number(match[1]);
  const m = Number(match[2]);
  const d = Number(match[3]);
  const probe = new Date(Date.UTC(y, m - 1, d));
  if (probe.getUTCFullYear() !== y || probe.getUTCMonth() !== m - 1 || probe.getUTCDate() !== d) {
    return null;
  }
  return { y, m, d };
}

function pad(n: number, width = 2): string {
  return String(n).padStart(width, '0');
}

/**
 * `date` minus `months` calendar months, clamping the day to the end of the month
 * (same rule as chrono's `checked_sub_months` used by the Rust validation).
 */
export function subtractMonths(date: string, months: number): string | null {
  const parsed = parseDate(date);
  if (!parsed) return null;
  const total = parsed.y * 12 + (parsed.m - 1) - months;
  const y = Math.floor(total / 12);
  const m = total - y * 12 + 1;
  const lastDay = new Date(Date.UTC(y, m, 0)).getUTCDate();
  return `${pad(y, 4)}-${pad(m)}-${pad(Math.min(parsed.d, lastDay))}`;
}

const UUID_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;
const HASH_RE = /^[0-9a-f]{64}$/;

/** Inline validation of the filters for `report`; `null` when they can be sent. */
export function validateReportFilters(report: ReportKind, filters: ReportFilters): string | null {
  if (filters.worker_ids.some(id => !UUID_RE.test(id))) {
    return 'Hay un trabajador seleccionado con un identificador no válido.';
  }
  if (filters.worker_ids.length > MAX_SELECTED_WORKERS) {
    return `Selecciona hasta ${MAX_SELECTED_WORKERS} trabajadores o deja la selección vacía para incluir a todos.`;
  }
  const hasRange = Boolean(filters.from || filters.to);
  if (filters.period && hasRange) {
    return 'Elige un período predeterminado o un rango de fechas, no ambos.';
  }
  if (!filters.period) {
    if (!filters.from || !filters.to) {
      return 'Elige un período predeterminado o completa las dos fechas del rango.';
    }
    if (!parseDate(filters.from) || !parseDate(filters.to)) {
      return 'Las fechas del rango no son válidas.';
    }
    if (filters.from > filters.to) {
      return 'La fecha de inicio no puede ser posterior a la de término.';
    }
    const earliest = subtractMonths(filters.to, maxRangeMonths(report));
    if (earliest && filters.from < earliest) {
      return `El rango de este reporte no puede superar ${rangeLimitLabel(report)} (R38 art. 25).`;
    }
  }
  for (const [value, label] of [
    [filters.search, 'de búsqueda'],
    [filters.job_title, 'de cargo'],
  ] as const) {
    if (value && value.length > MAX_TEXT_FILTER_CHARS) {
      return `El filtro ${label} admite hasta ${MAX_TEXT_FILTER_CHARS} caracteres.`;
    }
  }
  if (filters.workplace_id && !UUID_RE.test(filters.workplace_id)) {
    return 'El lugar de trabajo seleccionado no es válido.';
  }
  if (filters.hash && !HASH_RE.test(filters.hash)) {
    return 'El hash debe tener 64 caracteres hexadecimales.';
  }
  return null;
}

// ---------------------------------------------------------------- cells

/** Visible marker of an altered value (ATT-05); explained in the legend. */
export const ALTERED_MARK = '*';
export const ALTERED_LEGEND = 'Valor alterado por una corrección (R38 arts. 39–41).';

export function isAlteredCell(cell: ReportCell): boolean {
  return typeof cell === 'object' && cell !== null && cell.altered === true;
}

/** The value exactly as the server formatted it (no rounding or reformatting). */
export function cellText(cell: ReportCell): string {
  if (cell === null || cell === undefined) return '';
  if (typeof cell === 'object') return cell.v ?? '';
  return String(cell);
}

/** Text for screen readers: altered values say so; empty cells are announced. */
export function cellAccessibleText(cell: ReportCell): string {
  const text = cellText(cell);
  const shown = text.trim() === '' ? 'vacío' : text;
  return isAlteredCell(cell) ? `${shown} (valor alterado)` : shown;
}

/** True when any row or total of the report has an altered cell (shows the legend). */
export function hasAlteredCells(rows: ReportCell[][]): boolean {
  return rows.some(row => row.some(isAlteredCell));
}

export function workplaceLabel(workplace: ReportWorkplace | null | undefined): string {
  if (!workplace) return '';
  if (typeof workplace === 'string') return workplace;
  return workplace.address ? `${workplace.name} — ${workplace.address}` : workplace.name;
}

// ---------------------------------------------------------------- worker picker

export interface ReportWorkerOption {
  id: string;
  name: string;
  run: string;
  workplaceId?: string | null;
  workplaceName?: string | null;
  jobTitle?: string | null;
}

function fold(value: string): string {
  return value
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '')
    .toLowerCase();
}

function runDigits(value: string): string {
  return value.replace(/[^0-9kK]/g, '').toLowerCase();
}

/** Alphabetical order by name (es-CL collation, accents ignored). */
export function sortWorkerOptions(options: ReportWorkerOption[]): ReportWorkerOption[] {
  return [...options].sort((a, b) => a.name.localeCompare(b.name, 'es-CL', { sensitivity: 'base' }));
}

/** Options matching `query` by name (accent/case-insensitive) or RUN (with or without dots). */
export function filterWorkerOptions(options: ReportWorkerOption[], query: string): ReportWorkerOption[] {
  const q = query.trim();
  if (!q) return sortWorkerOptions(options);
  const needle = fold(q);
  const digits = runDigits(q);
  return sortWorkerOptions(
    options.filter(o => {
      if (fold(o.name).includes(needle)) return true;
      return digits.length >= 3 && runDigits(o.run).includes(digits);
    }),
  );
}

/** Distinct job titles for the "cargo" suggestions, alphabetical. */
export function uniqueJobTitles(options: ReportWorkerOption[]): string[] {
  const set = new Set<string>();
  for (const o of options) {
    const title = (o.jobTitle ?? '').trim();
    if (title) set.add(title);
  }
  return [...set].sort((a, b) => a.localeCompare(b, 'es-CL', { sensitivity: 'base' }));
}

export const EXPORT_LABELS: Record<'xlsx' | 'pdf' | 'docx', string> = {
  xlsx: 'Excel',
  pdf: 'PDF',
  docx: 'Word',
};

export function savedToastText(path: string): string {
  return `Guardado en ${path}`;
}
