import React, { useState, useEffect } from 'react';
import {
  Calendar,
  Clock,
  RefreshCw,
  AlertCircle,
  FileEdit,
  ShieldAlert,
  ChevronDown,
  User,
  WifiOff,
  History,
  CalendarX,
  BookOpen,
} from 'lucide-react';
import {
  WorkerItem,
  DaySummary,
  AttendanceMark,
  Absence,
  attendanceDays,
  getWorkersList,
} from '../api/tauri-bridge';
import {
  formatR38DayDate,
  formatR38Time,
  MARK_TYPE_SHORT,
  describeAnomaly,
  effectiveTs,
  effectiveLocalDate,
  sortMarksByEffective,
  describeDayStatus,
  canQualifyAbsence,
  abbreviationMeaning,
  ABSENCE_KIND_LABELS,
  R38_ABBREVIATIONS,
} from '../lib/attendance';
import { describeError } from '../lib/errors';
import { MarkDetailDrawer } from './MarkDetailDrawer';
import { CorrectionModal } from './CorrectionModal';
import { CorrectionsList } from './CorrectionsList';
import { AbsenceModal, AbsenceModalMode, CurrentAbsence } from './AbsenceModal';

interface AttendanceDaysProps {
  initialWorkerId?: string;
  onWorkerChanged?: (workerId: string) => void;
}

type RangePreset = 'today' | 'yesterday' | '7days' | '30days' | 'custom';

export const AttendanceDays: React.FC<AttendanceDaysProps> = ({
  initialWorkerId,
  onWorkerChanged,
}) => {
  const [workers, setWorkers] = useState<WorkerItem[]>([]);
  const [selectedWorkerId, setSelectedWorkerId] = useState<string>(initialWorkerId || '');
  const [activeTab, setActiveTab] = useState<'days' | 'corrections'>('days');

  // Date range state (RF-ADMIN-15-4: <= 62 days)
  const [rangePreset, setRangePreset] = useState<RangePreset>('7days');
  const [customFrom, setCustomFrom] = useState<string>('');
  const [customTo, setCustomTo] = useState<string>('');
  const [fromDate, setFromDate] = useState<string>('');
  const [toDate, setToDate] = useState<string>('');

  const [days, setDays] = useState<DaySummary[]>([]);
  const [loading, setLoading] = useState<boolean>(false);
  const [error, setError] = useState<string | null>(null);

  // Modals / Drawers
  const [selectedMark, setSelectedMark] = useState<AttendanceMark | null>(null);
  const [showCorrectionModal, setShowCorrectionModal] = useState<boolean>(false);
  const [correctionTargetMark, setCorrectionTargetMark] = useState<AttendanceMark | null>(null);

  // Absence qualification (RF-ADMIN-17-3). Absences saved in this session are kept by
  // `worker:date` so their id/abbreviation are known even if the day summary does not carry them.
  const [absenceTarget, setAbsenceTarget] = useState<
    { day: DaySummary; mode: AbsenceModalMode } | null
  >(null);
  const [savedAbsences, setSavedAbsences] = useState<Record<string, Absence | null>>({});

  // Calculate dates from preset
  const computeDatesFromPreset = (preset: RangePreset) => {
    const today = new Date();
    const toStr = today.toISOString().slice(0, 10);

    if (preset === 'today') {
      return { from: toStr, to: toStr };
    }
    if (preset === 'yesterday') {
      const y = new Date(Date.now() - 24 * 3600 * 1000);
      const yStr = y.toISOString().slice(0, 10);
      return { from: yStr, to: yStr };
    }
    if (preset === '7days') {
      const past = new Date(Date.now() - 7 * 24 * 3600 * 1000);
      return { from: past.toISOString().slice(0, 10), to: toStr };
    }
    if (preset === '30days') {
      const past = new Date(Date.now() - 30 * 24 * 3600 * 1000);
      return { from: past.toISOString().slice(0, 10), to: toStr };
    }
    return { from: customFrom || toStr, to: customTo || toStr };
  };

  // Load workers list
  useEffect(() => {
    getWorkersList()
      .then(list => {
        setWorkers(list);
        if (!selectedWorkerId && list.length > 0) {
          const first = list[0].id;
          setSelectedWorkerId(first);
          onWorkerChanged?.(first);
        }
      })
      .catch(() => undefined);
  }, []);

  // Update dates when preset changes
  useEffect(() => {
    if (rangePreset !== 'custom') {
      const { from, to } = computeDatesFromPreset(rangePreset);
      setFromDate(from);
      setToDate(to);
    } else if (customFrom && customTo) {
      setFromDate(customFrom);
      setToDate(customTo);
    }
  }, [rangePreset, customFrom, customTo]);

  // Load days summary
  const loadDays = async () => {
    if (!selectedWorkerId || !fromDate || !toDate) return;
    setLoading(true);
    setError(null);
    try {
      const data = await attendanceDays(selectedWorkerId, fromDate, toDate);
      setDays(data);
    } catch (err) {
      setError(describeError(err, 'No se pudo cargar el registro de días de asistencia.'));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    loadDays();
  }, [selectedWorkerId, fromDate, toDate]);

  const selectedWorker = workers.find(w => w.id === selectedWorkerId);
  const dayTimezone = (day: DaySummary) =>
    day.timezone || selectedWorker?.timezone || 'America/Santiago';

  /** Current qualification of a day: the server summary, completed with this session's saves. */
  const currentAbsence = (day: DaySummary): CurrentAbsence | null => {
    if (!day.absence) return null;
    const saved = savedAbsences[`${selectedWorkerId}:${day.date}`];
    const fromSession = saved && saved.kind === day.absence ? saved : null;
    return {
      id: day.absence_id ?? fromSession?.id ?? null,
      kind: day.absence,
      abbreviation: day.absence_abbreviation ?? fromSession?.abbreviation ?? null,
      note: day.absence_note ?? fromSession?.note ?? null,
    };
  };

  return (
    <div className="space-y-5">
      {/* Top Header & Filters */}
      <div className="bg-surface border border-default rounded-card p-4 space-y-4">
        <div className="flex flex-wrap items-center justify-between gap-4">
          <div>
            <h2 className="text-base font-bold text-fg flex items-center gap-2">
              <Calendar className="w-5 h-5 text-accent" />
              Marcas y Correcciones de Asistencia (AS2)
            </h2>
            <p className="text-xs text-muted mt-0.5">
              Registro diario de jornada, balance al segundo (ATT-09) y cadena criptográfica R38.
            </p>
          </div>

          <div className="flex items-center gap-2">
            <button
              onClick={() => {
                setCorrectionTargetMark(null);
                setShowCorrectionModal(true);
              }}
              className="px-3 py-1.5 bg-surface hover:bg-surface-subtle text-fg border border-default text-xs font-semibold rounded-control flex items-center gap-1.5 transition"
            >
              <FileEdit className="w-3.5 h-3.5 text-accent" />
              Proponer corrección
            </button>

            <button
              onClick={loadDays}
              disabled={loading}
              className="p-1.5 bg-surface hover:bg-surface-subtle text-muted hover:text-fg border border-default rounded-control transition"
              title="Refrescar días"
            >
              <RefreshCw className={`w-4 h-4 ${loading ? 'animate-spin' : ''}`} />
            </button>
          </div>
        </div>

        {/* Worker & Date Filter Controls */}
        <div className="flex flex-wrap items-center gap-4 pt-3 border-t border-default text-xs">
          {/* Worker Selector */}
          <div className="flex items-center gap-2">
            <User className="w-3.5 h-3.5 text-muted" />
            <span className="font-semibold text-muted">Trabajador:</span>
            <div className="relative">
              <select
                value={selectedWorkerId}
                onChange={e => {
                  setSelectedWorkerId(e.target.value);
                  onWorkerChanged?.(e.target.value);
                }}
                className="bg-surface-subtle border border-default text-fg text-xs font-semibold rounded-control pl-3 pr-8 py-1.5 appearance-none focus:outline-hidden focus:border-accent cursor-pointer"
              >
                {workers.map(w => (
                  <option key={w.id} value={w.id}>
                    {w.name} {w.run ? `(${w.run})` : ''}
                  </option>
                ))}
              </select>
              <ChevronDown className="w-3.5 h-3.5 text-muted absolute right-2.5 top-1/2 -translate-y-1/2 pointer-events-none" />
            </div>
          </div>

          {/* Date Range Selector */}
          <div className="flex items-center gap-2">
            <Clock className="w-3.5 h-3.5 text-muted" />
            <span className="font-semibold text-muted">Período:</span>
            <div className="relative">
              <select
                value={rangePreset}
                onChange={e => setRangePreset(e.target.value as RangePreset)}
                className="bg-surface-subtle border border-default text-fg text-xs font-medium rounded-control pl-3 pr-8 py-1.5 appearance-none focus:outline-hidden focus:border-accent cursor-pointer"
              >
                <option value="today">Hoy</option>
                <option value="yesterday">Ayer</option>
                <option value="7days">Últimos 7 días</option>
                <option value="30days">Últimos 30 días</option>
                <option value="custom">Personalizado (≤ 62 días)</option>
              </select>
              <ChevronDown className="w-3.5 h-3.5 text-muted absolute right-2.5 top-1/2 -translate-y-1/2 pointer-events-none" />
            </div>

            {rangePreset === 'custom' && (
              <div className="flex items-center gap-1.5 text-xs">
                <input
                  type="date"
                  value={customFrom}
                  onChange={e => setCustomFrom(e.target.value)}
                  className="bg-surface-subtle border border-default text-fg rounded-sm px-2 py-1 text-xs focus:outline-hidden focus:border-accent"
                />
                <span className="text-muted">-</span>
                <input
                  type="date"
                  value={customTo}
                  onChange={e => setCustomTo(e.target.value)}
                  className="bg-surface-subtle border border-default text-fg rounded-sm px-2 py-1 text-xs focus:outline-hidden focus:border-accent"
                />
              </div>
            )}
          </div>
        </div>
      </div>

      {/* Tabs: Días y marcas / Registro de correcciones */}
      <div className="flex border-b border-default">
        <button
          onClick={() => setActiveTab('days')}
          className={`px-4 py-2 text-xs font-semibold border-b-2 transition -mb-px flex items-center gap-2 ${
            activeTab === 'days'
              ? 'border-accent text-accent'
              : 'border-transparent text-muted hover:text-fg'
          }`}
        >
          <Calendar className="w-4 h-4" />
          Días y marcas de asistencia
        </button>
        <button
          onClick={() => setActiveTab('corrections')}
          className={`px-4 py-2 text-xs font-semibold border-b-2 transition -mb-px flex items-center gap-2 ${
            activeTab === 'corrections'
              ? 'border-accent text-accent'
              : 'border-transparent text-muted hover:text-fg'
          }`}
        >
          <History className="w-4 h-4" />
          Registro de correcciones (R38 art. 40)
        </button>
      </div>

      {error && (
        <div className="p-3 bg-danger-subtle border border-danger/30 rounded-control text-xs text-danger flex items-center gap-2">
          <AlertCircle className="w-4 h-4 shrink-0" />
          <span>{error}</span>
        </div>
      )}

      {/* TAB 1: DÍAS Y MARCAS */}
      {activeTab === 'days' && (
        <>
          {loading ? (
            <div className="py-16 text-center text-muted text-xs flex items-center justify-center gap-2">
              <RefreshCw className="w-4 h-4 animate-spin text-accent" />
              Cargando historial de días de asistencia…
            </div>
          ) : days.length === 0 ? (
            <div className="py-16 text-center border border-dashed border-default rounded-card p-8 bg-surface">
              <Clock className="w-8 h-8 text-muted mx-auto mb-2 opacity-50" />
              <p className="text-sm font-medium text-fg">Sin registros de asistencia</p>
              <p className="text-xs text-muted mt-1 max-w-sm mx-auto">
                No hay marcaciones ni registros para el período seleccionado ({fromDate} al {toDate}).
              </p>
            </div>
          ) : (
            <div className="border border-default rounded-card overflow-hidden bg-surface">
              <table className="w-full text-left border-collapse text-xs">
                <thead>
                  <tr className="bg-surface-subtle border-b border-default text-muted font-semibold">
                    <th className="py-2.5 px-3 whitespace-nowrap">Fecha</th>
                    <th className="py-2.5 px-3">Jornada pactada</th>
                    <th className="py-2.5 px-3 min-w-[280px]">Marcaciones del día</th>
                    <th className="py-2.5 px-3 text-right">Faltante</th>
                    <th className="py-2.5 px-3 text-right">Extra</th>
                    <th className="py-2.5 px-3">Observaciones / Anomalías</th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-default">
                  {days.map(day => (
                    <tr key={day.date} className="hover:bg-surface-subtle/50 transition">
                      {/* Date */}
                      <td className="py-3 px-3 font-mono font-medium text-fg whitespace-nowrap">
                        {formatR38DayDate(day.date)}
                        <span className="block text-[10px] text-muted font-sans">
                          {describeDayStatus(day)}
                        </span>
                      </td>

                      {/* Agreed Schedule */}
                      <td className="py-3 px-3 whitespace-nowrap">
                        {day.agreed ? (
                          <div className="font-mono text-fg">
                            <span>{day.agreed.start.slice(0, 5)} - {day.agreed.end.slice(0, 5)}</span>
                            {day.agreed.break && (
                              <span className="block text-[10px] text-muted">
                                Colación: {day.agreed.break}
                              </span>
                            )}
                          </div>
                        ) : (
                          <span className="text-muted text-[11px]">Sin jornada pactada</span>
                        )}
                      </td>

                      {/* Marks Chips */}
                      <td className="py-3 px-3">
                        {day.marks.length === 0 ? (
                          <span className="text-muted text-[11px] italic">Sin marcas</span>
                        ) : (
                          <div className="flex flex-wrap gap-1.5 items-center">
                            {sortMarksByEffective(day.marks).map(mark => {
                              const shortLabel = MARK_TYPE_SHORT[mark.type] || mark.type;
                              // v3.1: the time that counts, in the worker's zone (RF-ADMIN-17-1).
                              const tz = dayTimezone(day);
                              const timeStr = formatR38Time(effectiveTs(mark), tz);
                              const markDate = effectiveLocalDate(mark, tz);
                              const hasMismatch = mark.hash_verified === false;

                              return (
                                <button
                                  key={mark.id}
                                  onClick={() => setSelectedMark(mark)}
                                  className={`px-2 py-1 rounded-control text-[11px] font-mono border flex items-center gap-1.5 hover:border-strong transition ${
                                    hasMismatch
                                      ? 'bg-danger-subtle border-danger text-danger'
                                      : 'bg-surface-subtle border-default text-fg'
                                  }`}
                                  title="Ver detalle de la marca y verificar hash"
                                >
                                  <span className="font-bold">{shortLabel}</span>
                                  <span>{timeStr}</span>
                                  {markDate && markDate !== day.date && (
                                    <span
                                      className="text-[10px] text-muted"
                                      title="La hora efectiva de esta marca cae en otra fecha"
                                    >
                                      ({formatR38DayDate(markDate)})
                                    </span>
                                  )}

                                  {mark.offline && (
                                    <span
                                      className="text-warning text-[10px]"
                                      title="Marca guardada sin conexión (offline)"
                                    >
                                      <WifiOff className="w-3 h-3" />
                                    </span>
                                  )}

                                  {(mark.source === 'CORRECTION' || mark.corrected_by) && (
                                    <span
                                      className="text-accent text-[10px] font-sans font-semibold"
                                      title="Marca generada o alterada por corrección"
                                    >
                                      [Corregida]
                                    </span>
                                  )}

                                  {hasMismatch && (
                                    <span
                                      className="text-danger text-[10px] font-sans font-bold flex items-center gap-0.5"
                                      title="Integridad del hash alterada (no coincide)"
                                    >
                                      <ShieldAlert className="w-3 h-3 text-danger" />
                                      Integridad: no coincide
                                    </span>
                                  )}
                                </button>
                              );
                            })}
                          </div>
                        )}
                      </td>

                      {/* Missing Time (ATT-09: verbatim from server) */}
                      <td className="py-3 px-3 text-right font-mono text-xs whitespace-nowrap">
                        <span
                          className={
                            day.missing && day.missing !== '-00:00:00'
                              ? 'text-danger font-semibold'
                              : 'text-muted'
                          }
                        >
                          {day.missing || '—'}
                        </span>
                      </td>

                      {/* Extra Time (ATT-09: verbatim from server) */}
                      <td className="py-3 px-3 text-right font-mono text-xs whitespace-nowrap">
                        <span
                          className={
                            day.extra && day.extra !== '+00:00:00'
                              ? 'text-success font-semibold'
                              : 'text-muted'
                          }
                        >
                          {day.extra || '—'}
                        </span>
                      </td>

                      {/* Anomalies / Observations in plain words */}
                      <td className="py-3 px-3 space-y-1.5">
                        {(() => {
                          const absence = currentAbsence(day);
                          if (!absence) return null;
                          const meaning = abbreviationMeaning(absence.abbreviation);
                          return (
                            <div className="text-[11px] space-y-0.5">
                              <span
                                className={`inline-block px-2 py-0.5 rounded-badge border font-medium ${
                                  absence.kind === 'JUSTIFIED'
                                    ? 'bg-info-subtle text-info border-info/30'
                                    : 'bg-danger-subtle text-danger border-danger/30'
                                }`}
                              >
                                {ABSENCE_KIND_LABELS[absence.kind]}
                                {absence.abbreviation ? ` · ${absence.abbreviation}` : ''}
                              </span>
                              {meaning && <span className="block text-muted">{meaning}</span>}
                              {absence.note && <span className="block text-muted">{absence.note}</span>}
                            </div>
                          );
                        })()}
                        {day.anomalies.length > 0 ? (
                          <div className="space-y-0.5">
                            {day.anomalies.map((anom, idx) => (
                              <span
                                key={idx}
                                className="inline-block text-[11px] text-warning bg-warning-subtle/50 px-2 py-0.5 rounded-badge border border-warning/30 font-medium"
                              >
                                {describeAnomaly(anom)}
                              </span>
                            ))}
                          </div>
                        ) : day.altered ? (
                          <span className="text-[11px] text-accent font-medium">
                            Jornada modificada
                          </span>
                        ) : !day.absence ? (
                          <span className="text-muted text-[11px]">
                            {day.marks.length === 0 && day.agreed ? 'Sin marcas' : '—'}
                          </span>
                        ) : null}

                        {canQualifyAbsence(day, new Date(), dayTimezone(day)) && (
                          <div className="flex flex-wrap gap-1.5 pt-0.5">
                            {day.absence ? (
                              <>
                                <button
                                  onClick={() => setAbsenceTarget({ day, mode: 'qualify' })}
                                  className="px-2 py-0.5 text-[11px] font-semibold text-accent border border-default rounded-control hover:bg-surface-subtle transition"
                                >
                                  Editar
                                </button>
                                <button
                                  onClick={() => setAbsenceTarget({ day, mode: 'remove' })}
                                  disabled={!currentAbsence(day)?.id}
                                  title={
                                    currentAbsence(day)?.id
                                      ? undefined
                                      : 'El servidor no informó el identificador de esta calificación; usa Editar para cambiarla.'
                                  }
                                  className="px-2 py-0.5 text-[11px] font-semibold text-danger border border-default rounded-control hover:bg-surface-subtle transition disabled:opacity-50"
                                >
                                  Quitar calificación
                                </button>
                              </>
                            ) : (
                              <button
                                onClick={() => setAbsenceTarget({ day, mode: 'qualify' })}
                                className="px-2 py-0.5 text-[11px] font-semibold text-accent border border-default rounded-control hover:bg-surface-subtle transition flex items-center gap-1"
                              >
                                <CalendarX className="w-3 h-3" />
                                Calificar ausencia
                              </button>
                            )}
                          </div>
                        )}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}

          {/* Abbreviations legend (R38 art. 28 g) */}
          <div className="bg-surface border border-default rounded-card p-4 space-y-2">
            <h4 className="text-xs font-bold text-fg flex items-center gap-1.5">
              <BookOpen className="w-3.5 h-3.5 text-muted" />
              Abreviaturas (Res. Ex. N°38, art. 28)
            </h4>
            <dl className="grid grid-cols-2 md:grid-cols-3 gap-x-4 gap-y-1 text-[11px]">
              {R38_ABBREVIATIONS.map(a => (
                <div key={a.code} className="flex gap-1.5">
                  <dt className="font-mono font-semibold text-fg">{a.code}</dt>
                  <dd className="text-muted">{a.meaning}</dd>
                </div>
              ))}
            </dl>
            <p className="text-[11px] text-muted">
              Un día laboral sin marcas se muestra como "Sin marcas" hasta que tú califiques la
              ausencia. El sistema nunca la califica por su cuenta.
            </p>
          </div>
        </>
      )}

      {/* TAB 2: CORRECCIONES */}
      {activeTab === 'corrections' && (
        <CorrectionsList
          workerId={selectedWorkerId}
          onRefreshParent={loadDays}
        />
      )}

      {/* Mark Detail Drawer */}
      <MarkDetailDrawer
        mark={selectedMark}
        timezone={selectedWorker?.timezone || undefined}
        onClose={() => setSelectedMark(null)}
        onProposeCorrection={mark => {
          setSelectedMark(null);
          setCorrectionTargetMark(mark);
          setShowCorrectionModal(true);
        }}
      />

      {/* Absence qualification (RF-ADMIN-17-3) */}
      {absenceTarget && selectedWorker && (
        <AbsenceModal
          isOpen
          mode={absenceTarget.mode}
          worker={{ id: selectedWorker.id, name: selectedWorker.name, timezone: dayTimezone(absenceTarget.day) }}
          date={absenceTarget.day.date}
          current={currentAbsence(absenceTarget.day)}
          onClose={() => setAbsenceTarget(null)}
          onSaved={absence => {
            const key = `${selectedWorker.id}:${absenceTarget.day.date}`;
            setSavedAbsences(prev => ({ ...prev, [key]: absence }));
            loadDays();
          }}
        />
      )}

      {/* Correction Modal */}
      <CorrectionModal
        isOpen={showCorrectionModal}
        onClose={() => {
          setShowCorrectionModal(false);
          setCorrectionTargetMark(null);
        }}
        onSuccess={() => {
          loadDays();
        }}
        initialWorker={selectedWorker ? { id: selectedWorker.id, name: selectedWorker.name, timezone: selectedWorker.timezone } : null}
        initialMark={correctionTargetMark}
        allWorkers={workers.map(w => ({ id: w.id, name: w.name, run: w.run, timezone: w.timezone }))}
        workerTimezone={selectedWorker?.timezone}
      />
    </div>
  );
};
