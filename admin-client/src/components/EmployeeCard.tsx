import React from 'react';
import { WorkerItem, WorkerStatus } from '../api/tauri-bridge';
import { Shield, Clock, MapPin, Eye, Pause, Play, QrCode, Pencil, Trash2, Laptop, AlertTriangle } from 'lucide-react';

interface EmployeeCardProps {
  worker: WorkerItem;
  onSelect: (worker: WorkerItem) => void;
  onToggleStatus: (worker: WorkerItem) => void;
  onShowEnrollment: (worker: WorkerItem) => void;
  onEdit: (worker: WorkerItem) => void;
  onDelete: (worker: WorkerItem) => void;
}

export const STATUS_BADGES: Record<WorkerStatus, { label: string; className: string }> = {
  ACTIVE: { label: 'Activo', className: 'bg-emerald-500/20 text-emerald-400 border-emerald-500/30' },
  PENDING_CONSENT: { label: 'Esperando consentimiento', className: 'bg-sky-500/20 text-sky-300 border-sky-500/30' },
  PENDING_ENROLLMENT: { label: 'Invitado', className: 'bg-slate-500/20 text-slate-300 border-slate-500/30' },
  PAUSED: { label: 'Pausado', className: 'bg-amber-500/20 text-amber-300 border-amber-500/30' },
  REVOKED: { label: 'Revocado', className: 'bg-rose-500/20 text-rose-400 border-rose-500/30' },
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
}) => {
  const badge = STATUS_BADGES[worker.status];
  const canToggle = worker.status === 'ACTIVE' || worker.status === 'PAUSED';
  const accepted = worker.acceptedConsentVersion;
  const versionOutdated = accepted !== null && accepted < worker.consentVersion;

  return (
    <div className="bg-slate-800/80 border border-slate-700/80 rounded-xl p-5 shadow-lg hover:border-indigo-500/50 transition-all flex flex-col justify-between">
      <div>
        <div className="flex justify-between items-start mb-3 gap-2">
          <div className="min-w-0">
            <h3 className="text-base font-semibold text-white truncate">{worker.name}</h3>
            <p className="text-xs text-slate-400 font-mono mt-0.5 truncate">{worker.email}</p>
          </div>
          <span className={`px-2.5 py-1 text-xs font-semibold rounded-full border whitespace-nowrap ${badge.className}`}>
            {badge.label}
          </span>
        </div>

        <div className="flex flex-wrap gap-2 text-xs mb-3">
          <span
            className={`px-2 py-0.5 rounded font-medium flex items-center gap-1 ${
              worker.role === 'FIELD'
                ? 'bg-amber-500/20 text-amber-300 border border-amber-500/30'
                : 'bg-indigo-500/20 text-indigo-300 border border-indigo-500/30'
            }`}
          >
            {worker.role === 'FIELD' ? <MapPin className="w-3 h-3" /> : <Shield className="w-3 h-3" />}
            {worker.role}
          </span>
          <span className="bg-slate-700/60 text-slate-300 px-2 py-0.5 rounded flex items-center gap-1">
            <Clock className="w-3 h-3 text-slate-400" />
            {worker.workHoursStart} - {worker.workHoursEnd}
          </span>
          <span
            className={`px-2 py-0.5 rounded flex items-center gap-1 ${
              versionOutdated ? 'bg-amber-500/15 text-amber-300' : 'bg-slate-700/60 text-slate-300'
            }`}
            title="Versión de consentimiento aceptada / vigente"
          >
            {versionOutdated && <AlertTriangle className="w-3 h-3" />}v{accepted ?? '–'} / v{worker.consentVersion}
          </span>
        </div>

        <p className="text-[11px] text-slate-400 mb-3 flex items-center gap-1">
          <Laptop className="w-3 h-3" />
          {worker.deviceHostname ?? 'Sin equipo vinculado'}
        </p>

        <div className="border-t border-slate-700/60 pt-3 mb-4">
          <p className="text-xs font-medium text-slate-400 mb-2">Consentimientos vigentes:</p>
          <div className="grid grid-cols-3 gap-1.5 text-[11px]">
            {CONSENT_BADGES.map(({ key, label }) => (
              <span
                key={key}
                className={`p-1 rounded text-center truncate ${
                  worker.consents[key]
                    ? 'bg-indigo-950 text-indigo-300 border border-indigo-800'
                    : 'bg-slate-900/60 text-slate-500'
                }`}
              >
                {key === 'location' && worker.role === 'OFFICE' ? 'GPS (N/A)' : label}
              </span>
            ))}
          </div>
        </div>
      </div>

      <div className="flex gap-2 pt-2 border-t border-slate-700/60">
        <button
          onClick={() => onSelect(worker)}
          className="flex-1 bg-indigo-600 hover:bg-indigo-500 text-white text-xs font-medium py-2 px-3 rounded-lg flex items-center justify-center gap-1.5 transition"
        >
          <Eye className="w-3.5 h-3.5" />
          Ver Telemetría
        </button>
        <button
          onClick={() => onEdit(worker)}
          disabled={worker.status === 'REVOKED'}
          className="bg-slate-700 hover:bg-slate-600 text-slate-200 p-2 rounded-lg text-xs transition disabled:opacity-40"
          title="Editar datos del trabajador"
        >
          <Pencil className="w-4 h-4" />
        </button>
        <button
          onClick={() => onShowEnrollment(worker)}
          className="bg-slate-700 hover:bg-slate-600 text-slate-200 p-2 rounded-lg text-xs transition"
          title={worker.deviceHostname ? 'Mover a un equipo nuevo (nuevo código)' : 'Generar código de enrolamiento'}
        >
          <QrCode className="w-4 h-4" />
        </button>
        <button
          onClick={() => onToggleStatus(worker)}
          disabled={!canToggle}
          className={`p-2 rounded-lg text-xs transition disabled:opacity-40 ${
            worker.status === 'ACTIVE'
              ? 'bg-amber-950/60 hover:bg-amber-900 text-amber-300 border border-amber-800/40'
              : 'bg-emerald-950/60 hover:bg-emerald-900 text-emerald-300 border border-emerald-800/40'
          }`}
          title={worker.status === 'ACTIVE' ? 'Pausar recolección' : worker.status === 'PAUSED' ? 'Reanudar recolección' : 'Solo disponible para trabajadores activos o pausados'}
        >
          {worker.status === 'ACTIVE' ? <Pause className="w-4 h-4" /> : <Play className="w-4 h-4" />}
        </button>
        <button
          onClick={() => onDelete(worker)}
          className="bg-rose-950/60 hover:bg-rose-900 text-rose-300 border border-rose-800/40 p-2 rounded-lg text-xs transition"
          title="Dar de baja"
        >
          <Trash2 className="w-4 h-4" />
        </button>
      </div>
    </div>
  );
};
