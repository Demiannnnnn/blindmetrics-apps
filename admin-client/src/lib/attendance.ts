// admin-client/src/lib/attendance.ts — R38 formatting, anomaly mapping and correction validation (TASK-ADMIN-15)

import type {
  AttendanceState,
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
}

export interface CorrectionValidationResult {
  valid: boolean;
  errors: Record<string, string>;
  reasonCode?: string;
}

/**
 * Validates a proposed correction locally before calling Tauri invoke (RF-ADMIN-15-5).
 * Mirrors server rules:
 * 1. `confirmNoHarm` is required.
 * 2. `reason` length 5-500.
 * 3. MODIFY requires `originalMarkId`.
 * 4. Not same day or future (`CORRECTION_TOO_EARLY`).
 * 5. Does not harm the worker (`CORRECTION_HARMS_WORKER`):
 *    - Start can only move earlier.
 *    - End can only move later.
 */
export function validateCorrectionForm(
  input: CorrectionValidationInput,
  now = new Date(),
): CorrectionValidationResult {
  const errors: Record<string, string> = {};
  let reasonCode: string | undefined;

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
      // Check date: cannot be today or future (only from next business day)
      const proposedDayString = proposedDate.toISOString().slice(0, 10);
      const todayDayString = now.toISOString().slice(0, 10);
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
