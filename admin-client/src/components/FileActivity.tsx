import React, { useState, useEffect } from 'react';
import {
  FileText,
  Filter,
  Search,
  AlertCircle,
  FolderOpen,
  FilePlus,
  FileEdit,
  FileX,
  FileCode,
} from 'lucide-react';
import {
  TimeRangeFilter,
  getTimeRangeIso,
  fetchAndDecryptTelemetry,
  isDemoMode,
  sanitizeHtmlText,
} from '../api/tauri-bridge';

interface DecryptedFileOperation {
  id: string;
  time: string;
  action: 'CREATED' | 'MODIFIED' | 'DELETED' | 'RENAMED' | string;
  watchedRoot: string;
  relativePath: string;
  previousRelativePath?: string | null;
  extension: string;
}

interface FileActivityProps {
  workerId?: string;
  workerName: string;
  timeRange: TimeRangeFilter;
}

const mockFileOperations: DecryptedFileOperation[] = [
  { id: "f-1", time: "09:22:10", action: "CREATED", watchedRoot: "Corporate", relativePath: "Q3/budget-2026.xlsx", extension: "xlsx" },
  { id: "f-2", time: "10:14:05", action: "MODIFIED", watchedRoot: "Corporate", relativePath: "src/commands/telemetry.rs", extension: "rs" },
  { id: "f-3", time: "11:05:32", action: "MODIFIED", watchedRoot: "Corporate", relativePath: "contracts/NDA-Partner-Signed.docx", extension: "docx" },
  { id: "f-4", time: "11:42:19", action: "RENAMED", watchedRoot: "Corporate", relativePath: "reports/final-audit-2026.pdf", previousRelativePath: "reports/draft-audit.pdf", extension: "pdf" },
  { id: "f-5", time: "12:10:44", action: "DELETED", watchedRoot: "Corporate", relativePath: "scratch/temp_analysis.py", extension: "py" },
];

export const FileActivity: React.FC<FileActivityProps> = ({
  workerId,
  workerName,
  timeRange,
}) => {
  const [loading, setLoading] = useState<boolean>(false);
  const [operations, setOperations] = useState<DecryptedFileOperation[]>([]);
  const [actionFilter, setActionFilter] = useState<string>('ALL');
  const [extFilter, setExtFilter] = useState<string>('ALL');
  const [searchQuery, setSearchQuery] = useState<string>('');

  useEffect(() => {
    console.debug("Dashboard context updated:", { workerId, range: timeRange });

    if (!workerId) {
      if (isDemoMode) {
        setOperations(mockFileOperations);
      } else {
        setOperations([]);
      }
      return;
    }

    const { from, to } = getTimeRangeIso(timeRange);
    let isCancelled = false;
    setLoading(true);

    fetchAndDecryptTelemetry(workerId, 'files', from, to)
      .then(records => {
        if (isCancelled) return;

        const ops: DecryptedFileOperation[] = [];

        for (const rec of records) {
          const items = Array.isArray(rec.payload?.items) ? rec.payload.items : [rec.payload];
          for (let i = 0; i < items.length; i++) {
            const item = items[i];
            if (!item) continue;

            const ts = item.ts || rec.client_timestamp || '';
            const dateObj = new Date(ts);
            const timeLabel = isNaN(dateObj.getTime())
              ? ts
              : dateObj.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' });

            ops.push({
              id: `${rec.id}-${i}`,
              time: timeLabel,
              action: item.action || 'MODIFIED',
              watchedRoot: sanitizeHtmlText(item.watched_root || 'Corporate'),
              relativePath: sanitizeHtmlText(item.relative_path || ''),
              previousRelativePath: item.previous_relative_path ? sanitizeHtmlText(item.previous_relative_path) : null,
              extension: item.extension || item.relative_path?.split('.').pop() || 'unknown',
            });
          }
        }

        if (ops.length > 0) {
          setOperations(ops);
        } else if (isDemoMode) {
          setOperations(mockFileOperations);
        } else {
          setOperations([]);
        }
      })
      .catch(err => {
        if (isCancelled) return;
        console.error("FileActivity fetch error:", err);
        if (isDemoMode) {
          setOperations(mockFileOperations);
        } else {
          setOperations([]);
        }
      })
      .finally(() => {
        if (!isCancelled) setLoading(false);
      });

    return () => {
      isCancelled = true;
    };
  }, [workerId, timeRange.preset, timeRange.from, timeRange.to]);

  // Unique extensions present in operations
  const availableExtensions = Array.from(
    new Set(operations.map(o => o.extension.toLowerCase()).filter(Boolean))
  ).sort();

  // Filtered dataset
  const filteredOps = operations.filter(op => {
    if (actionFilter !== 'ALL' && op.action !== actionFilter) return false;
    if (extFilter !== 'ALL' && op.extension.toLowerCase() !== extFilter.toLowerCase()) return false;
    if (searchQuery.trim().length > 0) {
      const q = searchQuery.toLowerCase();
      const matchPath = op.relativePath.toLowerCase().includes(q);
      const matchOld = op.previousRelativePath?.toLowerCase().includes(q) ?? false;
      const matchRoot = op.watchedRoot.toLowerCase().includes(q);
      if (!matchPath && !matchOld && !matchRoot) return false;
    }
    return true;
  });

  const getActionBadge = (action: string) => {
    switch (action) {
      case 'CREATED':
        return (
          <span className="px-2 py-0.5 rounded text-[10px] font-bold bg-emerald-500/20 text-emerald-300 border border-emerald-500/40 flex items-center gap-1">
            <FilePlus className="w-3 h-3" /> Creado
          </span>
        );
      case 'MODIFIED':
        return (
          <span className="px-2 py-0.5 rounded text-[10px] font-bold bg-sky-500/20 text-sky-300 border border-sky-500/40 flex items-center gap-1">
            <FileEdit className="w-3 h-3" /> Modificado
          </span>
        );
      case 'DELETED':
        return (
          <span className="px-2 py-0.5 rounded text-[10px] font-bold bg-rose-500/20 text-rose-300 border border-rose-500/40 flex items-center gap-1">
            <FileX className="w-3 h-3" /> Eliminado
          </span>
        );
      case 'RENAMED':
        return (
          <span className="px-2 py-0.5 rounded text-[10px] font-bold bg-amber-500/20 text-amber-300 border border-amber-500/40 flex items-center gap-1">
            <FileCode className="w-3 h-3" /> Renombrado
          </span>
        );
      default:
        return (
          <span className="px-2 py-0.5 rounded text-[10px] font-bold bg-slate-700 text-slate-300">
            {action}
          </span>
        );
    }
  };

  return (
    <div className="bg-slate-800/80 border border-slate-700/80 rounded-xl p-5 shadow-lg space-y-5">
      {/* Header */}
      <div className="flex flex-wrap justify-between items-center gap-3">
        <div>
          <h2 className="text-lg font-semibold text-white flex items-center gap-2">
            <FileText className="w-5 h-5 text-indigo-400" />
            Registro de Actividad de Archivos Corporativos
          </h2>
          <p className="text-xs text-slate-400">
            Operaciones sobre carpetas designadas (DP-03: solo metadatos, sin contenido) para:{' '}
            <span className="text-indigo-400 font-medium">{workerName}</span>
          </p>
        </div>

        {/* Action Counters */}
        <div className="flex gap-2 text-xs">
          <div className="bg-slate-900/60 px-3 py-1.5 rounded-lg border border-slate-700 font-mono text-slate-300">
            Total Eventos: <strong className="text-white">{operations.length}</strong>
          </div>
        </div>
      </div>

      {/* Filter Toolbar */}
      <div className="flex flex-wrap items-center justify-between gap-3 bg-slate-900/50 p-3 rounded-xl border border-slate-700/60 text-xs">
        <div className="flex flex-wrap items-center gap-3">
          {/* Action Filter */}
          <div className="flex items-center gap-1.5">
            <Filter className="w-3.5 h-3.5 text-slate-400" />
            <span className="text-slate-400 font-medium">Acción:</span>
            <select
              value={actionFilter}
              onChange={e => setActionFilter(e.target.value)}
              className="bg-slate-800 border border-slate-700 text-slate-200 rounded-lg px-2.5 py-1 focus:outline-none focus:border-indigo-500"
            >
              <option value="ALL">Todas las acciones</option>
              <option value="CREATED">Creado</option>
              <option value="MODIFIED">Modificado</option>
              <option value="DELETED">Eliminado</option>
              <option value="RENAMED">Renombrado</option>
            </select>
          </div>

          {/* Extension Filter */}
          <div className="flex items-center gap-1.5">
            <span className="text-slate-400 font-medium">Extensión:</span>
            <select
              value={extFilter}
              onChange={e => setExtFilter(e.target.value)}
              className="bg-slate-800 border border-slate-700 text-slate-200 rounded-lg px-2.5 py-1 focus:outline-none focus:border-indigo-500"
            >
              <option value="ALL">Todas (.docx, .xlsx, .rs, etc.)</option>
              <option value="docx">.docx</option>
              <option value="xlsx">.xlsx</option>
              <option value="rs">.rs</option>
              <option value="pdf">.pdf</option>
              {availableExtensions
                .filter(ext => !['docx', 'xlsx', 'rs', 'pdf'].includes(ext))
                .map(ext => (
                  <option key={ext} value={ext}>
                    .{ext}
                  </option>
                ))}
            </select>
          </div>
        </div>

        {/* Search Input */}
        <div className="relative">
          <input
            type="text"
            placeholder="Buscar ruta o archivo..."
            value={searchQuery}
            onChange={e => setSearchQuery(e.target.value)}
            className="bg-slate-800 border border-slate-700 text-slate-200 rounded-lg pl-8 pr-3 py-1 text-xs placeholder-slate-500 focus:outline-none focus:border-indigo-500 w-48 sm:w-64"
          />
          <Search className="w-3.5 h-3.5 text-slate-400 absolute left-2.5 top-1/2 -translate-y-1/2" />
        </div>
      </div>

      {/* Content Table / Loading / Empty State */}
      {loading ? (
        <div className="h-64 w-full flex flex-col items-center justify-center bg-slate-900/40 rounded-xl border border-slate-700/50">
          <div className="w-8 h-8 border-2 border-indigo-500 border-t-transparent rounded-full animate-spin"></div>
          <p className="text-xs text-slate-400 mt-3 font-mono">Descifrando actividad de archivos en RAM...</p>
        </div>
      ) : filteredOps.length === 0 ? (
        <div className="bg-slate-900/50 border border-slate-700/60 rounded-xl p-12 text-center space-y-3">
          <div className="w-12 h-12 rounded-full bg-slate-800 text-slate-400 flex items-center justify-center mx-auto">
            <AlertCircle className="w-6 h-6 text-indigo-400" />
          </div>
          <h3 className="text-base font-semibold text-white">Sin Operaciones de Archivos</h3>
          <p className="text-xs text-slate-400 max-w-md mx-auto leading-relaxed">
            No se detectaron eventos sobre carpetas corporativas para{' '}
            <strong className="text-slate-200">{workerName}</strong> con los filtros actuales.
          </p>
        </div>
      ) : (
        <div className="space-y-2">
          <div className="text-[11px] text-slate-400 flex justify-between items-center px-1">
            <span>Mostrando {filteredOps.length} operaciones registradas</span>
            <span className="font-mono">Solo Metadatos (Cero acceso al contenido)</span>
          </div>

          <div className="border border-slate-700/70 rounded-xl overflow-hidden shadow-sm">
            <table className="w-full text-left text-xs text-slate-300">
              <thead className="bg-slate-900 text-slate-400 uppercase text-[10px] tracking-wider border-b border-slate-700">
                <tr>
                  <th className="px-4 py-3">Hora</th>
                  <th className="px-4 py-3">Acción</th>
                  <th className="px-4 py-3">Directorio Designado</th>
                  <th className="px-4 py-3">Ruta Relativa del Archivo</th>
                  <th className="px-4 py-3 text-right">Extensión</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-slate-800 bg-slate-900/40">
                {filteredOps.map(op => (
                  <tr key={op.id} className="hover:bg-slate-800/40 transition">
                    <td className="px-4 py-3 font-mono text-indigo-300 whitespace-nowrap">
                      {op.time}
                    </td>
                    <td className="px-4 py-3 whitespace-nowrap">
                      {getActionBadge(op.action)}
                    </td>
                    <td className="px-4 py-3 whitespace-nowrap">
                      <span className="px-2 py-0.5 rounded bg-slate-800 text-slate-300 font-mono text-[11px] border border-slate-700 inline-flex items-center gap-1">
                        <FolderOpen className="w-3 h-3 text-amber-400" />
                        {op.watchedRoot}
                      </span>
                    </td>
                    <td className="px-4 py-3 font-mono text-slate-200">
                      {op.action === 'RENAMED' && op.previousRelativePath ? (
                        <div className="flex items-center gap-1.5 flex-wrap">
                          <span className="line-through text-slate-500">
                            {op.previousRelativePath}
                          </span>
                          <span className="text-amber-400 font-bold">&rarr;</span>
                          <span className="text-slate-100 font-medium">
                            {op.relativePath}
                          </span>
                        </div>
                      ) : (
                        <span>{op.relativePath}</span>
                      )}
                    </td>
                    <td className="px-4 py-3 text-right whitespace-nowrap font-mono text-slate-400">
                      <span className="bg-slate-800 px-2 py-0.5 rounded border border-slate-700 text-[10px] uppercase">
                        .{op.extension}
                      </span>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      )}
    </div>
  );
};
