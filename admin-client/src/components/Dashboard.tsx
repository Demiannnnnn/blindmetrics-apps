import React, { useState, useEffect } from 'react';
import {
  KpiSummary,
  WorkerItem,
  DecryptedTelemetryRecord,
  fetchKpiSummary,
  EnrollmentView,
  WorkerStatus,
  WorkMode,
  Workplace,
  PaginationMeta,
  getWorkersPage,
  pauseOrResumeWorker,
  toWorkerItem,
  getAdminPublicKey,
  simulateAndDecrypt,
  workersRegenerateToken,
  workersSetStatus,
  AdminProfile,
  employerGet,
  workplacesList,
  attendanceBoard,
  attendanceCorrections,
  workersTelemetryHealth,
  workersGet,
} from '../api/tauri-bridge';
import type { AttendanceState, TelemetryHealthResponse } from '../api/types';
import { describeError, errorIs } from '../lib/errors';
import { boardKpis } from '../lib/attendance';
import {
  formatLastTelemetry,
  formatRejectionDetail,
  formatRemainingCount,
  formatWithoutDataAlertLabel,
  formatRejectionsAlertLabel,
} from '../lib/telemetryHealth';
import { EmployeeCard } from './EmployeeCard';
import { EnrollmentModal } from './EnrollmentModal';
import { CreateWorkerModal } from './CreateWorkerModal';
import { WorkerEditModal } from './WorkerEditModal';
import { ConfirmDeleteWorkerModal } from './ConfirmDeleteWorkerModal';
import { WorkerDetailModal } from './WorkerDetailModal';
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
  ChevronLeft,
  ChevronRight,
  ChevronDown,
  Building2,
  Clock,
  AlertTriangle,
  FileCheck,
} from 'lucide-react';

const PER_PAGE = 12;

interface DashboardProps {
  organization: string;
  onSelectWorker: (worker: WorkerItem) => void;
  /** Opens "Bóveda & Cumplimiento" (key backups, rotation, devices). */
  onOpenVault: () => void;
  onProfileChanged: (profile: AdminProfile) => void;
  onNavigateToEmployer?: () => void;
  onNavigateToAttendance?: (workerId?: string) => void;
}

/** RF-ADMIN-09-4: the worker accepted consent on a device; revoke it before a new code. */
const RevokeFirstDialog: React.FC<{
  worker: WorkerItem;
  onClose: () => void;
  onRegenerated: (pkg: EnrollmentView) => void;
}> = ({ worker, onClose, onRegenerated }) => {
  const [typed, setTyped] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const confirmAndRegenerate = async () => {
    setBusy(true);
    setError(null);
    try {
      await workersSetStatus(worker.id, 'REVOKED', 'Cambio de equipo');
      onRegenerated(await workersRegenerateToken(worker.id, 24));
    } catch (e) {
      setError(describeError(e));
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className="fixed inset-0 z-50 bg-black/80 flex items-center justify-center p-4">
      <div className="bg-surface border border-default rounded-card max-w-md w-full p-6 space-y-3 ">
        <h3 className="text-base font-semibold text-fg">Vincular un equipo nuevo</h3>
        <p className="text-xs text-fg">
          {worker.name} ya aceptó el consentimiento en un equipo ({worker.deviceHostname ?? 'equipo actual'}). Para vincular un
          equipo nuevo, primero revoca el actual: dejará de enviar datos y el agente borrará sus credenciales.
        </p>
        <p className="text-xs text-muted">
          Escribe <span className="font-semibold text-fg">{worker.name}</span> para confirmar.
        </p>
        <input
          value={typed}
          onChange={e => setTyped(e.target.value)}
          className="w-full bg-bg border border-default rounded-control px-3 py-2 text-xs text-fg focus:outline-hidden focus:border-danger"
        />
        {error && <p className="text-xs text-danger">{error}</p>}
        <div className="flex justify-end gap-2">
          <button onClick={onClose} className="px-3 py-2 bg-surface-subtle hover:bg-surface-subtle text-fg text-xs rounded-control">
            Cancelar
          </button>
          <button
            disabled={busy || typed.trim() !== worker.name.trim()}
            onClick={confirmAndRegenerate}
            className="px-4 py-2 bg-danger hover:bg-danger-hover text-accent-fg text-xs font-semibold rounded-control disabled:opacity-40"
          >
            {busy ? 'Procesando…' : 'Revocar equipo actual y generar código'}
          </button>
        </div>
      </div>
    </div>
  );
};

export const Dashboard: React.FC<DashboardProps> = ({
  organization,
  onSelectWorker,
  onOpenVault,
  onProfileChanged,
  onNavigateToEmployer,
  onNavigateToAttendance,
}) => {
  const [kpis, setKpis] = useState<KpiSummary | null>(null);
  const [workers, setWorkers] = useState<WorkerItem[]>([]);
  const [searchInput, setSearchInput] = useState<string>('');
  const [search, setSearch] = useState<string>('');
  const [page, setPage] = useState<number>(1);
  const [pagination, setPagination] = useState<PaginationMeta | null>(null);
  const [editingWorker, setEditingWorker] = useState<WorkerItem | null>(null);
  const [deletingWorker, setDeletingWorker] = useState<WorkerItem | null>(null);
  const [detailWorker, setDetailWorker] = useState<WorkerItem | null>(null);
  const [roleFilter, setRoleFilter] = useState<'ALL' | 'OFFICE' | 'FIELD'>('ALL');
  const [workModeFilter, setWorkModeFilter] = useState<'ALL' | WorkMode>('ALL');
  const [workplaceFilter, setWorkplaceFilter] = useState<string>('ALL');
  const [workplaces, setWorkplaces] = useState<Workplace[]>([]);
  const [statusFilter, setStatusFilter] = useState<'ALL' | WorkerStatus>('ALL');
  const [loading, setLoading] = useState<boolean>(true);
  const [listError, setListError] = useState<string | null>(null);
  const [hasEmployerProfile, setHasEmployerProfile] = useState<boolean>(true);

  // Attendance KPIs & States (RF-ADMIN-15-7)
  const [attendanceStates, setAttendanceStates] = useState<Record<string, AttendanceState>>({});
  const [workingNowCount, setWorkingNowCount] = useState<number>(0);
  const [unmarkedTodayCount, setUnmarkedTodayCount] = useState<number>(0);
  const [lateStartCount, setLateStartCount] = useState<number>(0);
  const [pendingCorrectionsCount, setPendingCorrectionsCount] = useState<number>(0);

  // Telemetry Health Alerts (RF-ADMIN-25-2, RF-ADMIN-25-3, RF-ADMIN-25-4)
  const [telemetryHealth, setTelemetryHealth] = useState<TelemetryHealthResponse | null>(null);
  const [expandedWithoutData, setExpandedWithoutData] = useState<boolean>(false);
  const [expandedRejections, setExpandedRejections] = useState<boolean>(false);

  // Master Crypto Key State (read-only here; managed in Bóveda, TASK-ADMIN-11)
  const [masterPubKey, setMasterPubKey] = useState<string>('Cargando clave...');
  const [copiedKey, setCopiedKey] = useState<boolean>(false);
  const [revokeFirst, setRevokeFirst] = useState<WorkerItem | null>(null);

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
      getAdminPublicKey()
        .then(setMasterPubKey)
        .catch(() => setMasterPubKey('Sin llave en este equipo'));
      employerGet()
        .then(p => setHasEmployerProfile(p !== null))
        .catch(() => setHasEmployerProfile(true));
      workplacesList()
        .then(setWorkplaces)
        .catch(e => console.warn('Failed to load workplaces', e));
      attendanceBoard()
        .then(workers => {
          const map: Record<string, AttendanceState> = {};
          for (const bw of workers) {
            map[bw.worker_id] = bw.state;
          }
          const kpis = boardKpis(workers);
          setAttendanceStates(map);
          setWorkingNowCount(kpis.working);
          setUnmarkedTodayCount(kpis.unmarked);
          setLateStartCount(kpis.lateStart);
        })
        .catch(e => console.warn('Failed to load attendance board', e));
      attendanceCorrections({ status: 'PENDING' })
        .then(res => setPendingCorrectionsCount(res.pagination?.total ?? res.data.length))
        .catch(e => console.warn('Failed to load pending corrections', e));
      workersTelemetryHealth()
        .then(setTelemetryHealth)
        .catch(e => {
          console.warn('Failed to load telemetry health', e);
          setTelemetryHealth(null);
        });
      const [k, result] = await Promise.all([
        fetchKpiSummary(),
        getWorkersPage({
          page,
          per_page: PER_PAGE,
          search: search.trim().length >= 2 ? search.trim() : undefined,
          role_type: roleFilter === 'ALL' ? undefined : roleFilter,
          status: statusFilter === 'ALL' ? undefined : statusFilter,
          workplace_id: workplaceFilter === 'ALL' ? undefined : workplaceFilter,
          work_mode: workModeFilter === 'ALL' ? undefined : workModeFilter,
        }),
      ]);
      setKpis(k);
      setWorkers(result.items);
      setPagination(result.pagination);
    } catch (e: any) {
      console.error('Failed to load dashboard data', e);
      setListError(describeError(e));
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
  }, [search, roleFilter, workModeFilter, workplaceFilter, statusFilter]);

  useEffect(() => {
    loadData();
  }, [search, roleFilter, workModeFilter, workplaceFilter, statusFilter, page]);

  const replaceWorker = (updated: WorkerItem) =>
    setWorkers(list => list.map(w => (w.id === updated.id ? updated : w)));

  const handleToggle = async (worker: WorkerItem) => {
    try {
      const updated = await pauseOrResumeWorker(worker, 'Estado actualizado por el administrador');
      // Reflect the new badge immediately, then refresh KPIs.
      replaceWorker(toWorkerItem(updated));
      fetchKpiSummary().then(setKpis).catch(() => undefined);
    } catch (e: any) {
      setListError(describeError(e));
    }
  };

  const handleShowEnrollment = async (worker: WorkerItem) => {
    // RF-ADMIN-08-5: a code for a device that never accepted consent unlinks that device.
    if (
      worker.status === 'PENDING_TERMS' &&
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
      if (errorIs(e, 'ENROLLMENT_ALREADY_COMPLETED')) {
        setRevokeFirst(worker);
        return;
      }
      setListError(`No se pudo generar el código de enrolamiento: ${describeError(e)}`);
    }
  };

  const handleRunDecryptionTest = async () => {
    setTestingDecryption(true);
    try {
      const decrypted = await simulateAndDecrypt();
      setDecryptionTestResult(decrypted);
    } catch (err: any) {
      alert(`Error en prueba de descifrado: ${describeError(err)}`);
    } finally {
      setTestingDecryption(false);
    }
  };

  const copyPublicKey = () => {
    navigator.clipboard.writeText(masterPubKey);
    setCopiedKey(true);
    setTimeout(() => setCopiedKey(false), 2000);
  };

  /** Opens worker detail modal (W4) from an alert list item (RF-ADMIN-25-3). */
  const handleOpenWorkerFromAlert = async (workerId: string) => {
    const existing = workers.find(w => w.id === workerId);
    if (existing) {
      setDetailWorker(existing);
      return;
    }
    try {
      const raw = await workersGet(workerId);
      setDetailWorker(toWorkerItem(raw));
    } catch (e) {
      console.warn('Failed to load worker detail from alert', e);
    }
  };

  return (
    <div className="space-y-6">
      {/* Onboarding Banner: Employer Profile Missing (RF-ADMIN-13-6) */}
      {!hasEmployerProfile && (
        <div className="p-4 bg-accent-subtle border border-default rounded-card flex flex-col sm:flex-row items-start sm:items-center justify-between gap-4">
          <div className="flex items-center gap-3">
            <div className="p-2 bg-accent/20 rounded-control text-accent shrink-0">
              <Building2 className="w-5 h-5" />
            </div>
            <div>
              <h3 className="text-xs font-semibold text-fg">Completa los datos de tu empresa para crear trabajadores</h3>
              <p className="text-xs text-muted">
                La legislación laboral (R38) exige identificar la razón social, RUT y lugares de trabajo antes de enrolar trabajadores.
              </p>
            </div>
          </div>
          {onNavigateToEmployer && (
            <button
              type="button"
              onClick={onNavigateToEmployer}
              className="px-3.5 py-1.5 bg-accent hover:bg-accent-hover text-accent-fg text-xs font-semibold rounded-control transition-colors cursor-pointer shrink-0"
            >
              Ir a Empresa
            </button>
          )}
        </div>
      )}

      {/* Active Enrollment Modal */}
      {activeEnrollment && (
        <EnrollmentModal
          worker={activeEnrollment.worker}
          enrollment={activeEnrollment.pkg}
          onClose={() => setActiveEnrollment(null)}
        />
      )}

      {/* Zero-Knowledge Master Crypto Vault Banner */}
      <div className="bg-surface border border-default rounded-card p-5 relative overflow-hidden">
        <div className="flex flex-col lg:flex-row items-start lg:items-center justify-between gap-4 relative z-10">
          <div className="space-y-1">
            <div className="flex items-center gap-2">
              <span className="p-1.5 bg-accent-subtle text-accent rounded-control">
                <Key className="w-4 h-4" />
              </span>
              <h2 className="text-sm font-bold text-fg tracking-wide">
                Bóveda Criptográfica Zero-Knowledge (OS Keystore)
              </h2>
              <span className="px-2 py-0.5 bg-success-subtle text-success text-[10px] font-mono font-semibold rounded-sm border border-default">
                HPKE X25519 / AES-256-GCM
              </span>
            </div>
            <p className="text-xs text-fg">
              Clave Pública Master activa para el cifrado asimétrico de los agentes en estaciones de trabajo:
            </p>
            <div className="flex items-center gap-2 pt-1 font-mono text-xs">
              <code className="bg-surface-subtle text-fg px-3 py-1.5 rounded-control border border-default text-[11px] truncate max-w-md select-all">
                {masterPubKey}
              </code>
              <button
                onClick={copyPublicKey}
                className="p-1.5 bg-surface hover:bg-surface-subtle text-fg rounded-control border border-default transition"
                title="Copiar Clave Pública"
              >
                {copiedKey ? <Check className="w-3.5 h-3.5 text-success" /> : <Copy className="w-3.5 h-3.5" />}
              </button>
            </div>
          </div>

          <div className="flex flex-wrap items-center gap-2">
            <button
              onClick={handleRunDecryptionTest}
              disabled={testingDecryption}
              className="px-3.5 py-2 bg-surface hover:bg-surface-subtle text-fg border border-default rounded-control text-xs font-medium flex items-center gap-1.5 transition disabled:opacity-50"
            >
              <Cpu className="w-3.5 h-3.5" />
              <span>{testingDecryption ? 'Descifrando…' : 'Autoprueba de descifrado'}</span>
            </button>

            <button
              onClick={onOpenVault}
              className="px-3 py-2 bg-surface-subtle hover:bg-surface-subtle text-fg rounded-card text-xs font-medium border border-default transition flex items-center gap-1.5"
            >
              <Key className="w-3.5 h-3.5" />
              <span>Respaldos y llave</span>
            </button>
          </div>
        </div>
      </div>

      {/* Decryption Test Result Modal */}
      {decryptionTestResult && (
        <div className="fixed inset-0 z-50 bg-black/80 flex items-center justify-center p-4">
          <div className="bg-surface border border-default rounded-card max-w-lg w-full p-6 relative">
            <button
              onClick={() => setDecryptionTestResult(null)}
              className="absolute top-4 right-4 text-muted hover:text-fg"
            >
              <X className="w-5 h-5" />
            </button>

            <div className="flex items-center gap-3 mb-4">
              <div className="w-10 h-10 rounded-card bg-success-subtle text-success flex items-center justify-center">
                <Key className="w-5 h-5" />
              </div>
              <div>
                <h3 className="text-base font-bold text-fg">Resultado de Descifrado Paralelo (Rayon)</h3>
                <p className="text-xs text-success font-mono">100% Verificado con Clave Privada de Keystore</p>
              </div>
            </div>

            <div className="bg-bg p-4 rounded-card border border-default font-mono text-xs text-fg max-h-64 overflow-y-auto mb-4 select-all">
              <pre>{JSON.stringify(decryptionTestResult.payload, null, 2)}</pre>
            </div>

            <div className="p-3 bg-surface-subtle border border-default rounded-card text-xs text-fg space-y-1 mb-4">
              <p className="font-semibold text-accent">Detalles del Pipeline Ejecutado:</p>
              <p>• Clave Efímera X25519 recibida y validada.</p>
              <p>• Derivación de clave simétrica mediante HKDF-SHA256.</p>
              <p>• Descifrado y autenticación de tag Poly1305 / AES-GCM sin exponer claves.</p>
            </div>

            <div className="flex justify-end">
              <button
                onClick={() => setDecryptionTestResult(null)}
                className="px-4 py-2 bg-accent hover:bg-accent-hover text-accent-fg rounded-control text-xs font-semibold transition"
              >
                Cerrar Prueba
              </button>
            </div>
          </div>
        </div>
      )}

      {/* KPI Cards (RF-ADMIN-14-7) */}
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-5 gap-4">
        <div className="bg-surface-subtle border border-default rounded-card p-4 flex items-center gap-4">
          <div className="w-12 h-12 rounded-card bg-accent-subtle text-accent flex items-center justify-center">
            <Users className="w-6 h-6" />
          </div>
          <div>
            <p className="text-xs text-muted font-medium">Trabajadores Activos</p>
            <h3 className="text-2xl font-bold text-fg">
              {kpis?.activeWorkers ?? 0} <span className="text-xs font-normal text-muted">/ {kpis?.totalWorkers ?? 0}</span>
            </h3>
          </div>
        </div>

        <div className="bg-surface-subtle border border-default rounded-card p-4">
          <p className="text-xs text-muted font-medium mb-1.5 flex items-center gap-1.5">
            <HardDrive className="w-3.5 h-3.5" /> Por estado
          </p>
          <div className="grid grid-cols-2 gap-x-3 gap-y-0.5 text-xs">
            <span className="text-success">Activos</span>
            <span className="text-right text-fg font-semibold">{kpis?.byStatus.ACTIVE ?? 0}</span>
            <span className="text-info">Esperando términos</span>
            <span className="text-right text-fg font-semibold">{kpis?.byStatus.PENDING_TERMS ?? 0}</span>
            <span className="text-fg">Invitados</span>
            <span className="text-right text-fg font-semibold">{kpis?.byStatus.PENDING_ENROLLMENT ?? 0}</span>
            <span className="text-danger">Revocados</span>
            <span className="text-right text-fg font-semibold">{kpis?.byStatus.REVOKED ?? 0}</span>
          </div>
        </div>

        <div className="bg-surface-subtle border border-default rounded-card p-4">
          <p className="text-xs text-muted font-medium mb-1.5 flex items-center gap-1.5">
            <ShieldCheck className="w-3.5 h-3.5" /> Por telemetría
          </p>
          <div className="grid grid-cols-2 gap-x-3 gap-y-0.5 text-xs">
            <span className="text-success">Aceptada</span>
            <span className="text-right text-fg font-semibold">{kpis?.byTelemetryStatus?.ACCEPTED ?? 0}</span>
            <span className="text-info">Pendiente</span>
            <span className="text-right text-fg font-semibold">{kpis?.byTelemetryStatus?.PENDING_DECISION ?? 0}</span>
            <span className="text-danger">Rechazada</span>
            <span className="text-right text-fg font-semibold">{kpis?.byTelemetryStatus?.DECLINED ?? 0}</span>
            <span className="text-warning">Pausada</span>
            <span className="text-right text-fg font-semibold">{kpis?.byTelemetryStatus?.PAUSED ?? 0}</span>
          </div>
        </div>

        <div className="bg-surface-subtle border border-default rounded-card p-4 flex items-center gap-4">
          <div className="w-12 h-12 rounded-card bg-info-subtle text-info flex items-center justify-center">
            <Key className="w-6 h-6" />
          </div>
          <div>
            <p className="text-xs text-muted font-medium">Bóveda OS Keystore</p>
            <div className="flex items-center gap-2 mt-0.5">
              <span className={`w-2.5 h-2.5 rounded-full ${kpis?.keystoreProtected ? 'bg-success' : 'bg-danger'}`}></span>
              <span className={`text-sm font-semibold ${kpis?.keystoreProtected ? 'text-success' : 'text-danger'}`}>
                {kpis?.keystoreProtected ? 'Llave local cargada' : 'Sin llave local'}
              </span>
            </div>
            {kpis && kpis.keystoreProtected && !kpis.backupConfigured && (
              <button onClick={onOpenVault} className="text-[11px] text-warning underline mt-0.5">
                Sin respaldo en el servidor
              </button>
            )}
          </div>
        </div>

        <div className="bg-surface-subtle border border-default rounded-card p-4 flex items-start gap-4">
          <div className="w-12 h-12 rounded-card bg-accent-subtle text-accent flex items-center justify-center shrink-0">
            <ShieldCheck className="w-6 h-6" />
          </div>
          <div className="text-xs space-y-1 min-w-0 flex-1">
            <p className="text-muted font-medium">Alertas</p>
            {kpis?.keyMismatch && <p className="text-danger font-semibold">Llave del servidor distinta a la local (K2)</p>}
            <p className={kpis?.expiringCodes ? 'text-warning font-semibold' : 'text-fg'}>
              {kpis?.expiringCodes ?? 0} código(s) por vencer (&lt; 2 h)
            </p>

            {/* RF-ADMIN-25-2, RF-ADMIN-25-3, RF-ADMIN-25-4: Active without data */}
            {telemetryHealth ? (
              <div>
                {telemetryHealth.active_without_data.total > 0 ? (
                  <button
                    type="button"
                    aria-expanded={expandedWithoutData}
                    onClick={() => setExpandedWithoutData(v => !v)}
                    className="text-left text-warning font-semibold hover:underline flex items-center gap-1 cursor-pointer w-full"
                  >
                    <span>
                      {formatWithoutDataAlertLabel(
                        telemetryHealth.active_without_data.total,
                        telemetryHealth.stale_hours
                      )}
                    </span>
                    {expandedWithoutData ? (
                      <ChevronDown className="w-3 h-3 shrink-0" />
                    ) : (
                      <ChevronRight className="w-3 h-3 shrink-0" />
                    )}
                  </button>
                ) : (
                  <p className="text-fg">
                    {formatWithoutDataAlertLabel(0, telemetryHealth.stale_hours)}
                  </p>
                )}
                {expandedWithoutData && telemetryHealth.active_without_data.total > 0 && (
                  <ul className="mt-1 space-y-1 pl-2 border-l border-default text-[11px] max-h-40 overflow-y-auto">
                    {telemetryHealth.active_without_data.workers.map(w => (
                      <li key={w.id}>
                        <button
                          type="button"
                          onClick={() => handleOpenWorkerFromAlert(w.id)}
                          className="text-left hover:underline text-fg font-medium cursor-pointer"
                        >
                          {w.name}
                          <span className="text-muted font-normal ml-1">
                            ({formatLastTelemetry(w.last_telemetry_at)})
                          </span>
                        </button>
                      </li>
                    ))}
                    {formatRemainingCount(
                      telemetryHealth.active_without_data.total,
                      telemetryHealth.active_without_data.workers.length
                    ) && (
                      <li className="text-muted italic">
                        {formatRemainingCount(
                          telemetryHealth.active_without_data.total,
                          telemetryHealth.active_without_data.workers.length
                        )}
                      </li>
                    )}
                  </ul>
                )}
              </div>
            ) : (
              <p
                className="text-fg cursor-help"
                title="No se pudo cargar la información de salud de telemetría"
              >
                {formatWithoutDataAlertLabel(null, 24)}
              </p>
            )}

            {/* RF-ADMIN-25-2, RF-ADMIN-25-3, RF-ADMIN-25-4: Recent rejections */}
            {telemetryHealth ? (
              <div>
                {telemetryHealth.recent_rejections.total > 0 ? (
                  <button
                    type="button"
                    aria-expanded={expandedRejections}
                    onClick={() => setExpandedRejections(v => !v)}
                    className="text-left text-warning font-semibold hover:underline flex items-center gap-1 cursor-pointer w-full"
                  >
                    <span>
                      {formatRejectionsAlertLabel(
                        telemetryHealth.recent_rejections.total,
                        telemetryHealth.since_days
                      )}
                    </span>
                    {expandedRejections ? (
                      <ChevronDown className="w-3 h-3 shrink-0" />
                    ) : (
                      <ChevronRight className="w-3 h-3 shrink-0" />
                    )}
                  </button>
                ) : (
                  <p className="text-fg">
                    {formatRejectionsAlertLabel(0, telemetryHealth.since_days)}
                  </p>
                )}
                {expandedRejections && telemetryHealth.recent_rejections.total > 0 && (
                  <ul className="mt-1 space-y-1 pl-2 border-l border-default text-[11px] max-h-40 overflow-y-auto">
                    {telemetryHealth.recent_rejections.workers.map(w => (
                      <li key={w.id}>
                        <button
                          type="button"
                          onClick={() => handleOpenWorkerFromAlert(w.id)}
                          className="text-left hover:underline text-fg font-medium cursor-pointer"
                        >
                          {w.name}
                          <span className="text-muted font-normal ml-1">
                            ({formatRejectionDetail(w)})
                          </span>
                        </button>
                      </li>
                    ))}
                    {formatRemainingCount(
                      telemetryHealth.recent_rejections.total,
                      telemetryHealth.recent_rejections.workers.length
                    ) && (
                      <li className="text-muted italic">
                        {formatRemainingCount(
                          telemetryHealth.recent_rejections.total,
                          telemetryHealth.recent_rejections.workers.length
                        )}
                      </li>
                    )}
                  </ul>
                )}
              </div>
            ) : (
              <p
                className="text-fg cursor-help"
                title="No se pudo cargar la información de salud de telemetría"
              >
                {formatRejectionsAlertLabel(null, 7)}
              </p>
            )}
          </div>
        </div>
      </div>

      {/* Attendance KPI Cards (RF-ADMIN-15-7) */}
      <div className="grid grid-cols-1 sm:grid-cols-3 gap-4">
        <div
          onClick={() => onNavigateToAttendance?.()}
          className={`bg-surface-subtle border border-default rounded-card p-4 flex items-center justify-between gap-4 ${
            onNavigateToAttendance ? 'cursor-pointer hover:border-brand/40 transition' : ''
          }`}
          title="Ver Asistencia en vivo"
        >
          <div className="flex items-center gap-3.5">
            <div className="w-10 h-10 rounded-card bg-success-subtle text-success flex items-center justify-center">
              <Clock className="w-5 h-5" />
            </div>
            <div>
              <p className="text-xs text-muted font-medium">En jornada ahora</p>
              <h3 className="text-2xl font-bold text-fg">{workingNowCount}</h3>
            </div>
          </div>
          <span className="text-[11px] text-muted font-medium">Asistencia AS1</span>
        </div>

        <div
          onClick={() => onNavigateToAttendance?.()}
          className={`bg-surface-subtle border border-default rounded-card p-4 flex items-center justify-between gap-4 ${
            onNavigateToAttendance ? 'cursor-pointer hover:border-brand/40 transition' : ''
          }`}
          title="Ver Asistencia en vivo"
        >
          <div className="flex items-center gap-3.5">
            <div className="w-10 h-10 rounded-card bg-danger-subtle text-danger flex items-center justify-center">
              <AlertTriangle className="w-5 h-5" />
            </div>
            <div>
              <p className="text-xs text-muted font-medium">Sin marcar hoy</p>
              <h3 className="text-2xl font-bold text-fg">{unmarkedTodayCount}</h3>
            </div>
          </div>
          <span className="text-[11px] text-muted font-medium">
            {lateStartCount > 0 ? `${lateStartCount} con entrada atrasada` : 'Ninguno atrasado'}
          </span>
        </div>

        <div
          onClick={() => onNavigateToAttendance?.()}
          className={`bg-surface-subtle border border-default rounded-card p-4 flex items-center justify-between gap-4 ${
            onNavigateToAttendance ? 'cursor-pointer hover:border-brand/40 transition' : ''
          }`}
          title="Ver Marcas y correcciones"
        >
          <div className="flex items-center gap-3.5">
            <div className="w-10 h-10 rounded-card bg-warning-subtle text-warning flex items-center justify-center">
              <FileCheck className="w-5 h-5" />
            </div>
            <div>
              <p className="text-xs text-muted font-medium">Correcciones pendientes</p>
              <h3 className="text-2xl font-bold text-fg">{pendingCorrectionsCount}</h3>
            </div>
          </div>
          <span className="text-[11px] text-muted font-medium">Plazo 48 h</span>
        </div>
      </div>

      {/* Deep Filtering & Action Bar */}
      <div className="bg-surface-subtle border border-default rounded-card p-4 flex flex-wrap items-center justify-between gap-4">
        <div className="flex flex-wrap items-center gap-3 flex-1 min-w-[320px]">
          {/* Search */}
          <div className="relative flex-1 min-w-[200px]">
            <Search className="w-4 h-4 text-muted absolute left-3 top-1/2 -translate-y-1/2" />
            <input
              type="text"
              placeholder="Buscar por nombre o correo corporativo..."
              value={searchInput}
              onChange={e => setSearchInput(e.target.value)}
              className="w-full bg-surface border border-default text-fg text-xs rounded-control pl-9 pr-3 py-2 focus:outline-hidden focus:border-accent"
            />
          </div>

          {/* Workplace Filter */}
          <div className="flex items-center gap-1.5 bg-surface border border-default rounded-control px-2.5 py-1.5 text-xs text-fg">
            <Building2 className="w-3.5 h-3.5 text-muted" />
            <select
              value={workplaceFilter}
              onChange={e => setWorkplaceFilter(e.target.value)}
              className="bg-transparent focus:outline-hidden cursor-pointer"
            >
              <option value="ALL">Todos los Lugares</option>
              {workplaces.map(wp => (
                <option key={wp.id} value={wp.id}>
                  {wp.name}
                </option>
              ))}
            </select>
          </div>

          {/* Work Mode Filter */}
          <div className="flex items-center gap-1.5 bg-surface border border-default rounded-control px-2.5 py-1.5 text-xs text-fg">
            <select
              value={workModeFilter}
              onChange={e => setWorkModeFilter(e.target.value as 'ALL' | WorkMode)}
              className="bg-transparent focus:outline-hidden cursor-pointer"
            >
              <option value="ALL">Todas las Modalidades</option>
              <option value="ON_SITE">Presencial</option>
              <option value="REMOTE">Teletrabajo</option>
              <option value="HYBRID">Mixta</option>
            </select>
          </div>

          {/* Role Filter */}
          <div className="flex items-center gap-1.5 bg-surface border border-default rounded-control px-2.5 py-1.5 text-xs text-fg">
            <Filter className="w-3.5 h-3.5 text-muted" />
            <select
              value={roleFilter}
              onChange={e => setRoleFilter(e.target.value as 'ALL' | 'OFFICE' | 'FIELD')}
              className="bg-transparent focus:outline-hidden cursor-pointer"
            >
              <option value="ALL">Todos los Roles</option>
              <option value="OFFICE">Oficina (OFFICE)</option>
              <option value="FIELD">Terreno (FIELD)</option>
            </select>
          </div>

          {/* Status Filter */}
          <div className="flex items-center gap-1.5 bg-surface border border-default rounded-control px-2.5 py-1.5 text-xs text-fg">
            <select
              value={statusFilter}
              onChange={e => setStatusFilter(e.target.value as 'ALL' | WorkerStatus)}
              className="bg-transparent focus:outline-hidden cursor-pointer"
            >
              <option value="ALL">Todos los Estados</option>
              <option value="ACTIVE">Activos</option>
              <option value="PENDING_TERMS">Esperando términos</option>
              <option value="PENDING_ENROLLMENT">Invitados</option>
              <option value="REVOKED">Revocados</option>
            </select>
          </div>
        </div>

        <div className="flex items-center gap-2">
          <button
            onClick={loadData}
            className="p-2 bg-surface-subtle hover:bg-strong text-fg rounded-control text-xs transition"
            title="Recargar datos"
          >
            <RefreshCw className="w-4 h-4" />
          </button>
          <button
            onClick={() => setShowCreateModal(true)}
            className="bg-accent hover:bg-accent-hover text-accent-fg text-xs font-semibold px-4 py-2 rounded-control flex items-center gap-2 transition"
          >
            <UserPlus className="w-4 h-4" />
            Nuevo Trabajador
          </button>
        </div>
      </div>

      {listError && (
        <div className="p-3 bg-danger-subtle border border-danger rounded-card text-xs text-danger">{listError}</div>
      )}

      {/* Workers Grid */}
      {loading ? (
        <div className="py-20 text-center text-muted text-sm">Descargando y actualizando lista de trabajadores...</div>
      ) : workers.length === 0 ? (
        <div className="py-16 text-center bg-surface-subtle rounded-card border border-default">
          <p className="text-muted text-sm">No se encontraron trabajadores que coincidan con los filtros.</p>
        </div>
      ) : (
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          {workers.map(w => (
            <EmployeeCard
              key={w.id}
              worker={w}
              todayAttendanceState={attendanceStates[w.id]}
              onSelect={onSelectWorker}
              onToggleStatus={handleToggle}
              onShowEnrollment={handleShowEnrollment}
              onEdit={setEditingWorker}
              onDelete={setDeletingWorker}
              onOpenDetail={setDetailWorker}
            />
          ))}
        </div>
      )}

      {pagination && pagination.total_pages > 1 && (
        <div className="flex items-center justify-between text-xs text-muted">
          <span>
            {pagination.total} trabajadores · página {pagination.page} de {pagination.total_pages}
          </span>
          <div className="flex gap-2">
            <button
              onClick={() => setPage(p => Math.max(1, p - 1))}
              disabled={page <= 1 || loading}
              className="p-2 bg-surface-subtle hover:bg-surface-subtle rounded-control border border-default disabled:opacity-40"
              title="Página anterior"
            >
              <ChevronLeft className="w-4 h-4" />
            </button>
            <button
              onClick={() => setPage(p => Math.min(pagination.total_pages, p + 1))}
              disabled={page >= pagination.total_pages || loading}
              className="p-2 bg-surface-subtle hover:bg-surface-subtle rounded-control border border-default disabled:opacity-40"
              title="Página siguiente"
            >
              <ChevronRight className="w-4 h-4" />
            </button>
          </div>
        </div>
      )}

      {detailWorker && (
        <WorkerDetailModal
          worker={detailWorker}
          organization={organization}
          onClose={() => setDetailWorker(null)}
          onChanged={updated => {
            replaceWorker(updated);
            fetchKpiSummary().then(setKpis).catch(() => undefined);
          }}
          onRegenerate={worker => {
            setDetailWorker(null);
            handleShowEnrollment(worker);
          }}
          onDelete={worker => {
            setDetailWorker(null);
            setDeletingWorker(worker);
          }}
          onViewTelemetry={worker => {
            setDetailWorker(null);
            onSelectWorker(worker);
          }}
          onNavigateToAttendance={
            onNavigateToAttendance
              ? workerId => {
                  setDetailWorker(null);
                  onNavigateToAttendance(workerId);
                }
              : undefined
          }
        />
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

      {revokeFirst && (
        <RevokeFirstDialog
          worker={revokeFirst}
          onClose={() => setRevokeFirst(null)}
          onRegenerated={pkg => {
            setActiveEnrollment({ worker: revokeFirst, pkg });
            setRevokeFirst(null);
            loadData();
          }}
        />
      )}

      {showCreateModal && (
        <CreateWorkerModal
          organization={organization}
          onProfileChanged={onProfileChanged}
          onMissingEmployerProfile={onNavigateToEmployer}
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
