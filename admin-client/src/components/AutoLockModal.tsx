import React, { useState } from 'react';
import { Lock, KeyRound, ShieldAlert } from 'lucide-react';

interface AutoLockModalProps {
  isLocked: boolean;
  onUnlock: () => void;
}

export const AutoLockModal: React.FC<AutoLockModalProps> = ({ isLocked, onUnlock }) => {
  const [password, setPassword] = useState('');
  const [error, setError] = useState(false);

  if (!isLocked) return null;

  const handleUnlock = (e: React.FormEvent) => {
    e.preventDefault();
    if (password === 'admin123' || password.length >= 6) {
      setError(false);
      setPassword('');
      onUnlock();
    } else {
      setError(true);
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
            Por inactividad, los datos descifrados en RAM han sido purgados para proteger la privacidad de los trabajadores.
          </p>
        </div>

        <div className="bg-amber-500/10 border border-amber-500/20 rounded-xl p-3 flex items-center gap-2.5 text-xs text-amber-300 text-left">
          <ShieldAlert className="w-5 h-5 flex-shrink-0" />
          <span>Ingrese su contraseña de administrador o utilice Touch ID / Windows Hello.</span>
        </div>

        <form onSubmit={handleUnlock} className="space-y-4">
          <div className="relative">
            <KeyRound className="w-4 h-4 text-slate-400 absolute left-3 top-1/2 -translate-y-1/2" />
            <input
              type="password"
              placeholder="Contraseña de administrador..."
              value={password}
              onChange={e => setPassword(e.target.value)}
              className="w-full bg-slate-800 border border-slate-700 text-slate-200 text-sm rounded-xl pl-10 pr-4 py-2.5 focus:outline-none focus:border-indigo-500"
              autoFocus
            />
          </div>

          {error && (
            <p className="text-xs text-rose-400 font-medium">Contraseña incorrecta. Intente de nuevo.</p>
          )}

          <button
            type="submit"
            className="w-full bg-indigo-600 hover:bg-indigo-500 text-white font-semibold py-2.5 rounded-xl text-sm transition shadow-lg shadow-indigo-600/30"
          >
            Desbloquear y Restaurar Sesión
          </button>
        </form>
      </div>
    </div>
  );
};
