// admin-client/src/lib/telemetryHealth.ts — Telemetry health alerts helpers (TASK-ADMIN-25)

import React from 'react';
import { ChevronDown, ChevronRight } from 'lucide-react';

import type {
  ConsentRejectionDecision,
  RecentRejectionWorker,
  TelemetryHealthResponse,
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

/**
 * Resolves a worker detail (W4) from an alert list item (RF-ADMIN-25-3).
 * Checks the local worker list first; if absent, invokes the remote fetcher.
 */
export async function resolveWorkerFromAlert<T extends { id: string }>(
  workerId: string,
  localWorkers: T[],
  fetchRemote?: (id: string) => Promise<T | null>
): Promise<T | null> {
  const existing = localWorkers.find(w => w.id === workerId);
  if (existing) {
    return existing;
  }
  if (!fetchRemote) {
    return null;
  }
  try {
    return await fetchRemote(workerId);
  } catch (e) {
    console.warn('Failed to resolve worker from alert', e);
    return null;
  }
}

export interface TelemetryHealthAlertsProps {
  telemetryHealth: TelemetryHealthResponse | null;
  expandedWithoutData?: boolean;
  onToggleWithoutData?: () => void;
  expandedRejections?: boolean;
  onToggleRejections?: () => void;
  onOpenWorker?: (workerId: string) => void;
  timezone?: string;
  now?: number;
}

/**
 * Telemetry health alerts component implemented with React.createElement (RF-ADMIN-25-2, RF-ADMIN-25-3, RF-ADMIN-25-4).
 */
export function TelemetryHealthAlerts(props: TelemetryHealthAlertsProps): React.ReactElement {
  const {
    telemetryHealth,
    expandedWithoutData,
    onToggleWithoutData,
    expandedRejections,
    onToggleRejections,
    onOpenWorker,
    timezone = 'America/Santiago',
    now = Date.now(),
  } = props;

  const [internalExpandedWithoutData, setInternalExpandedWithoutData] = React.useState(false);
  const [internalExpandedRejections, setInternalExpandedRejections] = React.useState(false);

  const isWithoutDataExpanded =
    expandedWithoutData !== undefined ? expandedWithoutData : internalExpandedWithoutData;
  const isRejectionsExpanded =
    expandedRejections !== undefined ? expandedRejections : internalExpandedRejections;

  const handleToggleWithoutData = () => {
    if (onToggleWithoutData) {
      onToggleWithoutData();
    } else {
      setInternalExpandedWithoutData(v => !v);
    }
  };

  const handleToggleRejections = () => {
    if (onToggleRejections) {
      onToggleRejections();
    } else {
      setInternalExpandedRejections(v => !v);
    }
  };

  // RF-ADMIN-25-4: Fault tolerance - if telemetryHealth is null/error, render "—" with descriptive title
  if (!telemetryHealth) {
    return React.createElement(
      React.Fragment,
      null,
      React.createElement(
        'p',
        {
          className: 'text-fg cursor-help',
          title: 'No se pudo cargar la información de salud de telemetría',
        },
        formatWithoutDataAlertLabel(null, 24)
      ),
      React.createElement(
        'p',
        {
          className: 'text-fg cursor-help',
          title: 'No se pudo cargar la información de salud de telemetría',
        },
        formatRejectionsAlertLabel(null, 7)
      )
    );
  }

  // Active without data section
  const withoutDataChildren: React.ReactNode[] = [];
  if (telemetryHealth.active_without_data.total > 0) {
    withoutDataChildren.push(
      React.createElement(
        'button',
        {
          key: 'btn-without-data',
          type: 'button',
          'aria-expanded': isWithoutDataExpanded,
          onClick: handleToggleWithoutData,
          className:
            'text-left text-warning font-semibold hover:underline flex items-center gap-1 cursor-pointer w-full',
        },
        React.createElement(
          'span',
          null,
          formatWithoutDataAlertLabel(
            telemetryHealth.active_without_data.total,
            telemetryHealth.stale_hours
          )
        ),
        React.createElement(isWithoutDataExpanded ? ChevronDown : ChevronRight, {
          className: 'w-3 h-3 shrink-0',
        })
      )
    );

    if (isWithoutDataExpanded) {
      const listItems: React.ReactNode[] = telemetryHealth.active_without_data.workers.map(w =>
        React.createElement(
          'li',
          { key: w.id },
          React.createElement(
            'button',
            {
              type: 'button',
              onClick: () => onOpenWorker?.(w.id),
              className: 'text-left hover:underline text-fg font-medium cursor-pointer',
            },
            w.name,
            React.createElement(
              'span',
              { className: 'text-muted font-normal ml-1' },
              `(${formatLastTelemetry(w.last_telemetry_at, now)})`
            )
          )
        )
      );

      const remaining = formatRemainingCount(
        telemetryHealth.active_without_data.total,
        telemetryHealth.active_without_data.workers.length
      );
      if (remaining) {
        listItems.push(
          React.createElement(
            'li',
            { key: 'remaining', className: 'text-muted italic' },
            remaining
          )
        );
      }

      withoutDataChildren.push(
        React.createElement(
          'ul',
          {
            key: 'list-without-data',
            className:
              'mt-1 space-y-1 pl-2 border-l border-default text-[11px] max-h-40 overflow-y-auto',
          },
          ...listItems
        )
      );
    }
  } else {
    withoutDataChildren.push(
      React.createElement(
        'p',
        { key: 'p-without-data', className: 'text-fg' },
        formatWithoutDataAlertLabel(0, telemetryHealth.stale_hours)
      )
    );
  }

  // Recent rejections section
  const rejectionsChildren: React.ReactNode[] = [];
  if (telemetryHealth.recent_rejections.total > 0) {
    rejectionsChildren.push(
      React.createElement(
        'button',
        {
          key: 'btn-rejections',
          type: 'button',
          'aria-expanded': isRejectionsExpanded,
          onClick: handleToggleRejections,
          className:
            'text-left text-warning font-semibold hover:underline flex items-center gap-1 cursor-pointer w-full',
        },
        React.createElement(
          'span',
          null,
          formatRejectionsAlertLabel(
            telemetryHealth.recent_rejections.total,
            telemetryHealth.since_days
          )
        ),
        React.createElement(isRejectionsExpanded ? ChevronDown : ChevronRight, {
          className: 'w-3 h-3 shrink-0',
        })
      )
    );

    if (isRejectionsExpanded) {
      const listItems: React.ReactNode[] = telemetryHealth.recent_rejections.workers.map(w =>
        React.createElement(
          'li',
          { key: w.id },
          React.createElement(
            'button',
            {
              type: 'button',
              onClick: () => onOpenWorker?.(w.id),
              className: 'text-left hover:underline text-fg font-medium cursor-pointer',
            },
            w.name,
            React.createElement(
              'span',
              { className: 'text-muted font-normal ml-1' },
              `(${formatRejectionDetail(w, timezone)})`
            )
          )
        )
      );

      const remaining = formatRemainingCount(
        telemetryHealth.recent_rejections.total,
        telemetryHealth.recent_rejections.workers.length
      );
      if (remaining) {
        listItems.push(
          React.createElement(
            'li',
            { key: 'remaining', className: 'text-muted italic' },
            remaining
          )
        );
      }

      rejectionsChildren.push(
        React.createElement(
          'ul',
          {
            key: 'list-rejections',
            className:
              'mt-1 space-y-1 pl-2 border-l border-default text-[11px] max-h-40 overflow-y-auto',
          },
          ...listItems
        )
      );
    }
  } else {
    rejectionsChildren.push(
      React.createElement(
        'p',
        { key: 'p-rejections', className: 'text-fg' },
        formatRejectionsAlertLabel(0, telemetryHealth.since_days)
      )
    );
  }

  return React.createElement(
    React.Fragment,
    null,
    React.createElement('div', { key: 'block-without-data' }, ...withoutDataChildren),
    React.createElement('div', { key: 'block-rejections' }, ...rejectionsChildren)
  );
}

/**
 * Render helper for TelemetryHealthAlerts using React.createElement.
 */
export function renderTelemetryHealthAlerts(props: TelemetryHealthAlertsProps): React.ReactElement {
  return React.createElement(TelemetryHealthAlerts, props);
}
