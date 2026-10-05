import React, { useState } from 'react';
import { Lock, KeyRound, ShieldAlert, Fingerprint, LogOut, AlertCircle } from 'lucide-react';
import { adminLoginGoogle, adminUnlock, adminUnlockWithBiometric, AdminProfile } from '../api/tauri-bridge';
import { describeError, errorIs } from '../lib/errors';

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
  const [attempts, setAttempts] = useState(0);

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
    setErrorMessage(null);
    try {
      onUnlock(await adminLoginGoogle());
    } catch (err) {
      setErrorMessage(describeError(err, 'No se pudo iniciar sesión con Google.'));
    } finally {
      setLoading(false);
    }
  };

  const handleBiometricUnlock = async () => {
    setLoading(true);
    setErrorMessage(null);

    try {
      const profile = await adminUnlockWithBiometric();
      setPassword('');
      setAttempts(0);
      onUnlock(profile);
    } catch (err: any) {
      setErrorMessage(describeError(err, 'Biometría no disponible o rechazada'));
    } finally {
      setLoading(false);
    }
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
          <span>Reingrese su contraseña o use biometría para regenerar credenciales de sesión en RAM.</span>
        </div>

        {!hasPassword && (
          <button
            type="button"
            onClick={handleGoogleUnlock}
            disabled={loading}
            className="w-full py-2.5 bg-surface hover:bg-surface-subtle text-muted font-medium rounded-card text-sm flex items-center justify-center gap-2 transition disabled:opacity-40"
          >
            <span className="font-bold text-accent">G</span>
            Continuar con Google
          </button>
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

            {/* Biometric Quick Unlock (RF-ADMIN-07-3) */}
            <button
              type="button"
              onClick={handleBiometricUnlock}
              disabled={loading}
              className="w-full bg-surface-subtle hover:bg-surface-subtle text-fg font-medium py-2 rounded-card text-xs transition border border-default flex items-center justify-center gap-2"
            >
              <Fingerprint className="w-4 h-4 text-accent" />
              <span>Desbloquear con Biometría (Touch ID / Windows Hello)</span>
            </button>
          </div>
        </form>

        <div className="pt-2 border-t border-default flex justify-between items-center text-xs text-muted">
          <span className="font-mono">Intentos: {attempts}/5</span>
          <button
            onClick={onForcedLogout}
            className="text-danger hover:text-danger flex items-center gap-1 transition"
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
