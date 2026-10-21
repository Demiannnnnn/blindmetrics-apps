// Pure helpers of the attendance window (TASK-AGENT-13). No React, no Tauri: tested with node --test.
// Formats follow R38 (dd/mm/aa, hh:mm:ss) and the worker app spec §3–§4.

import type {
  AppSnapshot,
  BannerKind,
  Correction,
  CorrectionStatus,
  LocalMarkStatus,
  MarkRecord,
  MarkSource,
  MarkType,
  ServerMark,
  TodayView,
  WorkerSchedule,
} from '../api/types';

// ---------------------------------------------------------------------------------------------
// Dates and times in the worker's timezone
// ---------------------------------------------------------------------------------------------

export interface ZonedParts {
  year: number;
  month: number;
  day: number;
  hour: number;
  minute: number;
  second: number;
  /** 0 = Sunday … 6 = Saturday. */
  weekday: number;
}

const WEEKDAY_INDEX: Record<string, number> = {
  Sun: 0,
  Mon: 1,
  Tue: 2,
  Wed: 3,
  Thu: 4,
  Fri: 5,
  Sat: 6,
};

const WEEKDAYS_ES = ['Domingo', 'Lunes', 'Martes', 'Miércoles', 'Jueves', 'Viernes', 'Sábado'];

function toDate(value: string | Date | number): Date | null {
  const d = value instanceof Date ? value : new Date(value);
  return Number.isNaN(d.getTime()) ? null : d;
}

/** Calendar parts of an instant in `timeZone` (device zone when absent or invalid). */
export function zonedParts(value: string | Date | number, timeZone?: string | null): ZonedParts | null {
  const d = toDate(value);
  if (!d) return null;
  const build = (tz: string | undefined) =>
    new Intl.DateTimeFormat('en-US', {
      timeZone: tz,
      year: 'numeric',
      month: '2-digit',
      day: '2-digit',
      hour: '2-digit',
      minute: '2-digit',
      second: '2-digit',
      weekday: 'short',
      hourCycle: 'h23',
    });
  let fmt: Intl.DateTimeFormat;
  try {
    fmt = build(timeZone || undefined);
  } catch {
    fmt = build(undefined);
  }
  const parts: Record<string, string> = {};
  for (const p of fmt.formatToParts(d)) parts[p.type] = p.value;
  return {
    year: Number(parts.year),
    month: Number(parts.month),
    day: Number(parts.day),
    hour: Number(parts.hour) % 24,
    minute: Number(parts.minute),
    second: Number(parts.second),
    weekday: WEEKDAY_INDEX[parts.weekday] ?? 0,
  };
}

const pad2 = (n: number) => String(n).padStart(2, '0');

/** `dd/mm/aa` (R38). */
export function formatDateCl(value: string | Date | number, timeZone?: string | null): string {
  const p = zonedParts(value, timeZone);
  if (!p) return '—';
  return `${pad2(p.day)}/${pad2(p.month)}/${pad2(p.year % 100)}`;
}

/** `hh:mm:ss` 24 h (R38). */
export function formatTimeCl(value: string | Date | number, timeZone?: string | null): string {
  const p = zonedParts(value, timeZone);
  if (!p) return '—';
  return `${pad2(p.hour)}:${pad2(p.minute)}:${pad2(p.second)}`;
}

/** "Sábado 11/10/26": weekday in words + `dd/mm/aa`. */
export function formatDayHeadingCl(value: string | Date | number, timeZone?: string | null): string {
  const p = zonedParts(value, timeZone);
  if (!p) return '—';
  return `${WEEKDAYS_ES[p.weekday]} ${pad2(p.day)}/${pad2(p.month)}/${pad2(p.year % 100)}`;
}

/** `YYYY-MM-DD` of an instant in the worker's timezone (grouping key, ATT-09). */
export function localDateKey(value: string | Date | number, timeZone?: string | null): string {
  const p = zonedParts(value, timeZone);
  if (!p) return '';
  return `${p.year}-${pad2(p.month)}-${pad2(p.day)}`;
}

/** `YYYY-MM-DD` (a calendar date, no timezone) → "Viernes 09/10/26". */
export function formatCalendarDate(dateKey: string): string {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(dateKey);
  if (!m) return dateKey;
  const [y, mo, d] = [Number(m[1]), Number(m[2]), Number(m[3])];
  const weekday = new Date(Date.UTC(y, mo - 1, d)).getUTCDay();
  return `${WEEKDAYS_ES[weekday]} ${pad2(d)}/${pad2(mo)}/${pad2(y % 100)}`;
}

/** "09:00:00" / "09:00" → "09:00" (agreed schedule times). */
export function shortClock(value: string | null | undefined): string {
  if (!value) return '';
  const m = /^(\d{1,2}):(\d{2})/.exec(value.trim());
  return m ? `${pad2(Number(m[1]))}:${m[2]}` : value;
}

/** Server time now: device clock plus the offset measured by the service (ATT-10). */
export function serverNowMs(deviceNowMs: number, offsetSecs: number | null | undefined): number {
  return deviceNowMs + (offsetSecs ?? 0) * 1000;
}

/** ATT-10: show the muted note when the device clock differs from the server by more than 60 s. */
export function hasClockSkew(offsetSecs: number | null | undefined): boolean {
  return offsetSecs !== null && offsetSecs !== undefined && Math.abs(offsetSecs) > 60;
}

// ---------------------------------------------------------------------------------------------
// Hashes and email
// ---------------------------------------------------------------------------------------------

/** Hash in groups of 4 separated by spaces ("3f9a 21c0 …"). */
export function groupHash(hash: string | null | undefined): string {
  if (!hash) return '';
  const clean = hash.replace(/\s+/g, '');
  const groups: string[] = [];
  for (let i = 0; i < clean.length; i += 4) groups.push(clean.slice(i, i + 4));
  return groups.join(' ');
}

/** First two groups of the hash, for lists ("3f9a 21c0 …"). */
export function shortHash(hash: string | null | undefined): string {
  if (!hash) return '';
  const clean = hash.replace(/\s+/g, '');
  return clean.length <= 8 ? groupHash(clean) : `${groupHash(clean.slice(0, 8))} …`;
}

/** "juan.perez@gmail.com" → "j•••@gmail.com"; null when it is not an email. */
export function maskEmail(email: string | null | undefined): string | null {
  if (!email) return null;
  const trimmed = email.trim();
  const at = trimmed.lastIndexOf('@');
  if (at < 1 || at === trimmed.length - 1) return null;
  const local = trimmed.slice(0, at);
  const domain = trimmed.slice(at + 1);
  if (!domain.includes('.')) return null;
  return `${Array.from(local)[0]}•••@${domain}`;
}

// ---------------------------------------------------------------------------------------------
// Mark types, next mark and statuses
// ---------------------------------------------------------------------------------------------

export const MARK_ORDER: MarkType[] = ['WORKDAY_START', 'BREAK_START', 'BREAK_END', 'WORKDAY_END'];

const MARK_TYPE_LABELS: Record<MarkType, string> = {
  WORKDAY_START: 'Entrada',
  BREAK_START: 'Inicio de colación',
  BREAK_END: 'Término de colación',
  WORKDAY_END: 'Salida',
};

const MARK_ACTION_LABELS: Record<MarkType, string> = {
  WORKDAY_START: 'Marcar entrada',
  BREAK_START: 'Iniciar colación',
  BREAK_END: 'Terminar colación',
  WORKDAY_END: 'Marcar salida',
};

export function markTypeLabel(type: MarkType): string {
  return MARK_TYPE_LABELS[type] ?? type;
}

/** Label of the primary button / dialog title for a mark type (spec §3). */
export function nextMarkLabel(type: MarkType): string {
  return MARK_ACTION_LABELS[type] ?? type;
}

/** The three marks listed under "Otras marcas" (always allowed, ATT-02). */
export function otherMarks(next: MarkType): MarkType[] {
  return MARK_ORDER.filter((t) => t !== next);
}

/** Next mark from the last one of the day, used only when the server's `next_mark` is unknown. */
export function nextMarkAfter(last: MarkType | null | undefined): MarkType {
  switch (last) {
    case 'WORKDAY_START':
      return 'BREAK_START';
    case 'BREAK_START':
      return 'BREAK_END';
    case 'BREAK_END':
      return 'WORKDAY_END';
    default:
      return 'WORKDAY_START';
  }
}

export type Tone = 'success' | 'warning' | 'danger' | 'info' | 'muted';

const REJECTION_REASONS: Record<string, string> = {
  DEVICE_TS_IN_FUTURE: 'la hora de tu equipo estaba adelantada',
  OFFLINE_MARK_TOO_OLD: 'la marca sin conexión tenía más de 30 días',
  INVALID_MARK_TYPE: 'tipo de marca no válido',
};

/** Status label of a mark of the local journal (spec §3 "Today's marks list"). */
export function markStatusLabel(
  status: LocalMarkStatus,
  rejectionReason?: string | null
): { label: string; tone: Tone } {
  switch (status) {
    case 'CONFIRMED':
      return { label: 'Enviada', tone: 'success' };
    case 'PENDING':
      return { label: 'Pendiente de envío', tone: 'warning' };
    case 'HASH_MISMATCH':
      return { label: 'Hash no coincide', tone: 'danger' };
    case 'REJECTED': {
      const reason = rejectionReason ? REJECTION_REASONS[rejectionReason] ?? rejectionReason : null;
      return { label: reason ? `Rechazada: ${reason}` : 'Rechazada', tone: 'danger' };
    }
    default:
      return { label: String(status), tone: 'muted' };
  }
}

export const HASH_MISMATCH_ADVICE = 'Guarda este comprobante y avisa a tu empleador.';

export function markSourceLabel(source: MarkSource): string {
  switch (source) {
    case 'AGENT':
      return 'App de este equipo';
    case 'PORTAL':
      return 'Portal web';
    case 'CORRECTION':
      return 'Corrección del empleador';
    default:
      return String(source);
  }
}

/**
 * The time that counts for a mark (contract v3.1 §9.1): `effective_ts` when the server sends it;
 * otherwise `device_ts` for offline or correction marks and `server_ts` for the rest.
 */
export function effectiveTs(mark: Pick<ServerMark, 'effective_ts' | 'offline' | 'source' | 'device_ts' | 'server_ts'>): string {
  if (mark.effective_ts) return mark.effective_ts;
  return mark.offline || mark.source === 'CORRECTION' ? mark.device_ts : mark.server_ts;
}

/** One row of a mark list: a journal record, a server mark, or both. */
export interface MarkRow {
  key: string;
  type: MarkType;
  /** Effective time (display and grouping). */
  ts: string;
  /** Local state; `CONFIRMED` for server marks without a local record. */
  status: LocalMarkStatus;
  rejectionReason: string | null;
  hash: string | null;
  corrected: boolean;
  superseded: boolean;
  server: ServerMark | null;
  local: MarkRecord | null;
}

function rowFromServer(mark: ServerMark, local: MarkRecord | null): MarkRow {
  return {
    key: mark.id,
    type: mark.type,
    ts: effectiveTs(mark),
    status: local?.status ?? 'CONFIRMED',
    rejectionReason: local?.rejection_reason ?? null,
    hash: mark.hash,
    corrected: !!mark.corrected_by || mark.source === 'CORRECTION',
    superseded: !!mark.superseded_by,
    server: mark,
    local,
  };
}

function rowFromLocal(record: MarkRecord): MarkRow {
  if (record.server_mark) return rowFromServer(record.server_mark, record);
  return {
    key: record.client_mark_id,
    type: record.type,
    ts: record.device_ts,
    status: record.status,
    rejectionReason: record.rejection_reason,
    hash: null,
    corrected: false,
    superseded: false,
    server: null,
    local: record,
  };
}

/**
 * Merges server marks and journal records (matched by `client_mark_id`), oldest first.
 * Journal records keep their local status; server-only marks (portal, corrections) are `Enviada`.
 */
export function mergeMarks(serverMarks: ServerMark[], localMarks: MarkRecord[]): MarkRow[] {
  const rows: MarkRow[] = [];
  const localById = new Map<string, MarkRecord>();
  for (const r of localMarks) localById.set(r.client_mark_id, r);
  const usedLocal = new Set<string>();
  const serverIds = new Set<string>();
  for (const m of serverMarks) {
    serverIds.add(m.id);
    const local = m.client_mark_id ? localById.get(m.client_mark_id) ?? null : null;
    if (local) usedLocal.add(local.client_mark_id);
    rows.push(rowFromServer(m, local));
  }
  for (const r of localMarks) {
    if (usedLocal.has(r.client_mark_id)) continue;
    if (r.server_mark && serverIds.has(r.server_mark.id)) continue;
    rows.push(rowFromLocal(r));
  }
  rows.sort((a, b) => Date.parse(a.ts) - Date.parse(b.ts));
  return rows;
}

/** Today's marks of the snapshot view. */
export function todayRows(view: TodayView | null | undefined): MarkRow[] {
  if (!view) return [];
  return mergeMarks(view.server?.marks ?? [], view.local_marks ?? []);
}

/** Primary mark of A: the server's `next_mark`, or the local journal when offline (spec §3). */
export function primaryMark(view: TodayView | null | undefined): MarkType {
  if (view?.server?.next_mark && view.source === 'SERVER') return view.server.next_mark;
  const rows = todayRows(view).filter((r) => r.status !== 'REJECTED' && !r.superseded);
  const last = rows.length > 0 ? rows[rows.length - 1].type : null;
  if (!last && view?.server?.next_mark) return view.server.next_mark;
  return nextMarkAfter(last);
}

/** "En jornada desde 09:02:11" / "En colación desde …" / "Fuera de jornada". */
export function dayStateLabel(view: TodayView | null | undefined, timeZone?: string | null): string {
  const rows = todayRows(view).filter((r) => r.status !== 'REJECTED' && !r.superseded);
  const last = rows.length > 0 ? rows[rows.length - 1] : null;
  const serverState = view?.source === 'SERVER' ? view.server?.state : undefined;
  const state =
    serverState && serverState !== 'UNKNOWN'
      ? serverState
      : last?.type === 'WORKDAY_START' || last?.type === 'BREAK_END'
        ? 'WORKING'
        : last?.type === 'BREAK_START'
          ? 'ON_BREAK'
          : 'OFF';
  if (state === 'WORKING') {
    const start = [...rows].reverse().find((r) => r.type === 'WORKDAY_START');
    return start ? `En jornada desde ${formatTimeCl(start.ts, timeZone)}` : 'En jornada';
  }
  if (state === 'ON_BREAK') {
    const br = [...rows].reverse().find((r) => r.type === 'BREAK_START');
    return br ? `En colación desde ${formatTimeCl(br.ts, timeZone)}` : 'En colación';
  }
  return 'Fuera de jornada';
}

// ---------------------------------------------------------------------------------------------
// Receipts
// ---------------------------------------------------------------------------------------------

/** Plain-text printable receipt of a mark (Registros → *Copiar comprobante*). */
export function receiptText(
  row: MarkRow,
  timeZone?: string | null,
  ctx?: { organization?: string | null; worker?: string | null }
): string {
  const lines: string[] = ['Comprobante de marca de asistencia'];
  if (ctx?.organization) lines.push(`Empleador: ${ctx.organization}`);
  if (ctx?.worker) lines.push(`Trabajador: ${ctx.worker}`);
  lines.push(`Tipo: ${markTypeLabel(row.type)}`);
  lines.push(`Fecha: ${formatDateCl(row.ts, timeZone)}`);
  lines.push(`Hora: ${formatTimeCl(row.ts, timeZone)}`);
  const m = row.server;
  if (m) {
    lines.push(
      `Registrada por el servidor: ${formatDateCl(m.server_ts, timeZone)} ${formatTimeCl(m.server_ts, timeZone)}`
    );
    lines.push(`Hora del equipo: ${formatDateCl(m.device_ts, timeZone)} ${formatTimeCl(m.device_ts, timeZone)}`);
    lines.push(`Sin conexión: ${m.offline ? 'Sí' : 'No'}`);
    lines.push(`Origen: ${markSourceLabel(m.source)}`);
    if (m.workplace?.name) {
      lines.push(`Lugar de trabajo: ${m.workplace.name}${m.workplace.address ? ` (${m.workplace.address})` : ''}`);
    }
    if (m.location) {
      lines.push(`Ubicación: ${m.location.lat}, ${m.location.lon} (±${m.location.accuracy_m} m)`);
    }
    if (row.corrected) lines.push('Corregida: Sí');
    lines.push(`Hash: ${groupHash(m.hash)}`);
    lines.push(`Hash anterior: ${groupHash(m.prev_hash)}`);
  } else {
    lines.push(`Hora del equipo: ${formatDateCl(row.ts, timeZone)} ${formatTimeCl(row.ts, timeZone)}`);
  }
  lines.push(`Estado: ${markStatusLabel(row.status, row.rejectionReason).label}`);
  return lines.join('\n');
}

// ---------------------------------------------------------------------------------------------
// Banners (spec §1: one at a time, by priority)
// ---------------------------------------------------------------------------------------------

export type BannerAction = 'DIAGNOSTICS' | 'ATTENDANCE' | 'RECORDS' | 'TERMS' | 'TELEMETRY';

export interface BannerView {
  kind: BannerKind;
  tone: 'danger' | 'warning' | 'info';
  message: string;
  actionLabel: string;
  action: BannerAction;
}

export const BANNER_PRIORITY: BannerKind[] = [
  'SERVICE_PROBLEM',
  'PENDING_MARKS_OFFLINE',
  'CORRECTION_TO_REVIEW',
  'NEW_TERMS_VERSION',
  'TELEMETRY_NOTICE',
];

type BannerInput = Pick<
  AppSnapshot,
  'banners' | 'pending_marks' | 'pending_corrections' | 'run_mode' | 'last_error' | 'pending_withdrawal'
>;

function telemetryNoticeMessage(snap: BannerInput): string {
  if (snap.pending_withdrawal) {
    return 'Retiro de consentimiento pendiente de confirmación. La recolección está detenida.';
  }
  switch (snap.run_mode) {
    case 'EMPLOYER_SUSPENDED':
      return 'La telemetría está detenida por la suscripción de tu empleador. Tu asistencia funciona normalmente.';
    case 'HALTED':
      return 'La telemetría se detuvo por seguridad. Tu asistencia funciona normalmente.';
    case 'AWAITING_CONSENT':
      return 'Tu empleador solicita datos de este equipo. Revisa la solicitud para decidir.';
    default:
      return 'Hay un aviso sobre la telemetría de este equipo.';
  }
}

/** The single banner to show (highest priority), or null. */
export function selectBanner(snap: BannerInput | null | undefined): BannerView | null {
  if (!snap) return null;
  const present = new Set<BannerKind>(snap.banners ?? []);
  if (snap.pending_withdrawal) present.add('TELEMETRY_NOTICE');
  const kind = BANNER_PRIORITY.find((k) => present.has(k));
  if (!kind) return null;
  switch (kind) {
    case 'SERVICE_PROBLEM':
      return {
        kind,
        tone: 'danger',
        message: snap.last_error
          ? `El servicio tiene un problema: ${snap.last_error}`
          : 'El servicio tiene un problema. Tus marcas se guardan en este equipo.',
        actionLabel: 'Ver diagnóstico',
        action: 'DIAGNOSTICS',
      };
    case 'PENDING_MARKS_OFFLINE': {
      const n = snap.pending_marks;
      return {
        kind,
        tone: 'warning',
        message:
          n === 1
            ? 'Tienes 1 marca pendiente de envío. Se enviará automáticamente al recuperar la conexión.'
            : `Tienes ${n} marcas pendientes de envío. Se enviarán automáticamente al recuperar la conexión.`,
        actionLabel: 'Ver marcas',
        action: 'ATTENDANCE',
      };
    }
    case 'CORRECTION_TO_REVIEW':
      return {
        kind,
        tone: 'info',
        message: 'Tu empleador propuso una corrección. Tienes 48 horas para revisarla.',
        actionLabel: 'Revisar',
        action: 'RECORDS',
      };
    case 'NEW_TERMS_VERSION':
      return {
        kind,
        tone: 'info',
        message: 'Hay una nueva versión de los términos. Léela.',
        actionLabel: 'Leer términos',
        action: 'TERMS',
      };
    case 'TELEMETRY_NOTICE':
      return {
        kind,
        tone: snap.run_mode === 'HALTED' ? 'danger' : 'warning',
        message: telemetryNoticeMessage(snap),
        actionLabel: 'Ver telemetría',
        action: 'TELEMETRY',
      };
    default:
      return null;
  }
}

// ---------------------------------------------------------------------------------------------
// Navigation (RF-AGENT-13-8)
// ---------------------------------------------------------------------------------------------

export type TopScreen = 'ENROLLMENT' | 'TERMS' | 'PIN' | 'SHELL';

/** E1 → T1 → P1 → A, from the service's AttendanceMode. The rail only exists on `SHELL`. */
export function screenForMode(snap: Pick<AppSnapshot, 'is_enrolled' | 'attendance_mode'>): TopScreen {
  if (!snap.is_enrolled || snap.attendance_mode === 'UNLINKED') return 'ENROLLMENT';
  if (snap.attendance_mode === 'NEEDS_TERMS') return 'TERMS';
  if (snap.attendance_mode === 'NEEDS_PIN') return 'PIN';
  return 'SHELL';
}

// ---------------------------------------------------------------------------------------------
// Terms reading progress (LEG-04)
// ---------------------------------------------------------------------------------------------

/** True when the end of a document is visible in the scroll viewport (8 px tolerance). */
export function reachedEnd(
  docEndOffset: number,
  scrollTop: number,
  clientHeight: number,
  tolerance = 8
): boolean {
  return scrollTop + clientHeight + tolerance >= docEndOffset;
}

/** *He leído y acepto* is enabled only after reaching the end of both documents. */
export function canAcceptTerms(progress: { terms: boolean; privacy: boolean }, busy = false): boolean {
  return progress.terms && progress.privacy && !busy;
}

/** First 16 hex of a document hash, shown at the top of T1. */
export function hashPrefix(hash: string): string {
  return hash.slice(0, 16);
}

// ---------------------------------------------------------------------------------------------
// PIN (informative mirror; the service applies the real rule, SEC-20)
// ---------------------------------------------------------------------------------------------

/** First policy problem of a new PIN, or null. Mirrors `telemetry_crypto_core::pin::validate_pin`. */
export function pinPolicyMessage(pin: string): string | null {
  if (pin.length === 0) return 'Ingresa un PIN de 6 a 8 dígitos.';
  if (!/^\d+$/.test(pin)) return 'El PIN solo puede tener números.';
  if (pin.length < 6 || pin.length > 8) return 'El PIN debe tener entre 6 y 8 dígitos.';
  if (/^(\d)\1+$/.test(pin)) return 'No uses el mismo número repetido, como 111111.';
  const digits = Array.from(pin, Number);
  const asc = digits.every((d, i) => i === 0 || d === digits[i - 1] + 1);
  const desc = digits.every((d, i) => i === 0 || d === digits[i - 1] - 1);
  if (asc || desc) return 'No uses números repetidos ni seguidos como 123456.';
  return null;
}

/** Validation of the two "new PIN" fields of P1 / *Cambiar PIN* / reset. */
export function newPinMessage(pin: string, confirm: string): string | null {
  const policy = pinPolicyMessage(pin);
  if (policy) return policy;
  if (pin !== confirm) return 'Los PIN no coinciden.';
  return null;
}

/** Keeps only digits, at most 8 (PIN fields). */
export function sanitizePinInput(raw: string): string {
  return raw.replace(/\D/g, '').slice(0, 8);
}

/** Message of a service error in the PIN/mark flows (spec §2, §3, §6). */
export function attendanceErrorMessage(err: { code: string; message: string; retry_after_secs?: number }): string {
  switch (err.code) {
    case 'PIN_INCORRECT':
    case 'PIN_RETRY_LATER':
      return err.retry_after_secs && err.retry_after_secs > 0
        ? `PIN incorrecto. Espera ${err.retry_after_secs} s para reintentar.`
        : 'PIN incorrecto.';
    case 'TERMS_REQUIRED':
      return 'Acepta los términos antes de marcar.';
    case 'TERMS_OUTDATED':
      return 'Hay una versión más nueva de esta app. Actualízala para continuar.';
    case 'PIN_NOT_SET':
      return 'Crea tu PIN de asistencia antes de continuar.';
    case 'ATTENDANCE_EXEMPT':
      return 'Tu contrato no está sujeto a control de jornada (art. 22 del Código del Trabajo). No necesitas marcar.';
    case 'SERVICE_NOT_RUNNING':
    case 'IO_ERROR':
    case 'TIMEOUT':
      return 'No se pudo hablar con el servicio de este equipo. Intenta de nuevo.';
    default:
      return err.message || 'No se pudo completar la acción.';
  }
}

/** Seconds left of a retry delay that started at `startedAtMs`. */
export function remainingDelay(totalSecs: number, startedAtMs: number, nowMs: number): number {
  return Math.max(0, Math.ceil(totalSecs - (nowMs - startedAtMs) / 1000));
}

// ---------------------------------------------------------------------------------------------
// Registros: ranges and day summaries
// ---------------------------------------------------------------------------------------------

export type RangePreset = 'THIS_WEEK' | 'LAST_WEEK' | 'THIS_MONTH' | 'LAST_MONTH' | 'CUSTOM';

export interface DateRange {
  from: string;
  to: string;
}

function keyOf(d: Date): string {
  return `${d.getUTCFullYear()}-${pad2(d.getUTCMonth() + 1)}-${pad2(d.getUTCDate())}`;
}

function parseKey(key: string): Date | null {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(key);
  if (!m) return null;
  const d = new Date(Date.UTC(Number(m[1]), Number(m[2]) - 1, Number(m[3])));
  return keyOf(d) === key ? d : null;
}

/** Range of a preset relative to `today` (`YYYY-MM-DD` in the worker's zone). Weeks start Monday. */
export function rangeForPreset(preset: Exclude<RangePreset, 'CUSTOM'>, today: string): DateRange | null {
  const t = parseKey(today);
  if (!t) return null;
  const day = 24 * 3600 * 1000;
  const mondayOffset = (t.getUTCDay() + 6) % 7;
  const monday = new Date(t.getTime() - mondayOffset * day);
  switch (preset) {
    case 'THIS_WEEK':
      return { from: keyOf(monday), to: today };
    case 'LAST_WEEK': {
      const prevMonday = new Date(monday.getTime() - 7 * day);
      const prevSunday = new Date(monday.getTime() - day);
      return { from: keyOf(prevMonday), to: keyOf(prevSunday) };
    }
    case 'THIS_MONTH':
      return { from: keyOf(new Date(Date.UTC(t.getUTCFullYear(), t.getUTCMonth(), 1))), to: today };
    case 'LAST_MONTH': {
      const first = new Date(Date.UTC(t.getUTCFullYear(), t.getUTCMonth() - 1, 1));
      const last = new Date(Date.UTC(t.getUTCFullYear(), t.getUTCMonth(), 0));
      return { from: keyOf(first), to: keyOf(last) };
    }
    default:
      return null;
  }
}

/** Days between two `YYYY-MM-DD` dates (inclusive count − 1). */
export function daysBetween(from: string, to: string): number {
  const a = parseKey(from);
  const b = parseKey(to);
  if (!a || !b) return NaN;
  return Math.round((b.getTime() - a.getTime()) / (24 * 3600 * 1000));
}

/** Server day summaries are available for at most 62 days per query (`api-contract.md` §9.2). */
export const MAX_SUMMARY_DAYS = 62;

/** Error of a custom range, or null: valid dates, from ≤ to, not in the future, ≤ 5 years back. */
export function customRangeError(range: DateRange, today: string): string | null {
  const from = parseKey(range.from);
  const to = parseKey(range.to);
  const t = parseKey(today);
  if (!from || !to || !t) return 'Elige una fecha inicial y una final.';
  if (from.getTime() > to.getTime()) return 'La fecha inicial debe ser anterior a la final.';
  if (to.getTime() > t.getTime()) return 'La fecha final no puede ser posterior a hoy.';
  const oldest = new Date(Date.UTC(t.getUTCFullYear() - 5, t.getUTCMonth(), t.getUTCDate()));
  if (from.getTime() < oldest.getTime()) return 'Puedes consultar hasta 5 años hacia atrás.';
  return null;
}

/** Groups mark rows by effective date in the worker's zone, newest day first (RF-AGENT-13-8b). */
export function groupRowsByDay(rows: MarkRow[], timeZone?: string | null): Array<{ date: string; rows: MarkRow[] }> {
  const map = new Map<string, MarkRow[]>();
  for (const r of rows) {
    const key = localDateKey(r.ts, timeZone);
    const list = map.get(key) ?? [];
    list.push(r);
    map.set(key, list);
  }
  return [...map.entries()]
    .sort((a, b) => (a[0] < b[0] ? 1 : a[0] > b[0] ? -1 : 0))
    .map(([date, list]) => ({
      date,
      rows: [...list].sort((x, y) => Date.parse(x.ts) - Date.parse(y.ts)),
    }));
}

/** Anomaly codes of `DaySummary` (API `src/attendance/days.rs`) in the worker's words (RF-ADMIN-21-3). */
const DAY_ANOMALIES: Readonly<Record<string, { label: string; hint: string }>> = {
  MISSING_START: { label: 'Entrada no registrada', hint: 'Ese día tenías jornada pactada y no hay marca de entrada.' },
  MISSING_END: { label: 'Salida no registrada', hint: 'Hay marca de entrada pero no de salida.' },
  BREAK_WITHOUT_END: { label: 'Colación sin término', hint: 'Marcaste el inicio de la colación pero no su término.' },
  BREAK_WITHOUT_START: { label: 'Término de colación sin inicio', hint: 'Marcaste el término de la colación sin marcar su inicio.' },
  MARK_ON_REST_DAY: { label: 'Marca en día de descanso', hint: 'Marcaste un día que tu jornada pactada no incluye.' },
  OFFLINE_MARK: { label: 'Marca sin conexión', hint: 'Una marca se guardó sin conexión y se envió después; cuenta la hora de tu equipo.' },
  CORRECTED_MARK: { label: 'Incluye una corrección', hint: 'Una marca de este día fue corregida por el procedimiento de corrección.' },
  NO_RECEIPT_EMAIL: {
    label: 'Sin correo para comprobantes',
    hint: 'Tu empleador no tiene registrado tu correo personal, por eso no recibes el comprobante de tus marcas por correo. Pídele que lo agregue.',
  },
  OUT_OF_ORDER: { label: 'Marca fuera de secuencia', hint: '' },
  DEVICE_CLOCK_OFFSET: { label: 'Desfase en el reloj del equipo', hint: '' },
};

/** Explanation of an anomaly code ('' when unknown). */
export function anomalyHint(code: string): string {
  return Object.prototype.hasOwnProperty.call(DAY_ANOMALIES, code) ? DAY_ANOMALIES[code].hint : '';
}

export function describeAnomaly(code: string): string {
  if (Object.prototype.hasOwnProperty.call(DAY_ANOMALIES, code)) return DAY_ANOMALIES[code].label;
  return code.replace(/_/g, ' ').toLowerCase().replace(/^\w/, (c) => c.toUpperCase());
}

/** Absence of a day summary in words; the system never labels it by itself (v3.1). */
export function absenceLabel(absence: string | null | undefined, hasMarks: boolean, agreed: boolean): string | null {
  if (absence === 'JUSTIFIED') return 'Ausencia justificada';
  if (absence === 'UNJUSTIFIED') return 'Ausencia injustificada';
  if (!hasMarks && agreed) return 'Sin marcas';
  return null;
}

// ---------------------------------------------------------------------------------------------
// Corrections (ATT-05)
// ---------------------------------------------------------------------------------------------

export function correctionStatusLabel(status: CorrectionStatus): { label: string; tone: Tone } {
  switch (status) {
    case 'PENDING':
      return { label: 'Pendiente', tone: 'warning' };
    case 'ACCEPTED':
      return { label: 'Aceptada', tone: 'success' };
    case 'OPPOSED':
      return { label: 'Te opusiste: se mantiene tu marca original', tone: 'muted' };
    case 'CONSOLIDATED':
      return { label: 'Consolidada (sin respuesta en 48 horas)', tone: 'info' };
    case 'WITHDRAWN':
      return { label: 'Retirada por tu empleador', tone: 'muted' };
    default:
      return { label: 'Estado desconocido', tone: 'muted' };
  }
}

/** Pending corrections first (closest deadline first), then history (newest first). */
export function sortCorrections(list: Correction[]): { pending: Correction[]; history: Correction[] } {
  const pending = list
    .filter((c) => c.status === 'PENDING')
    .sort((a, b) => Date.parse(a.respond_before ?? '') - Date.parse(b.respond_before ?? ''));
  const history = list
    .filter((c) => c.status !== 'PENDING')
    .sort(
      (a, b) =>
        Date.parse(b.responded_at ?? b.created_at ?? b.proposed_ts) -
        Date.parse(a.responded_at ?? a.created_at ?? a.proposed_ts)
    );
  return { pending, history };
}

/** "Quedan 1 d 3 h" style countdown to the deadline; "Plazo vencido" when it passed. */
export function countdownLabel(deadlineIso: string | null | undefined, nowMs: number): string | null {
  if (!deadlineIso) return null;
  const end = Date.parse(deadlineIso);
  if (Number.isNaN(end)) return null;
  const secs = Math.floor((end - nowMs) / 1000);
  if (secs <= 0) return 'Plazo vencido';
  const d = Math.floor(secs / 86400);
  const h = Math.floor((secs % 86400) / 3600);
  const m = Math.floor((secs % 3600) / 60);
  if (d > 0) return `Quedan ${d} d ${h} h`;
  if (h > 0) return `Quedan ${h} h ${m} min`;
  return `Quedan ${Math.max(1, m)} min`;
}

/** "Puedes oponerte hasta el 12/10/26 14:30" (spec §4). */
export function opposeUntilLabel(deadlineIso: string | null | undefined, timeZone?: string | null): string | null {
  if (!deadlineIso) return null;
  const p = zonedParts(deadlineIso, timeZone);
  if (!p) return null;
  return `Puedes oponerte hasta el ${formatDateCl(deadlineIso, timeZone)} ${pad2(p.hour)}:${pad2(p.minute)}`;
}

/**
 * Location line of the attendance home (spec §3, ATT-08). Never claims a position is recorded when
 * this computer cannot obtain one (RF-AGENT-15-1, PLAT-02); older services without the field are
 * treated as "not available".
 */
export function locationAtMarkNotice(atMark: boolean | undefined, available: boolean | undefined): string {
  if (!atMark) return 'Ubicación: no se registra al marcar.';
  if (available) return 'Ubicación: se registra tu ubicación solo en el momento de marcar.';
  return 'Ubicación: tu empleador pidió registrarla al marcar, pero este equipo no puede obtenerla. Tus marcas se envían sin ubicación.';
}

// ---------------------------------------------------------------------------------------------
// Agreed schedule in words (TASK-AGENT-16)
// ---------------------------------------------------------------------------------------------

/** R38 art. 25.1 e journey types (same list as the admin console). */
export const JOURNEY_TYPE_LABELS: Readonly<Record<string, string>> = {
  FIXED: 'Jornada fija',
  SHIFTS: 'Turnos',
  CYCLES: 'Ciclos',
  BIWEEKLY: 'Bisemanal',
  EXCEPTIONAL: 'Jornada excepcional',
  PART_TIME: 'Jornada parcial',
};

export function journeyTypeLabel(code: string): string {
  return Object.prototype.hasOwnProperty.call(JOURNEY_TYPE_LABELS, code) ? JOURNEY_TYPE_LABELS[code] : code;
}

const SCHEDULE_WEEKDAYS = ['Lunes', 'Martes', 'Miércoles', 'Jueves', 'Viernes', 'Sábado', 'Domingo'];

export interface ScheduleWeekRow {
  weekday: number;
  name: string;
  /** "09:00 – 18:00" or "Descanso". */
  hours: string;
  /** "Colación 13:00 – 14:00" or ''. */
  breakText: string;
  works: boolean;
}

/** Seven rows Monday → Sunday; days without an entry are rest days. */
export function scheduleWeekRows(schedule: WorkerSchedule): ScheduleWeekRow[] {
  return SCHEDULE_WEEKDAYS.map((name, index) => {
    const weekday = index + 1;
    const day = schedule.days.find((d) => d.weekday === weekday);
    if (!day) return { weekday, name, hours: 'Descanso', breakText: '', works: false };
    const overnight = shortClock(day.end) < shortClock(day.start) ? ' (termina al día siguiente)' : '';
    const breakText =
      day.break_start && day.break_end ? `Colación ${shortClock(day.break_start)} – ${shortClock(day.break_end)}` : '';
    return {
      weekday,
      name,
      hours: `${shortClock(day.start)} – ${shortClock(day.end)}${overnight}`,
      breakText,
      works: true,
    };
  });
}

/** Extra facts shown under the week table, in a stable order. */
export function scheduleFacts(schedule: WorkerSchedule): string[] {
  const facts = [`${journeyTypeLabel(schedule.journey_type)} · vigente desde el ${formatCalendarDate(schedule.effective_from)}`];
  if (schedule.exceptional_resolution) {
    facts.push(
      `Autorizada por resolución N° ${schedule.exceptional_resolution.number} del ${formatCalendarDate(schedule.exceptional_resolution.date)}`
    );
  }
  if (schedule.days.some((d) => d.break_start && d.break_end)) {
    facts.push(schedule.break_counts_as_work ? 'La colación se cuenta como tiempo trabajado.' : 'La colación no se cuenta como tiempo trabajado.');
  }
  if (schedule.disconnection) {
    facts.push(
      `Desconexión: de ${shortClock(schedule.disconnection.start)} a ${shortClock(schedule.disconnection.end)}. En ese horario no tienes que responder comunicaciones de trabajo.`
    );
  }
  if (schedule.requested_by === 'WORKER') facts.push('Esta jornada fue solicitada por ti.');
  if (schedule.note && schedule.note.trim()) facts.push(`Nota: ${schedule.note.trim()}`);
  return facts;
}

/** Heading of an upcoming version: "Desde el Sábado 01/11/26 tu jornada cambia". */
export function nextScheduleHeading(next: WorkerSchedule): string {
  return `Desde el ${formatCalendarDate(next.effective_from)} tu jornada cambia`;
}
