import React, { useState } from 'react';
import { ShieldCheck, KeyRound, Server, User, Building, ArrowRight, AlertTriangle, Sparkles } from 'lucide-react';
import { adminLogin, adminRegister, AdminProfile, getApiBaseUrl, isDemoMode } from '../api/tauri-bridge';

/** Tauri commands reject with a serialized CommandError { code, message }. */
function errorText(err: any): string {
  return (typeof err === 'string' ? err : err?.message) || 'Error al autenticar';
}

interface AuthViewProps {
  onLoginSuccess: (profile: AdminProfile) => void;
}

export const AuthView: React.FC<AuthViewProps> = ({ onLoginSuccess }) => {
  const [tab, setTab] = useState<'login' | 'register'>('login');
  const [email, setEmail] = useState('admin@acme.cl');
  const [password, setPassword] = useState('AdminSecret2026!');
  const [name, setName] = useState('Demian Administrador');
  const [organization, setOrganization] = useState('Acme SpA');
  const [apiUrl, setApiUrl] = useState(getApiBaseUrl());
  const [loading, setLoading] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setLoading(true);
    setErrorMessage(null);

    try {
      if (tab === 'login') {
        const profile = await adminLogin(email, password, apiUrl);
        onLoginSuccess(profile);
      } else {
        const profile = await adminRegister(name, email, password, organization, apiUrl);
        onLoginSuccess(profile);
      }
    } catch (err: any) {
      setErrorMessage(errorText(err));
    } finally {
      setLoading(false);
    }
  };

  // Only reachable in VITE_DEMO_MODE builds (docs/admin-app-spec.md §A1).
  const handleDemoAccess = async () => {
    setLoading(true);
    setErrorMessage(null);
    try {
      const profile = await adminLogin('demo@telemetry.local', 'demo-pass', apiUrl);
      onLoginSuccess(profile);
    } catch (err: any) {
      setErrorMessage(errorText(err));
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="min-h-screen bg-slate-950 flex flex-col justify-center items-center px-4 relative overflow-hidden text-slate-100">
      {/* Background glow effects */}
      <div className="absolute top-1/4 left-1/2 -translate-x-1/2 -translate-y-1/2 w-96 h-96 bg-indigo-600/15 rounded-full blur-3xl pointer-events-none" />
      <div className="absolute bottom-10 right-10 w-80 h-80 bg-emerald-600/10 rounded-full blur-3xl pointer-events-none" />

      <div className="w-full max-w-md bg-slate-900/90 border border-slate-800 rounded-2xl shadow-2xl p-8 backdrop-blur-xl relative z-10">
        {/* Brand Header */}
        <div className="flex flex-col items-center mb-8 text-center">
          <div className="w-14 h-14 bg-gradient-to-tr from-indigo-600 to-emerald-500 rounded-xl flex items-center justify-center shadow-lg shadow-indigo-500/20 mb-4 ring-1 ring-white/20">
            <ShieldCheck className="w-8 h-8 text-white" />
          </div>
          <h1 className="text-2xl font-bold tracking-tight text-white">Telemetry Admin</h1>
          <p className="text-xs text-slate-400 mt-1">Consola Ejecutiva con Cifrado Zero-Knowledge</p>
        </div>

        {/* Tab Switcher */}
        <div className="flex bg-slate-950/60 p-1 rounded-xl mb-6 border border-slate-800/80">
          <button
            type="button"
            onClick={() => { setTab('login'); setErrorMessage(null); }}
            className={`flex-1 py-2 text-xs font-semibold rounded-lg transition-all ${
              tab === 'login'
                ? 'bg-indigo-600 text-white shadow-md'
                : 'text-slate-400 hover:text-white'
            }`}
          >
            Iniciar Sesión
          </button>
          <button
            type="button"
            onClick={() => { setTab('register'); setErrorMessage(null); }}
            className={`flex-1 py-2 text-xs font-semibold rounded-lg transition-all ${
              tab === 'register'
                ? 'bg-indigo-600 text-white shadow-md'
                : 'text-slate-400 hover:text-white'
            }`}
          >
            Crear Cuenta Admin
          </button>
        </div>

        {errorMessage && (
          <div className="mb-4 p-3 bg-rose-500/10 border border-rose-500/30 rounded-lg flex items-start gap-2 text-xs text-rose-300">
            <AlertTriangle className="w-4 h-4 shrink-0 text-rose-400 mt-0.5" />
            <span>{errorMessage}</span>
          </div>
        )}

        <form onSubmit={handleSubmit} className="space-y-4">
          {tab === 'register' && (
            <>
              <div>
                <label className="block text-xs font-medium text-slate-300 mb-1.5 flex items-center gap-1.5">
                  <User className="w-3.5 h-3.5 text-slate-400" />
                  Nombre Completo
                </label>
                <input
                  type="text"
                  required
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  placeholder="Ej. Carlos Valenzuela"
                  className="w-full px-3 py-2 bg-slate-950 border border-slate-700/80 rounded-lg text-sm text-white placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-indigo-500"
                />
              </div>

              <div>
                <label className="block text-xs font-medium text-slate-300 mb-1.5 flex items-center gap-1.5">
                  <Building className="w-3.5 h-3.5 text-slate-400" />
                  Organización o Empresa
                </label>
                <input
                  type="text"
                  required
                  value={organization}
                  onChange={(e) => setOrganization(e.target.value)}
                  placeholder="Ej. Acme SpA"
                  className="w-full px-3 py-2 bg-slate-950 border border-slate-700/80 rounded-lg text-sm text-white placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-indigo-500"
                />
              </div>
            </>
          )}

          <div>
            <label className="block text-xs font-medium text-slate-300 mb-1.5">Correo Electrónico</label>
            <input
              type="email"
              required
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              placeholder="admin@empresa.com"
              className="w-full px-3 py-2 bg-slate-950 border border-slate-700/80 rounded-lg text-sm text-white placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-indigo-500"
            />
          </div>

          <div>
            <label className="block text-xs font-medium text-slate-300 mb-1.5 flex items-center gap-1.5">
              <KeyRound className="w-3.5 h-3.5 text-slate-400" />
              Contraseña Maestra
            </label>
            <input
              type="password"
              required
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              placeholder="••••••••••••"
              className="w-full px-3 py-2 bg-slate-950 border border-slate-700/80 rounded-lg text-sm text-white placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-indigo-500"
            />
          </div>

          <div>
            <label className="block text-xs font-medium text-slate-400 mb-1.5 flex items-center justify-between">
              <span className="flex items-center gap-1.5">
                <Server className="w-3.5 h-3.5 text-slate-500" />
                URL del Servidor API
              </span>
              <span className="text-[10px] text-emerald-400 font-mono">Configurable (.env)</span>
            </label>
            <input
              type="text"
              required
              value={apiUrl}
              onChange={(e) => setApiUrl(e.target.value)}
              placeholder="http://localhost:8080/api/v1"
              className="w-full px-3 py-2 bg-slate-950/70 border border-slate-800 rounded-lg text-xs font-mono text-slate-300 focus:outline-none focus:ring-2 focus:ring-indigo-500"
            />
          </div>

          <button
            type="submit"
            disabled={loading}
            className="w-full mt-2 py-2.5 bg-gradient-to-r from-indigo-600 to-indigo-700 hover:from-indigo-500 hover:to-indigo-600 text-white font-medium rounded-lg text-sm flex items-center justify-center gap-2 shadow-lg shadow-indigo-600/25 transition-all disabled:opacity-50"
          >
            {loading ? (
              <span>Autenticando...</span>
            ) : (
              <>
                <span>{tab === 'login' ? 'Acceder a la Consola' : 'Registrar y Generar Claves'}</span>
                <ArrowRight className="w-4 h-4" />
              </>
            )}
          </button>
        </form>

        {isDemoMode && (
          <div className="mt-6 pt-5 border-t border-slate-800">
            <button
              type="button"
              onClick={handleDemoAccess}
              disabled={loading}
              className="w-full py-2 bg-slate-800/60 hover:bg-slate-800 border border-slate-700/50 rounded-lg text-xs text-slate-300 flex items-center justify-center gap-2 transition-all"
            >
              <Sparkles className="w-3.5 h-3.5 text-amber-400" />
              <span>Entrar en Modo Demo</span>
            </button>
            <p className="text-[11px] text-slate-500 text-center mt-2">
              Solo para demostraciones: usa datos ficticios, sin servidor.
            </p>
          </div>
        )}
      </div>
    </div>
  );
};
