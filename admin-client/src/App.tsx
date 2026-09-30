import React, { useState, useEffect, useCallback, useRef } from 'react';
import {
  LayoutDashboard,
  Activity,
  Globe,
  Camera,
  MapPin,
  ShieldCheck,
  Lock,
  ChevronDown,
  LogOut,
  Server,
  User,
  FileText,
  Calendar,
  RefreshCw,
} from 'lucide-react';
import {
  WorkerItem,
  AdminProfile,
  SubscriptionStatus,
  VaultStatus,
  getWorkersList,
  getAdminSession,
  adminLogout,
  getApiBaseUrl,
  onSessionExpired,
  onSubscriptionBlocked,
  subscriptionStatus,
  vaultStatus,
  TimeRangeFilter,
  TimeRangePreset,
} from './api/tauri-bridge';
import { AuthView } from './components/AuthView';
import { Dashboard } from './components/Dashboard';
import { ActivityChart } from './components/ActivityChart';
import { NetworkExplorer } from './components/NetworkExplorer';
import { ScreenshotGallery } from './components/ScreenshotGallery';
import { FileActivity } from './components/FileActivity';
import { FieldMap } from './components/FieldMap';
import { ConsentSettings } from './components/ConsentSettings';
import { AutoLockModal } from './components/AutoLockModal';
import { SubscriptionLockScreen } from './components/SubscriptionLockScreen';
import { TrialBanner } from './components/TrialBanner';
import { OrganizationPrompt } from './components/OrganizationPrompt';
import { KeyRecoveryWizard } from './components/KeyRecoveryWizard';
import { useAutoLock } from './hooks/useAutoLock';
import { describeError } from './lib/errors';
import { isBlocked, shouldShowTrialBanner } from './lib/subscription';

type LockStatus = Pick<SubscriptionStatus, 'effective_status' | 'reason' | 'portal_url' | 'support_email'>;

/** What stands between a signed-in admin and the console (docs/admin-app-spec.md §1). */
type Gate =
  | { kind: 'checking' }
  | { kind: 'error'; message: string }
  | { kind: 'blocked'; status: LockStatus }
  | { kind: 'organization' }
  | { kind: 'key'; vault: VaultStatus }
  | { kind: 'ready'; subscription: SubscriptionStatus | null };

export const App: React.FC = () => {
  const [adminProfile, setAdminProfile] = useState<AdminProfile | null>(null);
  const [expiredReason, setExpiredReason] = useState<string | null>(null);
  const [gate, setGate] = useState<Gate>({ kind: 'checking' });
  const [organizationSkipped, setOrganizationSkipped] = useState(false);
  const profileRef = useRef<AdminProfile | null>(null);
  profileRef.current = adminProfile;
  const [activeTab, setActiveTab] = useState<
    'dashboard' | 'system' | 'network' | 'productivity' | 'files' | 'location' | 'compliance'
  >('dashboard');
  const [selectedWorker, setSelectedWorker] = useState<WorkerItem | null>(null);
  const [allWorkers, setAllWorkers] = useState<WorkerItem[]>([]);

  // RF-ADMIN-05-1: Global Time Range Context Filter
  const [timeRange, setTimeRange] = useState<TimeRangeFilter>({ preset: 'today' });
  const [customFrom, setCustomFrom] = useState<string>('');
  const [customTo, setCustomTo] = useState<string>('');

  /**
   * RF-ADMIN-12-1: subscription on every start/login/unlock, then organization (RF-ADMIN-09-5)
   * and the key state (TASK-ADMIN-11). Only a green gate mounts the console.
   */
  const runGates = useCallback(async () => {
    setGate({ kind: 'checking' });
    let subscription: SubscriptionStatus;
    try {
      subscription = await subscriptionStatus();
    } catch (e) {
      setGate({ kind: 'error', message: describeError(e, 'No se pudo verificar tu suscripción.') });
      return;
    }
    if (isBlocked(subscription)) {
      setGate({ kind: 'blocked', status: subscription });
      return;
    }
    if (!profileRef.current?.organization?.trim() && !organizationSkipped) {
      setGate({ kind: 'organization' });
      return;
    }
    try {
      const vault = await vaultStatus();
      if (vault.key_state !== 'OK') {
        setGate({ kind: 'key', vault });
        return;
      }
    } catch (e) {
      setGate({ kind: 'error', message: describeError(e, 'No se pudo leer el estado de la llave.') });
      return;
    }
    setGate({ kind: 'ready', subscription });
  }, [organizationSkipped]);

  useEffect(() => {
    getAdminSession().then(sess => {
      if (sess) setAdminProfile(sess);
    });
  }, []);

  // Events from the Rust core (TASK-INT-02): session expiry and subscription blocks.
  useEffect(() => {
    const unlisteners: Array<() => void> = [];
    onSessionExpired(reason => {
      setAdminProfile(null);
      setExpiredReason(reason);
    }).then(u => unlisteners.push(u));
    onSubscriptionBlocked(block => {
      setGate({
        kind: 'blocked',
        status: {
          effective_status: block.status,
          reason: block.reason,
          portal_url: block.portal_url,
          support_email: block.support_email,
        },
      });
    }).then(u => unlisteners.push(u));
    return () => unlisteners.forEach(u => u());
  }, []);

  useEffect(() => {
    if (adminProfile) runGates();
  }, [adminProfile?.id, runGates]);

  useEffect(() => {
    if (adminProfile && gate.kind === 'ready') {
      getWorkersList()
        .then(list => {
          setAllWorkers(list);
          if (list.length > 0 && !selectedWorker) {
            setSelectedWorker(list[0]);
          }
        })
        .catch(() => undefined);
    }
  }, [adminProfile, gate.kind]);

  // RF-ADMIN-07-1: Configurable Auto-Lock Hook with RAM Purge
  const {
    isLocked,
    setIsLocked,
    lockApp,
    timeoutMinutes,
    setTimeoutMinutes,
  } = useAutoLock(5);

  const resetSessionState = () => {
    setAdminProfile(null);
    setGate({ kind: 'checking' });
    setAllWorkers([]);
    setSelectedWorker(null);
    setOrganizationSkipped(false);
  };

  const handleLogout = async () => {
    await adminLogout();
    setExpiredReason(null);
    resetSessionState();
  };

  const handlePresetChange = (preset: TimeRangePreset) => {
    if (preset === 'custom') {
      const todayStr = new Date().toISOString().split('T')[0];
      setCustomFrom(todayStr);
      setCustomTo(todayStr);
      setTimeRange({
        preset: 'custom',
        from: new Date(`${todayStr}T00:00:00`).toISOString(),
        to: new Date(`${todayStr}T23:59:59`).toISOString(),
      });
    } else {
      setTimeRange({ preset });
    }
  };

  const handleCustomDateChange = (fromStr: string, toStr: string) => {
    setCustomFrom(fromStr);
    setCustomTo(toStr);
    if (fromStr && toStr) {
      setTimeRange({
        preset: 'custom',
        from: new Date(`${fromStr}T00:00:00`).toISOString(),
        to: new Date(`${toStr}T23:59:59`).toISOString(),
      });
    }
  };

  // If not logged in, render the login / register view
  if (!adminProfile) {
    return (
      <AuthView
        expiredReason={expiredReason}
        onLoginSuccess={profile => {
          setExpiredReason(null);
          setAdminProfile(profile);
        }}
      />
    );
  }

  if (gate.kind === 'checking') {
    return (
      <div className="min-h-screen bg-slate-950 flex items-center justify-center text-sm text-slate-400 gap-2">
        <RefreshCw className="w-4 h-4 animate-spin" /> Verificando suscripción y llave…
      </div>
    );
  }

  if (gate.kind === 'error') {
    return (
      <div className="min-h-screen bg-slate-950 flex items-center justify-center p-4">
        <div className="max-w-md w-full bg-slate-900 border border-slate-800 rounded-2xl p-6 space-y-4 text-center">
          <p className="text-sm text-rose-300">{gate.message}</p>
          <div className="flex justify-center gap-2">
            <button onClick={runGates} className="px-4 py-2 bg-indigo-600 hover:bg-indigo-500 text-white text-xs rounded-lg">
              Reintentar
            </button>
            <button onClick={handleLogout} className="px-4 py-2 bg-slate-800 hover:bg-slate-700 text-slate-300 text-xs rounded-lg">
              Cerrar sesión
            </button>
          </div>
        </div>
      </div>
    );
  }

  if (gate.kind === 'blocked') {
    return <SubscriptionLockScreen status={gate.status} onRecheck={runGates} onLogout={handleLogout} />;
  }

  if (gate.kind === 'organization') {
    return (
      <OrganizationPrompt
        initial={adminProfile.organization}
        onSaved={profile => {
          setAdminProfile(profile);
          profileRef.current = profile;
          runGates();
        }}
        onCancel={() => {
          setOrganizationSkipped(true);
        }}
      />
    );
  }

  if (gate.kind === 'key') {
    return <KeyRecoveryWizard status={gate.vault} onResolved={runGates} onLogout={handleLogout} />;
  }

  const trialDays = gate.subscription?.trial_days_left ?? adminProfile.trial_days_left ?? null;

  return (
    <div className="flex h-screen bg-slate-900 text-slate-100 font-sans overflow-hidden">
      {/* Auto-Lock Security Overlay (RF-ADMIN-07-1..3) */}
      <AutoLockModal
        isLocked={isLocked}
        hasPassword={adminProfile.has_password !== false}
        onUnlock={profile => {
          setIsLocked(false);
          if (profile) setAdminProfile(profile);
          // RF-ADMIN-12-1: re-check the subscription after every unlock.
          runGates();
        }}
        onForcedLogout={handleLogout}
      />

      {/* Sidebar Navigation */}
      <aside className="w-64 bg-slate-950 border-r border-slate-800 flex flex-col justify-between flex-shrink-0">
        <div>
          {/* Logo / Header */}
          <div className="p-5 border-b border-slate-800 flex items-center gap-3">
            <div className="w-9 h-9 rounded-xl bg-gradient-to-tr from-indigo-600 to-emerald-500 flex items-center justify-center font-bold text-white shadow-lg shadow-indigo-600/30 ring-1 ring-white/20">
              T
            </div>
            <div>
              <h1 className="text-sm font-bold text-white tracking-wide">Telemetry Admin</h1>
              <p className="text-[10px] text-slate-400 font-mono">Zero-Knowledge Console</p>
            </div>
          </div>

          {/* Navigation Links */}
          <nav className="p-3 space-y-1 text-xs">
            <button
              onClick={() => setActiveTab('dashboard')}
              className={`w-full flex items-center gap-3 px-3 py-2.5 rounded-xl font-medium transition ${
                activeTab === 'dashboard'
                  ? 'bg-indigo-600 text-white shadow-md shadow-indigo-600/20'
                  : 'text-slate-400 hover:bg-slate-900 hover:text-slate-200'
              }`}
            >
              <LayoutDashboard className="w-4 h-4" />
              <span>Tablero Principal & Enrolamiento</span>
            </button>

            <button
              onClick={() => setActiveTab('system')}
              className={`w-full flex items-center gap-3 px-3 py-2.5 rounded-xl font-medium transition ${
                activeTab === 'system'
                  ? 'bg-indigo-600 text-white shadow-md shadow-indigo-600/20'
                  : 'text-slate-400 hover:bg-slate-900 hover:text-slate-200'
              }`}
            >
              <Activity className="w-4 h-4" />
              <span>Actividad del Sistema</span>
            </button>

            <button
              onClick={() => setActiveTab('network')}
              className={`w-full flex items-center gap-3 px-3 py-2.5 rounded-xl font-medium transition ${
                activeTab === 'network'
                  ? 'bg-indigo-600 text-white shadow-md shadow-indigo-600/20'
                  : 'text-slate-400 hover:bg-slate-900 hover:text-slate-200'
              }`}
            >
              <Globe className="w-4 h-4" />
              <span>Navegación de Red (FQDN)</span>
            </button>

            <button
              onClick={() => setActiveTab('productivity')}
              className={`w-full flex items-center gap-3 px-3 py-2.5 rounded-xl font-medium transition ${
                activeTab === 'productivity'
                  ? 'bg-indigo-600 text-white shadow-md shadow-indigo-600/20'
                  : 'text-slate-400 hover:bg-slate-900 hover:text-slate-200'
              }`}
            >
              <Camera className="w-4 h-4" />
              <span>Productividad & Capturas</span>
            </button>

            <button
              onClick={() => setActiveTab('files')}
              className={`w-full flex items-center gap-3 px-3 py-2.5 rounded-xl font-medium transition ${
                activeTab === 'files'
                  ? 'bg-indigo-600 text-white shadow-md shadow-indigo-600/20'
                  : 'text-slate-400 hover:bg-slate-900 hover:text-slate-200'
              }`}
            >
              <FileText className="w-4 h-4" />
              <span>Actividad de Archivos</span>
            </button>

            <button
              onClick={() => setActiveTab('location')}
              className={`w-full flex items-center gap-3 px-3 py-2.5 rounded-xl font-medium transition ${
                activeTab === 'location'
                  ? 'bg-indigo-600 text-white shadow-md shadow-indigo-600/20'
                  : 'text-slate-400 hover:bg-slate-900 hover:text-slate-200'
              }`}
            >
              <MapPin className="w-4 h-4" />
              <span>Ubicación en Terreno (FIELD)</span>
            </button>

            <button
              onClick={() => setActiveTab('compliance')}
              className={`w-full flex items-center gap-3 px-3 py-2.5 rounded-xl font-medium transition ${
                activeTab === 'compliance'
                  ? 'bg-indigo-600 text-white shadow-md shadow-indigo-600/20'
                  : 'text-slate-400 hover:bg-slate-900 hover:text-slate-200'
              }`}
            >
              <ShieldCheck className="w-4 h-4" />
              <span>Bóveda & Cumplimiento</span>
            </button>
          </nav>
        </div>

        {/* User Session & Lock Button */}
        <div className="p-3 border-t border-slate-800 space-y-2">
          {/* Active Admin User Card */}
          <div className="p-2.5 bg-slate-900 rounded-xl border border-slate-800 text-xs">
            <div className="flex items-center gap-2 text-slate-200 font-semibold truncate">
              <User className="w-3.5 h-3.5 text-indigo-400 shrink-0" />
              <span className="truncate">{adminProfile.name}</span>
            </div>
            <div className="text-[11px] text-slate-400 truncate mt-0.5">{adminProfile.email}</div>
            <div className="flex items-center justify-between mt-2 pt-2 border-t border-slate-800/80 text-[10px] text-slate-500 font-mono">
              <span>{adminProfile.organization || 'Sin organización'}</span>
              <span className="flex items-center gap-1 text-emerald-400">
                <span className="w-1.5 h-1.5 rounded-full bg-emerald-400"></span>
                Autenticado
              </span>
            </div>
          </div>

          <div className="flex gap-2">
            <button
              onClick={lockApp}
              className="flex-1 bg-slate-900 hover:bg-slate-800 text-slate-300 border border-slate-700/80 px-2.5 py-2 rounded-xl text-xs flex items-center justify-center gap-1.5 transition"
              title="Bloquear sesión por inactividad y purgar RAM"
            >
              <Lock className="w-3.5 h-3.5 text-amber-400" />
              <span>Bloquear</span>
            </button>

            <button
              onClick={handleLogout}
              className="p-2 bg-slate-900 hover:bg-rose-500/20 text-slate-400 hover:text-rose-300 border border-slate-700/80 rounded-xl transition"
              title="Cerrar Sesión"
            >
              <LogOut className="w-3.5 h-3.5" />
            </button>
          </div>

          {/* Auto-Lock Inactivity Timeout Configuration (RF-ADMIN-07-1) */}
          <div className="flex items-center justify-between text-[11px] text-slate-400 px-1 pt-0.5">
            <span>Auto-bloqueo:</span>
            <select
              value={timeoutMinutes}
              onChange={e => setTimeoutMinutes(parseInt(e.target.value, 10))}
              className="bg-slate-900 border border-slate-700 text-slate-300 rounded px-1.5 py-0.5 text-[11px] focus:outline-none focus:border-indigo-500 cursor-pointer"
            >
              <option value={5}>5 min</option>
              <option value={10}>10 min</option>
              <option value={15}>15 min</option>
              <option value={30}>30 min</option>
            </select>
          </div>

          <div className="text-[10px] text-slate-500 font-mono text-center flex items-center justify-center gap-1">
            <Server className="w-3 h-3 text-slate-600" />
            <span className="truncate max-w-[200px]">{getApiBaseUrl()}</span>
          </div>
        </div>
      </aside>

      {/* Main Content Area */}
      <main className="flex-1 flex flex-col overflow-hidden">
        {gate.subscription && shouldShowTrialBanner(gate.subscription) && trialDays !== null && (
          <TrialBanner daysLeft={trialDays} />
        )}
        {/* RF-ADMIN-05-1: Global Context Header with Worker and Time Range Selectors */}
        <header className="h-16 border-b border-slate-800 px-6 flex items-center justify-between flex-shrink-0 bg-slate-900/60 backdrop-blur">
          <div className="flex items-center gap-4 flex-wrap">
            {/* Worker Selector Dropdown */}
            <div className="flex items-center gap-2">
              <span className="text-xs font-medium text-slate-400">Trabajador:</span>
              <div className="relative">
                <select
                  value={selectedWorker?.id ?? ''}
                  onChange={e => {
                    const w = allWorkers.find(item => item.id === e.target.value);
                    if (w) setSelectedWorker(w);
                  }}
                  className="bg-slate-800 border border-slate-700 text-slate-200 text-xs font-semibold rounded-lg pl-3 pr-8 py-1.5 appearance-none focus:outline-none focus:border-indigo-500 cursor-pointer"
                >
                  {allWorkers.map(w => (
                    <option key={w.id} value={w.id}>
                      {w.name} ({w.role}) {w.isActive ? '🟢' : '🔴'}
                    </option>
                  ))}
                </select>
                <ChevronDown className="w-3.5 h-3.5 text-slate-400 absolute right-2.5 top-1/2 -translate-y-1/2 pointer-events-none" />
              </div>
            </div>

            {/* Time Range Selector (RF-ADMIN-05-1) */}
            <div className="flex items-center gap-2">
              <Calendar className="w-3.5 h-3.5 text-slate-400" />
              <span className="text-xs font-medium text-slate-400">Rango:</span>
              <div className="relative">
                <select
                  value={timeRange.preset}
                  onChange={e => handlePresetChange(e.target.value as TimeRangePreset)}
                  className="bg-slate-800 border border-slate-700 text-slate-200 text-xs font-medium rounded-lg pl-3 pr-8 py-1.5 appearance-none focus:outline-none focus:border-indigo-500 cursor-pointer"
                >
                  <option value="today">Hoy</option>
                  <option value="yesterday">Ayer</option>
                  <option value="7days">Últimos 7 días</option>
                  <option value="custom">Personalizado</option>
                </select>
                <ChevronDown className="w-3.5 h-3.5 text-slate-400 absolute right-2.5 top-1/2 -translate-y-1/2 pointer-events-none" />
              </div>

              {/* Custom Date Inputs */}
              {timeRange.preset === 'custom' && (
                <div className="flex items-center gap-1.5 text-xs">
                  <input
                    type="date"
                    value={customFrom}
                    onChange={e => handleCustomDateChange(e.target.value, customTo)}
                    className="bg-slate-800 border border-slate-700 text-slate-200 rounded px-2 py-1 text-xs focus:outline-none focus:border-indigo-500"
                  />
                  <span className="text-slate-500">-</span>
                  <input
                    type="date"
                    value={customTo}
                    onChange={e => handleCustomDateChange(customFrom, e.target.value)}
                    className="bg-slate-800 border border-slate-700 text-slate-200 rounded px-2 py-1 text-xs focus:outline-none focus:border-indigo-500"
                  />
                </div>
              )}
            </div>
          </div>

          {/* Crypto Status & Time */}
          <div className="flex items-center gap-4 text-xs font-mono">
            <span className="bg-indigo-950/80 text-indigo-300 border border-indigo-800/60 px-2.5 py-1 rounded-md">
              X25519-AES256GCM
            </span>
            <span className="text-slate-400 hidden sm:inline">
              {selectedWorker?.timezone ?? 'America/Santiago'}
            </span>
          </div>
        </header>

        {/* View Content */}
        <div className="flex-1 overflow-y-auto p-6">
          {activeTab === 'dashboard' && (
            <Dashboard
              organization={adminProfile.organization}
              onSelectWorker={worker => {
                setSelectedWorker(worker);
                setActiveTab('system');
              }}
              onOpenVault={() => setActiveTab('compliance')}
              onProfileChanged={setAdminProfile}
            />
          )}

          {activeTab === 'system' && (
            <ActivityChart
              workerId={selectedWorker?.id}
              workerName={selectedWorker?.name ?? 'Empleado'}
              timeRange={timeRange}
            />
          )}

          {activeTab === 'network' && (
            <NetworkExplorer
              workerId={selectedWorker?.id}
              workerName={selectedWorker?.name ?? 'Empleado'}
              timeRange={timeRange}
            />
          )}

          {activeTab === 'productivity' && (
            <ScreenshotGallery
              workerId={selectedWorker?.id}
              workerName={selectedWorker?.name ?? 'Empleado'}
              adminProfile={adminProfile}
              timeRange={timeRange}
            />
          )}

          {activeTab === 'files' && (
            <FileActivity
              workerId={selectedWorker?.id}
              workerName={selectedWorker?.name ?? 'Empleado'}
              timeRange={timeRange}
            />
          )}

          {activeTab === 'location' && (
            <FieldMap
              workerId={selectedWorker?.id}
              workerName={selectedWorker?.name ?? 'Empleado'}
              workerRole={selectedWorker?.role ?? 'OFFICE'}
              workHours={{
                start: selectedWorker?.workHoursStart ?? '09:00',
                end: selectedWorker?.workHoursEnd ?? '18:00',
              }}
              timezone={selectedWorker?.timezone ?? 'America/Santiago'}
              timeRange={timeRange}
            />
          )}

          {activeTab === 'compliance' && (
            <ConsentSettings
              registeredFingerprint={adminProfile.public_key_fingerprint}
              onCurrentDeviceRevoked={() => {
                setExpiredReason('DEVICE_REVOKED');
                resetSessionState();
              }}
              onKeyChanged={runGates}
            />
          )}
        </div>
      </main>
    </div>
  );
};

export default App;
