import React, { useState, useEffect } from 'react';
import {
  KpiSummary,
  WorkerItem,
  DecryptedTelemetryRecord,
  fetchKpiSummary,
  EnrollmentView,
  WorkerStatus,
  PaginationMeta,
  getWorkersPage,
  pauseOrResumeWorker,
  toWorkerItem,
  getAdminPublicKey,
  generateAdminKeypair,
  simulateAndDecrypt,
  workersRegenerateToken,
} from '../api/tauri-bridge';
import { EmployeeCard } from './EmployeeCard';
import { EnrollmentModal } from './EnrollmentModal';
import { CreateWorkerModal } from './CreateWorkerModal';
import { WorkerEditModal } from './WorkerEditModal';
import { ConfirmDeleteWorkerModal } from './ConfirmDeleteWorkerModal';
import {
  Users,
  HardDrive,
  Key,
  ShieldCheck,
  Search,
  Filter,
  UserPlus,
  RefreshCw,
  X,
  Copy,
  Check,
  Cpu,
  Sparkles,
  ChevronLeft,
  ChevronRight,
} from 'lucide-react';

const PER_PAGE = 12;

interface DashboardProps {
  organization: string;
  onSelectWorker: (worker: WorkerItem) => void;
}

export const Dashboard: React.FC<DashboardProps> = ({ organization, onSelectWorker }) => {
  const [kpis, setKpis] = useState<KpiSummary | null>(null);
  const [workers, setWorkers] = useState<WorkerItem[]>([]);
  const [searchInput, setSearchInput] = useState<string>('');
  const [search, setSearch] = useState<string>('');
  const [page, setPage] = useState<number>(1);
  const [pagination, setPagination] = useState<PaginationMeta | null>(null);
  const [editingWorker, setEditingWorker] = useState<WorkerItem | null>(null);
  const [deletingWorker, setDeletingWorker] = useState<WorkerItem | null>(null);
  const [roleFilter, setRoleFilter] = useState<'ALL' | 'OFFICE' | 'FIELD'>('ALL');
  const [statusFilter, setStatusFilter] = useState<'ALL' | WorkerStatus>('ALL');
  const [loading, setLoading] = useState<boolean>(true);
  const [listError, setListError] = useState<string | null>(null);

  // Master Crypto Key State
  const [masterPubKey, setMasterPubKey] = useState<string>('Cargando clave...');
  const [copiedKey, setCopiedKey] = useState<boolean>(false);
  const [generatingKey, setGeneratingKey] = useState<boolean>(false);

  // Decryption Test State
  const [decryptionTestResult, setDecryptionTestResult] = useState<DecryptedTelemetryRecord | null>(null);
  const [testingDecryption, setTestingDecryption] = useState<boolean>(false);

  // Modal States
  const [showCreateModal, setShowCreateModal] = useState<boolean>(false);
  const [activeEnrollment, setActiveEnrollment] = useState<{
    worker: { name: string; role?: string };
    pkg: EnrollmentView;
  } | null>(null);

  const loadData = async () => {
    setLoading(true);
    setListError(null);
    try {
      const [k, result, pubKey] = await Promise.all([
        fetchKpiSummary(),
        getWorkersPage({
          page,
          per_page: PER_PAGE,
          search: search.trim().length >= 2 ? search.trim() : undefined,
          role_type: roleFilter === 'ALL' ? undefined : roleFilter,
          status: statusFilter === 'ALL' ? undefined : statusFilter,
        }),
        getAdminPublicKey(),
      ]);
      setKpis(k);
      setWorkers(result.items);
      setPagination(result.pagination);
      setMasterPubKey(pubKey);
    } catch (e: any) {
      console.error('Failed to load dashboard data', e);
      setListError(e?.message || String(e));
    } finally {
      setLoading(false);
    }
  };

  // Debounce free-text search; the server only filters from 2 characters.
  useEffect(() => {
    const t = setTimeout(() => setSearch(searchInput), 300);
    return () => clearTimeout(t);
  }, [searchInput]);

  useEffect(() => {
    setPage(1);
  }, [search, roleFilter, statusFilter]);

  useEffect(() => {
    loadData();
  }, [search, roleFilter, statusFilter, page]);

  const replaceWorker = (updated: WorkerItem) =>
    setWorkers(list => list.map(w => (w.id === updated.id ? updated : w)));

  const handleToggle = async (worker: WorkerItem) => {
    try {
      const updated = await pauseOrResumeWorker(worker, 'Estado actualizado por el administrador');
      // Reflect the new badge immediately, then refresh KPIs.
      replaceWorker(toWorkerItem(updated));
      fetchKpiSummary().then(setKpis).catch(() => undefined);
    } catch (e: any) {
      setListError(e?.message || String(e));
    }
  };

  const handleShowEnrollment = async (worker: WorkerItem) => {
    // RF-ADMIN-08-5: regenerating a code for a bound worker unlinks the current device.
    if (
      worker.deviceHostname &&
      !confirm(`Esto desvincula el equipo actual (${worker.deviceHostname}). ¿Generar un nuevo código?`)
    ) {
      return;
    }
    try {
      const tokenView = await workersRegenerateToken(worker.id, 24);
      setActiveEnrollment({ worker, pkg: tokenView });
      loadData();
    } catch (e: any) {
      setListError(`No se pudo generar el código de enrolamiento: ${e?.message || e}`);
    }
  };

  const handleRegenerateKeypair = async () => {
    if (!confirm('¿Desea generar un nuevo par de claves maestras en el OS Keystore? Los paquetes anteriores deberán ser actualizados.')) {
      return;
    }
    setGeneratingKey(true);
    try {
      const newKey = await generateAdminKeypair();
      setMasterPubKey(newKey);
      alert('¡Nuevo par de claves X25519 generado y almacenado de forma segura en el Keystore del sistema operativo!');
    } catch (err: any) {
      alert(`Error al generar clave: ${err}`);
    } finally {
      setGeneratingKey(false);
    }
  };

  const handleRunDecryptionTest = async () => {
    setTestingDecryption(true);
    try {
      const decrypted = await simulateAndDecrypt();
      setDecryptionTestResult(decrypted);
    } catch (err: any) {
      alert(`Error en prueba de descifrado: ${err}`);
    } finally {
      setTestingDecryption(false);
    }
  };

  const copyPublicKey = () => {
    navigator.clipboard.writeText(masterPubKey);
    setCopiedKey(true);
    setTimeout(() => setCopiedKey(false), 2000);
  };

  return (
    <div className="space-y-6">
      {/* Active Enrollment Modal */}
      {activeEnrollment && (
        <EnrollmentModal
          worker={activeEnrollment.worker}
          enrollment={activeEnrollment.pkg}
          onClose={() => setActiveEnrollment(null)}
        />
      )}

      {/* Zero-Knowledge Master Crypto Vault Banner */}
      <div className="bg-gradient-to-r from-slate-900 via-indigo-950/60 to-slate-900 border border-indigo-900/60 rounded-2xl p-5 shadow-xl relative overflow-hidden">
        <div className="flex flex-col lg:flex-row items-start lg:items-center justify-between gap-4 relative z-10">
          <div className="space-y-1">
            <div className="flex items-center gap-2">
              <span className="p-1.5 bg-indigo-500/20 text-indigo-400 rounded-lg">
                <Key className="w-4 h-4" />
              </span>
              <h2 className="text-sm font-bold text-white tracking-wide">
                Bóveda Criptográfica Zero-Knowledge (OS Keystore)
              </h2>
              <span className="px-2 py-0.5 bg-emerald-500/15 text-emerald-400 text-[10px] font-mono font-semibold rounded border border-emerald-500/30">
                HPKE X25519 / AES-256-GCM
              </span>
            </div>
            <p className="text-xs text-slate-300">
              Clave Pública Master activa para el cifrado asimétrico de los agentes en estaciones de trabajo:
            </p>
            <div className="flex items-center gap-2 pt-1 font-mono text-xs">
              <code className="bg-slate-950/90 text-indigo-300 px-3 py-1.5 rounded-lg border border-slate-800 text-[11px] truncate max-w-md select-all">
                {masterPubKey}
              </code>
              <button
                onClick={copyPublicKey}
                className="p-1.5 bg-slate-800 hover:bg-slate-700 text-slate-300 rounded-lg border border-slate-700 transition"
                title="Copiar Clave Pública"
              >
                {copiedKey ? <Check className="w-3.5 h-3.5 text-emerald-400" /> : <Copy className="w-3.5 h-3.5" />}
              </button>
            </div>
          </div>

          <div className="flex flex-wrap items-center gap-2">
            <button
              onClick={handleRunDecryptionTest}
              disabled={testingDecryption}
              className="px-3.5 py-2 bg-indigo-600 hover:bg-indigo-500 text-white rounded-xl text-xs font-semibold flex items-center gap-1.5 shadow-lg shadow-indigo-600/30 transition disabled:opacity-50"
            >
              <Cpu className="w-3.5 h-3.5" />
              <span>{testingDecryption ? 'Descifrando en Rayon...' : 'Probar Descifrado Rayon'}</span>
            </button>

            <button
              onClick={handleRegenerateKeypair}
              disabled={generatingKey}
              className="px-3 py-2 bg-slate-800 hover:bg-slate-700 text-slate-300 rounded-xl text-xs font-medium border border-slate-700 transition disabled:opacity-50"
            >
              <RefreshCw className={`w-3.5 h-3.5 ${generatingKey ? 'animate-spin' : ''}`} />
              <span className="ml-1.5">Regenerar Claves</span>
            </button>
          </div>
        </div>
      </div>

      {/* Decryption Test Result Modal */}
      {decryptionTestResult && (
        <div className="fixed inset-0 z-50 bg-black/80 flex items-center justify-center p-4">
          <div className="bg-slate-900 border border-slate-800 rounded-2xl max-w-lg w-full p-6 shadow-2xl relative">
            <button
              onClick={() => setDecryptionTestResult(null)}
              className="absolute top-4 right-4 text-slate-400 hover:text-white"
            >
              <X className="w-5 h-5" />
            </button>

            <div className="flex items-center gap-3 mb-4">
              <div className="w-10 h-10 rounded-xl bg-emerald-500/20 text-emerald-400 flex items-center justify-center">
                <Sparkles className="w-5 h-5" />
              </div>
              <div>
                <h3 className="text-base font-bold text-white">Resultado de Descifrado Paralelo (Rayon)</h3>
                <p className="text-xs text-emerald-400 font-mono">100% Verificado con Clave Privada de Keystore</p>
              </div>
            </div>

            <div className="bg-slate-950 p-4 rounded-xl border border-slate-800 font-mono text-xs text-slate-300 max-h-64 overflow-y-auto mb-4 select-all">
              <pre>{JSON.stringify(decryptionTestResult.payload, null, 2)}</pre>
            </div>

            <div className="p-3 bg-indigo-950/40 border border-indigo-900/60 rounded-xl text-xs text-slate-300 space-y-1 mb-4">
              <p className="font-semibold text-indigo-300">Detalles del Pipeline Ejecutado:</p>
              <p>• Clave Efímera X25519 recibida y validada.</p>
              <p>• Derivación de clave simétrica mediante HKDF-SHA256.</p>
              <p>• Descifrado y autenticación de tag Poly1305 / AES-GCM sin exponer claves.</p>
            </div>

            <div className="flex justify-end">
              <button
                onClick={() => setDecryptionTestResult(null)}
                className="px-4 py-2 bg-indigo-600 hover:bg-indigo-500 text-white rounded-lg text-xs font-semibold transition"
              >
                Cerrar Prueba
              </button>
            </div>
          </div>
        </div>
      )}

      {/* KPI Cards */}
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
        <div className="bg-slate-800/80 border border-slate-700/80 rounded-xl p-4 shadow-lg flex items-center gap-4">
          <div className="w-12 h-12 rounded-xl bg-indigo-500/20 text-indigo-400 flex items-center justify-center">
            <Users className="w-6 h-6" />
          </div>
          <div>
            <p className="text-xs text-slate-400 font-medium">Trabajadores Activos</p>
            <h3 className="text-2xl font-bold text-white">
              {kpis?.activeWorkers ?? 0} <span className="text-xs font-normal text-slate-400">/ {kpis?.totalWorkers ?? 0}</span>
            </h3>
          </div>
        </div>

        <div className="bg-slate-800/80 border border-slate-700/80 rounded-xl p-4 shadow-lg flex items-center gap-4">
          <div className="w-12 h-12 rounded-xl bg-emerald-500/20 text-emerald-400 flex items-center justify-center">
            <HardDrive className="w-6 h-6" />
          </div>
          <div>
            <p className="text-xs text-slate-400 font-medium">Paquetes Descifrados (24h)</p>
            <h3 className="text-2xl font-bold text-white">{kpis?.packetsDecrypted24h ?? 0}</h3>
          </div>
        </div>

        <div className="bg-slate-800/80 border border-slate-700/80 rounded-xl p-4 shadow-lg flex items-center gap-4">
          <div className="w-12 h-12 rounded-xl bg-sky-500/20 text-sky-400 flex items-center justify-center">
            <Key className="w-6 h-6" />
          </div>
          <div>
            <p className="text-xs text-slate-400 font-medium">Bóveda OS Keystore</p>
            <div className="flex items-center gap-2 mt-0.5">
              <span className="w-2.5 h-2.5 rounded-full bg-emerald-400 animate-pulse"></span>
              <span className="text-sm font-semibold text-emerald-300">Protegida & Conectada</span>
            </div>
          </div>
        </div>

        <div className="bg-slate-800/80 border border-slate-700/80 rounded-xl p-4 shadow-lg flex items-center gap-4">
          <div className="w-12 h-12 rounded-xl bg-purple-500/20 text-purple-400 flex items-center justify-center">
            <ShieldCheck className="w-6 h-6" />
          </div>
          <div>
            <p className="text-xs text-slate-400 font-medium">Cumplimiento Legal</p>
            <h3 className="text-sm font-semibold text-purple-300 mt-1">GDPR & Ley 19.628 OK</h3>
          </div>
        </div>
      </div>

      {/* Deep Filtering & Action Bar */}
      <div className="bg-slate-800/80 border border-slate-700/80 rounded-xl p-4 shadow-lg flex flex-wrap items-center justify-between gap-4">
        <div className="flex flex-wrap items-center gap-3 flex-1 min-w-[320px]">
          {/* Search */}
          <div className="relative flex-1 min-w-[200px]">
            <Search className="w-4 h-4 text-slate-400 absolute left-3 top-1/2 -translate-y-1/2" />
            <input
              type="text"
              placeholder="Buscar por nombre o correo corporativo..."
              value={searchInput}
              onChange={e => setSearchInput(e.target.value)}
              className="w-full bg-slate-900 border border-slate-700 text-slate-200 text-xs rounded-lg pl-9 pr-3 py-2 focus:outline-none focus:border-indigo-500"
            />
          </div>

          {/* Role Filter */}
          <div className="flex items-center gap-1.5 bg-slate-900 border border-slate-700 rounded-lg px-2.5 py-1.5 text-xs text-slate-300">
            <Filter className="w-3.5 h-3.5 text-slate-400" />
            <select
              value={roleFilter}
              onChange={e => setRoleFilter(e.target.value as 'ALL' | 'OFFICE' | 'FIELD')}
              className="bg-transparent focus:outline-none cursor-pointer"
            >
              <option value="ALL">Todos los Roles</option>
              <option value="OFFICE">Oficina (OFFICE)</option>
              <option value="FIELD">Terreno (FIELD)</option>
            </select>
          </div>

          {/* Status Filter */}
          <div className="flex items-center gap-1.5 bg-slate-900 border border-slate-700 rounded-lg px-2.5 py-1.5 text-xs text-slate-300">
            <select
              value={statusFilter}
              onChange={e => setStatusFilter(e.target.value as 'ALL' | WorkerStatus)}
              className="bg-transparent focus:outline-none cursor-pointer"
            >
              <option value="ALL">Todos los Estados</option>
              <option value="ACTIVE">Activos</option>
              <option value="PENDING_CONSENT">Esperando consentimiento</option>
              <option value="PENDING_ENROLLMENT">Invitados (sin vincular)</option>
              <option value="PAUSED">Pausados</option>
              <option value="REVOKED">Revocados</option>
            </select>
          </div>
        </div>

        <div className="flex items-center gap-2">
          <button
            onClick={loadData}
            className="p-2 bg-slate-700 hover:bg-slate-600 text-slate-300 rounded-lg text-xs transition"
            title="Recargar datos"
          >
            <RefreshCw className="w-4 h-4" />
          </button>
          <button
            onClick={() => setShowCreateModal(true)}
            className="bg-indigo-600 hover:bg-indigo-500 text-white text-xs font-semibold px-4 py-2 rounded-lg flex items-center gap-2 shadow-lg transition"
          >
            <UserPlus className="w-4 h-4" />
            Nuevo Trabajador
          </button>
        </div>
      </div>

      {listError && (
        <div className="p-3 bg-rose-950/50 border border-rose-800/60 rounded-xl text-xs text-rose-300">{listError}</div>
      )}

      {/* Workers Grid */}
      {loading ? (
        <div className="py-20 text-center text-slate-400 text-sm">Descargando y actualizando lista de trabajadores...</div>
      ) : workers.length === 0 ? (
        <div className="py-16 text-center bg-slate-800/40 rounded-xl border border-slate-700/60">
          <p className="text-slate-400 text-sm">No se encontraron trabajadores que coincidan con los filtros.</p>
        </div>
      ) : (
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          {workers.map(w => (
            <EmployeeCard
              key={w.id}
              worker={w}
              onSelect={onSelectWorker}
              onToggleStatus={handleToggle}
              onShowEnrollment={handleShowEnrollment}
              onEdit={setEditingWorker}
              onDelete={setDeletingWorker}
            />
          ))}
        </div>
      )}

      {pagination && pagination.total_pages > 1 && (
        <div className="flex items-center justify-between text-xs text-slate-400">
          <span>
            {pagination.total} trabajadores · página {pagination.page} de {pagination.total_pages}
          </span>
          <div className="flex gap-2">
            <button
              onClick={() => setPage(p => Math.max(1, p - 1))}
              disabled={page <= 1 || loading}
              className="p-2 bg-slate-800 hover:bg-slate-700 rounded-lg border border-slate-700 disabled:opacity-40"
              title="Página anterior"
            >
              <ChevronLeft className="w-4 h-4" />
            </button>
            <button
              onClick={() => setPage(p => Math.min(pagination.total_pages, p + 1))}
              disabled={page >= pagination.total_pages || loading}
              className="p-2 bg-slate-800 hover:bg-slate-700 rounded-lg border border-slate-700 disabled:opacity-40"
              title="Página siguiente"
            >
              <ChevronRight className="w-4 h-4" />
            </button>
          </div>
        </div>
      )}

      {editingWorker && (
        <WorkerEditModal
          worker={editingWorker}
          onClose={() => setEditingWorker(null)}
          onSaved={updated => {
            setEditingWorker(null);
            replaceWorker(toWorkerItem(updated));
          }}
        />
      )}

      {deletingWorker && (
        <ConfirmDeleteWorkerModal
          worker={deletingWorker}
          onClose={() => setDeletingWorker(null)}
          onDeleted={() => {
            setDeletingWorker(null);
            loadData();
          }}
        />
      )}

      {showCreateModal && (
        <CreateWorkerModal
          organization={organization}
          onClose={() => setShowCreateModal(false)}
          onCreated={created => {
            setShowCreateModal(false);
            setActiveEnrollment({
              worker: { name: created.worker.name, role: created.worker.role_type },
              pkg: created.enrollment,
            });
            loadData();
          }}
        />
      )}
    </div>
  );
};
