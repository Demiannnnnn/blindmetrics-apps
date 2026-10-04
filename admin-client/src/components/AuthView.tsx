import React, { useEffect, useState } from 'react';
import { ShieldCheck, KeyRound, User, Building, ArrowRight, AlertTriangle } from 'lucide-react';
import {
  adminLogin,
  adminLoginGoogle,
  adminRegister,
  AdminProfile,
  googleSignInAvailable,
  isDemoMode,
} from '../api/tauri-bridge';
import { describeError, describeLoginError, ERROR_MESSAGES } from '../lib/errors';

interface AuthViewProps {
  onLoginSuccess: (profile: AdminProfile) => void;
  /** Why the previous session ended (`session-expired` event), shown once. */
  expiredReason?: string | null;
}

const PASSWORD_RULE = /^(?=.*[a-z])(?=.*[A-Z])(?=.*\d)(?=.*[^A-Za-z0-9]).{8,128}$/;

export const AuthView: React.FC<AuthViewProps> = ({ onLoginSuccess, expiredReason }) => {
  const [tab, setTab] = useState<'login' | 'register'>('login');
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [name, setName] = useState('');
  const [organization, setOrganization] = useState('');
  const [loading, setLoading] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(
    expiredReason ? ERROR_MESSAGES[expiredReason] ?? ERROR_MESSAGES.SESSION_EXPIRED : null,
  );
  const [googleAvailable, setGoogleAvailable] = useState(false);

  useEffect(() => {
    googleSignInAvailable().then(setGoogleAvailable);
  }, []);

  const handleGoogle = async () => {
    setLoading(true);
    setErrorMessage(null);
    try {
      onLoginSuccess(await adminLoginGoogle());
    } catch (err) {
      setErrorMessage(describeError(err, 'No se pudo iniciar sesión con Google.'));
    } finally {
      setLoading(false);
    }
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setLoading(true);
    setErrorMessage(null);

    if (tab === 'register' && !PASSWORD_RULE.test(password)) {
      setErrorMessage('La contraseña debe tener 8 a 128 caracteres, con mayúscula, minúscula, número y símbolo.');
      setLoading(false);
      return;
    }

    try {
      if (tab === 'login') {
        const profile = await adminLogin(email, password);
        onLoginSuccess(profile);
      } else {
        const profile = await adminRegister(name, email, password, organization);
        onLoginSuccess(profile);
      }
    } catch (err: any) {
      setErrorMessage(tab === 'login' ? describeLoginError(err) : describeError(err, 'No se pudo crear la cuenta.'));
    } finally {
      setPassword('');
      setLoading(false);
    }
  };

  // Only reachable in VITE_DEMO_MODE builds (docs/admin-app-spec.md §A1).
  const handleDemoAccess = async () => {
    setLoading(true);
    setErrorMessage(null);
    try {
      const profile = await adminLogin('demo@telemetry.local', 'demo-pass');
      onLoginSuccess(profile);
    } catch (err: any) {
      setErrorMessage(describeError(err));
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="min-h-screen bg-bg flex flex-col text-fg">
      {/* Ink top bar with the logo (ecosystem auth layout) */}
      <header className="h-14 shrink-0 bg-ink border-b border-ink-line flex items-center px-6 gap-3">
        <span className="w-7 h-7 rounded-control bg-brand flex items-center justify-center" aria-hidden="true">
          <ShieldCheck className="w-4 h-4 text-white" strokeWidth={2} />
        </span>
        <span className="text-sm font-semibold text-ink-fg">Telemetry</span>
        <span className="text-xs text-ink-muted">Consola de administración</span>
      </header>

      <div className="flex-1 flex justify-center items-start sm:items-center px-4 py-10 overflow-y-auto">
      <div className="w-full max-w-md bg-surface border border-default rounded-card p-8">
        <div className="mb-6">
          <h1 className="text-2xl font-semibold text-fg">
            {tab === 'login' ? 'Inicia sesión' : 'Crea tu cuenta de administrador'}
          </h1>
          <p className="text-sm text-muted mt-1">
            Tus datos se cifran en este equipo; el servidor nunca puede leerlos.
          </p>
        </div>

        {/* Tab Switcher */}
        <div className="flex bg-surface-subtle p-1 rounded-control mb-6 border border-default">
          <button
            type="button"
            onClick={() => { setTab('login'); setErrorMessage(null); }}
            className={`flex-1 py-1.5 text-xs font-medium rounded-control transition-colors ${
              tab === 'login'
                ? 'bg-surface text-fg border border-default'
                : 'text-muted hover:text-fg border border-transparent'
            }`}
          >
            Iniciar Sesión
          </button>
          <button
            type="button"
            onClick={() => { setTab('register'); setErrorMessage(null); }}
            className={`flex-1 py-1.5 text-xs font-medium rounded-control transition-colors ${
              tab === 'register'
                ? 'bg-surface text-fg border border-default'
                : 'text-muted hover:text-fg border border-transparent'
            }`}
          >
            Crear Cuenta Admin
          </button>
        </div>

        {errorMessage && (
          <div className="mb-4 p-3 bg-danger-subtle border border-default rounded-control flex items-start gap-2 text-xs text-danger">
            <AlertTriangle className="w-4 h-4 shrink-0 text-danger mt-0.5" />
            <span>{errorMessage}</span>
          </div>
        )}

        <form onSubmit={handleSubmit} className="space-y-4">
          {tab === 'register' && (
            <>
              <div>
                <label className="block text-xs font-medium text-fg mb-1.5 flex items-center gap-1.5">
                  <User className="w-3.5 h-3.5 text-muted" />
                  Nombre Completo
                </label>
                <input
                  type="text"
                  required
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  placeholder="Tu nombre completo"
                  className="w-full px-3 py-2 bg-bg border border-default rounded-control text-sm text-fg placeholder-muted focus:outline-none focus:ring-2 focus:ring-accent"
                />
              </div>

              <div>
                <label className="block text-xs font-medium text-fg mb-1.5 flex items-center gap-1.5">
                  <Building className="w-3.5 h-3.5 text-muted" />
                  Organización o Empresa
                </label>
                <input
                  type="text"
                  required
                  value={organization}
                  onChange={(e) => setOrganization(e.target.value)}
                  placeholder="Nombre de tu empresa"
                  className="w-full px-3 py-2 bg-bg border border-default rounded-control text-sm text-fg placeholder-muted focus:outline-none focus:ring-2 focus:ring-accent"
                />
              </div>
            </>
          )}

          <div>
            <label className="block text-xs font-medium text-fg mb-1.5">Correo Electrónico</label>
            <input
              type="email"
              required
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              placeholder="admin@empresa.com"
              className="w-full px-3 py-2 bg-bg border border-default rounded-control text-sm text-fg placeholder-muted focus:outline-none focus:ring-2 focus:ring-accent"
            />
          </div>

          <div>
            <label className="block text-xs font-medium text-fg mb-1.5 flex items-center gap-1.5">
              <KeyRound className="w-3.5 h-3.5 text-muted" />
              Contraseña de la cuenta
            </label>
            <input
              type="password"
              required
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              placeholder="••••••••••••"
              className="w-full px-3 py-2 bg-bg border border-default rounded-control text-sm text-fg placeholder-muted focus:outline-none focus:ring-2 focus:ring-accent"
            />
          </div>


          <button
            type="submit"
            disabled={loading}
            className="w-full mt-2 py-2.5 bg-accent text-accent-fg font-medium rounded-control text-sm flex items-center justify-center gap-2 transition-all disabled:opacity-50"
          >
            {loading ? (
              <span>Autenticando...</span>
            ) : (
              <>
                <span>{tab === 'login' ? 'Acceder a la Consola' : 'Crear cuenta (14 días de prueba)'}</span>
                <ArrowRight className="w-4 h-4" />
              </>
            )}
          </button>
        </form>

        {/* TASK-ADMIN-10: Google sign-in through the system browser (loopback + PKCE). */}
        <div className="mt-4">
          <button
            type="button"
            onClick={handleGoogle}
            disabled={loading || !googleAvailable}
            title={googleAvailable ? undefined : 'El inicio con Google no está configurado en esta versión'}
            className="w-full py-2.5 bg-surface hover:bg-surface-subtle text-muted font-medium rounded-control text-sm flex items-center justify-center gap-2 transition disabled:opacity-40"
          >
            <span className="font-bold text-accent">G</span>
            <span>Continuar con Google</span>
          </button>
          {!googleAvailable && (
            <p className="text-[11px] text-muted text-center mt-1.5">
              El inicio con Google no está configurado en esta versión.
            </p>
          )}
          {googleAvailable && (
            <p className="text-[11px] text-muted text-center mt-1.5">
              Se abrirá tu navegador; esta app nunca ve tu contraseña de Google.
            </p>
          )}
        </div>

        {isDemoMode && (
          <div className="mt-6 pt-5 border-t border-default">
            <button
              type="button"
              onClick={handleDemoAccess}
              disabled={loading}
              className="w-full py-2 bg-surface-subtle hover:bg-surface-subtle border border-default rounded-control text-xs text-fg flex items-center justify-center gap-2 transition-all"
            >
              <span>Entrar en modo demo</span>
            </button>
            <p className="text-[11px] text-muted text-center mt-2">
              Solo para demostraciones: usa datos ficticios, sin servidor.
            </p>
          </div>
        )}
      </div>
      </div>
    </div>
  );
};
