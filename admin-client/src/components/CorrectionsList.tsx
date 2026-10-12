import React, { useState, useEffect } from 'react';
import {
  FileEdit,
  RefreshCw,
  AlertCircle,
  Filter,
} from 'lucide-react';
import {
  Correction,
  CorrectionStatus,
  attendanceCorrections,
  attendanceWithdrawCorrection,
} from '../api/tauri-bridge';
import {
  CORRECTION_STATUS_LABELS,
  CORRECTION_KIND_LABELS,
  MARK_TYPE_LABELS,
  formatR38DateTime,
} from '../lib/attendance';
import { describeError } from '../lib/errors';

interface CorrectionsListProps {
  workerId?: string;
  onRefreshParent?: () => void;
}

export const CorrectionsList: React.FC<CorrectionsListProps> = ({
  workerId,
  onRefreshParent,
}) => {
  const [corrections, setCorrections] = useState<Correction[]>([]);
  const [loading, setLoading] = useState<boolean>(true);
  const [error, setError] = useState<string | null>(null);
  const [statusFilter, setStatusFilter] = useState<'ALL' | CorrectionStatus>('ALL');
  const [withdrawingId, setWithdrawingId] = useState<string | null>(null);
  const [busyWithdraw, setBusyWithdraw] = useState<boolean>(false);

  const loadCorrections = async () => {
    setLoading(true);
    setError(null);
    try {
      const resp = await attendanceCorrections({
        worker_id: workerId || undefined,
        status: statusFilter === 'ALL' ? undefined : statusFilter,
        per_page: 50,
      });
      setCorrections(resp.data);
    } catch (err) {
      setError(describeError(err, 'No se pudieron cargar las correcciones.'));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    loadCorrections();
  }, [workerId, statusFilter]);

  const handleWithdraw = async (id: string) => {
    setBusyWithdraw(true);
    setError(null);
    try {
      await attendanceWithdrawCorrection(id);
      setWithdrawingId(null);
      await loadCorrections();
      onRefreshParent?.();
    } catch (err) {
      setError(describeError(err, 'No se pudo retirar la corrección.'));
    } finally {
      setBusyWithdraw(false);
    }
  };

  return (
    <div className="space-y-4">
      {/* Filters Bar */}
      <div className="flex flex-wrap items-center justify-between gap-3 bg-surface-subtle border border-default rounded-card p-3">
        <div className="flex items-center gap-2">
          <Filter className="w-4 h-4 text-muted" />
          <span className="text-xs font-semibold text-fg">Estado:</span>
          <select
            value={statusFilter}
            onChange={e => setStatusFilter(e.target.value as any)}
            className="bg-surface border border-default text-fg text-xs rounded-control px-2.5 py-1.5 focus:outline-hidden focus:border-accent cursor-pointer"
          >
            <option value="ALL">Todos los estados</option>
            <option value="PENDING">Pendientes de respuesta</option>
            <option value="ACCEPTED">Aceptadas</option>
            <option value="OPPOSED">Opuestas por el trabajador</option>
            <option value="CONSOLIDATED">Consolidadas (48 h)</option>
            <option value="WITHDRAWN">Retiradas</option>
          </select>
        </div>

        <button
          onClick={loadCorrections}
          disabled={loading}
          className="p-1.5 bg-surface hover:bg-surface-subtle text-muted hover:text-fg rounded-control border border-default transition"
          title="Actualizar listado"
        >
          <RefreshCw className={`w-4 h-4 ${loading ? 'animate-spin' : ''}`} />
        </button>
      </div>

      {error && (
        <div className="p-3 bg-danger-subtle border border-danger/30 rounded-control text-xs text-danger flex items-center gap-2">
          <AlertCircle className="w-4 h-4 shrink-0" />
          <span>{error}</span>
        </div>
      )}

      {/* Confirmation Dialog for Withdraw */}
      {withdrawingId && (
        <div className="fixed inset-0 z-50 bg-black/60 flex items-center justify-center p-4">
          <div className="bg-surface border border-default rounded-card max-w-md w-full p-6 space-y-4 shadow-xl">
            <h4 className="text-base font-bold text-fg">¿Retirar esta propuesta de corrección?</h4>
            <p className="text-xs text-muted leading-relaxed">
              La propuesta quedará sin efecto y se cancelará el plazo de 48 horas. La marca original
              se mantendrá como registro oficial.
            </p>
            <div className="flex justify-end gap-2 pt-2">
              <button
                onClick={() => setWithdrawingId(null)}
                disabled={busyWithdraw}
                className="px-3 py-1.5 bg-surface-subtle hover:bg-surface text-fg text-xs rounded-control border border-default transition"
              >
                Cancelar
              </button>
              <button
                onClick={() => handleWithdraw(withdrawingId)}
                disabled={busyWithdraw}
                className="px-4 py-1.5 bg-danger hover:bg-danger-hover text-accent-fg text-xs font-semibold rounded-control transition disabled:opacity-40"
              >
                {busyWithdraw ? 'Retirando…' : 'Sí, retirar propuesta'}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Table */}
      {loading ? (
        <div className="py-12 text-center text-muted text-xs flex items-center justify-center gap-2">
          <RefreshCw className="w-4 h-4 animate-spin text-accent" />
          Cargando registro de correcciones…
        </div>
      ) : corrections.length === 0 ? (
        <div className="py-12 text-center border border-dashed border-default rounded-card p-8">
          <FileEdit className="w-8 h-8 text-muted mx-auto mb-2 opacity-50" />
          <p className="text-sm font-medium text-fg">No hay correcciones registradas</p>
          <p className="text-xs text-muted mt-1 max-w-sm mx-auto">
            {statusFilter === 'ALL'
              ? 'Aún no se han propuesto correcciones para este criterio de búsqueda.'
              : 'No hay correcciones con el estado seleccionado.'}
          </p>
        </div>
      ) : (
        <div className="border border-default rounded-card overflow-hidden bg-surface">
          <table className="w-full text-left border-collapse text-xs">
            <thead>
              <tr className="bg-surface-subtle border-b border-default text-muted font-semibold">
                <th className="py-2.5 px-3">Fecha propuesta</th>
                <th className="py-2.5 px-3">Tipo</th>
                <th className="py-2.5 px-3">Trabajador</th>
                <th className="py-2.5 px-3">Motivo</th>
                <th className="py-2.5 px-3">Estado</th>
                <th className="py-2.5 px-3">Plazo 48 h</th>
                <th className="py-2.5 px-3 text-right">Acción</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-default">
              {corrections.map(corr => {
                const statusMeta = CORRECTION_STATUS_LABELS[corr.status] || {
                  label: corr.status,
                  className: 'bg-surface text-muted border-default',
                };
                return (
                  <tr key={corr.id} className="hover:bg-surface-subtle/50 transition">
                    <td className="py-2.5 px-3 font-mono font-medium text-fg whitespace-nowrap">
                      {formatR38DateTime(corr.proposed_ts)}
                    </td>
                    <td className="py-2.5 px-3">
                      <span className="font-semibold text-fg">
                        {MARK_TYPE_LABELS[corr.type] || corr.type}
                      </span>
                      <span className="block text-[11px] text-muted">
                        {CORRECTION_KIND_LABELS[corr.kind] || corr.kind}
                      </span>
                    </td>
                    <td className="py-2.5 px-3 font-medium text-fg">
                      {corr.worker_name || corr.worker_id}
                    </td>
                    <td className="py-2.5 px-3 max-w-xs truncate text-muted" title={corr.reason}>
                      {corr.reason}
                    </td>
                    <td className="py-2.5 px-3 whitespace-nowrap">
                      <span
                        className={`inline-block px-2 py-0.5 rounded-full text-[11px] font-semibold border ${statusMeta.className}`}
                      >
                        {statusMeta.label}
                      </span>
                    </td>
                    <td className="py-2.5 px-3 font-mono text-[11px] text-muted whitespace-nowrap">
                      {corr.respond_before ? formatR38DateTime(corr.respond_before) : '—'}
                    </td>
                    <td className="py-2.5 px-3 text-right whitespace-nowrap">
                      {corr.status === 'PENDING' ? (
                        <button
                          onClick={() => setWithdrawingId(corr.id)}
                          className="px-2.5 py-1 text-xs text-danger hover:bg-danger-subtle rounded-control border border-danger/30 font-medium transition"
                        >
                          Retirar
                        </button>
                      ) : (
                        <span className="text-muted text-[11px]">—</span>
                      )}
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
};
