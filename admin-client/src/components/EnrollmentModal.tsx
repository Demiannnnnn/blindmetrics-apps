import React, { useEffect, useRef, useState } from 'react';
import { Copy, Check, X, Shield, Laptop, AlertTriangle, QrCode, Key, Clock, Download } from 'lucide-react';
import { QRCodeSVG } from 'qrcode.react';
import type { EnrollmentView } from '../api/tauri-bridge';
import { displayUrl } from '../lib/subscription';

interface EnrollmentModalProps {
  worker: { name: string; role?: string };
  enrollment: EnrollmentView;
  onClose: () => void;
}

function formatRemaining(ms: number): string {
  if (ms <= 0) return 'Expirado';
  const totalMin = Math.floor(ms / 60000);
  const h = Math.floor(totalMin / 60);
  const m = totalMin % 60;
  return h > 0 ? `${h} h ${m} min` : `${m} min`;
}

/**
 * W3 — one-time enrollment view (RF-ADMIN-08-4). The code lives only in this
 * component's props; closing the modal discards it (SEC-05).
 */
export const EnrollmentModal: React.FC<EnrollmentModalProps> = ({ worker, enrollment, onClose }) => {
  const [copiedCode, setCopiedCode] = useState(false);
  const [copiedPayload, setCopiedPayload] = useState(false);
  const [now, setNow] = useState(() => Date.now());
  const qrRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const t = setInterval(() => setNow(Date.now()), 30000);
    return () => clearInterval(t);
  }, []);

  const raw = enrollment.code.replace(/[-\s]/g, '');
  const formattedCode = raw.length === 12 ? `${raw.slice(0, 4)}-${raw.slice(4, 8)}-${raw.slice(8, 12)}` : enrollment.code;
  const expiresAt = new Date(enrollment.expires_at).getTime();
  const remainingMs = Number.isNaN(expiresAt) ? 0 : expiresAt - now;

  const handleCopyCode = () => {
    navigator.clipboard.writeText(formattedCode);
    setCopiedCode(true);
    setTimeout(() => setCopiedCode(false), 2500);
  };

  const handleCopyPayload = () => {
    navigator.clipboard.writeText(enrollment.qr_payload);
    setCopiedPayload(true);
    setTimeout(() => setCopiedPayload(false), 2500);
  };

  const handleSavePng = () => {
    const svg = qrRef.current?.querySelector('svg');
    if (!svg) return;
    const size = 512;
    const img = new Image();
    img.onload = () => {
      const canvas = document.createElement('canvas');
      canvas.width = size;
      canvas.height = size;
      const ctx = canvas.getContext('2d');
      if (!ctx) return;
      ctx.fillStyle = '#ffffff';
      ctx.fillRect(0, 0, size, size);
      ctx.drawImage(img, 32, 32, size - 64, size - 64);
      const a = document.createElement('a');
      a.href = canvas.toDataURL('image/png');
      a.download = `enrolamiento-${worker.name.replace(/\s+/g, '_')}.png`;
 a.click();
 };
 img.src = `data:image/svg+xml;charset=utf-8,${encodeURIComponent(new XMLSerializer().serializeToString(svg))}`;
 };

 return (
 <div className="fixed inset-0 z-50 bg-bg flex items-center justify-center p-4">
 <div className="bg-surface border border-default rounded-card max-w-xl w-full p-6 shadow-md relative max-h-[90vh] overflow-y-auto">
 <button
 onClick={onClose}
 className="absolute top-4 right-4 text-muted hover:text-fg p-1 rounded-control hover:bg-surface-subtle transition"
 >
 <X className="w-5 h-5" />
 </button>

 <div className="flex items-center gap-3 mb-4">
 <div className="w-10 h-10 rounded-card bg-accent-subtle border border-default flex items-center justify-center text-accent">
 <QrCode className="w-5 h-5" />
 </div>
 <div>
 <h3 className="text-lg font-bold text-fg">Vinculación de Dispositivo Worker</h3>
 <p className="text-xs text-muted">
 Trabajador: <span className="text-fg font-medium">{worker.name}</span>
 {'role'in worker && worker.role && ` (${worker.role})`}
 </p>
 </div>
 </div>

 {/* Security Ephemeral Notice */}
 <div className="bg-warning-subtle border border-default rounded-card p-3 mb-4 flex items-start gap-2.5 text-xs text-warning">
 <AlertTriangle className="w-4 h-4 mt-0.5 flex-shrink-0 text-warning" />
 <div>
 <p className="font-semibold text-warning">Código de un solo uso (SEC-05)</p>
 <p className="text-warning mt-0.5">
 Por razones de seguridad, este código no se guarda en el servidor ni en este equipo.
 Se muestra una sola vez. Al cerrar este diálogo, solo podrá generarse un nuevo token.
 </p>
 </div>
 </div>

 <div className="space-y-4">
 {/* QR Code Container */}
 <div className="flex flex-col items-center justify-center p-4 bg-bg border border-default rounded-card">
 <div className="p-3 bg-surface rounded-card ">
 <QRCodeSVG
 value={enrollment.qr_payload}
 size={180}
 level="M"
 includeMargin={false}
 />
 </div>
 <p className="text-xs text-muted mt-2.5 flex items-center gap-1.5">
 <Laptop className="w-3.5 h-3.5 text-accent" />
 En la app Telemetry Worker: "Código QR" (escanear o pegar su contenido) o "Ingresar código"
 </p>
 {enrollment.api_url && (
 <p className="text-[11px] text-muted mt-1">
 Servidor: <span className="font-mono text-fg">{displayUrl(enrollment.api_url)}</span>
 </p>
 )}
 <p className="text-[11px] text-success mt-1">
 Verificado con la llave de este equipo: el servidor no puede haber sustituido la llave.
 </p>
 </div>

 {/* Manual Code & Safety Code */}
 <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
 {/* 12-char Crockford code */}
 <div className="bg-bg border border-default rounded-card p-3">
 <span className="text-[11px] font-medium text-muted flex items-center gap-1">
 <Key className="w-3 h-3 text-accent" />
 Código de Enrolamiento
 </span>
 <div className="flex items-center justify-between mt-1">
 <span className="font-mono text-sm font-bold text-accent tracking-wider">
 {formattedCode}
 </span>
 <button
 onClick={handleCopyCode}
 className="p-1.5 text-muted hover:text-fg rounded-control hover:bg-surface-subtle transition"
 title="Copiar código"
 >
 {copiedCode ? <Check className="w-4 h-4 text-success" /> : <Copy className="w-4 h-4" />}
 </button>
 </div>
 </div>

 {/* 12-digit Safety Code */}
 <div className="bg-bg border border-default rounded-card p-3">
 <span className="text-[11px] font-medium text-muted flex items-center gap-1">
 <Shield className="w-3 h-3 text-success" />
 Safety Code (12 dígitos)
 </span>
 <div className="mt-1">
 <span className="font-mono text-sm font-bold text-success tracking-wider">
 {enrollment.safety_code}
 </span>
 <p className="text-[10px] text-muted mt-0.5">
 Si el trabajador escribe el código, léale estos dígitos y confirme que coinciden con su pantalla
 </p>
 </div>
 </div>
 </div>

 {/* Expiration date */}
 {enrollment.expires_at && (
 <div className="flex items-center gap-1.5 text-xs text-muted px-1">
 <Clock className="w-3.5 h-3.5 text-muted" />
 <span>
 Expira el: <span className="text-fg font-medium">
 {new Date(enrollment.expires_at).toLocaleString()}
 </span>{' '}
                <span className={remainingMs < 2 * 3600 * 1000 ? 'text-warning' : 'text-muted'}>
 ({formatRemaining(remainingMs)})
 </span>
 </span>
 </div>
 )}
 </div>

 <div className="flex justify-between items-center gap-3 mt-5 pt-3 border-t border-default">
 <button
 onClick={handleCopyPayload}
 className="text-xs text-muted hover:text-fg flex items-center gap-1.5 transition"
 >
 <Copy className="w-3.5 h-3.5" />
 <span>{copiedPayload ?'¡Contenido copiado!' : 'Copiar contenido del QR'}</span>
          </button>
          <button
            onClick={handleSavePng}
            className="text-xs text-muted hover:text-fg flex items-center gap-1.5 transition"
          >
            <Download className="w-3.5 h-3.5" />
            <span>Guardar PNG</span>
          </button>
          <div className="flex-1" />
          <button
            onClick={onClose}
            className="px-4 py-2 bg-accent hover:bg-accent-hover text-accent-fg text-xs font-semibold rounded-control transition"
          >
            Entendido, cerrar
          </button>
        </div>
      </div>
    </div>
  );
};
