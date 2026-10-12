import React, { useState, useEffect, useRef } from 'react';
import {
  Users,
  Building2,
  RefreshCw,
  Search,
  Filter,
  AlertTriangle,
  Clock,
  ArrowRight,
  WifiOff,
} from 'lucide-react';
import {
  BoardWorker,
  Workplace,
  attendanceBoard,
  workplacesList,
} from '../api/tauri-bridge';
import {
  ATTENDANCE_STATE_LABELS,
  MARK_TYPE_SHORT,
  formatR38Time,
  describeAnomaly,
} from '../lib/attendance';
import { describeError } from '../lib/errors';

interface AttendanceBoardProps {
  onSelectWorker: (workerId: string) => void;
  onNavigateToDashboard?: () => void;
}

export const AttendanceBoard: React.FC<AttendanceBoardProps> = ({
  onSelectWorker,
  onNavigateToDashboard,
}) => {
  const [board, setBoard] = useState<BoardWorker[]>([]);
  const [workplaces, setWorkplaces] = useState<Workplace[]>([]);
  const [loading, setLoading] = useState<boolean>(true);
  const [error, setError] = useState<string | null>(null);

  // Filters (RF-ADMIN-15-3)
  const [search, setSearch] = useState<string>('');
  const [workplaceFilter, setWorkplaceFilter] = useState<string>('ALL');
  const [stateFilter, setStateFilter] = useState<string>('ALL');

  const [lastRefreshed, setLastRefreshed] = useState<Date>(new Date());
  const timerRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const loadData = async (silent = false) => {
    if (!silent) setLoading(true);
    setError(null);
    try {
      const [workers, wpList] = await Promise.all([
        attendanceBoard(),
        workplacesList().catch(() => []),
      ]);
      setBoard(workers);
      setWorkplaces(wpList);
      setLastRefreshed(new Date());
    } catch (err) {
      setError(describeError(err, 'No se pudo cargar el tablero de asistencia en vivo.'));
    } finally {
      if (!silent) setLoading(false);
    }
  };

  useEffect(() => {
    loadData();

    // Auto-refresh every 60 s, paused if document not visible (RF-ADMIN-15-3)
    const interval = setInterval(() => {
      if (document.visibilityState === 'visible') {
        loadData(true);
      }
    }, 60000);
    timerRef.current = interval;

    const onVisibilityChange = () => {
      if (document.visibilityState === 'visible') {
        loadData(true);
      }
    };
    document.addEventListener('visibilitychange', onVisibilityChange);

    return () => {
      clearInterval(interval);
      document.removeEventListener('visibilitychange', onVisibilityChange);
    };
  }, []);

  // Filter board workers
  const filteredWorkers = board.filter(w => {
    // Workplace filter
    if (workplaceFilter !== 'ALL') {
      if (w.workplace?.id !== workplaceFilter && w.workplace?.name !== workplaceFilter) {
        return false;
      }
    }
    // State filter
    if (stateFilter !== 'ALL') {
      if (w.state !== stateFilter) return false;
    }
    // Search
    if (search.trim()) {
      const q = search.trim().toLowerCase();
      const matchName = w.name.toLowerCase().includes(q);
      const matchRun = w.run?.toLowerCase().includes(q) ?? false;
      if (!matchName && !matchRun) return false;
    }
    return true;
  });

  return (
    <div className="space-y-5">
      {/* Header & Meta */}
      <div className="bg-surface border border-default rounded-card p-4 space-y-4">
        <div className="flex flex-wrap items-center justify-between gap-4">
          <div>
            <h2 className="text-base font-bold text-fg flex items-center gap-2">
              <Clock className="w-5 h-5 text-accent" />
              Asistencia en Vivo (AS1)
            </h2>
            <p className="text-xs text-muted mt-0.5">
              Estado de jornada en tiempo real de los trabajadores activos. Actualización cada 60 s.
            </p>
          </div>

          <div className="flex items-center gap-3">
            <span className="text-[11px] text-muted font-mono">
              Actualizado: {lastRefreshed.toLocaleTimeString('es-CL')}
            </span>
            <button
              onClick={() => loadData(false)}
              disabled={loading}
              className="p-1.5 bg-surface-subtle hover:bg-surface text-muted hover:text-fg border border-default rounded-control transition flex items-center gap-1.5 text-xs"
              title="Refrescar ahora"
            >
              <RefreshCw className={`w-3.5 h-3.5 ${loading ? 'animate-spin' : ''}`} />
              <span>Actualizar</span>
            </button>
          </div>
        </div>

        {/* Filters Bar (RF-ADMIN-15-3) */}
        <div className="grid grid-cols-1 sm:grid-cols-3 gap-3 pt-3 border-t border-default text-xs">
          {/* Search */}
          <div className="relative">
            <Search className="w-4 h-4 text-muted absolute left-3 top-1/2 -translate-y-1/2" />
            <input
              type="text"
              placeholder="Buscar por nombre o RUN…"
              value={search}
              onChange={e => setSearch(e.target.value)}
              className="w-full bg-surface-subtle border border-default rounded-control pl-9 pr-3 py-1.5 text-xs text-fg focus:outline-hidden focus:border-accent"
            />
          </div>

          {/* Workplace Filter */}
          <div className="flex items-center gap-2">
            <Building2 className="w-4 h-4 text-muted shrink-0" />
            <select
              value={workplaceFilter}
              onChange={e => setWorkplaceFilter(e.target.value)}
              className="w-full bg-surface-subtle border border-default rounded-control px-2.5 py-1.5 text-xs text-fg focus:outline-hidden focus:border-accent cursor-pointer"
            >
              <option value="ALL">Todos los lugares de trabajo</option>
              {workplaces.map(wp => (
                <option key={wp.id} value={wp.id}>
                  {wp.name}
                </option>
              ))}
            </select>
          </div>

          {/* State Filter */}
          <div className="flex items-center gap-2">
            <Filter className="w-4 h-4 text-muted shrink-0" />
            <select
              value={stateFilter}
              onChange={e => setStateFilter(e.target.value)}
              className="w-full bg-surface-subtle border border-default rounded-control px-2.5 py-1.5 text-xs text-fg focus:outline-hidden focus:border-accent cursor-pointer"
            >
              <option value="ALL">Todos los estados</option>
              <option value="WORKING">En jornada</option>
              <option value="ON_BREAK">En colación</option>
              <option value="OFF">Fuera de jornada</option>
              <option value="UNMARKED">Sin marcar hoy</option>
            </select>
          </div>
        </div>
      </div>

      {error && (
        <div className="p-3 bg-danger-subtle border border-danger/30 rounded-control text-xs text-danger flex items-center gap-2">
          <AlertTriangle className="w-4 h-4 shrink-0" />
          <span>{error}</span>
        </div>
      )}

      {/* Board Table */}
      {loading ? (
        <div className="py-16 text-center text-muted text-xs flex items-center justify-center gap-2">
          <RefreshCw className="w-4 h-4 animate-spin text-accent" />
          Cargando tablero de asistencia en vivo…
        </div>
      ) : filteredWorkers.length === 0 ? (
        <div className="py-16 text-center border border-dashed border-default rounded-card p-8 bg-surface space-y-3">
          <Users className="w-8 h-8 text-muted mx-auto opacity-50" />
          <p className="text-sm font-medium text-fg">
            Aún no hay trabajadores con asistencia
          </p>
          <p className="text-xs text-muted max-w-sm mx-auto">
            {board.length === 0
              ? 'Aún no hay trabajadores vinculados con registros de asistencia. Crea uno desde el Tablero Principal.'
              : 'Ningún trabajador coincide con los filtros aplicados.'}
          </p>
          {board.length === 0 && onNavigateToDashboard && (
            <button
              onClick={onNavigateToDashboard}
              className="px-4 py-2 bg-accent hover:bg-accent-hover text-accent-fg text-xs font-semibold rounded-control transition"
            >
              Ir al Tablero Principal
            </button>
          )}
        </div>
      ) : (
        <div className="border border-default rounded-card overflow-hidden bg-surface">
          <table className="w-full text-left border-collapse text-xs">
            <thead>
              <tr className="bg-surface-subtle border-b border-default text-muted font-semibold">
                <th className="py-2.5 px-3">Trabajador</th>
                <th className="py-2.5 px-3">Lugar de trabajo</th>
                <th className="py-2.5 px-3">Estado</th>
                <th className="py-2.5 px-3">Última marca</th>
                <th className="py-2.5 px-3">Marcas de hoy</th>
                <th className="py-2.5 px-3">Alertas</th>
                <th className="py-2.5 px-3 text-right">Detalle</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-default">
              {filteredWorkers.map(worker => {
                const stateMeta = ATTENDANCE_STATE_LABELS[worker.state] || {
                  label: worker.state,
                  className: 'bg-surface text-muted border-default',
                };

                return (
                  <tr
                    key={worker.worker_id}
                    onClick={() => onSelectWorker(worker.worker_id)}
                    className="hover:bg-surface-subtle/70 cursor-pointer transition"
                    title="Haz clic para ver marcas y correcciones de este trabajador"
                  >
                    {/* Worker */}
                    <td className="py-3 px-3">
                      <div className="font-semibold text-fg hover:text-accent">
                        {worker.name}
                      </div>
                      <div className="flex items-center gap-2 mt-0.5">
                        <span className="text-[10px] font-mono text-muted bg-surface-subtle px-1.5 py-0.2 rounded-sm border border-default">
                          {worker.run || '—'}
                        </span>
                        {worker.work_mode && (
                          <span className="text-[10px] text-muted">
                            {worker.work_mode === 'ON_SITE'
                              ? 'Presencial'
                              : worker.work_mode === 'REMOTE'
                                ? 'Teletrabajo'
                                : 'Mixta'}
                          </span>
                        )}
                      </div>
                    </td>

                    {/* Workplace */}
                    <td className="py-3 px-3">
                      <span className="text-fg font-medium">
                        {worker.workplace?.name || 'Casa matriz'}
                      </span>
                    </td>

                    {/* State badge */}
                    <td className="py-3 px-3 whitespace-nowrap">
                      <span
                        className={`inline-block px-2.5 py-0.5 rounded-full text-[11px] font-semibold border ${stateMeta.className}`}
                      >
                        {stateMeta.label}
                      </span>
                    </td>

                    {/* Last mark */}
                    <td className="py-3 px-3 whitespace-nowrap">
                      {worker.last_mark ? (
                        <div className="flex items-center gap-1.5 font-mono">
                          <span className="font-semibold text-fg">
                            {formatR38Time(worker.last_mark.server_ts)}
                          </span>
                          <span className="text-[11px] text-muted font-sans">
                            ({MARK_TYPE_SHORT[worker.last_mark.type] || worker.last_mark.type})
                          </span>
                          {worker.last_mark.offline && (
                            <span title="Sin conexión">
                              <WifiOff className="w-3 h-3 text-warning shrink-0" />
                            </span>
                          )}
                        </div>
                      ) : (
                        <span className="text-muted text-[11px] italic">Sin marcas</span>
                      )}
                    </td>

                    {/* Today's marks count & list */}
                    <td className="py-3 px-3">
                      {worker.today_marks.length === 0 ? (
                        <span className="text-muted text-[11px]">0 marcas</span>
                      ) : (
                        <div className="flex flex-wrap gap-1 items-center">
                          {worker.today_marks.map(m => (
                            <span
                              key={m.id}
                              className="px-1.5 py-0.5 bg-surface-subtle border border-default rounded-sm text-[10px] font-mono text-fg"
                            >
                              {MARK_TYPE_SHORT[m.type] || m.type} {formatR38Time(m.server_ts).slice(0, 5)}
                            </span>
                          ))}
                        </div>
                      )}
                    </td>

                    {/* Alerts in words */}
                    <td className="py-3 px-3">
                      {worker.alerts.length > 0 ? (
                        <div className="space-y-0.5">
                          {worker.alerts.map((alert, idx) => (
                            <span
                              key={idx}
                              className="inline-block text-[11px] text-danger bg-danger-subtle px-2 py-0.5 rounded-badge border border-danger/30 font-semibold"
                            >
                              {describeAnomaly(alert)}
                            </span>
                          ))}
                        </div>
                      ) : (
                        <span className="text-muted text-[11px]">Sin alertas</span>
                      )}
                    </td>

                    {/* Link action */}
                    <td className="py-3 px-3 text-right">
                      <span className="text-muted hover:text-accent p-1 inline-flex items-center">
                        <ArrowRight className="w-4 h-4" />
                      </span>
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
