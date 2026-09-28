import React, { useState, useEffect } from 'react';
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
} from 'lucide-react';
import {
  WorkerItem,
  AdminProfile,
  getWorkersList,
  lockAndFlushMemory,
  getAdminSession,
  adminLogout,
  getApiBaseUrl,
} from './api/tauri-bridge';
import { AuthView } from './components/AuthView';
import { Dashboard } from './components/Dashboard';
import { ActivityChart } from './components/ActivityChart';
import { NetworkExplorer } from './components/NetworkExplorer';
import { ScreenshotGallery } from './components/ScreenshotGallery';
import { FieldMap } from './components/FieldMap';
import { ConsentSettings } from './components/ConsentSettings';
import { AutoLockModal } from './components/AutoLockModal';

export const App: React.FC = () => {
  const [adminProfile, setAdminProfile] = useState<AdminProfile | null>(null);
  const [activeTab, setActiveTab] = useState<'dashboard' | 'system' | 'network' | 'productivity' | 'location' | 'compliance'>('dashboard');
  const [selectedWorker, setSelectedWorker] = useState<WorkerItem | null>(null);
  const [allWorkers, setAllWorkers] = useState<WorkerItem[]>([]);
  const [isLocked, setIsLocked] = useState<boolean>(false);

  useEffect(() => {
    getAdminSession().then(sess => {
      if (sess) setAdminProfile(sess);
    });
  }, []);

  useEffect(() => {
    if (adminProfile) {
      getWorkersList().then(list => {
        setAllWorkers(list);
        if (list.length > 0 && !selectedWorker) {
          setSelectedWorker(list[0]);
        }
      });
    }
  }, [adminProfile]);

  // Inactivity Auto-Lock Timer (5 minutes)
  useEffect(() => {
    let timer: ReturnType<typeof setTimeout>;

    const resetTimer = () => {
      if (isLocked || !adminProfile) return;
      clearTimeout(timer);
      timer = setTimeout(() => {
        setIsLocked(true);
        lockAndFlushMemory();
      }, 5 * 60 * 1000); // 5 min
    };

    const events = ['mousemove', 'keydown', 'mousedown', 'scroll'];
    events.forEach(e => window.addEventListener(e, resetTimer));
    resetTimer();

    return () => {
      events.forEach(e => window.removeEventListener(e, resetTimer));
      clearTimeout(timer);
    };
  }, [isLocked, adminProfile]);

  const handleLogout = async () => {
    await adminLogout();
    setAdminProfile(null);
  };

  // If not logged in, render the login / register view
  if (!adminProfile) {
    return <AuthView onLoginSuccess={(profile) => setAdminProfile(profile)} />;
  }

  return (
    <div className="flex h-screen bg-slate-900 text-slate-100 font-sans overflow-hidden">
      {/* Auto-Lock Security Overlay */}
      <AutoLockModal isLocked={isLocked} onUnlock={() => setIsLocked(false)} />

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
              <span>{adminProfile.organization}</span>
              <span className="flex items-center gap-1 text-emerald-400">
                <span className="w-1.5 h-1.5 rounded-full bg-emerald-400"></span>
                Autenticado
              </span>
            </div>
          </div>

          <div className="flex gap-2">
            <button
              onClick={() => {
                setIsLocked(true);
                lockAndFlushMemory();
              }}
              className="flex-1 bg-slate-900 hover:bg-slate-800 text-slate-300 border border-slate-700/80 px-2.5 py-2 rounded-xl text-xs flex items-center justify-center gap-1.5 transition"
              title="Bloquear sesión por inactividad"
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

          <div className="text-[10px] text-slate-500 font-mono text-center flex items-center justify-center gap-1">
            <Server className="w-3 h-3 text-slate-600" />
            <span className="truncate max-w-[200px]">{getApiBaseUrl()}</span>
          </div>
        </div>
      </aside>

      {/* Main Content Area */}
      <main className="flex-1 flex flex-col overflow-hidden">
        {/* Global Context Header */}
        <header className="h-16 border-b border-slate-800 px-6 flex items-center justify-between flex-shrink-0 bg-slate-900/60 backdrop-blur">
          {/* Worker Selector Dropdown */}
          <div className="flex items-center gap-3">
            <span className="text-xs font-medium text-slate-400">Trabajador Seleccionado:</span>
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

          {/* Crypto Status & Time */}
          <div className="flex items-center gap-4 text-xs font-mono">
            <span className="bg-indigo-950/80 text-indigo-300 border border-indigo-800/60 px-2.5 py-1 rounded-md">
              X25519-AES256GCM
            </span>
            <span className="text-slate-400">
              Zona Horaria: {selectedWorker?.timezone ?? 'America/Santiago'}
            </span>
          </div>
        </header>

        {/* View Content */}
        <div className="flex-1 overflow-y-auto p-6">
          {activeTab === 'dashboard' && (
            <Dashboard
              onSelectWorker={worker => {
                setSelectedWorker(worker);
                setActiveTab('system');
              }}
            />
          )}

          {activeTab === 'system' && (
            <ActivityChart workerName={selectedWorker?.name ?? 'Empleado'} />
          )}

          {activeTab === 'network' && (
            <NetworkExplorer workerName={selectedWorker?.name ?? 'Empleado'} />
          )}

          {activeTab === 'productivity' && (
            <ScreenshotGallery workerName={selectedWorker?.name ?? 'Empleado'} />
          )}

          {activeTab === 'location' && (
            <FieldMap
              workerName={selectedWorker?.name ?? 'Empleado'}
              workerRole={selectedWorker?.role ?? 'OFFICE'}
              workHours={{
                start: selectedWorker?.workHoursStart ?? '09:00',
                end: selectedWorker?.workHoursEnd ?? '18:00',
              }}
            />
          )}

          {activeTab === 'compliance' && <ConsentSettings />}
        </div>
      </main>
    </div>
  );
};

export default App;
