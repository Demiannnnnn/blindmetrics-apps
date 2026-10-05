// Subscription lock screen and trial banner logic (TASK-ADMIN-12 = TASK-APP-01).
// Pure module shared by the UI and node tests.

import type { SubscriptionStatus } from '../api/types';

export const RECHECK_INTERVAL_MS = 60_000;
export const TRIAL_BANNER_DAYS = 7;

export function isBlocked(status: Pick<SubscriptionStatus, 'effective_status'> | null | undefined): boolean {
  return status?.effective_status === 'SUSPENDED' || status?.effective_status === 'CANCELLED';
}

/** Title and body of the lock screen by effective status and reason (RF-ADMIN-12-2). */
export function lockScreenCopy(effectiveStatus: string, reason: string | null | undefined): { title: string; body: string } {
  if (effectiveStatus === 'CANCELLED' || reason === 'SUBSCRIPTION_CANCELLED') {
    return { title: 'Suscripción cancelada', body: 'Tu suscripción fue cancelada.' };
  }
  if (reason === 'TRIAL_EXPIRED') {
    return { title: 'Período de prueba terminado', body: 'Tu período de prueba de 14 días terminó.' };
  }
  return { title: 'Suscripción suspendida', body: 'Tu suscripción ha sido suspendida por falta de pago.' };
}

/** `https://app.tudominio.cl/dashboard/facturacion/` → `app.tudominio.cl/dashboard/facturacion`. */
export function displayUrl(url: string | null | undefined): string {
  if (!url) return '';
  return url.trim().replace(/^https?:\/\//i, '').replace(/\/+$/, '');
}

/** Same rules as the Rust side (SEC-17): https, host, no credentials. */
export function isSafePortalUrl(url: string | null | undefined): boolean {
  if (!url || url.length > 2048 || /\s/.test(url)) return false;
  const match = /^https:\/\/([^/?#]+)/i.exec(url.trim());
  return !!match && !match[1].includes('@') && !match[1].startsWith(':');
}

export function shouldShowTrialBanner(status: Pick<SubscriptionStatus, 'effective_status' | 'trial_days_left'> | null | undefined): boolean {
  return (
    status?.effective_status === 'TRIAL' &&
    typeof status.trial_days_left === 'number' &&
    status.trial_days_left <= TRIAL_BANNER_DAYS
  );
}

export function trialBannerText(daysLeft: number): string {
  if (daysLeft <= 0) return 'Tu período de prueba termina hoy.';
  return daysLeft === 1 ? 'Te queda 1 día de prueba.' : `Te quedan ${daysLeft} días de prueba.`;
}
