import React, { useState } from 'react';
import { Copy, Check, X, Shield, Laptop } from 'lucide-react';
import { EnrollmentPackage, WorkerItem } from '../api/tauri-bridge';

interface EnrollmentModalProps {
  worker: WorkerItem;
  enrollment: EnrollmentPackage;
  onClose: () => void;
}

export const EnrollmentModal: React.FC<EnrollmentModalProps> = ({ worker, enrollment, onClose }) => {
  const [copied, setCopied] = useState(false);
  const jsonString = JSON.stringify(enrollment, null, 2);

  const handleCopy = () => {
    navigator.clipboard.writeText(jsonString);
    setCopied(true);
    setTimeout(() => setCopied(false), 2500);
  };

  return (
    <div className="fixed inset-0 z-50 bg-slate-950/80 backdrop-blur-sm flex items-center justify-center p-4">
      <div className="bg-slate-900 border border-slate-800 rounded-2xl max-w-xl w-full p-6 shadow-2xl relative">
        <button
          onClick={onClose}
          className="absolute top-4 right-4 text-slate-400 hover:text-white p-1 rounded-lg hover:bg-slate-800 transition"
        >
          <X className="w-5 h-5" />
        </button>

        <div className="flex items-center gap-3 mb-4">
          <div className="w-10 h-10 rounded-xl bg-emerald-500/10 border border-emerald-500/30 flex items-center justify-center text-emerald-400">
            <Shield className="w-5 h-5" />
          </div>
          <div>
            <h3 className="text-lg font-bold text-white">Paquete de Enrolamiento Criptográfico</h3>
            <p className="text-xs text-slate-400">
              Generado para: <span className="text-white font-medium">{worker.name}</span> ({worker.role})
            </p>
          </div>
        </div>

        <div className="bg-slate-950/80 border border-slate-800/80 rounded-xl p-3.5 mb-4 text-xs text-slate-300 space-y-2">
          <div className="flex items-center gap-2 text-indigo-400 font-semibold">
            <Laptop className="w-4 h-4" />
            <span>Instrucciones para vincular el Agente Worker:</span>
          </div>
          <ol className="list-decimal list-inside space-y-1 text-slate-400 pl-1">
            <li>Abra la aplicación <strong>Telemetry Worker</strong> en el equipo del empleado.</li>
            <li>En la sección <strong>"Estado Criptográfico Zero-Knowledge"</strong>, pegue el paquete JSON.</li>
            <li>Haga clic en <strong>"Aplicar Paquete"</strong>. Las métricas quedarán cifradas con esta clave.</li>
          </ol>
        </div>

        <div className="relative mb-5">
          <pre className="p-4 bg-slate-950 border border-slate-800 rounded-xl font-mono text-xs text-emerald-400 overflow-x-auto max-h-56 leading-relaxed select-all">
            {jsonString}
          </pre>
          <button
            onClick={handleCopy}
            className={`absolute top-3 right-3 px-3 py-1.5 rounded-lg text-xs font-medium flex items-center gap-1.5 transition-all shadow-md ${
              copied
                ? 'bg-emerald-600 text-white'
                : 'bg-indigo-600 hover:bg-indigo-500 text-white'
            }`}
          >
            {copied ? (
              <>
                <Check className="w-3.5 h-3.5" />
                <span>¡Copiado!</span>
              </>
            ) : (
              <>
                <Copy className="w-3.5 h-3.5" />
                <span>Copiar JSON</span>
              </>
            )}
          </button>
        </div>

        <div className="flex justify-end gap-3">
          <button
            onClick={onClose}
            className="px-4 py-2 bg-slate-800 hover:bg-slate-700 text-slate-200 text-xs font-medium rounded-lg transition"
          >
            Cerrar
          </button>
        </div>
      </div>
    </div>
  );
};
