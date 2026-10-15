import React, { useState, useEffect } from 'react';
import {
  X,
  FileEdit,
  AlertCircle,
  ShieldCheck,
} from 'lucide-react';
import {
  AttendanceMark,
  CorrectionKind,
  MarkType,
  ProposeCorrectionInput,
  attendanceProposeCorrection,
} from '../api/tauri-bridge';
import {
  MARK_TYPE_LABELS,
  formatR38DateTime,
  formatR38Time,
  getDateStringInTimezone,
  validateCorrectionForm,
  describeCorrectionError,
  effectiveTs,
} from '../lib/attendance';
import { describeError } from '../lib/errors';

interface CorrectionModalProps {
  isOpen: boolean;
  onClose: () => void;
  onSuccess: () => void;
  initialWorker?: { id: string; name: string; timezone?: string } | null;
  initialMark?: AttendanceMark | null;
  allWorkers?: Array<{ id: string; name: string; run?: string | null; timezone?: string }>;
  workerTimezone?: string;
}

export const CorrectionModal: React.FC<CorrectionModalProps> = ({
  isOpen,
  onClose,
  onSuccess,
  initialWorker,
  initialMark,
  allWorkers = [],
  workerTimezone,
}) => {
  const [kind, setKind] = useState<CorrectionKind>('MODIFY');
  const [workerId, setWorkerId] = useState<string>(initialWorker?.id || '');
  const [markType, setMarkType] = useState<MarkType>('WORKDAY_START');
  const [dateStr, setDateStr] = useState<string>('');
  const [timeStr, setTimeStr] = useState<string>('09:00:00');
  const [reason, setReason] = useState<string>('');
  const [confirmNoHarm, setConfirmNoHarm] = useState<boolean>(false);
  const [submitting, setSubmitting] = useState<boolean>(false);
  const [error, setError] = useState<string | null>(null);

  const currentWorker = allWorkers.find(w => w.id === workerId);
  const effectiveTimezone =
    currentWorker?.timezone || initialWorker?.timezone || workerTimezone || 'America/Santiago';

  // Initialize form when opening
  useEffect(() => {
    if (!isOpen) return;

    setError(null);
    setConfirmNoHarm(false);

    if (initialMark) {
      setKind('MODIFY');
      setWorkerId(initialMark.worker_id);
      setMarkType(initialMark.type);
      try {
        setDateStr(getDateStringInTimezone(effectiveTs(initialMark), effectiveTimezone));
        setTimeStr(formatR38Time(effectiveTs(initialMark), effectiveTimezone));
      } catch {
        setDateStr('');
        setTimeStr('09:00:00');
      }
    } else {
      if (initialWorker?.id) setWorkerId(initialWorker.id);
      // Default to yesterday in worker's timezone
      const yesterday = new Date(Date.now() - 24 * 3600 * 1000);
      setDateStr(getDateStringInTimezone(yesterday, effectiveTimezone));
      setTimeStr('09:00:00');
    }
  }, [isOpen, initialMark, initialWorker, effectiveTimezone]);

  if (!isOpen) return null;

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);

    if (!workerId) {
      setError('Debes seleccionar un trabajador.');
      return;
    }

    if (!dateStr || !timeStr) {
      setError('Debes indicar fecha y hora.');
      return;
    }

    // Construct ISO RFC 3339 UTC timestamp
    const proposedIso = `${dateStr}T${timeStr}Z`;

    // Local mirror validation (RF-ADMIN-15-5)
    const validation = validateCorrectionForm({
      kind,
      originalMarkId: kind === 'MODIFY' ? initialMark?.id ?? null : null,
      originalMarkTs: kind === 'MODIFY' ? (initialMark ? effectiveTs(initialMark) : null) : null,
      markType,
      proposedTs: proposedIso,
      reason,
      confirmNoHarm,
      timezone: effectiveTimezone,
    });

    if (!validation.valid) {
      const firstError = Object.values(validation.errors)[0];
      setError(firstError || 'Corrige los errores del formulario.');
      return;
    }

    const payload: ProposeCorrectionInput = {
      worker_ids: [workerId],
      kind,
      original_mark_id: kind === 'MODIFY' ? initialMark?.id ?? null : null,
      type: markType,
      proposed_ts: proposedIso,
      reason: reason.trim(),
      confirm_no_harm: confirmNoHarm,
      original_mark_ts: kind === 'MODIFY' ? (initialMark ? effectiveTs(initialMark) : null) : null,
      timezone: effectiveTimezone,
    };

    setSubmitting(true);
    try {
      await attendanceProposeCorrection(payload);
      onSuccess();
      onClose();
    } catch (err: any) {
      const reasonCode = err?.reason || err?.details?.reason;
      if (reasonCode) {
        setError(describeCorrectionError(reasonCode, describeError(err)));
      } else {
        setError(describeError(err, 'No se pudo enviar la propuesta de corrección.'));
      }
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 bg-black/70 flex items-center justify-center p-4">
      <div className="bg-surface border border-default rounded-card max-w-lg w-full p-6 shadow-xl space-y-5 max-h-[90vh] overflow-y-auto">
        {/* Header */}
        <div className="flex items-start justify-between border-b border-default pb-3">
          <div>
            <div className="flex items-center gap-2">
              <FileEdit className="w-5 h-5 text-accent" />
              <h3 className="text-base font-bold text-fg">Proponer Corrección de Asistencia</h3>
            </div>
            <p className="text-xs text-muted mt-1">
              Procedimiento legal según R38 arts. 39 a 41 de la Dirección del Trabajo.
            </p>
          </div>
          <button
            onClick={onClose}
            className="p-1 text-muted hover:text-fg rounded-control transition"
          >
            <X className="w-5 h-5" />
          </button>
        </div>

        {error && (
          <div className="p-3 bg-danger-subtle border border-danger/30 rounded-control text-xs text-danger flex items-start gap-2">
            <AlertCircle className="w-4 h-4 shrink-0 mt-0.5" />
            <div>{error}</div>
          </div>
        )}

        <form onSubmit={handleSubmit} className="space-y-4">
          {/* Correction Kind Selection */}
          <div className="space-y-1.5">
            <label className="text-xs font-semibold text-fg">Tipo de acción:</label>
            <div className="grid grid-cols-2 gap-2">
              <button
                type="button"
                onClick={() => setKind('MODIFY')}
                className={`py-2 px-3 text-xs font-medium rounded-control border text-center transition ${
                  kind === 'MODIFY'
                    ? 'bg-accent text-accent-fg border-accent'
                    : 'bg-surface-subtle text-fg border-default hover:bg-surface'
                }`}
              >
                Modificar marca existente
              </button>
              <button
                type="button"
                onClick={() => setKind('ADD')}
                className={`py-2 px-3 text-xs font-medium rounded-control border text-center transition ${
                  kind === 'ADD'
                    ? 'bg-accent text-accent-fg border-accent'
                    : 'bg-surface-subtle text-fg border-default hover:bg-surface'
                }`}
              >
                Agregar marca olvidada
              </button>
            </div>
          </div>

          {/* Worker Selector */}
          <div className="space-y-1">
            <label className="text-xs font-semibold text-fg">Trabajador:</label>
            {initialWorker && allWorkers.length === 0 ? (
              <div className="p-2 bg-surface-subtle border border-default rounded-control text-xs font-medium text-fg">
                {initialWorker.name}
              </div>
            ) : (
              <select
                value={workerId}
                onChange={e => setWorkerId(e.target.value)}
                disabled={!!initialMark}
                className="w-full bg-surface-subtle border border-default text-fg text-xs rounded-control px-3 py-2 focus:outline-hidden focus:border-accent"
              >
                <option value="">-- Selecciona un trabajador --</option>
                {allWorkers.map(w => (
                  <option key={w.id} value={w.id}>
                    {w.name} {w.run ? `(${w.run})` : ''}
                  </option>
                ))}
              </select>
            )}
          </div>

          {/* Original Mark Banner if MODIFY */}
          {kind === 'MODIFY' && (
            <div className="p-3 bg-surface-subtle border border-default rounded-control text-xs space-y-1">
              <span className="text-muted font-medium">Marca original seleccionada:</span>
              {initialMark ? (
                <div className="flex items-center justify-between font-mono">
                  <span className="font-semibold text-fg">
                    {MARK_TYPE_LABELS[initialMark.type]}
                  </span>
                  <span className="text-muted">
                    {formatR38DateTime(effectiveTs(initialMark))}
                  </span>
                </div>
              ) : (
                <p className="text-warning italic">
                  No hay una marca original preseleccionada. Elige una desde el detalle de la marca.
                </p>
              )}
            </div>
          )}

          {/* Mark Type */}
          <div className="space-y-1">
            <label className="text-xs font-semibold text-fg">Tipo de marcación propuesta:</label>
            <select
              value={markType}
              onChange={e => setMarkType(e.target.value as MarkType)}
              className="w-full bg-surface-subtle border border-default text-fg text-xs rounded-control px-3 py-2 focus:outline-hidden focus:border-accent"
            >
              <option value="WORKDAY_START">Inicio de jornada (Entrada)</option>
              <option value="WORKDAY_END">Término de jornada (Salida)</option>
              <option value="BREAK_START">Inicio de colación</option>
              <option value="BREAK_END">Término de colación</option>
            </select>
          </div>

          {/* Proposed Date and Time */}
          <div className="grid grid-cols-2 gap-3">
            <div className="space-y-1">
              <label className="text-xs font-semibold text-fg">Fecha corregida:</label>
              <input
                type="date"
                value={dateStr}
                onChange={e => setDateStr(e.target.value)}
                className="w-full bg-surface-subtle border border-default text-fg text-xs rounded-control px-3 py-2 focus:outline-hidden focus:border-accent"
              />
              <span className="text-[10px] text-muted block">Debe ser anterior a hoy</span>
            </div>

            <div className="space-y-1">
              <label className="text-xs font-semibold text-fg">Hora propuesta (hh:mm:ss):</label>
              <input
                type="text"
                pattern="[0-2][0-9]:[0-5][0-9]:[0-5][0-9]"
                placeholder="09:00:00"
                value={timeStr}
                onChange={e => setTimeStr(e.target.value)}
                className="w-full bg-surface-subtle border border-default text-fg text-xs rounded-control px-3 py-2 font-mono focus:outline-hidden focus:border-accent"
              />
              <span className="text-[10px] text-muted block">Precisión de segundos (UTC)</span>
            </div>
          </div>

          {/* Reason (5 - 500 chars) */}
          <div className="space-y-1">
            <div className="flex justify-between items-center">
              <label className="text-xs font-semibold text-fg">
                Motivo legal de la corrección:
              </label>
              <span
                className={`text-[11px] font-mono ${
                  reason.trim().length >= 5 && reason.trim().length <= 500
                    ? 'text-muted'
                    : 'text-danger'
                }`}
              >
                {reason.trim().length} / 500
              </span>
            </div>
            <textarea
              rows={3}
              value={reason}
              onChange={e => setReason(e.target.value)}
              placeholder="Ej: Se elimina atraso autorizado previamente por jefatura directa de área."
              className="w-full bg-surface-subtle border border-default text-fg text-xs rounded-control p-2.5 focus:outline-hidden focus:border-accent"
            />
            <span className="text-[10px] text-muted block">
              Obligatorio (mínimo 5 caracteres). Será visible para el trabajador y la Dirección del Trabajo.
            </span>
          </div>

          {/* Mandatory Checkbox */}
          <div className="p-3 bg-surface-subtle border border-default rounded-control space-y-2">
            <label className="flex items-start gap-2.5 cursor-pointer text-xs">
              <input
                type="checkbox"
                checked={confirmNoHarm}
                onChange={e => setConfirmNoHarm(e.target.checked)}
                className="mt-0.5 rounded-sm border-default text-accent focus:ring-accent"
              />
              <span className="text-fg font-medium">
                Confirmo que esta corrección no perjudica al trabajador
              </span>
            </label>
            <p className="text-[11px] text-muted pl-6">
              R38 art. 41: La entrada solo puede adelantarse y la salida solo puede postergarse.
            </p>
          </div>

          {/* Fixed Statutory Notice */}
          <div className="p-3 bg-info-subtle/50 border border-info/30 rounded-control text-xs text-info space-y-1">
            <p className="font-semibold flex items-center gap-1.5">
              <ShieldCheck className="w-3.5 h-3.5" /> Plazo de 48 horas (R38 art. 40)
            </p>
            <p className="text-[11px] leading-relaxed">
              El trabajador recibirá un correo con esta propuesta y tendrá 48 horas contadas desde el envío del correo para
              oponerse. Si no se opone, la corrección se consolidará; si se opone, la marca original
              se mantiene inalterada.
            </p>
          </div>

          {/* Form Actions */}
          <div className="flex justify-end gap-3 pt-3 border-t border-default">
            <button
              type="button"
              onClick={onClose}
              disabled={submitting}
              className="px-4 py-2 bg-surface-subtle hover:bg-surface-subtle/80 text-fg text-xs font-medium rounded-control border border-default transition"
            >
              Cancelar
            </button>
            <button
              type="submit"
              disabled={submitting || !confirmNoHarm || reason.trim().length < 5}
              className="px-4 py-2 bg-accent hover:bg-accent-hover text-accent-fg text-xs font-semibold rounded-control transition disabled:opacity-40"
            >
              {submitting ? 'Enviando propuesta…' : 'Enviar propuesta al trabajador'}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
};
