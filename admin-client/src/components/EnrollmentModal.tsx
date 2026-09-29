import React, { useState } from 'react';
import { Copy, Check, X, Shield, Laptop, AlertTriangle, QrCode, Key, Clock } from 'lucide-react';
import { QRCodeSVG } from 'qrcode.react';
import type { EnrollmentView, EnrollmentPackage, WorkerItem } from '../api/tauri-bridge';

interface EnrollmentModalProps {
  worker: WorkerItem | { name: string; role?: string; id?: string };
  enrollment: EnrollmentView | EnrollmentPackage | any;
  onClose: () => void;
}

export const EnrollmentModal: React.FC<EnrollmentModalProps> = ({ worker, enrollment, onClose }) => {
  const [copiedCode, setCopiedCode] = useState(false);
  const [copiedPayload, setCopiedPayload] = useState(false);

  const isView = 'qr_payload' in enrollment && 'code' in enrollment;

  const rawCode = isView ? enrollment.code : (enrollment as EnrollmentPackage).worker_id;
  const formattedCode = rawCode.includes('-')
    ? rawCode
    : rawCode.length === 12
    ? `${rawCode.slice(0, 4)}-${rawCode.slice(4, 8)}-${rawCode.slice(8, 12)}`
    : rawCode;

  const handleCopyCode = () => {
    navigator.clipboard.writeText(formattedCode);
    setCopiedCode(true);
    setTimeout(() => setCopiedCode(false), 2500);
  };

  const handleCopyPayload = () => {
    const payload = isView ? enrollment.qr_payload : JSON.stringify(enrollment, null, 2);
    navigator.clipboard.writeText(payload);
    setCopiedPayload(true);
    setTimeout(() => setCopiedPayload(false), 2500);
  };

  return (
    <div className="fixed inset-0 z-50 bg-slate-950/80 backdrop-blur-sm flex items-center justify-center p-4">
      <div className="bg-slate-900 border border-slate-800 rounded-2xl max-w-xl w-full p-6 shadow-2xl relative max-h-[90vh] overflow-y-auto">
        <button
          onClick={onClose}
          className="absolute top-4 right-4 text-slate-400 hover:text-white p-1 rounded-lg hover:bg-slate-800 transition"
        >
          <X className="w-5 h-5" />
        </button>

        <div className="flex items-center gap-3 mb-4">
          <div className="w-10 h-10 rounded-xl bg-indigo-500/10 border border-indigo-500/30 flex items-center justify-center text-indigo-400">
            <QrCode className="w-5 h-5" />
          </div>
          <div>
            <h3 className="text-lg font-bold text-white">Vinculación de Dispositivo Worker</h3>
            <p className="text-xs text-slate-400">
              Trabajador: <span className="text-white font-medium">{worker.name}</span>
              {'role' in worker && worker.role && ` (${worker.role})`}
            </p>
          </div>
        </div>

        {/* Security Ephemeral Notice */}
        <div className="bg-amber-500/10 border border-amber-500/30 rounded-xl p-3 mb-4 flex items-start gap-2.5 text-xs text-amber-300">
          <AlertTriangle className="w-4 h-4 mt-0.5 flex-shrink-0 text-amber-400" />
          <div>
            <p className="font-semibold text-amber-200">Código de un solo uso (SEC-05)</p>
            <p className="text-amber-300/90 mt-0.5">
              Por razones de seguridad, este código no se guarda en el servidor ni en este equipo.
              Se muestra una sola vez. Al cerrar este diálogo, solo podrá generarse un nuevo token.
            </p>
          </div>
        </div>

        {isView ? (
          <div className="space-y-4">
            {/* QR Code Container */}
            <div className="flex flex-col items-center justify-center p-4 bg-slate-950/80 border border-slate-800 rounded-xl">
              <div className="p-3 bg-white rounded-xl shadow-lg">
                <QRCodeSVG
                  value={enrollment.qr_payload}
                  size={180}
                  level="M"
                  includeMargin={false}
                />
              </div>
              <p className="text-xs text-slate-400 mt-2.5 flex items-center gap-1.5">
                <Laptop className="w-3.5 h-3.5 text-indigo-400" />
                Escanee este código QR desde la app Telemetry en el equipo del trabajador
              </p>
            </div>

            {/* Manual Code & Safety Code */}
            <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
              {/* 12-char Crockford code */}
              <div className="bg-slate-950/70 border border-slate-800 rounded-xl p-3">
                <span className="text-[11px] font-medium text-slate-400 flex items-center gap-1">
                  <Key className="w-3 h-3 text-indigo-400" />
                  Código de Enrolamiento
                </span>
                <div className="flex items-center justify-between mt-1">
                  <span className="font-mono text-sm font-bold text-indigo-300 tracking-wider">
                    {formattedCode}
                  </span>
                  <button
                    onClick={handleCopyCode}
                    className="p-1.5 text-slate-400 hover:text-white rounded-lg hover:bg-slate-800 transition"
                    title="Copiar código"
                  >
                    {copiedCode ? <Check className="w-4 h-4 text-emerald-400" /> : <Copy className="w-4 h-4" />}
                  </button>
                </div>
              </div>

              {/* 12-digit Safety Code */}
              <div className="bg-slate-950/70 border border-slate-800 rounded-xl p-3">
                <span className="text-[11px] font-medium text-slate-400 flex items-center gap-1">
                  <Shield className="w-3 h-3 text-emerald-400" />
                  Safety Code (12 dígitos)
                </span>
                <div className="mt-1">
                  <span className="font-mono text-sm font-bold text-emerald-400 tracking-wider">
                    {enrollment.safety_code}
                  </span>
                  <p className="text-[10px] text-slate-500 mt-0.5">
                    Verifique que coincida en la pantalla del trabajador
                  </p>
                </div>
              </div>
            </div>

            {/* Expiration date */}
            {enrollment.expires_at && (
              <div className="flex items-center gap-1.5 text-xs text-slate-400 px-1">
                <Clock className="w-3.5 h-3.5 text-slate-500" />
                <span>
                  Expira el: <span className="text-slate-300 font-medium">
                    {new Date(enrollment.expires_at).toLocaleString()}
                  </span>
                </span>
              </div>
            )}
          </div>
        ) : (
          /* Fallback for legacy packages */
          <div className="relative mb-4">
            <pre className="p-4 bg-slate-950 border border-slate-800 rounded-xl font-mono text-xs text-emerald-400 overflow-x-auto max-h-56 leading-relaxed select-all">
              {JSON.stringify(enrollment, null, 2)}
            </pre>
            <button
              onClick={handleCopyPayload}
              className="absolute top-3 right-3 px-3 py-1.5 rounded-lg text-xs font-medium flex items-center gap-1.5 bg-indigo-600 hover:bg-indigo-500 text-white shadow-md"
            >
              {copiedPayload ? <Check className="w-3.5 h-3.5" /> : <Copy className="w-3.5 h-3.5" />}
              <span>{copiedPayload ? '¡Copiado!' : 'Copiar JSON'}</span>
            </button>
          </div>
        )}

        <div className="flex justify-between items-center gap-3 mt-5 pt-3 border-t border-slate-800">
          {isView && (
            <button
              onClick={handleCopyPayload}
              className="text-xs text-slate-400 hover:text-slate-200 flex items-center gap-1.5 transition"
            >
              <Copy className="w-3.5 h-3.5" />
              <span>{copiedPayload ? '¡Payload Copiado!' : 'Copiar Payload QR'}</span>
            </button>
          )}
          <div className="flex-1" />
          <button
            onClick={onClose}
            className="px-4 py-2 bg-indigo-600 hover:bg-indigo-500 text-white text-xs font-semibold rounded-lg transition"
          >
            Entendido, cerrar
          </button>
        </div>
      </div>
    </div>
  );
};
