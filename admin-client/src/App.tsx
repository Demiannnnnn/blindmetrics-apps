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
  User,
  FileText,
  Calendar,
  RefreshCw,
  Building2,
  Clock,
  CalendarCheck,
} from 'lucide-react';
import {
  WorkerItem,
  AdminProfile,
  SubscriptionStatus,
  VaultStatus,
  LegalStatus,
  legalStatus,
  getWorkersList,
  getAdminSession,
  adminLogout,
  onSessionExpired,
  onSubscriptionBlocked,
  subscriptionStatus,
  vaultStatus,
  TimeRangeFilter,
  TimeRangePreset,
  employerGet,
} from './api/tauri-bridge';
import { AuthView } from './components/AuthView';
import { Dashboard } from './components/Dashboard';
import { AttendanceBoard } from './components/AttendanceBoard';
import { AttendanceDays } from './components/AttendanceDays';
import { LegalReports } from './components/LegalReports';
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
import { TermsGate } from './components/TermsGate';
import { EmployerProfile } from './components/EmployerProfile';
import { useAutoLock } from './hooks/useAutoLock';
import { describeError } from './lib/errors';
import { isBlocked, shouldShowTrialBanner } from './lib/subscription';

type LockStatus = Pick<SubscriptionStatus, 'effective_status' | 'reason' | 'portal_url' | 'support_email'>;

export type MainTab =
  | 'dashboard'
  | 'attendance-live'
  | 'attendance-records'
  | 'attendance-reports'
  | 'system'
  | 'network'
  | 'productivity'
  | 'files'
  | 'location'
  | 'employer'
  | 'compliance';

/** What stands between a signed-in admin and the console (docs/admin-app-spec.md §1, §5.2). */
type Gate =
  | { kind: 'checking' }
  | { kind: 'terms'; status: LegalStatus }
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
  const [activeTab, setActiveTab] = useState<MainTab>('dashboard');
  const [attendanceWorkerId, setAttendanceWorkerId] = useState<string | null>(null);
  const [selectedWorker, setSelectedWorker] = useState<WorkerItem | null>(null);
  const [allWorkers, setAllWorkers] = useState<WorkerItem[]>([]);
  const hasInitializedDefaultTab = useRef(false);

  // RF-ADMIN-05-1: Global Time Range Context Filter
  const [timeRange, setTimeRange] = useState<TimeRangeFilter>({ preset: 'today' });
  const [customFrom, setCustomFrom] = useState<string>('');
  const [customTo, setCustomTo] = useState<string>('');

  /**
   * Gates sequencing (docs/admin-app-spec.md §5.2):
   * T1 terms → subscription (L2) → organization (O1) → key state (K*).
   * Only a green gate mounts the console.
   */
  const runGates = useCallback(async () => {
    setGate({ kind: 'checking' });
    try {
      const legal = await legalStatus();
      if (legal.needs_acceptance) {
        setGate({ kind: 'terms', status: legal });
        return;
      }
    } catch (e) {
      setGate({ kind: 'error', message: describeError(e, 'No se pudo verificar el estado de los términos legales.') });
      return;
    }

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

      if (!hasInitializedDefaultTab.current) {
        hasInitializedDefaultTab.current = true;
        employerGet()
          .then(profile => {
            if (profile) {
              setActiveTab('attendance-live');
            } else {
              setActiveTab('dashboard');
            }
          })
          .catch(() => {
            setActiveTab('dashboard');
          });
      }
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
    hasInitializedDefaultTab.current = false;
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
      <div className="min-h-screen bg-bg flex items-center justify-center text-sm text-muted gap-2">
        <RefreshCw className="w-4 h-4 animate-spin" /> Verificando suscripción y llave…
      </div>
    );
  }

  if (gate.kind === 'terms') {
    return <TermsGate onAccepted={runGates} onLogout={handleLogout} />;
  }

  if (gate.kind === 'error') {
    return (
      <div className="min-h-screen bg-bg flex items-center justify-center p-4">
        <div className="max-w-md w-full bg-surface border border-default rounded-card p-6 space-y-4 text-center">
          <p className="text-sm text-danger">{gate.message}</p>
          <div className="flex justify-center gap-2">
            <button onClick={runGates} className="px-4 py-2 bg-accent hover:bg-accent-hover text-accent-fg text-xs rounded-control">
              Reintentar
            </button>
            <button onClick={handleLogout} className="px-4 py-2 bg-surface-subtle hover:bg-surface-subtle text-fg text-xs rounded-control">
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

  const isTelemetryView = [
    'system',
    'network',
    'productivity',
    'files',
    'location',
  ].includes(activeTab);

  const handleNavigateToAttendance = (workerId?: string) => {
    if (workerId) {
      setAttendanceWorkerId(workerId);
      setActiveTab('attendance-records');
    } else {
      setActiveTab('attendance-live');
    }
  };

  return (
    <div className="flex h-screen bg-surface text-fg font-sans overflow-hidden">
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
      <aside className="w-64 bg-ink border-r border-ink-line flex flex-col justify-between shrink-0">
        <div>
          {/* Logo / Header */}
          <div className="p-5 border-b border-ink-line flex items-center gap-3">
            <div className="w-9 h-9 rounded-card bg-brand flex items-center justify-center font-bold text-white ">
              T
            </div>
            <div>
              <h1 className="text-sm font-semibold text-ink-fg">Telemetry</h1>
              <p className="text-[11px] text-ink-muted">Consola de administración</p>
            </div>
          </div>

          {/* Navigation Links */}
          <nav className="p-3 space-y-1 text-xs">
            <button
              onClick={() => setActiveTab('dashboard')}
              className={`w-full flex items-center gap-3 px-3 py-2 rounded-control font-medium border-l-2 transition-colors ${
                activeTab === 'dashboard'
                  ? 'bg-ink-raised text-ink-fg border-brand [&>svg]:text-brand'
                  : 'text-ink-muted border-transparent hover:bg-ink-raised hover:text-ink-fg'
              }`}
            >
              <LayoutDashboard className="w-4 h-4" />
              <span>Tablero Principal & Enrolamiento</span>
            </button>

            {/* Kicker: ASISTENCIA (RF-ADMIN-15-1) */}
            <div className="pt-3 pb-1 px-3 text-[10px] font-semibold text-ink-muted tracking-wider uppercase">
              Asistencia
            </div>

            <button
              onClick={() => setActiveTab('attendance-live')}
              className={`w-full flex items-center gap-3 px-3 py-2 rounded-control font-medium border-l-2 transition-colors ${
                activeTab === 'attendance-live'
                  ? 'bg-ink-raised text-ink-fg border-brand [&>svg]:text-brand'
                  : 'text-ink-muted border-transparent hover:bg-ink-raised hover:text-ink-fg'
              }`}
            >
              <Clock className="w-4 h-4" />
              <span>Asistencia en vivo</span>
            </button>

            <button
              onClick={() => setActiveTab('attendance-records')}
              className={`w-full flex items-center gap-3 px-3 py-2 rounded-control font-medium border-l-2 transition-colors ${
                activeTab === 'attendance-records'
                  ? 'bg-ink-raised text-ink-fg border-brand [&>svg]:text-brand'
                  : 'text-ink-muted border-transparent hover:bg-ink-raised hover:text-ink-fg'
              }`}
            >
              <CalendarCheck className="w-4 h-4" />
              <span>Marcas y correcciones</span>
            </button>

            <button
              onClick={() => setActiveTab('attendance-reports')}
              className={`w-full flex items-center gap-3 px-3 py-2 rounded-control font-medium border-l-2 transition-colors ${
                activeTab === 'attendance-reports'
                  ? 'bg-ink-raised text-ink-fg border-brand [&>svg]:text-brand'
                  : 'text-ink-muted border-transparent hover:bg-ink-raised hover:text-ink-fg'
              }`}
            >
              <FileText className="w-4 h-4" />
              <span>Reportes legales</span>
            </button>

            {/* Kicker: TELEMETRÍA (RF-ADMIN-15-1) */}
            <div className="pt-3 pb-1 px-3 text-[10px] font-semibold text-ink-muted tracking-wider uppercase">
              Telemetría
            </div>

            <button
              onClick={() => setActiveTab('system')}
              className={`w-full flex items-center gap-3 px-3 py-2 rounded-control font-medium border-l-2 transition-colors ${
                activeTab === 'system'
                  ? 'bg-ink-raised text-ink-fg border-brand [&>svg]:text-brand'
                  : 'text-ink-muted border-transparent hover:bg-ink-raised hover:text-ink-fg'
              }`}
            >
              <Activity className="w-4 h-4" />
              <span>Actividad del Sistema</span>
            </button>

            <button
              onClick={() => setActiveTab('network')}
              className={`w-full flex items-center gap-3 px-3 py-2 rounded-control font-medium border-l-2 transition-colors ${
                activeTab === 'network'
                  ? 'bg-ink-raised text-ink-fg border-brand [&>svg]:text-brand'
                  : 'text-ink-muted border-transparent hover:bg-ink-raised hover:text-ink-fg'
              }`}
            >
              <Globe className="w-4 h-4" />
              <span>Navegación de Red (FQDN)</span>
            </button>

            <button
              onClick={() => setActiveTab('productivity')}
              className={`w-full flex items-center gap-3 px-3 py-2 rounded-control font-medium border-l-2 transition-colors ${
                activeTab === 'productivity'
                  ? 'bg-ink-raised text-ink-fg border-brand [&>svg]:text-brand'
                  : 'text-ink-muted border-transparent hover:bg-ink-raised hover:text-ink-fg'
              }`}
            >
              <Camera className="w-4 h-4" />
              <span>Productividad & Capturas</span>
            </button>

            <button
              onClick={() => setActiveTab('files')}
              className={`w-full flex items-center gap-3 px-3 py-2 rounded-control font-medium border-l-2 transition-colors ${
                activeTab === 'files'
                  ? 'bg-ink-raised text-ink-fg border-brand [&>svg]:text-brand'
                  : 'text-ink-muted border-transparent hover:bg-ink-raised hover:text-ink-fg'
              }`}
            >
              <FileText className="w-4 h-4" />
              <span>Actividad de Archivos</span>
            </button>

            <button
              onClick={() => setActiveTab('location')}
              className={`w-full flex items-center gap-3 px-3 py-2 rounded-control font-medium border-l-2 transition-colors ${
                activeTab === 'location'
                  ? 'bg-ink-raised text-ink-fg border-brand [&>svg]:text-brand'
                  : 'text-ink-muted border-transparent hover:bg-ink-raised hover:text-ink-fg'
              }`}
            >
              <MapPin className="w-4 h-4" />
              <span>Ubicación en Terreno (FIELD)</span>
            </button>

            <div className="pt-2 border-t border-ink-line my-1" />

            <button
              onClick={() => setActiveTab('employer')}
              className={`w-full flex items-center gap-3 px-3 py-2 rounded-control font-medium border-l-2 transition-colors ${
                activeTab === 'employer'
                  ? 'bg-ink-raised text-ink-fg border-brand [&>svg]:text-brand'
                  : 'text-ink-muted border-transparent hover:bg-ink-raised hover:text-ink-fg'
              }`}
            >
              <Building2 className="w-4 h-4" />
              <span>Empresa</span>
            </button>

            <button
              onClick={() => setActiveTab('compliance')}
              className={`w-full flex items-center gap-3 px-3 py-2 rounded-control font-medium border-l-2 transition-colors ${
                activeTab === 'compliance'
                  ? 'bg-ink-raised text-ink-fg border-brand [&>svg]:text-brand'
                  : 'text-ink-muted border-transparent hover:bg-ink-raised hover:text-ink-fg'
              }`}
            >
              <ShieldCheck className="w-4 h-4" />
              <span>Bóveda & Cumplimiento</span>
            </button>
          </nav>
        </div>

        {/* User Session & Lock Button */}
        <div className="p-3 border-t border-ink-line space-y-2">
          {/* Active Admin User Card */}
          <div className="p-2.5 bg-ink-raised rounded-card border border-ink-line text-xs">
            <div className="flex items-center gap-2 text-ink-fg font-semibold truncate">
              <User className="w-3.5 h-3.5 text-brand shrink-0" />
              <span className="truncate">{adminProfile.name}</span>
            </div>
            <div className="text-[11px] text-ink-muted truncate mt-0.5">{adminProfile.email}</div>
            <div className="flex items-center justify-between mt-2 pt-2 border-t border-ink-line text-[10px] text-ink-muted font-mono">
              <span>{adminProfile.organization || 'Sin organización'}</span>
              <span className="flex items-center gap-1 text-ink-muted">
                <span className="w-1.5 h-1.5 rounded-full bg-success"></span>
                Autenticado
              </span>
            </div>
          </div>

          <div className="flex gap-2">
            <button
              onClick={lockApp}
              className="flex-1 bg-ink-raised hover:bg-ink-line text-ink-fg border border-ink-line px-2.5 py-2 rounded-card text-xs flex items-center justify-center gap-1.5 transition"
              title="Bloquear sesión por inactividad y purgar RAM"
            >
              <Lock className="w-3.5 h-3.5 text-ink-muted" />
              <span>Bloquear</span>
            </button>

            <button
              onClick={handleLogout}
              className="p-2 bg-ink-raised hover:bg-ink-line text-ink-muted hover:text-brand border border-ink-line rounded-card transition"
              title="Cerrar Sesión"
            >
              <LogOut className="w-3.5 h-3.5" />
            </button>
          </div>

          {/* Auto-Lock Inactivity Timeout Configuration (RF-ADMIN-07-1) */}
          <div className="flex items-center justify-between text-[11px] text-ink-muted px-1 pt-0.5">
            <span>Auto-bloqueo:</span>
            <select
              value={timeoutMinutes}
              onChange={e => setTimeoutMinutes(parseInt(e.target.value, 10))}
              className="bg-ink-raised border border-ink-line text-ink-fg rounded-control px-1.5 py-0.5 text-[11px] focus:outline-hidden focus:border-brand cursor-pointer"
            >
              <option value={5}>5 min</option>
              <option value={10}>10 min</option>
              <option value={15}>15 min</option>
              <option value={30}>30 min</option>
            </select>
          </div>

        </div>
      </aside>

      {/* Main Content Area */}
      <main className="flex-1 flex flex-col overflow-hidden">
        {gate.subscription && shouldShowTrialBanner(gate.subscription) && trialDays !== null && (
          <TrialBanner daysLeft={trialDays} />
        )}
        {/* RF-ADMIN-05-1: Global Context Header with Worker and Time Range Selectors (visible only on telemetry) */}
        {isTelemetryView && (
          <header className="h-16 border-b border-default px-6 flex items-center justify-between shrink-0 bg-surface">
            <div className="flex items-center gap-4 flex-wrap">
              {/* Worker Selector Dropdown */}
              <div className="flex items-center gap-2">
                <span className="text-xs font-medium text-muted">Trabajador:</span>
                <div className="relative">
                  <select
                    value={selectedWorker?.id ?? ''}
                    onChange={e => {
                      const w = allWorkers.find(item => item.id === e.target.value);
                      if (w) setSelectedWorker(w);
                    }}
                    className="bg-surface-subtle border border-default text-fg text-xs font-semibold rounded-control pl-3 pr-8 py-1.5 appearance-none focus:outline-hidden focus:border-accent cursor-pointer"
                  >
                    {allWorkers.map(w => (
                      <option key={w.id} value={w.id}>
                        {w.name} ({w.role}){w.isActive ? '' : ' · inactivo'}
                      </option>
                    ))}
                  </select>
                  <ChevronDown className="w-3.5 h-3.5 text-muted absolute right-2.5 top-1/2 -translate-y-1/2 pointer-events-none" />
                </div>
              </div>

              {/* Time Range Selector (RF-ADMIN-05-1) */}
              <div className="flex items-center gap-2">
                <Calendar className="w-3.5 h-3.5 text-muted" />
                <span className="text-xs font-medium text-muted">Rango:</span>
                <div className="relative">
                  <select
                    value={timeRange.preset}
                    onChange={e => handlePresetChange(e.target.value as TimeRangePreset)}
                    className="bg-surface-subtle border border-default text-fg text-xs font-medium rounded-control pl-3 pr-8 py-1.5 appearance-none focus:outline-hidden focus:border-accent cursor-pointer"
                  >
                    <option value="today">Hoy</option>
                    <option value="yesterday">Ayer</option>
                    <option value="7days">Últimos 7 días</option>
                    <option value="custom">Personalizado</option>
                  </select>
                  <ChevronDown className="w-3.5 h-3.5 text-muted absolute right-2.5 top-1/2 -translate-y-1/2 pointer-events-none" />
                </div>

                {/* Custom Date Inputs */}
                {timeRange.preset === 'custom' && (
                  <div className="flex items-center gap-1.5 text-xs">
                    <input
                      type="date"
                      value={customFrom}
                      onChange={e => handleCustomDateChange(e.target.value, customTo)}
                      className="bg-surface-subtle border border-default text-fg rounded-sm px-2 py-1 text-xs focus:outline-hidden focus:border-accent"
                    />
                    <span className="text-muted">-</span>
                    <input
                      type="date"
                      value={customTo}
                      onChange={e => handleCustomDateChange(customFrom, e.target.value)}
                      className="bg-surface-subtle border border-default text-fg rounded-sm px-2 py-1 text-xs focus:outline-hidden focus:border-accent"
                    />
                  </div>
                )}
              </div>
            </div>

            {/* Crypto Status & Time */}
            <div className="flex items-center gap-4 text-xs font-mono">
              <span className="bg-surface-subtle text-muted border border-default px-2.5 py-1 rounded-badge">
                X25519-AES256GCM
              </span>
              <span className="text-muted hidden sm:inline">
                {selectedWorker?.timezone ?? 'America/Santiago'}
              </span>
            </div>
          </header>
        )}

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
              onNavigateToEmployer={() => setActiveTab('employer')}
              onNavigateToAttendance={handleNavigateToAttendance}
            />
          )}

          {activeTab === 'attendance-live' && (
            <AttendanceBoard
              onSelectWorker={workerId => {
                setAttendanceWorkerId(workerId);
                setActiveTab('attendance-records');
              }}
              onNavigateToDashboard={() => setActiveTab('dashboard')}
            />
          )}

          {activeTab === 'attendance-records' && (
            <AttendanceDays
              key={attendanceWorkerId ?? 'all'}
              initialWorkerId={attendanceWorkerId ?? undefined}
              onWorkerChanged={workerId => setAttendanceWorkerId(workerId)}
            />
          )}

          {activeTab === 'attendance-reports' && <LegalReports />}

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

          {activeTab === 'employer' && (
            <EmployerProfile onProfileUpdated={() => runGates()} />
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
