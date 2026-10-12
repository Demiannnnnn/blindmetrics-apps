import { WorkerItem, AttendanceState } from '../api/tauri-bridge';
import {
  Shield,
  MapPin,
  Eye,
  Pause,
  Play,
  QrCode,
  Pencil,
  Trash2,
  Laptop,
  Building2,
} from 'lucide-react';
import { STATUS_BADGES, getTelemetryBadge } from './consentCategories';
import { ATTENDANCE_STATE_LABELS } from '../lib/attendance';

export { STATUS_BADGES };

interface EmployeeCardProps {
  worker: WorkerItem;
  todayAttendanceState?: AttendanceState;
  onSelect: (worker: WorkerItem) => void;
  onToggleStatus: (worker: WorkerItem) => void;
  onShowEnrollment: (worker: WorkerItem) => void;
  onEdit: (worker: WorkerItem) => void;
  onDelete: (worker: WorkerItem) => void;
  onOpenDetail: (worker: WorkerItem) => void;
}

export function telemetryBadge(worker: WorkerItem): { label: string; className: string } {
  return getTelemetryBadge(
    worker.telemetryStatus,
    worker.raw.consent_flags,
    worker.raw.requested_consent_flags,
  );
}

export const EmployeeCard: React.FC<EmployeeCardProps> = ({
  worker,
  onSelect,
  onToggleStatus,
  onShowEnrollment,
  onEdit,
  onDelete,
  onOpenDetail,
  todayAttendanceState,
}) => {
  // RF-ADMIN-09-8: an unredeemed code that lapsed is shown as such.
  const expired = worker.status === 'PENDING_ENROLLMENT' && !!worker.raw.enrollment_expired;
  const badge = expired
    ? { label: 'Código expirado', className: 'bg-danger-subtle text-danger border-default' }
    : STATUS_BADGES[worker.status] || {
        label: worker.status,
        className: 'bg-surface text-muted border-default',
      };

  const teleBadge = telemetryBadge(worker);
  const canToggleTelemetry =
    worker.status !== 'REVOKED' &&
    (worker.telemetryStatus === 'ACCEPTED' || worker.telemetryStatus === 'PAUSED');

  const workModeLabel =
    worker.workMode === 'ON_SITE'
      ? 'Presencial'
      : worker.workMode === 'REMOTE'
        ? 'Teletrabajo'
        : 'Mixta';

  const attBadge = todayAttendanceState ? ATTENDANCE_STATE_LABELS[todayAttendanceState] : null;

  return (
    <div className="bg-surface-subtle border border-default rounded-card p-5 hover:border-strong transition-colors flex flex-col justify-between">
      <div>
        {/* Name, email, RUN, status badges */}
        <div className="flex justify-between items-start mb-3 gap-2">
          <div className="min-w-0">
            <button
              onClick={() => onOpenDetail(worker)}
              className="text-base font-semibold text-fg truncate hover:text-accent hover:underline text-left max-w-full"
              title="Ver detalle, jornada y telemetría"
            >
              {worker.name}
            </button>
            <div className="flex items-center gap-2 mt-0.5">
              <span className="text-xs font-mono font-medium text-fg bg-surface px-1.5 py-0.2 rounded-sm border border-default">
                {worker.run || '—'}
              </span>
              <p className="text-xs text-muted truncate">{worker.personalEmail || worker.email}</p>
            </div>
            {worker.jobTitle && (
              <p className="text-[11px] text-muted truncate mt-0.5">{worker.jobTitle}</p>
            )}
          </div>
          <div className="flex flex-col items-end gap-1">
            <span
              className={`px-2 py-0.5 text-[11px] font-semibold rounded-full border whitespace-nowrap ${badge.className}`}
            >
              {badge.label}
            </span>
          </div>
        </div>

        {/* W1 Columns/data: Lugar, Modalidad, Asistencia, Telemetría */}
        <div className="grid grid-cols-2 gap-2 text-xs mb-3 p-2.5 bg-surface border border-default rounded-control">
          <div>
            <span className="text-[10px] text-muted block">Lugar</span>
            <span className="font-medium text-fg truncate block flex items-center gap-1 mt-0.5">
              <Building2 className="w-3 h-3 text-muted shrink-0" />
              <span className="truncate">{worker.workplace?.name || '—'}</span>
            </span>
          </div>
          <div>
            <span className="text-[10px] text-muted block">Modalidad</span>
            <span className="font-medium text-fg block mt-0.5">{workModeLabel}</span>
          </div>
          <div>
            <span className="text-[10px] text-muted block">Asistencia (hoy)</span>
            {worker.art22Exempt ? (
              <span className="text-[11px] text-muted block mt-0.5 font-medium">Exento art. 22</span>
            ) : attBadge ? (
              <span
                className={`inline-block px-1.5 py-0.2 text-[10px] font-medium rounded-sm border mt-0.5 ${attBadge.className}`}
              >
                {attBadge.label}
              </span>
            ) : (
              <span className="text-muted block mt-0.5">—</span>
            )}
          </div>
          <div>
            <span className="text-[10px] text-muted block">Telemetría</span>
            <span
              className={`inline-block px-1.5 py-0.2 text-[10px] font-medium rounded-sm border mt-0.5 ${teleBadge.className}`}
            >
              {teleBadge.label}
            </span>
          </div>
        </div>

        <div className="flex flex-wrap items-center gap-2 text-xs mb-3">
          <span
            className={`px-2 py-0.5 rounded-badge font-medium flex items-center gap-1 border border-default ${
              worker.role === 'FIELD' ? 'bg-warning-subtle text-warning' : 'bg-surface text-muted'
            }`}
          >
            {worker.role === 'FIELD' ? <MapPin className="w-3 h-3" /> : <Shield className="w-3 h-3" />}
            {worker.role}
          </span>
          <p className="text-[11px] text-muted flex items-center gap-1">
            <Laptop className="w-3 h-3" />
            {worker.deviceHostname ?? 'Sin equipo'}
          </p>
        </div>
      </div>

      {/* Action buttons */}
      <div className="flex gap-2 pt-2 border-t border-default">
        <button
          onClick={() => onSelect(worker)}
          className="flex-1 bg-surface hover:bg-surface-subtle text-fg border border-default text-xs font-medium py-2 px-3 rounded-control flex items-center justify-center gap-1.5 transition"
        >
          <Eye className="w-3.5 h-3.5" />
          Telemetría
        </button>
        <button
          onClick={() => onEdit(worker)}
          disabled={worker.status === 'REVOKED'}
          className="bg-surface-subtle hover:bg-surface text-fg p-2 rounded-control text-xs border border-default transition disabled:opacity-40"
          title="Editar datos del trabajador"
        >
          <Pencil className="w-4 h-4" />
        </button>
        <button
          onClick={() => onShowEnrollment(worker)}
          className="bg-surface-subtle hover:bg-surface text-fg p-2 rounded-control text-xs border border-default transition"
          title={
            worker.deviceHostname
              ? 'Mover a un equipo nuevo (nuevo código)'
              : 'Generar código de vinculación'
          }
        >
          <QrCode className="w-4 h-4" />
        </button>
        <button
          onClick={() => onToggleStatus(worker)}
          disabled={!canToggleTelemetry}
          className={`p-2 rounded-control text-xs transition border border-default disabled:opacity-40 ${
            worker.telemetryStatus === 'PAUSED'
              ? 'bg-success-subtle text-success hover:bg-success-subtle/80'
              : 'bg-warning-subtle text-warning hover:bg-warning-subtle/80'
          }`}
          title={
            worker.telemetryStatus === 'PAUSED'
              ? 'Reanudar telemetría'
              : worker.telemetryStatus === 'ACCEPTED'
                ? 'Pausar telemetría'
                : 'Pausar/Reanudar telemetría no disponible'
          }
        >
          {worker.telemetryStatus === 'PAUSED' ? (
            <Play className="w-4 h-4" />
          ) : (
            <Pause className="w-4 h-4" />
          )}
        </button>
        <button
          onClick={() => onDelete(worker)}
          className="bg-danger-subtle hover:bg-danger-subtle/80 text-danger border border-default p-2 rounded-control text-xs transition"
          title="Dar de baja / Revocar"
        >
          <Trash2 className="w-4 h-4" />
        </button>
      </div>
    </div>
  );
};
