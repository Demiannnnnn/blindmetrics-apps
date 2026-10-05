import React from 'react';
import { WorkerItem, WorkerStatus } from '../api/tauri-bridge';
import { Shield, Clock, MapPin, Eye, Pause, Play, QrCode, Pencil, Trash2, Laptop, AlertTriangle } from 'lucide-react';
import { consentSummary } from './consentCategories';

interface EmployeeCardProps {
  worker: WorkerItem;
  onSelect: (worker: WorkerItem) => void;
  onToggleStatus: (worker: WorkerItem) => void;
  onShowEnrollment: (worker: WorkerItem) => void;
  onEdit: (worker: WorkerItem) => void;
  onDelete: (worker: WorkerItem) => void;
  onOpenDetail: (worker: WorkerItem) => void;
}

export const STATUS_BADGES: Record<WorkerStatus, { label: string; className: string }> = {
  ACTIVE: { label: 'Activo', className: 'bg-success-subtle text-success border-default' },
  PENDING_CONSENT: { label: 'Esperando consentimiento', className: 'bg-info-subtle text-info border-default' },
  PENDING_ENROLLMENT: { label: 'Invitado', className: 'bg-strong text-fg border-strong' },
  PAUSED: { label: 'Pausado', className: 'bg-warning-subtle text-warning border-default' },
  REVOKED: { label: 'Revocado', className: 'bg-danger-subtle text-danger border-default' },
};

const CONSENT_BADGES: { key: keyof WorkerItem['consents']; label: string }[] = [
  { key: 'system_activity', label: 'Sistema' },
  { key: 'network_activity', label: 'Red (FQDN)' },
  { key: 'productivity_basic', label: 'Teclado/Clic' },
  { key: 'productivity_screenshots', label: 'Capturas' },
  { key: 'file_activity', label: 'Archivos' },
  { key: 'location', label: 'GPS' },
];

export const EmployeeCard: React.FC<EmployeeCardProps> = ({
  worker,
  onSelect,
  onToggleStatus,
  onShowEnrollment,
  onEdit,
  onDelete,
  onOpenDetail,
}) => {
  // RF-ADMIN-09-8: an unredeemed code that lapsed is shown as such.
  const expired = worker.status === 'PENDING_ENROLLMENT' && !!worker.raw.enrollment_expired;
  const badge = expired
    ? { label: 'Código expirado', className: 'bg-danger-subtle text-danger border-default' }
    : STATUS_BADGES[worker.status];
  const summary = consentSummary(worker.raw.consent_flags, worker.raw.requested_consent_flags);
  const canToggle = worker.status === 'ACTIVE' || worker.status === 'PAUSED';
  const accepted = worker.acceptedConsentVersion;
  const versionOutdated = accepted !== null && accepted < worker.consentVersion;

  return (
    <div className="bg-surface-subtle border border-default rounded-card p-5 hover:border-strong transition-colors flex flex-col justify-between">
      <div>
        <div className="flex justify-between items-start mb-3 gap-2">
          <div className="min-w-0">
            <button
              onClick={() => onOpenDetail(worker)}
              className="text-base font-semibold text-fg truncate hover:text-accent hover:underline text-left max-w-full"
              title="Ver detalle, consentimiento y acciones"
            >
              {worker.name}
            </button>
            <p className="text-xs text-muted font-mono mt-0.5 truncate">{worker.email}</p>
          </div>
          <span className={`px-2.5 py-1 text-xs font-semibold rounded-full border whitespace-nowrap ${badge.className}`}>
            {badge.label}
          </span>
        </div>

        <div className="flex flex-wrap gap-2 text-xs mb-3">
          <span
            className={`px-2 py-0.5 rounded-badge font-medium flex items-center gap-1 border border-default ${
              worker.role === 'FIELD' ? 'bg-warning-subtle text-warning' : 'bg-surface text-muted'
            }`}
          >
            {worker.role === 'FIELD' ? <MapPin className="w-3 h-3" /> : <Shield className="w-3 h-3" />}
            {worker.role}
          </span>
          <span className="bg-surface-subtle text-fg px-2 py-0.5 rounded-sm flex items-center gap-1">
            <Clock className="w-3 h-3 text-muted" />
            {worker.workHoursStart} - {worker.workHoursEnd}
          </span>
          <span
            className={`px-2 py-0.5 rounded flex items-center gap-1 ${
 versionOutdated ? 'bg-warning-subtle text-warning' : 'bg-surface-subtle text-fg'
 }`}
            title="Versión de consentimiento aceptada / vigente"
          >
            {versionOutdated && <AlertTriangle className="w-3 h-3" />}v{accepted ?? '–'} / v{worker.consentVersion}
          </span>
        </div>

        <p className="text-[11px] text-muted mb-3 flex items-center gap-1">
          <Laptop className="w-3 h-3" />
          {worker.deviceHostname ?? 'Sin equipo vinculado'}
        </p>

        <div className="border-t border-default pt-3 mb-4">
          <p className="text-xs font-medium text-muted mb-2">
            Consentimientos vigentes:{' '}
            {summary.requested > 0 && (worker.status === 'ACTIVE' || worker.status === 'PAUSED') && (
              <span className="text-fg">aceptó {summary.accepted} de {summary.requested}</span>
            )}
          </p>
          <div className="grid grid-cols-3 gap-1.5 text-[11px]">
            {CONSENT_BADGES.map(({ key, label }) => (
              <span
                key={key}
                className={`p-1 rounded-badge text-center truncate border border-default ${
                  worker.consents[key] ? 'bg-success-subtle text-success' : 'bg-surface text-muted'
                }`}
              >
                {key === 'location' && worker.role === 'OFFICE' ? 'GPS (N/A)' : label}
              </span>
            ))}
          </div>
        </div>
      </div>

      <div className="flex gap-2 pt-2 border-t border-default">
        <button
          onClick={() => onSelect(worker)}
          className="flex-1 bg-surface hover:bg-surface-subtle text-fg border border-default text-xs font-medium py-2 px-3 rounded-control flex items-center justify-center gap-1.5 transition"
        >
          <Eye className="w-3.5 h-3.5" />
          Ver Telemetría
        </button>
        <button
          onClick={() => onEdit(worker)}
          disabled={worker.status === 'REVOKED'}
          className="bg-surface-subtle hover:bg-strong text-fg p-2 rounded-control text-xs transition disabled:opacity-40"
          title="Editar datos del trabajador"
        >
          <Pencil className="w-4 h-4" />
        </button>
        <button
          onClick={() => onShowEnrollment(worker)}
          className="bg-surface-subtle hover:bg-strong text-fg p-2 rounded-control text-xs transition"
          title={worker.deviceHostname ? 'Mover a un equipo nuevo (nuevo código)' : 'Generar código de enrolamiento'}
        >
          <QrCode className="w-4 h-4" />
        </button>
        <button
          onClick={() => onToggleStatus(worker)}
          disabled={!canToggle}
          className={`p-2 rounded-control text-xs transition disabled:opacity-40 ${
 worker.status === 'ACTIVE'
 ? 'bg-warning-subtle hover:bg-warning-subtle text-warning border border-default'
 : 'bg-success-subtle hover:bg-success-subtle text-success border border-default'
 }`}
          title={worker.status === 'ACTIVE' ? 'Pausar recolección' : worker.status === 'PAUSED' ? 'Reanudar recolección' : 'Solo disponible para trabajadores activos o pausados'}
        >
          {worker.status === 'ACTIVE' ? <Pause className="w-4 h-4" /> : <Play className="w-4 h-4" />}
        </button>
        <button
          onClick={() => onDelete(worker)}
          className="bg-danger-subtle hover:bg-danger-subtle text-danger border border-default p-2 rounded-control text-xs transition"
          title="Dar de baja"
        >
          <Trash2 className="w-4 h-4" />
        </button>
      </div>
    </div>
  );
};
