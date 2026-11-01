// admin-client/src/lib/telemetryHealth.ts — Telemetry health alerts helpers (TASK-ADMIN-25)

import type {
  ConsentRejectionDecision,
  RecentRejectionWorker,
} from '../api/types';

/**
 * Formats a date in Chilean R38 official format: dd/mm/aa (e.g. 25/10/26).
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
 * Calculates a human-readable relative time string in es-CL (e.g. "hace 25 h", "hace 3 días").
 */
export function formatRelativeTime(iso: string | null, now = Date.now()): string {
  if (!iso) return 'nunca';
  const timestamp = new Date(iso).getTime();
  if (isNaN(timestamp)) return 'nunca';
  const diffMs = now - timestamp;
  if (diffMs < 0) return 'hace un momento';
  const minutes = Math.round(diffMs / 60000);
  if (minutes < 2) return 'hace un momento';
  if (minutes < 60) return `hace ${minutes} min`;
  const hours = Math.round(minutes / 60);
  if (hours < 48) return `hace ${hours} h`;
  const days = Math.round(hours / 24);
  return `hace ${days} días`;
}

/**
 * Formats telemetry status for "sin datos" list item (RF-ADMIN-25-3):
 * "último dato hace X" or "nunca envió datos".
 */
export function formatLastTelemetry(iso: string | null, now = Date.now()): string {
  if (!iso) return 'nunca envió datos';
  return `último dato ${formatRelativeTime(iso, now)}`;
}

/**
 * Returns "Rechazó" or "Retiró" based on decision (RF-ADMIN-25-3).
 */
export function formatRejectionDecision(decision: ConsentRejectionDecision | string): string {
  return decision.trim().toUpperCase() === 'WITHDRAWN' ? 'Retiró' : 'Rechazó';
}

/**
 * Formats rejection detail for recent rejections list item (RF-ADMIN-25-3):
 * "Rechazó/Retiró · fecha · vN".
 */
export function formatRejectionDetail(
  worker: RecentRejectionWorker,
  timezone = 'America/Santiago'
): string {
  const decisionText = formatRejectionDecision(worker.decision);
  const dateText = formatR38Date(worker.decided_at, timezone);
  return `${decisionText} · ${dateText} · v${worker.consent_version}`;
}

/**
 * Returns "y M más" when total > count of workers in the list (RF-ADMIN-25-3).
 */
export function formatRemainingCount(total: number, shown: number): string | null {
  if (total > shown) {
    return `y ${total - shown} más`;
  }
  return null;
}

/**
 * Formats alert line text for active without data (RF-ADMIN-25-2, RF-ADMIN-25-4).
 */
export function formatWithoutDataAlertLabel(count: number | null, staleHours: number = 24): string {
  const countStr = count === null ? '—' : count.toString();
  return `${countStr} trabajador(es) activos sin datos hace más de ${staleHours} h`;
}

/**
 * Formats alert line text for recent rejections/withdrawals (RF-ADMIN-25-2, RF-ADMIN-25-4).
 */
export function formatRejectionsAlertLabel(count: number | null, sinceDays: number = 7): string {
  const countStr = count === null ? '—' : count.toString();
  return `${countStr} rechazo(s) o retiro(s) de consentimiento (${sinceDays} días)`;
}
