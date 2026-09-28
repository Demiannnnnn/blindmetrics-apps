import React from 'react';
import { WorkerItem } from '../api/tauri-bridge';
import { Shield, Clock, MapPin, Eye, Power, QrCode } from 'lucide-react';

interface EmployeeCardProps {
  worker: WorkerItem;
  onSelect: (worker: WorkerItem) => void;
  onToggleStatus: (id: string) => void;
  onShowEnrollment: (worker: WorkerItem) => void;
}

export const EmployeeCard: React.FC<EmployeeCardProps> = ({
  worker,
  onSelect,
  onToggleStatus,
  onShowEnrollment,
}) => {
  return (
    <div className="bg-slate-800/80 border border-slate-700/80 rounded-xl p-5 shadow-lg hover:border-indigo-500/50 transition-all flex flex-col justify-between">
      <div>
        <div className="flex justify-between items-start mb-3">
          <div>
            <h3 className="text-base font-semibold text-white flex items-center gap-2">
              {worker.name}
            </h3>
            <p className="text-xs text-slate-400 font-mono mt-0.5">{worker.email}</p>
          </div>
          <span
            className={`px-2.5 py-1 text-xs font-semibold rounded-full flex items-center gap-1 ${
              worker.isActive
                ? 'bg-emerald-500/20 text-emerald-400 border border-emerald-500/30'
                : 'bg-rose-500/20 text-rose-400 border border-rose-500/30'
            }`}
          >
            <span className={`w-1.5 h-1.5 rounded-full ${worker.isActive ? 'bg-emerald-400 animate-pulse' : 'bg-rose-400'}`}></span>
            {worker.isActive ? 'Activo' : 'Inactivo'}
          </span>
        </div>

        {/* Roles and Timezone */}
        <div className="flex flex-wrap gap-2 text-xs mb-4">
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
        </div>

        {/* Consent Badges */}
        <div className="border-t border-slate-700/60 pt-3 mb-4">
          <p className="text-xs font-medium text-slate-400 mb-2">Consentimientos Otorgados:</p>
          <div className="grid grid-cols-3 gap-1.5 text-[11px]">
            <span
              className={`p-1 rounded text-center truncate ${
                worker.consents.system_activity ? 'bg-indigo-950 text-indigo-300 border border-indigo-800' : 'bg-slate-900/60 text-slate-500'
              }`}
            >
              Sistema
            </span>
            <span
              className={`p-1 rounded text-center truncate ${
                worker.consents.network_activity ? 'bg-indigo-950 text-indigo-300 border border-indigo-800' : 'bg-slate-900/60 text-slate-500'
              }`}
            >
              Red (FQDN)
            </span>
            <span
              className={`p-1 rounded text-center truncate ${
                worker.consents.productivity_basic ? 'bg-indigo-950 text-indigo-300 border border-indigo-800' : 'bg-slate-900/60 text-slate-500'
              }`}
            >
              Teclado/Clic
            </span>
            <span
              className={`p-1 rounded text-center truncate ${
                worker.consents.productivity_screenshots ? 'bg-indigo-950 text-indigo-300 border border-indigo-800' : 'bg-slate-900/60 text-slate-500'
              }`}
            >
              Capturas
            </span>
            <span
              className={`p-1 rounded text-center truncate ${
                worker.consents.file_activity ? 'bg-indigo-950 text-indigo-300 border border-indigo-800' : 'bg-slate-900/60 text-slate-500'
              }`}
            >
              Archivos
            </span>
            <span
              className={`p-1 rounded text-center truncate ${
                worker.consents.location
                  ? 'bg-amber-950 text-amber-300 border border-amber-800'
                  : 'bg-slate-900/60 text-slate-500'
              }`}
            >
              {worker.role === 'FIELD' ? 'GPS (Campo)' : 'GPS (N/A)'}
            </span>
          </div>
        </div>
      </div>

      {/* Action Buttons */}
      <div className="flex gap-2 pt-2 border-t border-slate-700/60">
        <button
          onClick={() => onSelect(worker)}
          className="flex-1 bg-indigo-600 hover:bg-indigo-500 text-white text-xs font-medium py-2 px-3 rounded-lg flex items-center justify-center gap-1.5 transition"
        >
          <Eye className="w-3.5 h-3.5" />
          Ver Telemetría
        </button>
        <button
          onClick={() => onShowEnrollment(worker)}
          className="bg-slate-700 hover:bg-slate-600 text-slate-200 p-2 rounded-lg text-xs transition"
          title="Ver paquete de enrolamiento (JSON / QR)"
        >
          <QrCode className="w-4 h-4" />
        </button>
        <button
          onClick={() => onToggleStatus(worker.id)}
          className={`p-2 rounded-lg text-xs transition ${
            worker.isActive
              ? 'bg-rose-950/60 hover:bg-rose-900 text-rose-300 border border-rose-800/40'
              : 'bg-emerald-950/60 hover:bg-emerald-900 text-emerald-300 border border-emerald-800/40'
          }`}
          title={worker.isActive ? 'Desactivar Trabajador' : 'Activar Trabajador'}
        >
          <Power className="w-4 h-4" />
        </button>
      </div>
    </div>
  );
};
