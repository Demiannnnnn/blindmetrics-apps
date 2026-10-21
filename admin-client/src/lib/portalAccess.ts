// admin-client/src/lib/portalAccess.ts — ex-worker portal access (TASK-ADMIN-20, API TASK-27, R38 art. 22.1).
// Pure module: imported by the UI and by node tests (only erasable TypeScript here).

/** Days the API keeps a granted portal access (ATTENDANCE.md §11.5). */
export const PORTAL_ACCESS_DAYS = 90;

const DEFAULT_TIMEZONE = 'America/Santiago';

/** Same rule as `validate_personal_email` in the Rust core: trimmed, ≤ 254 chars, one `@`, dotted domain. */
export function portalAccessEmailError(raw: string): string | null {
  const e = raw.trim();
  if (!e) return 'Ingresa el correo del ex-trabajador.';
  const at = e.indexOf('@');
  const local = at > 0 ? e.slice(0, at) : '';
  const domain = at > 0 ? e.slice(at + 1) : '';
  const ok =
    [...e].length <= 254 &&
    !/\s/.test(e) &&
    local.length > 0 &&
    !domain.includes('@') &&
    domain.includes('.') &&
    !domain.startsWith('.') &&
    !domain.endsWith('.');
  return ok ? null : 'Ingresa un correo válido para el acceso al portal.';
}

/** Lower-cased, trimmed email as sent to the API. */
export function normalizePortalAccessEmail(raw: string): string {
  return raw.trim().toLowerCase();
}

export interface PortalAccessState {
  /** True while `portal_access_until` is in the future. */
  enabled: boolean;
  /** Parsed deadline, or null when absent/invalid. */
  until: Date | null;
}

/** Whether the ex-worker can still sign in to the portal at `now`. */
export function portalAccessState(
  portalAccessUntil: string | null | undefined,
  now: Date = new Date(),
): PortalAccessState {
  if (!portalAccessUntil) return { enabled: false, until: null };
  const until = new Date(portalAccessUntil);
  if (isNaN(until.getTime())) return { enabled: false, until: null };
  return { enabled: until.getTime() > now.getTime(), until };
}

function isValidTz(tz: string): boolean {
  try {
    new Intl.DateTimeFormat('es-CL', { timeZone: tz });
    return true;
  } catch {
    return false;
  }
}

/** `dd/mm/yyyy, hh:mm` in the worker's time zone (es-CL), falling back to America/Santiago. */
export function formatPortalAccessUntil(iso: string, timezone?: string | null): string {
  const d = new Date(iso);
  if (isNaN(d.getTime())) return iso;
  const tz = timezone && isValidTz(timezone) ? timezone : DEFAULT_TIMEZONE;
  const parts = new Intl.DateTimeFormat('es-CL', {
    timeZone: tz,
    day: '2-digit',
    month: '2-digit',
    year: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
    hour12: false,
  }).formatToParts(d);
  const get = (type: string) => parts.find(p => p.type === type)?.value ?? '';
  return `${get('day')}/${get('month')}/${get('year')} ${get('hour')}:${get('minute')}`;
}

/** Specific Spanish message for the API errors of `POST|DELETE /workers/:id/portal-access`, or null. */
export function portalAccessErrorMessage(err: unknown): string | null {
  const code =
    err && typeof err === 'object' && 'code' in err && typeof err.code === 'string' ? err.code : undefined;
  switch (code) {
    case 'VALIDATION_ERROR':
      return 'El acceso al portal solo se puede habilitar para ex-trabajadores (estado revocado) y con un correo válido.';
    case 'NOT_FOUND':
      return 'El trabajador no existe o no pertenece a tu cuenta. Recarga la lista de trabajadores.';
    case 'CONFLICT':
      return 'Ese correo ya lo usa otro trabajador de tu cuenta (como correo personal o con acceso vigente). Usa otro correo.';
    default:
      return null;
  }
}
