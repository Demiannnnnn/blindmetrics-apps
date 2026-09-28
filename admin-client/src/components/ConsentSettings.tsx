import React from 'react';
import { ShieldCheck, Lock, FileText, UserCheck } from 'lucide-react';

export const ConsentSettings: React.FC = () => {
  return (
    <div className="bg-slate-800/80 border border-slate-700/80 rounded-xl p-5 shadow-lg space-y-6">
      <div>
        <h2 className="text-lg font-semibold text-white flex items-center gap-2">
          <ShieldCheck className="w-5 h-5 text-indigo-400" />
          Políticas de Privacidad, Consentimientos & Cumplimiento Normativo
        </h2>
        <p className="text-xs text-slate-400">Parámetros corporativos auditables según GDPR y Ley Chilena 19.628 / 21.719</p>
      </div>

      <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
        <div className="p-4 bg-slate-900/50 rounded-xl border border-slate-700/60 space-y-2">
          <div className="flex items-center gap-2 text-indigo-400 text-sm font-semibold">
            <Lock className="w-4 h-4" />
            Zero-Knowledge Cryptographic Core
          </div>
          <p className="text-xs text-slate-300 leading-relaxed">
            El servidor en PostgreSQL almacena únicamente blobs binarios cifrados. La clave privada maestra reside exclusivamente en el OS Keystore de esta estación de trabajo.
          </p>
        </div>

        <div className="p-4 bg-slate-900/50 rounded-xl border border-slate-700/60 space-y-2">
          <div className="flex items-center gap-2 text-emerald-400 text-sm font-semibold">
            <UserCheck className="w-4 h-4" />
            Soberanía y Revocación en 1-Clic
          </div>
          <p className="text-xs text-slate-300 leading-relaxed">
            Los trabajadores conservan el derecho inalienable de desactivar cualquiera de sus consentimientos desde su interfaz egui en tiempo real.
          </p>
        </div>

        <div className="p-4 bg-slate-900/50 rounded-xl border border-slate-700/60 space-y-2">
          <div className="flex items-center gap-2 text-sky-400 text-sm font-semibold">
            <FileText className="w-4 h-4" />
            Minimización de Red & Archivos
          </div>
          <p className="text-xs text-slate-300 leading-relaxed">
            El agente jamás recolecta URLs completas, argumentos GET o cuerpos de peticiones HTTP. La auditoría de archivos monitorea únicamente metadatos (crear, renombrar, eliminar).
          </p>
        </div>

        <div className="p-4 bg-slate-900/50 rounded-xl border border-slate-700/60 space-y-2">
          <div className="flex items-center gap-2 text-amber-400 text-sm font-semibold">
            <ShieldCheck className="w-4 h-4" />
            Geolocalización FIELD Estricta
          </div>
          <p className="text-xs text-slate-300 leading-relaxed">
            El hardware GPS solo se activa si el trabajador es de campo y se encuentra dentro de su franja horaria pactada. Fuera de turno, se suspende automáticamente por software.
          </p>
        </div>
      </div>
    </div>
  );
};
