import React, { useEffect, useState } from 'react';
import { X, CalendarX, AlertCircle, AlertTriangle, Trash2 } from 'lucide-react';
import {
  Absence,
  AbsenceType,
  attendanceDeleteAbsence,
  attendanceSetAbsence,
} from '../api/tauri-bridge';
import {
  ABSENCE_KIND_LABELS,
  ABSENCE_NOTE_MAX_CHARS,
  abbreviationsForKind,
  formatR38DayDate,
  validateAbsenceForm,
} from '../lib/attendance';
import { describeError } from '../lib/errors';

export type AbsenceModalMode = 'qualify' | 'remove';

/** Current qualification of the day, when there is one. */
export interface CurrentAbsence {
  id?: string | null;
  kind: AbsenceType;
  abbreviation?: string | null;
  note?: string | null;
}

interface AbsenceModalProps {
  isOpen: boolean;
  mode: AbsenceModalMode;
  worker: { id: string; name: string; timezone?: string | null };
  /** Local date of the day, `YYYY-MM-DD`. */
  date: string;
  current?: CurrentAbsence | null;
  onClose: () => void;
  /** Called with the saved absence, or `null` after the qualification was removed. */
  onSaved: (absence: Absence | null) => void;
}

/**
 * Employer qualification of an absence (RF-ADMIN-17-3, R38 arts. 27.1 and 28 g, contract v3.1).
 * Nothing is chosen by default; "Injustificada" requires an explicit confirmation because it shows
 * up in the worker's legal reports.
 */
export const AbsenceModal: React.FC<AbsenceModalProps> = ({
  isOpen,
  mode,
  worker,
  date,
  current,
  onClose,
  onSaved,
}) => {
  const [kind, setKind] = useState<AbsenceType | null>(null);
  const [abbreviation, setAbbreviation] = useState<string>('');
  const [note, setNote] = useState<string>('');
  const [confirmUnjustified, setConfirmUnjustified] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!isOpen) return;
    setKind(current?.kind ?? null);
    setAbbreviation(current?.abbreviation ?? '');
    setNote(current?.note ?? '');
    setConfirmUnjustified(false);
    setError(null);
  }, [isOpen, current, date, worker.id]);

  if (!isOpen) return null;

  const chooseKind = (next: AbsenceType) => {
    setKind(next);
    setConfirmUnjustified(false);
    if (abbreviation && !abbreviationsForKind(next).some(a => a.code === abbreviation)) {
      setAbbreviation('');
    }
  };

  const handleSave = async (e: React.FormEvent) => {
    e.preventDefault();
    setError(null);
    const errors = validateAbsenceForm({ kind, abbreviation, note, confirmUnjustified });
    const first = Object.values(errors)[0];
    if (first || !kind) {
      setError(first ?? 'Elige si la ausencia es justificada o injustificada.');
      return;
    }
    setSubmitting(true);
    try {
      const saved = await attendanceSetAbsence({
        worker_id: worker.id,
        date,
        kind,
        abbreviation: abbreviation.trim() || null,
        note: note.trim() || null,
        timezone: worker.timezone ?? null,
      });
      onSaved(saved);
      onClose();
    } catch (err) {
      setError(describeError(err, 'No se pudo guardar la calificación de la ausencia.'));
    } finally {
      setSubmitting(false);
    }
  };

  const handleRemove = async () => {
    if (!current?.id) return;
    setError(null);
    setSubmitting(true);
    try {
      await attendanceDeleteAbsence(current.id);
      onSaved(null);
      onClose();
    } catch (err) {
      setError(describeError(err, 'No se pudo quitar la calificación de la ausencia.'));
    } finally {
      setSubmitting(false);
    }
  };

  const noteLength = [...note.trim()].length;
  const options = kind ? abbreviationsForKind(kind) : [];

  return (
    <div className="fixed inset-0 z-50 bg-black/70 flex items-center justify-center p-4">
      <div className="bg-surface border border-default rounded-card max-w-lg w-full p-6 shadow-xl space-y-5 max-h-[90vh] overflow-y-auto">
        <div className="flex items-start justify-between border-b border-default pb-3">
          <div>
            <div className="flex items-center gap-2">
              <CalendarX className="w-5 h-5 text-accent" />
              <h3 className="text-base font-bold text-fg">
                {mode === 'remove'
                  ? 'Quitar calificación de ausencia'
                  : current
                    ? 'Editar calificación de ausencia'
                    : 'Calificar ausencia'}
              </h3>
            </div>
            <p className="text-xs text-muted mt-1">
              {worker.name} · {formatR38DayDate(date)}
            </p>
          </div>
          <button
            onClick={onClose}
            className="p-1 text-muted hover:text-fg rounded-control transition"
            title="Cerrar"
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

        {mode === 'remove' ? (
          <div className="space-y-4">
            <div className="p-3 bg-surface-subtle border border-default rounded-control text-xs space-y-1">
              <p className="text-muted">Calificación actual:</p>
              <p className="font-semibold text-fg">
                {current ? ABSENCE_KIND_LABELS[current.kind] : '—'}
                {current?.abbreviation ? ` (${current.abbreviation})` : ''}
              </p>
              {current?.note && <p className="text-muted">{current.note}</p>}
            </div>
            <p className="text-xs text-muted">
              Al quitarla, el día vuelve a mostrarse como "Sin marcas" en la consola y en los reportes
              legales. El sistema no califica ausencias por su cuenta.
            </p>
            <div className="flex justify-end gap-2">
              <button
                type="button"
                onClick={onClose}
                className="px-4 py-2 bg-surface-subtle text-fg text-xs font-medium rounded-control border border-default transition"
              >
                Cancelar
              </button>
              <button
                type="button"
                onClick={handleRemove}
                disabled={submitting || !current?.id}
                className="px-4 py-2 bg-danger hover:bg-danger-hover text-accent-fg text-xs font-semibold rounded-control flex items-center gap-1.5 transition disabled:opacity-50"
              >
                <Trash2 className="w-3.5 h-3.5" />
                {submitting ? 'Quitando…' : 'Quitar calificación'}
              </button>
            </div>
          </div>
        ) : (
          <form onSubmit={handleSave} className="space-y-4">
            <div className="space-y-1.5">
              <label className="text-xs font-semibold text-fg">Calificación:</label>
              <div className="grid grid-cols-2 gap-2" role="radiogroup">
                {(['JUSTIFIED', 'UNJUSTIFIED'] as const).map(option => (
                  <button
                    key={option}
                    type="button"
                    role="radio"
                    aria-checked={kind === option}
                    onClick={() => chooseKind(option)}
                    className={`py-2 px-3 text-xs font-medium rounded-control border text-center transition ${
                      kind === option
                        ? 'bg-accent text-accent-fg border-accent'
                        : 'bg-surface-subtle text-fg border-default hover:bg-surface'
                    }`}
                  >
                    {option === 'JUSTIFIED' ? 'Justificada' : 'Injustificada'}
                  </button>
                ))}
              </div>
              {!kind && (
                <span className="text-[10px] text-muted block">
                  Nada se califica por defecto: elige una opción.
                </span>
              )}
            </div>

            <div className="space-y-1">
              <label className="text-xs font-semibold text-fg">Abreviatura (opcional):</label>
              <select
                value={abbreviation}
                onChange={e => setAbbreviation(e.target.value)}
                disabled={!kind}
                className="w-full bg-surface-subtle border border-default text-fg text-xs rounded-control px-3 py-2 focus:outline-hidden focus:border-accent disabled:opacity-60"
              >
                <option value="">Sin abreviatura</option>
                {options.map(a => (
                  <option key={a.code} value={a.code}>
                    {a.code} — {a.meaning}
                  </option>
                ))}
              </select>
              <span className="text-[10px] text-muted block">
                Abreviaturas oficiales de la Res. Ex. N°38, art. 28.
              </span>
            </div>

            <div className="space-y-1">
              <div className="flex justify-between items-center">
                <label className="text-xs font-semibold text-fg">Nota (opcional):</label>
                <span
                  className={`text-[11px] font-mono ${
                    noteLength > ABSENCE_NOTE_MAX_CHARS ? 'text-danger' : 'text-muted'
                  }`}
                >
                  {noteLength} / {ABSENCE_NOTE_MAX_CHARS}
                </span>
              </div>
              <textarea
                rows={3}
                value={note}
                onChange={e => setNote(e.target.value)}
                placeholder="Ej: Presentó licencia médica el lunes."
                className="w-full bg-surface-subtle border border-default text-fg text-xs rounded-control p-2.5 focus:outline-hidden focus:border-accent"
              />
            </div>

            {kind === 'UNJUSTIFIED' && (
              <div className="p-3 bg-warning-subtle/40 border border-warning/40 rounded-control space-y-2">
                <p className="text-xs text-fg flex items-start gap-2">
                  <AlertTriangle className="w-4 h-4 text-warning shrink-0 mt-0.5" />
                  Esta calificación aparece en los reportes legales del trabajador.
                </p>
                <label className="flex items-start gap-2.5 cursor-pointer text-xs">
                  <input
                    type="checkbox"
                    checked={confirmUnjustified}
                    onChange={e => setConfirmUnjustified(e.target.checked)}
                    className="mt-0.5 accent-accent"
                  />
                  <span className="text-fg font-medium">
                    Confirmo que quiero calificar esta ausencia como injustificada.
                  </span>
                </label>
              </div>
            )}

            <div className="flex justify-end gap-2 pt-2 border-t border-default">
              <button
                type="button"
                onClick={onClose}
                className="px-4 py-2 bg-surface-subtle text-fg text-xs font-medium rounded-control border border-default transition"
              >
                Cancelar
              </button>
              <button
                type="submit"
                disabled={submitting || !kind || (kind === 'UNJUSTIFIED' && !confirmUnjustified)}
                className="px-4 py-2 bg-accent hover:bg-accent-hover text-accent-fg text-xs font-semibold rounded-control transition disabled:opacity-50"
              >
                {submitting ? 'Guardando…' : 'Guardar calificación'}
              </button>
            </div>
          </form>
        )}
      </div>
    </div>
  );
};
