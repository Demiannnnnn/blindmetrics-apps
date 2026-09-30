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
    <div className="fixed inset-0 z-50 bg-slate-950/95 backdrop-blur-md flex items-center justify-center p-4">
      <div className="bg-slate-900 border border-slate-700 rounded-2xl max-w-md w-full p-8 text-center space-y-6 shadow-2xl">
        <div className="w-16 h-16 rounded-2xl bg-indigo-500/20 text-indigo-400 flex items-center justify-center mx-auto border border-indigo-500/30">
          <Lock className="w-8 h-8" />
        </div>

        <div>
          <h2 className="text-xl font-bold text-white">Consola Administrativa Bloqueada</h2>
          <p className="text-xs text-slate-400 mt-1">
            Por inactividad, los datos descifrados en RAM han sido purgados para proteger la privacidad de los trabajadores (SEC-11).
          </p>
        </div>

        <div className="bg-amber-500/10 border border-amber-500/20 rounded-xl p-3 flex items-center gap-2.5 text-xs text-amber-300 text-left">
          <ShieldAlert className="w-5 h-5 flex-shrink-0" />
          <span>Reingrese su contraseña o use biometría para regenerar credenciales de sesión en RAM.</span>
        </div>

        {!hasPassword && (
          <button
            type="button"
            onClick={handleGoogleUnlock}
            disabled={loading}
            className="w-full py-2.5 bg-white hover:bg-slate-100 text-slate-900 font-medium rounded-xl text-sm flex items-center justify-center gap-2 transition disabled:opacity-40"
          >
            <span className="font-bold text-indigo-600">G</span>
            Continuar con Google
          </button>
        )}

        <form onSubmit={handleUnlock} className="space-y-4">
          {hasPassword && (
          <div className="relative">
            <KeyRound className="w-4 h-4 text-slate-400 absolute left-3 top-1/2 -translate-y-1/2" />
            <input
              type="password"
              placeholder="Contraseña de administrador..."
              value={password}
              onChange={e => setPassword(e.target.value)}
              className="w-full bg-slate-800 border border-slate-700 text-slate-200 text-sm rounded-xl pl-10 pr-4 py-2.5 focus:outline-none focus:border-indigo-500"
              autoFocus
              disabled={loading}
            />
          </div>
          )}

          {errorMessage && (
            <div className="p-2.5 bg-rose-500/10 border border-rose-500/30 rounded-lg text-xs text-rose-300 flex items-center gap-2 text-left">
              <AlertCircle className="w-4 h-4 shrink-0 text-rose-400" />
              <span>{errorMessage}</span>
            </div>
          )}

          <div className="space-y-2">
            {hasPassword && (
            <button
              type="submit"
              disabled={loading || !password}
              className="w-full bg-indigo-600 hover:bg-indigo-500 disabled:opacity-50 text-white font-semibold py-2.5 rounded-xl text-sm transition shadow-lg shadow-indigo-600/30 flex items-center justify-center gap-2"
            >
              {loading ? (
                <div className="w-4 h-4 border-2 border-white border-t-transparent rounded-full animate-spin"></div>
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
              className="w-full bg-slate-800 hover:bg-slate-700 text-slate-300 font-medium py-2 rounded-xl text-xs transition border border-slate-700 flex items-center justify-center gap-2"
            >
              <Fingerprint className="w-4 h-4 text-indigo-400" />
              <span>Desbloquear con Biometría (Touch ID / Windows Hello)</span>
            </button>
          </div>
        </form>

        <div className="pt-2 border-t border-slate-800 flex justify-between items-center text-xs text-slate-400">
          <span className="font-mono">Intentos: {attempts}/5</span>
          <button
            onClick={onForcedLogout}
            className="text-rose-400 hover:text-rose-300 flex items-center gap-1 transition"
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
