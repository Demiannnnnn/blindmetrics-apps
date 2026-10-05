import React, { useEffect, useState } from 'react';
import { X, Laptop, Pause, Play, Ban, Trash2, QrCode, Eye, Download, Pencil } from 'lucide-react';
import {
  ConsentDescriptions,
  ConsentFlags,
  ConsentRecord,
  WorkerItem,
  WorkerResponse,
  toWorkerItem,
  workersGet,
  workersGetConsentRecords,
  workersSetStatus,
  workersUpdateConsentConfig,
} from '../api/tauri-bridge';
import { STATUS_BADGES } from './EmployeeCard';
import { ConsentEditor, ConsentPreview } from './ConsentEditor';
import {
  CONSENT_CATEGORIES,
  acceptedCategoriesOf,
  categoryLabel,
  consentRecordsToCsv,
  consentSummary,
  decisionLabel,
  descriptionErrors,
  normalizeDescriptions,
} from './consentCategories';
import { describeError } from '../lib/errors';

interface WorkerDetailModalProps {
  worker: WorkerItem;
  organization: string;
  onClose: () => void;
  /** Called with the fresh worker after any server-side change. */
  onChanged: (worker: WorkerItem) => void;
  onRegenerate: (worker: WorkerItem) => void;
  onDelete: (worker: WorkerItem) => void;
  onViewTelemetry: (worker: WorkerItem) => void;
}

function fmtDate(iso: string | null | undefined): string {
  return iso ? new Date(iso).toLocaleString() : '—';
}

/** W4 — Worker detail and lifecycle actions (RF-ADMIN-08-7/8). */
export const WorkerDetailModal: React.FC<WorkerDetailModalProps> = ({
  worker: initial,
  organization,
  onClose,
  onChanged,
  onRegenerate,
  onDelete,
  onViewTelemetry,
}) => {
  const [worker, setWorker] = useState<WorkerResponse>(initial.raw);
  const [records, setRecords] = useState<ConsentRecord[] | null>(null);
  const [recordsError, setRecordsError] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [reason, setReason] = useState('');
  const [revokeTyped, setRevokeTyped] = useState('');
  const [showRevoke, setShowRevoke] = useState(false);
  const [editingConsent, setEditingConsent] = useState(false);
  const [draftFlags, setDraftFlags] = useState<ConsentFlags>(initial.raw.requested_consent_flags);
  const [draftDescriptions, setDraftDescriptions] = useState<ConsentDescriptions>(initial.raw.consent_descriptions);

  const item = toWorkerItem(worker);
  const badge = STATUS_BADGES[worker.status];
  const hasConsent = worker.accepted_consent_version !== null;
  const summary = consentSummary(worker.consent_flags, worker.requested_consent_flags);

  useEffect(() => {
    workersGet(initial.id)
      .then(setWorker)
      .catch(e => setError(describeError(e)));
    workersGetConsentRecords(initial.id)
      .then(setRecords)
      .catch(e => setRecordsError(describeError(e)));
  }, [initial.id]);

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
      // The API is the authority on transitions.
      setError(describeError(e));
    } finally {
      setBusy(false);
    }
  };

  const setStatus = (status: 'ACTIVE' | 'PAUSED' | 'REVOKED') =>
    run(async () => {
      const updated = await workersSetStatus(worker.id, status, reason.trim() || undefined);
      setReason('');
      setShowRevoke(false);
      setRevokeTyped('');
      apply(updated);
    });

  const saveConsent = () =>
    run(async () => {
      const flags = worker.role_type === 'OFFICE' ? { ...draftFlags, location: false } : draftFlags;
      if (!Object.values(flags).some(Boolean)) throw new Error('Active al menos una categoría de datos.');
      if (Object.keys(descriptionErrors(flags, draftDescriptions)).length > 0) {
        throw new Error('Revise las descripciones de las categorías activas.');
      }
      const res = await workersUpdateConsentConfig(worker.id, flags, normalizeDescriptions(flags, draftDescriptions));
      setEditingConsent(false);
      apply(
        res.worker,
        res.requires_worker_acceptance
          ? `Versión v${res.consent_version} publicada. El trabajador debe aceptarla antes de reanudar la recolección.`
          : `Versión v${res.consent_version} publicada. No requiere nueva aceptación.`,
      );
    });

  const exportCsv = () => {
    if (!records) return;
    const blob = new Blob([consentRecordsToCsv(records)], { type: 'text/csv;charset=utf-8' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `consentimiento-${worker.email}-${new Date().toISOString().slice(0, 10)}.csv`;
    a.click();
    URL.revokeObjectURL(url);
  };

  const btn = 'px-3 py-1.5 rounded-control text-xs font-medium flex items-center gap-1.5 transition disabled:opacity-40';

  return (
    <div className="fixed inset-0 z-50 bg-black/80 flex items-center justify-center p-4">
      <div className="bg-surface-subtle border border-default rounded-card max-w-4xl w-full p-6 shadow-md max-h-[92vh] overflow-y-auto space-y-5">
        {/* Header */}
        <div className="flex justify-between items-start border-b border-default pb-3">
          <div>
            <div className="flex items-center gap-2">
              <h3 className="text-lg font-semibold text-fg">{worker.name}</h3>
              <span className={`px-2.5 py-0.5 text-xs font-semibold rounded-full border ${badge.className}`}>{badge.label}</span>
              <span className="text-xs text-muted">{worker.role_type}</span>
            </div>
            <p className="text-xs text-muted font-mono">{worker.email}</p>
            <p className="text-[11px] text-muted mt-1 flex items-center gap-1">
              <Laptop className="w-3 h-3" />
              {worker.device_info
                ? `${worker.device_info.hostname} · ${worker.device_info.os} · agent ${worker.device_info.app_version}`
                : 'Sin equipo vinculado'}
            </p>
            <p className="text-[11px] text-muted">
              Vinculado: {fmtDate(worker.enrollment_completed_at)} · Último cambio de estado: {fmtDate(worker.status_changed_at)}
            </p>
          </div>
          <button onClick={onClose} className="text-muted hover:text-fg">
            <X className="w-5 h-5" />
          </button>
        </div>

        {error && <div className="p-2.5 bg-danger-subtle border border-danger rounded-control text-xs text-danger">{error}</div>}
        {notice && <div className="p-2.5 bg-success-subtle border border-success rounded-control text-xs text-success">{notice}</div>}

        {/* Lifecycle actions (enabled per status; the API decides) */}
        <section className="space-y-2">
          <h4 className="text-xs font-semibold text-fg uppercase tracking-wide">Acciones</h4>
          <div className="flex flex-wrap items-center gap-2">
            <button onClick={() => onViewTelemetry(item)} className={`${btn} bg-accent hover:bg-accent-hover text-accent-fg`}>
              <Eye className="w-3.5 h-3.5" /> Ver telemetría
            </button>
            <button onClick={() => onRegenerate(item)} className={`${btn} bg-surface-subtle hover:bg-strong text-fg`}>
              <QrCode className="w-3.5 h-3.5" />
              {worker.device_identifier ? 'Mover a equipo nuevo' : 'Regenerar código'}
            </button>
            {(worker.status === 'ACTIVE' || worker.status === 'PAUSED') && (
              <input
                value={reason}
                onChange={e => setReason(e.target.value)}
                placeholder="Motivo (opcional)"
                className="bg-surface border border-default text-fg text-xs rounded-control px-2.5 py-1.5 w-44 focus:outline-none focus:border-accent"
              />
            )}
            {worker.status === 'ACTIVE' && (
              <button disabled={busy} onClick={() => setStatus('PAUSED')} className={`${btn} bg-warning-subtle hover:bg-warning-subtle text-warning`}>
                <Pause className="w-3.5 h-3.5" /> Pausar
              </button>
            )}
            {worker.status === 'PAUSED' && (
              <button
                disabled={busy || !hasConsent}
                onClick={() => setStatus('ACTIVE')}
                title={hasConsent ? undefined : 'El trabajador no tiene un consentimiento vigente'}
                className={`${btn} bg-success-subtle hover:bg-success-subtle text-success`}
              >
                <Play className="w-3.5 h-3.5" /> Reanudar
              </button>
            )}
            {worker.status !== 'REVOKED' && (
              <button disabled={busy} onClick={() => setShowRevoke(v => !v)} className={`${btn} bg-danger-subtle hover:bg-danger-subtle text-danger`}>
                <Ban className="w-3.5 h-3.5" /> Revocar
              </button>
            )}
            <button onClick={() => onDelete(item)} className={`${btn} bg-danger-subtle hover:bg-danger-subtle text-danger`}>
              <Trash2 className="w-3.5 h-3.5" /> Eliminar
            </button>
          </div>
          {showRevoke && (
            <div className="p-3 bg-danger-subtle border border-danger rounded-control space-y-2 text-xs text-fg">
              <p>
                Revocar desvincula el equipo: el agente borra sus credenciales, la llave fijada y la cola local. Escriba{' '}
                <span className="font-semibold text-fg">{worker.name}</span> para confirmar.
              </p>
              <div className="flex gap-2">
                <input
                  value={revokeTyped}
                  onChange={e => setRevokeTyped(e.target.value)}
                  className="flex-1 bg-surface border border-default text-fg text-xs rounded-control px-2.5 py-1.5 focus:outline-none focus:border-danger"
                />
                <button
                  disabled={busy || revokeTyped.trim() !== worker.name.trim()}
                  onClick={() => setStatus('REVOKED')}
                  className={`${btn} bg-danger hover:bg-danger-hover text-accent-fg`}
                >
                  Confirmar revocación
                </button>
              </div>
            </div>
          )}
        </section>

        {/* Consent */}
        <section className="space-y-2">
          <div className="flex items-center justify-between">
            <h4 className="text-xs font-semibold text-fg uppercase tracking-wide">
              Consentimiento · vigente v{worker.consent_version} · aceptada v{worker.accepted_consent_version ?? '–'}
              {hasConsent && summary.requested > 0 && (
                <span className="normal-case font-normal text-muted">
                  {' '}
                  · aceptó {summary.accepted} de {summary.requested} categorías solicitadas
                </span>
              )}
            </h4>
            {!editingConsent && worker.status !== 'REVOKED' && (
              <button onClick={() => setEditingConsent(true)} className={`${btn} bg-surface-subtle hover:bg-strong text-fg`}>
                <Pencil className="w-3.5 h-3.5" /> Editar datos a recolectar
              </button>
            )}
          </div>

          {editingConsent ? (
            <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
              <ConsentEditor
                role={worker.role_type}
                flags={worker.role_type === 'OFFICE' ? { ...draftFlags, location: false } : draftFlags}
                descriptions={draftDescriptions}
                onChange={(f, d) => {
                  setDraftFlags(f);
                  setDraftDescriptions(d);
                }}
              />
              <div className="space-y-3">
                <ConsentPreview organization={organization} flags={draftFlags} descriptions={draftDescriptions} />
                <p className="text-[11px] text-warning">
                  Ampliar lo que se recolecta publica una nueva versión: las categorías nuevas solo se recolectan si el
                  trabajador las acepta; mientras tanto siguen vigentes las que ya había aceptado.
                </p>
                <div className="flex justify-end gap-2">
                  <button onClick={() => setEditingConsent(false)} className={`${btn} bg-surface-subtle hover:bg-strong text-fg`}>
                    Cancelar
                  </button>
                  <button disabled={busy} onClick={saveConsent} className={`${btn} bg-accent hover:bg-accent-hover text-accent-fg`}>
                    Publicar nueva versión
                  </button>
                </div>
              </div>
            </div>
          ) : (
            <table className="w-full text-xs">
              <thead className="text-muted">
                <tr className="text-left">
                  <th className="py-1 font-medium">Categoría</th>
                  <th className="py-1 font-medium">Solicitado</th>
                  <th className="py-1 font-medium">Aceptado por el trabajador</th>
                  <th className="py-1 font-medium">Descripción mostrada al trabajador</th>
                </tr>
              </thead>
              <tbody className="text-fg">
                {CONSENT_CATEGORIES.map(c => (
                  <tr key={c.key} className="border-t border-default align-top">
                    <td className="py-1.5 pr-2">{c.label}</td>
                    <td className="py-1.5">{worker.requested_consent_flags[c.key] ? 'Sí' : '—'}</td>
                    <td className="py-1.5">{worker.consent_flags[c.key] ? 'Sí' : '—'}</td>
                    <td className="py-1.5 text-muted italic">{worker.consent_descriptions[c.key] || ''}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </section>

        {/* Consent evidence */}
        <section className="space-y-2">
          <div className="flex items-center justify-between">
            <h4 className="text-xs font-semibold text-fg uppercase tracking-wide">Historial de consentimiento</h4>
            <button
              onClick={exportCsv}
              disabled={!records || records.length === 0}
              className={`${btn} bg-surface-subtle hover:bg-strong text-fg`}
            >
              <Download className="w-3.5 h-3.5" /> Exportar CSV
            </button>
          </div>
          {recordsError ? (
            <p className="text-xs text-danger">No se pudo cargar el historial: {recordsError}</p>
          ) : records === null ? (
            <p className="text-xs text-muted">Cargando…</p>
          ) : records.length === 0 ? (
            <p className="text-xs text-muted">Sin decisiones registradas.</p>
          ) : (
            <table className="w-full text-xs">
              <thead className="text-muted">
                <tr className="text-left">
                  <th className="py-1 font-medium">Fecha</th>
                  <th className="py-1 font-medium">Versión</th>
                  <th className="py-1 font-medium">Decisión</th>
                  <th className="py-1 font-medium">Categorías aceptadas</th>
                  <th className="py-1 font-medium">IP · app</th>
                  <th className="py-1 font-medium">Manifiesto</th>
                </tr>
              </thead>
              <tbody className="text-fg">
                {records.map(r => (
                  <tr key={r.id} className="border-t border-default">
                    <td className="py-1.5">{fmtDate(r.created_at)}</td>
                    <td className="py-1.5">v{r.consent_version}</td>
                    <td className="py-1.5">{decisionLabel(r.decision)}</td>
                    <td className="py-1.5 text-muted">
                      {acceptedCategoriesOf(r).map(categoryLabel).join(', ') || '—'}
                    </td>
                    <td className="py-1.5 font-mono">
                      {r.ip_address ?? '—'}
                      {r.app_version ? <span className="text-muted"> · {r.app_version}</span> : null}
                    </td>
                    <td className="py-1.5 font-mono" title={r.manifest_hash ?? undefined}>
                      {r.manifest_hash ? `${r.manifest_hash.slice(0, 12)}…` : '—'}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </section>
      </div>
    </div>
  );
};
