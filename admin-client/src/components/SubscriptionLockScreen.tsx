import React, { useEffect, useState } from 'react';
import { ExternalLink, Copy, Check, RefreshCw, LogOut } from 'lucide-react';
import type { SubscriptionStatus } from '../api/types';
import { openBillingPortal } from '../api/tauri-bridge';
import { describeError } from '../lib/errors';
import { RECHECK_INTERVAL_MS, displayUrl, isSafePortalUrl, lockScreenCopy } from '../lib/subscription';

interface SubscriptionLockScreenProps {
  status: Pick<SubscriptionStatus, 'effective_status' | 'reason' | 'portal_url' | 'support_email'>;
  onRecheck: () => Promise<void>;
  onLogout: () => void;
}

/**
 * L2 — replaces the whole console while the subscription is suspended or cancelled
 * (TASK-ADMIN-12 = TASK-APP-01, BILL-02). Payment happens only in the web portal.
 */
export const SubscriptionLockScreen: React.FC<SubscriptionLockScreenProps> = ({ status, onRecheck, onLogout }) => {
  const [checking, setChecking] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const copy = lockScreenCopy(status.effective_status, status.reason);
  const portalOk = isSafePortalUrl(status.portal_url);

  const recheck = async () => {
    setChecking(true);
    setError(null);
    try {
      await onRecheck();
    } catch (e) {
      setError(describeError(e, 'No se pudo verificar la suscripción.'));
    } finally {
      setChecking(false);
    }
  };

  // RF-ADMIN-12-5: activation arrives through Flow's webhook; re-check every minute.
  useEffect(() => {
    const timer = setInterval(() => {
      onRecheck().catch(() => undefined);
    }, RECHECK_INTERVAL_MS);
    return () => clearInterval(timer);
  }, [onRecheck]);

  const openPortal = async () => {
    setError(null);
    try {
      await openBillingPortal();
    } catch (e) {
      setError(describeError(e));
    }
  };

  const copyEmail = () => {
    if (!status.support_email) return;
    navigator.clipboard.writeText(status.support_email);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div className="min-h-screen bg-slate-950 flex items-center justify-center p-4 text-slate-100">
      <div className="max-w-md w-full bg-slate-900 border border-amber-500/30 rounded-2xl p-8 shadow-2xl space-y-5">
        <div>
          <h1 className="text-xl font-bold text-white">{copy.title}</h1>
          <p className="text-sm text-slate-300 mt-2">{copy.body}</p>
        </div>

        <div className="space-y-2">
          <p className="text-sm text-slate-300">Para reactivar tu cuenta, accede al portal web:</p>
          {portalOk ? (
            <>
              <p className="font-mono text-sm text-indigo-300 break-all">{displayUrl(status.portal_url)}</p>
              <button
                onClick={openPortal}
                className="w-full py-2.5 bg-indigo-600 hover:bg-indigo-500 text-white text-sm font-semibold rounded-lg flex items-center justify-center gap-2 transition"
              >
                <ExternalLink className="w-4 h-4" />
                Abrir en navegador
              </button>
            </>
          ) : (
            <p className="text-sm text-amber-300">El servidor no informó un portal válido. Contacta a soporte.</p>
          )}
        </div>

        {status.support_email && status.support_email.includes('@') && (
          <p className="text-xs text-slate-400 flex items-center gap-2">
            ¿Preguntas? <span className="font-mono text-slate-200 select-all">{status.support_email}</span>
            <button onClick={copyEmail} className="p-1 rounded hover:bg-slate-800" title="Copiar correo">
              {copied ? <Check className="w-3.5 h-3.5 text-emerald-400" /> : <Copy className="w-3.5 h-3.5" />}
            </button>
          </p>
        )}

        <p className="text-[11px] text-slate-500">
          Mientras tanto tus trabajadores dejan de enviar datos; lo ya recibido se conserva cifrado.
        </p>

        {error && <p className="text-xs text-rose-300">{error}</p>}

        <div className="flex gap-2 pt-2 border-t border-slate-800">
          <button
            onClick={recheck}
            disabled={checking}
            className="flex-1 py-2 bg-slate-800 hover:bg-slate-700 text-slate-200 text-xs rounded-lg flex items-center justify-center gap-1.5 disabled:opacity-50"
          >
            <RefreshCw className={`w-3.5 h-3.5 ${checking ? 'animate-spin' : ''}`} />
            Ya pagué, verificar de nuevo
          </button>
          <button
            onClick={onLogout}
            className="px-3 py-2 bg-slate-800 hover:bg-rose-900/60 text-slate-300 text-xs rounded-lg flex items-center gap-1.5"
          >
            <LogOut className="w-3.5 h-3.5" />
            Salir
          </button>
        </div>
      </div>
    </div>
  );
};
