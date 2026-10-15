import React, { useEffect, useState } from 'react';
import {
  X,
  Laptop,
  Pause,
  Play,
  Ban,
  Trash2,
  QrCode,
  Eye,
  Download,
  Clock,
  Calendar,
  FileCheck,
  Shield,
  Building2,
} from 'lucide-react';
import {
  ConsentDescriptions,
  ConsentFlags,
  ConsentRecord,
  ScheduleInput,
  WorkerItem,
  WorkerResponse,
  toWorkerItem,
  workersGet,
  workersGetConsentRecords,
  workersSetStatus,
  workersSetTelemetryStatus,
  workersUpdateConsentConfig,
  workersUpdateSchedule,
  attendanceDays,
} from '../api/tauri-bridge';
import type { DaySummary } from '../api/types';
import {
  formatR38DayDate,
  formatR38Time,
  describeAnomaly,
  describeDayStatus,
  effectiveTs,
  sortMarksByEffective,
  MARK_TYPE_SHORT,
} from '../lib/attendance';
import { STATUS_BADGES } from './EmployeeCard';
import { ConsentEditor } from './ConsentEditor';
import { ScheduleEditor } from './ScheduleEditor';
import {
  CONSENT_CATEGORIES,
  consentRecordsToCsv,
  consentSummary,
  decisionLabel,
  descriptionErrors,
  normalizeDescriptions,
} from './consentCategories';
import { describeError } from '../lib/errors';
import {
  JOURNEY_TYPE_LABELS,
  WEEKDAY_NAMES,
  calculateDayHours,
  calculateWeeklyTotalHours,
  formatMinutes,
} from '../lib/schedule';

interface WorkerDetailModalProps {
  worker: WorkerItem;
  organization: string;
  onClose: () => void;
  onChanged: (worker: WorkerItem) => void;
  onRegenerate: (worker: WorkerItem) => void;
  onDelete: (worker: WorkerItem) => void;
  onViewTelemetry: (worker: WorkerItem) => void;
  onNavigateToAttendance?: (workerId?: string) => void;
}

type TabType = 'attendance' | 'schedule' | 'telemetry' | 'evidence';

function fmtDate(iso: string | null | undefined): string {
  return iso ? new Date(iso).toLocaleString() : '—';
}

/** W4 — Worker detail with Attendance, Schedule, Telemetry, and Evidence tabs (RF-ADMIN-14-5). */
export const WorkerDetailModal: React.FC<WorkerDetailModalProps> = ({
  worker: initial,
  organization: _organization,
  onClose,
  onChanged,
  onRegenerate,
  onDelete,
  onViewTelemetry,
  onNavigateToAttendance,
}) => {
  const [activeTab, setActiveTab] = useState<TabType>('schedule');
  const [worker, setWorker] = useState<WorkerResponse>(initial.raw);
  const [records, setRecords] = useState<ConsentRecord[] | null>(null);
  const [recordsError, setRecordsError] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  // Attendance 7-day summary (RF-ADMIN-15-7)
  const [attendanceDaysList, setAttendanceDaysList] = useState<DaySummary[] | null>(null);
  const [attendanceLoading, setAttendanceLoading] = useState(false);
  const [attendanceError, setAttendanceError] = useState<string | null>(null);

  // Lifecycle
  const [reason, setReason] = useState('');
  const [revokeTyped, setRevokeTyped] = useState('');
  const [showRevoke, setShowRevoke] = useState(false);

  // Telemetry edit
  const [editingConsent, setEditingConsent] = useState(false);
  const [draftFlags, setDraftFlags] = useState<ConsentFlags>(initial.raw.requested_consent_flags);
  const [draftDescriptions, setDraftDescriptions] = useState<ConsentDescriptions>(
    initial.raw.consent_descriptions,
  );

  // Schedule edit
  const [editingSchedule, setEditingSchedule] = useState(false);
  const [scheduleDraft, setScheduleDraft] = useState<ScheduleInput>(() => {
    if (initial.schedule) {
      return {
        journey_type: initial.schedule.journey_type,
        exceptional_resolution: initial.schedule.exceptional_resolution ?? null,
        days: initial.schedule.days,
        break_counts_as_work: initial.schedule.break_counts_as_work,
        disconnection: initial.schedule.disconnection ?? null,
        location_at_mark: initial.schedule.location_at_mark,
        effective_from:
          initial.schedule.effective_from || new Date().toISOString().slice(0, 10),
        requested_by: initial.schedule.requested_by || 'EMPLOYER',
        note: initial.schedule.note ?? null,
      };
    }
    return {
      journey_type: 'FIXED',
      exceptional_resolution: null,
      days: [],
      break_counts_as_work: false,
      disconnection: null,
      location_at_mark: false,
      effective_from: new Date().toISOString().slice(0, 10),
      requested_by: 'EMPLOYER',
      note: null,
    };
  });
  const [scheduleServerWarnings, setScheduleServerWarnings] = useState<string[]>([]);

  const item = toWorkerItem(worker);
  const badge = STATUS_BADGES[worker.status];
  const summary = consentSummary(worker.consent_flags, worker.requested_consent_flags);

  useEffect(() => {
    workersGet(initial.id)
      .then(w => {
        setWorker(w);
        if (w.schedule) {
          setScheduleDraft({
            journey_type: w.schedule.journey_type,
            exceptional_resolution: w.schedule.exceptional_resolution ?? null,
            days: w.schedule.days,
            break_counts_as_work: w.schedule.break_counts_as_work,
            disconnection: w.schedule.disconnection ?? null,
            location_at_mark: w.schedule.location_at_mark,
            effective_from: w.schedule.effective_from || new Date().toISOString().slice(0, 10),
            requested_by: w.schedule.requested_by || 'EMPLOYER',
            note: w.schedule.note ?? null,
          });
        }
      })
      .catch(e => setError(describeError(e)));
    workersGetConsentRecords(initial.id)
      .then(setRecords)
      .catch(e => setRecordsError(describeError(e)));
  }, [initial.id]);

  useEffect(() => {
    if (activeTab === 'attendance' && !initial.art22Exempt) {
      setAttendanceLoading(true);
      setAttendanceError(null);
      const to = new Date().toISOString().slice(0, 10);
      const from = new Date(Date.now() - 6 * 86400000).toISOString().slice(0, 10);
      attendanceDays(initial.id, from, to)
        .then(setAttendanceDaysList)
        .catch(e => setAttendanceError(describeError(e)))
        .finally(() => setAttendanceLoading(false));
    }
  }, [activeTab, initial.id, initial.art22Exempt]);

  const apply = (updated: WorkerResponse, message?: string) => {
    setWorker(updated);
    onChanged(toWorkerItem(updated));
    setNotice(message ?? null);
  };

  const run = async (action: () => Promise<void>) => {
    setBusy(true);
    setError(null);
    setNotice(null);
    try {
      await action();
    } catch (e: any) {
      setError(describeError(e));
    } finally {
      setBusy(false);
    }
  };

  // Revoke worker (RF-ADMIN-14-5: workersSetStatus only accepts REVOKED)
  const revokeWorker = () =>
    run(async () => {
      const updated = await workersSetStatus(worker.id, 'REVOKED', reason.trim() || undefined);
      setReason('');
      setShowRevoke(false);
      setRevokeTyped('');
      apply(updated, 'Trabajador revocado.');
    });

  // Telemetry status pause/resume
  const setTelemetryStatus = (status: 'PAUSED' | 'ACTIVE') =>
    run(async () => {
      const updated = await workersSetTelemetryStatus(
        worker.id,
        status,
        reason.trim() || undefined,
      );
      setReason('');
      apply(
        updated,
        status === 'PAUSED' ? 'Telemetría pausada.' : 'Telemetría reanudada.',
      );
    });

  const saveConsent = () =>
    run(async () => {
      const flags =
        worker.role_type === 'OFFICE' ? { ...draftFlags, location: false } : draftFlags;
      if (Object.keys(descriptionErrors(flags, draftDescriptions)).length > 0) {
        throw new Error('Revise las descripciones de las categorías activas.');
      }
      const res = await workersUpdateConsentConfig(
        worker.id,
        flags,
        normalizeDescriptions(flags, draftDescriptions),
      );
      setEditingConsent(false);
      apply(
        res.worker,
        res.requires_worker_acceptance
          ? `Versión v${res.consent_version} publicada. Requiere nueva decisión del trabajador.`
          : `Versión v${res.consent_version} publicada.`,
      );
    });

  const saveSchedule = () =>
    run(async () => {
      const res = await workersUpdateSchedule(worker.id, scheduleDraft);
      setScheduleServerWarnings(res.warnings || []);
      setEditingSchedule(false);
      const updatedWorker = await workersGet(worker.id);
      apply(
        updatedWorker,
        res.warnings && res.warnings.length > 0
          ? 'Jornada actualizada con advertencias legales.'
          : 'Jornada pactada actualizada exitosamente.',
      );
    });

  const exportCsv = () => {
    if (!records) return;
    const blob = new Blob([consentRecordsToCsv(records)], { type: 'text/csv;charset=utf-8' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `consentimiento-${worker.personal_email || worker.email}-${new Date()
      .toISOString()
      .slice(0, 10)}.csv`;
    a.click();
    URL.revokeObjectURL(url);
  };

  const btn =
    'px-3 py-1.5 rounded-control text-xs font-medium flex items-center gap-1.5 transition disabled:opacity-40';

  const workModeLabel =
    worker.work_mode === 'ON_SITE'
      ? 'Presencial'
      : worker.work_mode === 'REMOTE'
        ? 'Teletrabajo'
        : 'Mixta';

  return (
    <div className="fixed inset-0 z-50 bg-black/80 flex items-center justify-center p-4">
      <div className="bg-surface-subtle border border-default rounded-card max-w-4xl w-full p-6 shadow-md max-h-[92vh] overflow-y-auto space-y-5">
        {/* Header with legal data */}
        <div className="flex justify-between items-start border-b border-default pb-3">
          <div>
            <div className="flex flex-wrap items-center gap-2">
              <h3 className="text-lg font-semibold text-fg">{worker.name}</h3>
              <span className={`px-2.5 py-0.5 text-xs font-semibold rounded-full border ${badge.className}`}>
                {badge.label}
              </span>
              <span className="text-xs font-mono font-medium text-fg bg-surface px-2 py-0.5 rounded-control border border-default">
                RUN: {worker.run || '—'}
              </span>
              <span className="text-xs text-muted">{worker.job_title || 'Sin cargo'}</span>
            </div>

            <div className="flex flex-wrap items-center gap-x-4 gap-y-1 text-xs text-muted mt-1.5">
              <span>{worker.personal_email || worker.email}</span>
              <span className="flex items-center gap-1">
                <Building2 className="w-3 h-3" />
                {worker.workplace?.name || 'Sin lugar asignado'}
              </span>
              <span className="flex items-center gap-1 font-medium text-fg">
                Modalidad: {workModeLabel}
              </span>
              {worker.art22_exempt && (
                <span className="text-accent font-semibold">Excluido art. 22</span>
              )}
            </div>

            <p className="text-[11px] text-muted mt-1 flex items-center gap-1">
              <Laptop className="w-3 h-3" />
              {worker.device_info
                ? `${worker.device_info.hostname} · ${worker.device_info.os} · agent ${worker.device_info.app_version}`
                : 'Sin equipo vinculado'}
              {' · '}
              Vinculado: {fmtDate(worker.enrollment_completed_at)}
            </p>
          </div>
          <button onClick={onClose} className="text-muted hover:text-fg">
            <X className="w-5 h-5" />
          </button>
        </div>

        {error && (
          <div className="p-2.5 bg-danger-subtle border border-danger rounded-control text-xs text-danger">
            {error}
          </div>
        )}
        {notice && (
          <div className="p-2.5 bg-success-subtle border border-success rounded-control text-xs text-success">
            {notice}
          </div>
        )}

        {/* Global lifecycle actions (W4) */}
        <div className="flex flex-wrap items-center gap-2">
          <button
            onClick={() => onRegenerate(item)}
            className={`${btn} bg-surface hover:bg-surface-subtle text-fg border border-default`}
          >
            <QrCode className="w-3.5 h-3.5" />
            {worker.device_identifier ? 'Mover a equipo nuevo' : 'Generar código'}
          </button>
          {worker.status !== 'REVOKED' && (
            <button
              disabled={busy}
              onClick={() => setShowRevoke(v => !v)}
              className={`${btn} bg-danger-subtle hover:bg-danger-subtle text-danger border border-default`}
            >
              <Ban className="w-3.5 h-3.5" /> Revocar
            </button>
          )}
          <button
            onClick={() => onDelete(item)}
            className={`${btn} bg-danger-subtle hover:bg-danger-subtle text-danger border border-default`}
          >
            <Trash2 className="w-3.5 h-3.5" /> Eliminar
          </button>
        </div>

        {showRevoke && (
          <div className="p-3 bg-danger-subtle border border-danger rounded-control space-y-2 text-xs text-fg">
            <p>
              Revocar desvincula el equipo: el agente borra sus credenciales y la llave fijada. Escriba{' '}
              <span className="font-semibold text-fg">{worker.name}</span> para confirmar.
            </p>
            <div className="flex gap-2">
              <input
                value={revokeTyped}
                placeholder="Nombre del trabajador"
                onChange={e => setRevokeTyped(e.target.value)}
                className="flex-1 bg-surface border border-default text-fg text-xs rounded-control px-2.5 py-1.5 focus:outline-hidden focus:border-danger"
              />
              <button
                disabled={busy || revokeTyped.trim() !== worker.name.trim()}
                onClick={revokeWorker}
                className={`${btn} bg-danger hover:bg-danger-hover text-accent-fg font-semibold`}
              >
                Confirmar revocación
              </button>
            </div>
          </div>
        )}

        {/* Navigation Tabs (RF-ADMIN-14-5) */}
        <div className="flex items-center gap-1 border-b border-default">
          <button
            onClick={() => setActiveTab('attendance')}
            className={`px-4 py-2 text-xs font-semibold border-b-2 transition -mb-px flex items-center gap-1.5 ${
              activeTab === 'attendance'
                ? 'border-accent text-accent'
                : 'border-transparent text-muted hover:text-fg'
            }`}
          >
            <Clock className="w-3.5 h-3.5" />
            Asistencia
          </button>
          <button
            onClick={() => setActiveTab('schedule')}
            className={`px-4 py-2 text-xs font-semibold border-b-2 transition -mb-px flex items-center gap-1.5 ${
              activeTab === 'schedule'
                ? 'border-accent text-accent'
                : 'border-transparent text-muted hover:text-fg'
            }`}
          >
            <Calendar className="w-3.5 h-3.5" />
            Jornada
          </button>
          <button
            onClick={() => setActiveTab('telemetry')}
            className={`px-4 py-2 text-xs font-semibold border-b-2 transition -mb-px flex items-center gap-1.5 ${
              activeTab === 'telemetry'
                ? 'border-accent text-accent'
                : 'border-transparent text-muted hover:text-fg'
            }`}
          >
            <Shield className="w-3.5 h-3.5" />
            Telemetría
          </button>
          <button
            onClick={() => setActiveTab('evidence')}
            className={`px-4 py-2 text-xs font-semibold border-b-2 transition -mb-px flex items-center gap-1.5 ${
              activeTab === 'evidence'
                ? 'border-accent text-accent'
                : 'border-transparent text-muted hover:text-fg'
            }`}
          >
            <FileCheck className="w-3.5 h-3.5" />
            Evidencia
          </button>
        </div>

        {/* TAB 1: ASISTENCIA (RF-ADMIN-15-7) */}
        {activeTab === 'attendance' && (
          <div className="space-y-4">
            {worker.art22_exempt ? (
              <div className="p-5 bg-surface border border-default rounded-card space-y-2">
                <h4 className="text-sm font-semibold text-fg">
                  Excluido de limitación de jornada (art. 22 inc. 2°)
                </h4>
                <p className="text-xs text-muted leading-relaxed">
                  Este trabajador está exento de control de asistencia de acuerdo con la legislación laboral chilena (ATT-12).
                </p>
              </div>
            ) : (
              <div className="space-y-4">
                <div className="flex flex-wrap items-center justify-between gap-3">
                  <div>
                    <h4 className="text-xs font-semibold text-fg uppercase tracking-wide">
                      Últimos 7 Días de Asistencia
                    </h4>
                    <p className="text-xs text-muted">
                      Resumen legal según R38 (marcas, jornada pactada y balance de horas).
                    </p>
                  </div>
                  {onNavigateToAttendance && (
                    <button
                      onClick={() => {
                        onClose();
                        onNavigateToAttendance(initial.id);
                      }}
                      className="px-3.5 py-1.5 bg-accent hover:bg-accent-hover text-accent-fg text-xs font-semibold rounded-control flex items-center gap-1.5 transition"
                    >
                      <Clock className="w-3.5 h-3.5" />
                      <span>Ver marcas y correcciones (AS2)</span>
                    </button>
                  )}
                </div>

                {attendanceLoading && (
                  <div className="p-8 text-center text-xs text-muted">
                    Cargando registros de los últimos 7 días...
                  </div>
                )}

                {attendanceError && (
                  <div className="p-3 bg-danger-subtle text-danger border border-default rounded-card text-xs">
                    {attendanceError}
                  </div>
                )}

                {!attendanceLoading && !attendanceError && attendanceDaysList && (
                  attendanceDaysList.length === 0 ? (
                    <div className="p-6 bg-surface border border-default rounded-card text-center text-xs text-muted">
                      Sin registros de asistencia en los últimos 7 días.
                    </div>
                  ) : (
                    <div className="border border-default rounded-card overflow-hidden">
                      <table className="w-full text-left text-xs">
                        <thead>
                          <tr className="bg-surface-subtle border-b border-default text-muted font-medium text-[11px]">
                            <th className="py-2 px-3">Fecha</th>
                            <th className="py-2 px-3">Marcas</th>
                            <th className="py-2 px-3">Jornada Pactada</th>
                            <th className="py-2 px-3">Trabajado</th>
                            <th className="py-2 px-3">Faltante / Extra</th>
                            <th className="py-2 px-3">Observaciones</th>
                          </tr>
                        </thead>
                        <tbody className="divide-y divide-default bg-surface">
                          {attendanceDaysList.map(d => {
                            const hasOffline = d.marks.some(m => m.offline);
                            const hasCorrections = d.marks.some(m => m.source === 'CORRECTION' || m.corrected_by);
                            const hasMismatch = d.marks.some(m => m.hash_verified === false);

                            return (
                              <tr key={d.date} className="hover:bg-surface-subtle/40">
                                <td className="py-2 px-3 font-medium text-fg">
                                  {formatR38DayDate(d.date)}
                                </td>
                                <td className="py-2 px-3">
                                  <div className="flex items-center gap-1.5 flex-wrap">
                                    {d.marks.length === 0 ? (
                                      <span className="font-mono text-fg">0</span>
                                    ) : (
                                      sortMarksByEffective(d.marks).map(m => (
                                        <span
                                          key={m.id}
                                          className="px-1.5 py-0.5 bg-surface-subtle border border-default rounded-sm text-[10px] font-mono text-fg"
                                        >
                                          {MARK_TYPE_SHORT[m.type] || m.type}{' '}
                                          {formatR38Time(
                                            effectiveTs(m),
                                            d.timezone || initial.timezone || 'America/Santiago',
                                          )}
                                        </span>
                                      ))
                                    )}
                                    {hasOffline && (
                                      <span className="px-1.5 py-0.2 bg-warning-subtle text-warning border border-warning/30 rounded text-[10px]">
                                        Sin conexión
                                      </span>
                                    )}
                                    {hasCorrections && (
                                      <span className="px-1.5 py-0.2 bg-info-subtle text-info border border-info/30 rounded text-[10px]">
                                        Corregida
                                      </span>
                                    )}
                                    {hasMismatch && (
                                      <span className="px-1.5 py-0.2 bg-danger-subtle text-danger border border-danger/30 rounded text-[10px] font-semibold">
                                        Integridad: no coincide
                                      </span>
                                    )}
                                  </div>
                                </td>
                                <td className="py-2 px-3 font-mono text-muted">
                                  {d.agreed ? `${d.agreed.start.slice(0, 5)} - ${d.agreed.end.slice(0, 5)}` : 'Libre'}
                                </td>
                                <td className="py-2 px-3 font-mono font-medium text-fg">
                                  {describeDayStatus(d)}
                                  {d.absence_abbreviation && (
                                    <span className="text-muted"> · {d.absence_abbreviation}</span>
                                  )}
                                </td>
                                <td className="py-2 px-3 font-mono text-[11px]">
                                  {d.missing && d.missing !== '-00:00:00' && (
                                    <span className="text-danger mr-1.5">{d.missing}</span>
                                  )}
                                  {d.extra && d.extra !== '+00:00:00' && (
                                    <span className="text-success">{d.extra}</span>
                                  )}
                                  {(!d.missing || d.missing === '-00:00:00') && (!d.extra || d.extra === '+00:00:00') && (
                                    <span className="text-muted">0 min</span>
                                  )}
                                </td>
                                <td className="py-2 px-3 text-[11px] text-muted">
                                  {d.anomalies.length > 0
                                    ? d.anomalies.map(describeAnomaly).join(', ')
                                    : '—'}
                                </td>
                              </tr>
                            );
                          })}
                        </tbody>
                      </table>
                    </div>
                  )
                )}
              </div>
            )}
          </div>
        )}

        {/* TAB 2: JORNADA */}
        {activeTab === 'schedule' && (
          <div className="space-y-4">
            {worker.art22_exempt ? (
              <div className="p-5 bg-surface border border-default rounded-card space-y-2">
                <h4 className="text-sm font-semibold text-fg">
                  Excluido de limitación de jornada (art. 22 inc. 2°)
                </h4>
                <p className="text-xs text-muted leading-relaxed">
                  Este trabajador está contratado bajo el artículo 22 inciso 2° del Código del
                  Trabajo. No cuenta con una jornada laboral pactada ni está sujeto a registro de
                  asistencia y jornada (ATT-12).
                </p>
              </div>
            ) : editingSchedule ? (
              <div className="p-4 bg-surface border border-default rounded-card space-y-4">
                <div className="flex items-center justify-between border-b border-default pb-2">
                  <h4 className="text-xs font-semibold text-fg uppercase tracking-wide">
                    Cambiar Jornada Pactada
                  </h4>
                  <button
                    onClick={() => setEditingSchedule(false)}
                    className="text-xs text-muted hover:text-fg"
                  >
                    Cancelar
                  </button>
                </div>
                <ScheduleEditor
                  value={scheduleDraft}
                  onChange={setScheduleDraft}
                  workMode={worker.work_mode}
                  serverWarnings={scheduleServerWarnings}
                />
                <div className="flex justify-end gap-2 pt-2 border-t border-default">
                  <button
                    type="button"
                    onClick={() => setEditingSchedule(false)}
                    className="px-3 py-1.5 bg-surface-subtle text-fg rounded-control text-xs"
                  >
                    Cancelar
                  </button>
                  <button
                    type="button"
                    disabled={busy}
                    onClick={saveSchedule}
                    className="px-4 py-1.5 bg-accent hover:bg-accent-hover text-accent-fg rounded-control text-xs font-semibold"
                  >
                    {busy ? 'Guardando…' : 'Guardar Nueva Versión'}
                  </button>
                </div>
              </div>
            ) : worker.schedule ? (
              <div className="space-y-4">
                <div className="flex items-center justify-between">
                  <div className="space-y-0.5">
                    <h4 className="text-xs font-semibold text-fg uppercase tracking-wide">
                      Jornada Pactada Vigente
                    </h4>
                    <p className="text-xs text-muted">
                      Rige desde: <span className="font-medium text-fg">{worker.schedule.effective_from || '—'}</span>
                      {worker.schedule.requested_by && (
                        <span> · Solicitada por: {worker.schedule.requested_by === 'EMPLOYER' ? 'Empleador' : 'Trabajador'}</span>
                      )}
                    </p>
                  </div>
                  {worker.status !== 'REVOKED' && (
                    <button
                      onClick={() => setEditingSchedule(true)}
                      className={`${btn} bg-accent hover:bg-accent-hover text-accent-fg`}
                    >
                      Cambiar jornada
                    </button>
                  )}
                </div>

                <div className="grid grid-cols-1 sm:grid-cols-3 gap-3">
                  <div className="p-3 bg-surface border border-default rounded-card">
                    <p className="text-[11px] text-muted">Tipo de Jornada</p>
                    <p className="text-xs font-semibold text-fg mt-0.5">
                      {JOURNEY_TYPE_LABELS[worker.schedule.journey_type] || worker.schedule.journey_type}
                    </p>
                  </div>
                  <div className="p-3 bg-surface border border-default rounded-card">
                    <p className="text-[11px] text-muted">Total Semanal</p>
                    <p className="text-xs font-semibold text-fg mt-0.5">
                      {calculateWeeklyTotalHours(
                        worker.schedule.days,
                        worker.schedule.break_counts_as_work,
                      ).toFixed(1)}{' '}
                      horas
                    </p>
                  </div>
                  <div className="p-3 bg-surface border border-default rounded-card">
                    <p className="text-[11px] text-muted">Colación Imputable</p>
                    <p className="text-xs font-semibold text-fg mt-0.5">
                      {worker.schedule.break_counts_as_work ? 'Sí (cuenta como trabajo)' : 'No'}
                    </p>
                  </div>
                </div>

                {/* Days Table */}
                <div className="border border-default rounded-card overflow-hidden">
                  <table className="w-full text-left text-xs">
                    <thead>
                      <tr className="bg-surface-subtle border-b border-default text-muted font-medium text-[11px]">
                        <th className="py-2 px-3">Día</th>
                        <th className="py-2 px-3">Entrada</th>
                        <th className="py-2 px-3">Salida</th>
                        <th className="py-2 px-3">Colación</th>
                        <th className="py-2 px-3 text-right">Total</th>
                      </tr>
                    </thead>
                    <tbody className="divide-y divide-default bg-surface">
                      {[1, 2, 3, 4, 5, 6, 7].map(weekday => {
                        const day = worker.schedule?.days.find(d => d.weekday === weekday);
                        const hours = day
                          ? calculateDayHours(day, worker.schedule?.break_counts_as_work ?? false)
                          : 0;
                        return (
                          <tr key={weekday} className={day ? 'text-fg' : 'text-muted bg-surface-subtle/20'}>
                            <td className="py-2 px-3 font-medium">{WEEKDAY_NAMES[weekday]}</td>
                            <td className="py-2 px-3">{day ? day.start : '—'}</td>
                            <td className="py-2 px-3">{day ? day.end : '—'}</td>
                            <td className="py-2 px-3">
                              {day && day.break_start && day.break_end
                                ? `${day.break_start} - ${day.break_end}`
                                : '—'}
                            </td>
                            <td className="py-2 px-3 text-right font-mono">
                              {day ? formatMinutes(hours * 60) : '—'}
                            </td>
                          </tr>
                        );
                      })}
                    </tbody>
                  </table>
                </div>

                {worker.schedule.disconnection && (
                  <div className="p-3 bg-surface border border-default rounded-card text-xs space-y-1">
                    <p className="font-semibold text-fg">Franja de Desconexión Digital:</p>
                    <p className="text-muted">
                      {worker.schedule.disconnection.start} a {worker.schedule.disconnection.end} (≥
                      12 h continuas)
                    </p>
                  </div>
                )}
              </div>
            ) : (
              <div className="p-5 bg-surface border border-default rounded-card text-center space-y-2">
                <p className="text-xs text-muted">No hay jornada configurada para este trabajador.</p>
                {worker.status !== 'REVOKED' && (
                  <button
                    onClick={() => setEditingSchedule(true)}
                    className="px-3 py-1.5 bg-accent hover:bg-accent-hover text-accent-fg text-xs font-semibold rounded-control"
                  >
                    Configurar jornada
                  </button>
                )}
              </div>
            )}
          </div>
        )}

        {/* TAB 3: TELEMETRÍA */}
        {activeTab === 'telemetry' && (
          <div className="space-y-4">
            <div className="flex flex-wrap items-center justify-between gap-2">
              <div className="space-y-0.5">
                <h4 className="text-xs font-semibold text-fg uppercase tracking-wide">
                  Estado de Telemetría: <span className="font-bold text-accent">{worker.telemetry_status || 'NOT_REQUESTED'}</span>
                </h4>
                <p className="text-xs text-muted">
                  Vigente v{worker.consent_version} · Aceptada v{worker.accepted_consent_version ?? '–'}
                  {summary.requested > 0 && (
                    <span> · {summary.accepted} de {summary.requested} categorías aceptadas</span>
                  )}
                </p>
              </div>

              <div className="flex items-center gap-2">
                <button
                  onClick={() => onViewTelemetry(item)}
                  className={`${btn} bg-accent hover:bg-accent-hover text-accent-fg`}
                >
                  <Eye className="w-3.5 h-3.5" /> Ver telemetría descifrada
                </button>
                {worker.telemetry_status === 'ACCEPTED' && (
                  <button
                    disabled={busy}
                    onClick={() => setTelemetryStatus('PAUSED')}
                    className={`${btn} bg-warning-subtle hover:bg-warning-subtle text-warning border border-default`}
                  >
                    <Pause className="w-3.5 h-3.5" /> Pausar telemetría
                  </button>
                )}
                {worker.telemetry_status === 'PAUSED' && (
                  <button
                    disabled={busy}
                    onClick={() => setTelemetryStatus('ACTIVE')}
                    className={`${btn} bg-success-subtle hover:bg-success-subtle text-success border border-default`}
                  >
                    <Play className="w-3.5 h-3.5" /> Reanudar telemetría
                  </button>
                )}
              </div>
            </div>

            {editingConsent ? (
              <div className="p-4 bg-surface border border-default rounded-card space-y-3">
                <h4 className="text-xs font-semibold text-fg uppercase tracking-wide">
                  Editar Categorías Solicitadas
                </h4>
                <ConsentEditor
                  role={worker.role_type as 'OFFICE' | 'FIELD'}
                  workMode={worker.work_mode}
                  flags={draftFlags}
                  descriptions={draftDescriptions}
                  onChange={(f, d) => {
                    setDraftFlags(f);
                    setDraftDescriptions(d);
                  }}
                />
                <div className="flex justify-end gap-2 pt-2 border-t border-default">
                  <button
                    type="button"
                    onClick={() => setEditingConsent(false)}
                    className="px-3 py-1.5 bg-surface-subtle text-fg rounded-control text-xs"
                  >
                    Cancelar
                  </button>
                  <button
                    type="button"
                    disabled={busy}
                    onClick={saveConsent}
                    className="px-4 py-1.5 bg-accent hover:bg-accent-hover text-accent-fg rounded-control text-xs font-semibold"
                  >
                    {busy ? 'Publicando…' : 'Publicar cambios'}
                  </button>
                </div>
              </div>
            ) : (
              <div className="space-y-3">
                <div className="flex justify-between items-center">
                  <p className="text-xs font-medium text-muted">Categorías y descripciones vigentes:</p>
                  {worker.status !== 'REVOKED' && (
                    <button
                      onClick={() => setEditingConsent(true)}
                      className={`${btn} bg-surface-subtle hover:bg-strong text-fg`}
                    >
                      Editar datos a recolectar
                    </button>
                  )}
                </div>

                <div className="grid grid-cols-1 sm:grid-cols-2 gap-2 text-xs">
                  {CONSENT_CATEGORIES.map(c => {
                    const requested = worker.requested_consent_flags?.[c.key];
                    const accepted = worker.consent_flags?.[c.key];
                    return (
                      <div
                        key={c.key}
                        className={`p-2.5 rounded-card border ${
                          accepted
                            ? 'bg-success-subtle/40 border-success/30 text-fg'
                            : requested
                              ? 'bg-surface border-default text-fg'
                              : 'bg-surface-subtle/30 border-default text-muted'
                        }`}
                      >
                        <div className="flex justify-between items-center">
                          <span className="font-semibold">{c.label}</span>
                          <span
                            className={`text-[10px] px-1.5 py-0.5 rounded-sm font-medium ${
                              accepted
                                ? 'bg-success text-accent-fg'
                                : requested
                                  ? 'bg-info-subtle text-info'
                                  : 'bg-surface text-muted'
                            }`}
                          >
                            {accepted ? 'Aceptada' : requested ? 'Solicitada' : 'No solicitada'}
                          </span>
                        </div>
                        {requested && (
                          <p className="text-[11px] text-muted italic mt-1">
                            “{(worker.consent_descriptions?.[c.key] || '').trim()}”
                          </p>
                        )}
                      </div>
                    );
                  })}
                </div>
              </div>
            )}
          </div>
        )}

        {/* TAB 4: EVIDENCIA */}
        {activeTab === 'evidence' && (
          <div className="space-y-4">
            <div className="p-3 bg-surface border border-default rounded-card text-xs flex justify-between items-center">
              <div>
                <span className="font-semibold text-fg">Términos y Condiciones del Trabajador:</span>
                <p className="text-muted mt-0.5">
                  Versión aceptada:{' '}
                  <span className="font-medium text-fg">
                    {worker.terms_accepted_version || 'Pendiente de aceptación'}
                  </span>
                </p>
              </div>
            </div>

            <div className="flex justify-between items-center">
              <h4 className="text-xs font-semibold text-fg uppercase tracking-wide">
                Historial de Decisiones de Consentimiento
              </h4>
              {records && records.length > 0 && (
                <button
                  onClick={exportCsv}
                  className={`${btn} bg-surface hover:bg-surface-subtle text-fg border border-default`}
                >
                  <Download className="w-3.5 h-3.5" /> Exportar CSV
                </button>
              )}
            </div>

            {recordsError && (
              <div className="p-2.5 bg-danger-subtle border border-danger rounded-control text-xs text-danger">
                {recordsError}
              </div>
            )}

            {!records ? (
              <p className="text-xs text-muted">Cargando registros de evidencia…</p>
            ) : records.length === 0 ? (
              <p className="text-xs text-muted">
                No hay decisiones registradas aún para este trabajador.
              </p>
            ) : (
              <div className="border border-default rounded-card overflow-hidden">
                <table className="w-full text-left text-xs">
                  <thead>
                    <tr className="bg-surface-subtle border-b border-default text-muted font-medium text-[11px]">
                      <th className="py-2 px-3">Fecha</th>
                      <th className="py-2 px-3">Decisión</th>
                      <th className="py-2 px-3">Versión</th>
                      <th className="py-2 px-3">Categorías Aceptadas</th>
                      <th className="py-2 px-3">Agente</th>
                    </tr>
                  </thead>
                  <tbody className="divide-y divide-default bg-surface">
                    {records.map(r => (
                      <tr key={r.id} className="text-fg hover:bg-surface-subtle/50">
                        <td className="py-2 px-3 whitespace-nowrap">{fmtDate(r.created_at)}</td>
                        <td className="py-2 px-3 font-semibold">{decisionLabel(r.decision)}</td>
                        <td className="py-2 px-3">v{r.consent_version}</td>
                        <td className="py-2 px-3">
                          {r.decision === 'ACCEPTED' || r.decision === 'NARROWED'
                            ? r.accepted_consent_flags
                              ? Object.entries(r.accepted_consent_flags)
                                  .filter(([, v]) => v)
                                  .map(([k]) => k)
                                  .join(', ') || 'Ninguna'
                              : 'Todas las solicitadas'
                            : '—'}
                        </td>
                        <td className="py-2 px-3 text-muted text-[11px]">
                          {r.app_version || '—'}
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
};
