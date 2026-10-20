import React, { useRef, useState } from 'react';
import { Lock, KeyRound, ShieldAlert, LogOut, AlertCircle } from 'lucide-react';
import { adminCancelGoogleLogin, adminLoginGoogle, adminUnlock, AdminProfile } from '../api/tauri-bridge';
import { describeError, errorIs, getGoogleErrorMessage } from '../lib/errors';

interface AutoLockModalProps {
  isLocked: boolean;
  /** Google-only accounts (RF-ADMIN-10-4) unlock by signing in with Google again. */
  hasPassword: boolean;
  onUnlock: (profile?: AdminProfile) => void;
  onForcedLogout: () => void;
}

export const AutoLockModal: React.FC<AutoLockModalProps> = ({
  isLocked,
  hasPassword,
  onUnlock,
  onForcedLogout,
}) => {
  const [password, setPassword] = useState('');
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [waitingGoogle, setWaitingGoogle] = useState(false);
  const [attempts, setAttempts] = useState(0);
  const cancelledByButtonRef = useRef(false);

  if (!isLocked) return null;

  const handleUnlock = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!password) return;

    setLoading(true);
    setErrorMessage(null);

    try {
      const profile = await adminUnlock(password);
      setPassword('');
      setErrorMessage(null);
      setAttempts(0);
      onUnlock(profile);
    } catch (err: any) {
      const newAttempts = attempts + 1;
      setAttempts(newAttempts);
      setErrorMessage(describeError(err));

      // Force logout after 5 failed attempts (RF-ADMIN-07-3 & Section 5)
      if (newAttempts >= 5 || errorIs(err, 'UNLOCK_LIMIT_REACHED')) {
        alert("Demasiados intentos fallidos. Por seguridad, la sesión ha sido revocada.");
        onForcedLogout();
      }
    } finally {
      setPassword('');
      setLoading(false);
    }
  };

  const handleGoogleUnlock = async () => {
    setLoading(true);
    setWaitingGoogle(true);
    setErrorMessage(null);
    cancelledByButtonRef.current = false;
    try {
      onUnlock(await adminLoginGoogle());
    } catch (err) {
      const msg = getGoogleErrorMessage(err, cancelledByButtonRef.current);
      if (msg) {
        setErrorMessage(msg);
      }
    } finally {
      setLoading(false);
      setWaitingGoogle(false);
      cancelledByButtonRef.current = false;
    }
  };

  const handleCancelGoogleUnlock = async () => {
    cancelledByButtonRef.current = true;
    try {
      await adminCancelGoogleLogin();
    } catch (err) {
      console.warn('Error cancelling Google unlock:', err);
    }
    setLoading(false);
    setWaitingGoogle(false);
    setErrorMessage(null);
  };


  return (
    <div className="fixed inset-0 z-50 bg-bg flex items-center justify-center p-4">
      <div className="bg-surface border border-default rounded-card max-w-md w-full p-8 text-center space-y-6 shadow-md">
        <div className="w-16 h-16 rounded-card bg-accent-subtle text-accent flex items-center justify-center mx-auto border border-default">
          <Lock className="w-8 h-8" />
        </div>

        <div>
          <h2 className="text-xl font-bold text-fg">Consola Administrativa Bloqueada</h2>
          <p className="text-xs text-muted mt-1">
            Por inactividad, los datos descifrados en RAM han sido purgados para proteger la privacidad de los trabajadores (SEC-11).
          </p>
        </div>

        <div className="bg-warning-subtle border border-default rounded-card p-3 flex items-center gap-2.5 text-xs text-warning text-left">
          <ShieldAlert className="w-5 h-5 shrink-0" />
          <span>Reingrese su contraseña para regenerar credenciales de sesión en RAM.</span>
        </div>

        {!hasPassword && (
          waitingGoogle ? (
            <div className="space-y-2">
              <p className="text-xs text-muted text-center py-1">
                Completa el inicio de sesión en tu navegador…
              </p>
              <button
                type="button"
                onClick={handleCancelGoogleUnlock}
                className="w-full py-2.5 bg-surface-subtle hover:bg-surface border border-default text-fg font-medium rounded-card text-sm flex items-center justify-center gap-2 transition"
              >
                Cancelar
              </button>
            </div>
          ) : (
            <button
              type="button"
              onClick={handleGoogleUnlock}
              disabled={loading}
              className="w-full py-2.5 bg-surface hover:bg-surface-subtle text-muted font-medium rounded-card text-sm flex items-center justify-center gap-2 transition disabled:opacity-40"
            >
              <span className="font-bold text-accent">G</span>
              Continuar con Google
            </button>
          )
        )}

        <form onSubmit={handleUnlock} className="space-y-4">
          {hasPassword && (
          <div className="relative">
            <KeyRound className="w-4 h-4 text-muted absolute left-3 top-1/2 -translate-y-1/2" />
            <input
              type="password"
              placeholder="Contraseña de administrador..."
              value={password}
              onChange={e => setPassword(e.target.value)}
              className="w-full bg-surface-subtle border border-default text-fg text-sm rounded-card pl-10 pr-4 py-2.5 focus:outline-hidden focus:border-accent"
              autoFocus
              disabled={loading}
            />
          </div>
          )}

          {errorMessage && (
            <div className="p-2.5 bg-danger-subtle border border-default rounded-control text-xs text-danger flex items-center gap-2 text-left">
              <AlertCircle className="w-4 h-4 shrink-0 text-danger" />
              <span>{errorMessage}</span>
            </div>
          )}

          <div className="space-y-2">
            {hasPassword && (
            <button
              type="submit"
              disabled={loading || !password}
              className="w-full bg-accent hover:bg-accent-hover disabled:opacity-50 text-accent-fg font-semibold py-2.5 rounded-card text-sm transition flex items-center justify-center gap-2"
            >
              {loading ? (
                <div className="w-4 h-4 border-2 border-default border-t-transparent rounded-full animate-spin"></div>
              ) : (
                <span>Desbloquear y Restaurar Sesión</span>
              )}
            </button>
            )}
          </div>
        </form>

        <div className="pt-2 border-t border-default flex justify-between items-center text-xs text-muted">
          <span className="font-mono">Intentos: {attempts}/5</span>
          <button
            onClick={onForcedLogout}
            disabled={loading}
            className="text-danger hover:text-danger flex items-center gap-1 transition disabled:opacity-50"
          >
            <LogOut className="w-3.5 h-3.5" />
            <span>Cerrar sesión completamente</span>
          </button>
        </div>
      </div>
    </div>
  );
};

export default AutoLockModal;
