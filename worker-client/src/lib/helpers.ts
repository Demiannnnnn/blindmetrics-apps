import type { CategoryState, ConsentFlags } from '../api/types';

/**
 * Normalizes an enrollment code input into the canonical XXXX-XXXX-XXXX format.
 * Strips unexpected symbols, converts to uppercase, and adds hyphens.
 */
export function normalizeCode(raw: string): string {
  const clean = raw.replace(/[^a-zA-Z0-9]/g, '').toUpperCase().slice(0, 12);
  const parts: string[] = [];
  for (let i = 0; i < clean.length; i += 4) {
    parts.push(clean.slice(i, i + 4));
  }
  return parts.join('-');
}

/**
 * Formats a 12-digit safety code into "dddd dddd dddd" with spacing.
 */
export function formatSafetyCode(raw: string): string {
  const digits = raw.replace(/\D/g, '').slice(0, 12);
  const parts: string[] = [];
  for (let i = 0; i < digits.length; i += 4) {
    parts.push(digits.slice(i, i + 4));
  }
  return parts.join(' ');
}

/**
 * Maps CategoryState to Spanish UI label and visual tone.
 */
export function getCategoryStateDetails(state: CategoryState): {
  label: string;
  tone: 'active' | 'muted' | 'warning' | 'danger';
} {
  switch (state) {
    case 'ACTIVE':
      return { label: 'Activa', tone: 'active' };
    case 'NOT_ACCEPTED':
      return { label: 'No aceptada', tone: 'muted' };
    case 'NOT_REQUESTED':
      return { label: 'No solicitada', tone: 'muted' };
    case 'SLEEPING':
      return { label: 'Durmiendo', tone: 'warning' };
    case 'UNAVAILABLE':
      return { label: 'No disponible', tone: 'danger' };
    default:
      return { label: 'Desconocido', tone: 'muted' };
  }
}

/**
 * Counts the number of categories selected (true) in a ConsentFlags object.
 */
export function countSelectedCategories(flags?: ConsentFlags | null): number {
  if (!flags) return 0;
  return Object.values(flags).filter(Boolean).length;
}

/**
 * Returns contextual banner info based on the service's RunMode.
 */
export function getRunModeBanner(
  runMode: string,
  lastError?: string | null
): { title: string; message: string; tone: 'warning' | 'danger' | 'info' } | null {
  switch (runMode) {
    case 'PAUSED':
      return {
        title: 'Telemetría en pausa',
        message: 'Tu empleador pausó temporalmente la telemetría. No se recolectan datos.',
        tone: 'warning',
      };
    case 'EMPLOYER_SUSPENDED':
      return {
        title: 'Suscripción suspendida',
        message:
          'La suscripción de tu empleador está suspendida. La recolección de telemetría está detenida, pero tu asistencia sigue funcionando normalmente.',
        tone: 'warning',
      };
    case 'HALTED':
      return {
        title: 'Telemetría detenida por seguridad',
        message:
          lastError ||
          'Se detectó un cambio no verificado en la clave de cifrado. La telemetría se detuvo por precaución.',
        tone: 'danger',
      };
    case 'AWAITING_CONSENT':
      return {
        title: 'Solicitud de telemetría pendiente',
        message: 'Tu empleador solicita datos de este equipo. Revisa la solicitud para decidir.',
        tone: 'info',
      };
    default:
      return null;
  }
}

/**
 * Formats seconds into human-readable uptime (e.g. "2h 15m").
 */
export function formatUptime(seconds: number): string {
  if (seconds < 60) return `${seconds} s`;
  const m = Math.floor(seconds / 60);
  if (m < 60) return `${m} m`;
  const h = Math.floor(m / 60);
  const remainingM = m % 60;
  if (h < 24) return `${h} h ${remainingM} m`;
  const d = Math.floor(h / 24);
  const remainingH = h % 24;
  return `${d} d ${remainingH} h`;
}

/**
 * Formats byte counts to human-readable strings.
 */
export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}
