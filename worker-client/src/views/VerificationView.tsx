import React, { useState } from 'react';
import { Copy, Check, ShieldCheck } from 'lucide-react';
import { useApp } from '../context/AppContext';
import { formatSafetyCode } from '../lib/helpers';
import * as bridge from '../api/tauri-bridge';

export const VerificationView: React.FC = () => {
  const { safetyCode, setEnrollmentStep, refreshSnapshot } = useApp();
  const [copied, setCopied] = useState<boolean>(false);
  const [loading, setLoading] = useState<boolean>(false);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);

  const formattedCode = safetyCode ? formatSafetyCode(safetyCode) : '---- ---- ----';

  const handleCopy = async () => {
    if (!safetyCode) return;
    try {
      await navigator.clipboard.writeText(formattedCode);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch (_e) {
      // ignore
    }
  };

  const handleConfirm = async (matches: boolean) => {
    setLoading(true);
    setErrorMsg(null);
    try {
      await bridge.verifySafetyCode(matches);
      if (matches) {
        await refreshSnapshot();
      } else {
        setEnrollmentStep('tamper');
      }
    } catch (err: unknown) {
      if (!matches) {
        setEnrollmentStep('tamper');
      } else {
        const msg =
          err && typeof err === 'object' && 'message' in err
            ? String(err.message)
            : 'Error en la verificación del código';
        setErrorMsg(msg);
      }
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="h-full w-full flex flex-col items-center justify-center p-6 select-none">
      <div className="w-full max-w-sm bg-surface border border-default rounded-card p-6 shadow-sm text-center">
        <div className="w-10 h-10 rounded-full bg-blue-100 dark:bg-blue-950/60 flex items-center justify-center mx-auto mb-3">
          <ShieldCheck className="w-5 h-5 text-accent" />
        </div>

        <h1 className="text-base font-semibold text-fg mb-1">
          Código de seguridad
        </h1>

        <p className="text-xs text-muted mb-4">
          Compara este código con el que ve tu empleador en su pantalla.
        </p>

        <div className="bg-surface-subtle border border-default rounded-control p-3 mb-4 flex items-center justify-between">
          <span className="font-mono text-base font-semibold tracking-wider text-fg mx-auto">
            {formattedCode}
          </span>
          <button
            type="button"
            onClick={handleCopy}
            className="p-1.5 rounded text-muted hover:text-fg hover:bg-surface transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent ml-2"
            title="Copiar código"
            aria-label="Copiar código de seguridad"
          >
            {copied ? <Check className="w-4 h-4 text-emerald-600" /> : <Copy className="w-4 h-4" />}
          </button>
        </div>

        <div aria-live="polite" className="sr-only">
          {copied ? 'Código de seguridad copiado al portapapeles' : ''}
        </div>

        {errorMsg && (
          <div
            role="alert"
            aria-live="polite"
            className="p-2.5 mb-4 bg-red-50 dark:bg-red-950/40 border border-red-200 dark:border-red-800/60 rounded-control text-xs text-red-900 dark:text-red-200 text-left"
          >
            {errorMsg}
          </div>
        )}

        <div className="space-y-2">
          <button
            type="button"
            onClick={() => handleConfirm(true)}
            disabled={loading}
            className="w-full h-9 rounded-control bg-accent text-accent-fg hover:bg-accent-hover font-medium text-sm transition-colors disabled:opacity-50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent"
          >
            {loading ? 'Verificando…' : 'Coinciden exactamente'}
          </button>

          <button
            type="button"
            onClick={() => handleConfirm(false)}
            disabled={loading}
            className="w-full h-9 rounded-control border border-danger/40 text-danger hover:bg-red-50 dark:hover:bg-red-950/30 font-medium text-xs transition-colors disabled:opacity-50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent"
          >
            No coinciden
          </button>
        </div>
      </div>
    </div>
  );
};
