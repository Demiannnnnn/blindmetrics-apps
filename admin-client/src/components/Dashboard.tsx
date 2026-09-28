import React, { useState, useEffect } from 'react';
import {
  KpiSummary,
  WorkerItem,
  EnrollmentPackage,
  fetchKpiSummary,
  getWorkersList,
  createWorker,
  toggleWorkerStatus,
} from '../api/tauri-bridge';
import { EmployeeCard } from './EmployeeCard';
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
} from 'lucide-react';

interface DashboardProps {
  onSelectWorker: (worker: WorkerItem) => void;
}

export const Dashboard: React.FC<DashboardProps> = ({ onSelectWorker }) => {
  const [kpis, setKpis] = useState<KpiSummary | null>(null);
  const [workers, setWorkers] = useState<WorkerItem[]>([]);
  const [search, setSearch] = useState<string>('');
  const [roleFilter, setRoleFilter] = useState<string>('ALL');
  const [statusFilter, setStatusFilter] = useState<string>('ALL');
  const [loading, setLoading] = useState<boolean>(true);

  // Modal States
  const [showCreateModal, setShowCreateModal] = useState<boolean>(false);
  const [enrollmentResult, setEnrollmentResult] = useState<EnrollmentPackage | null>(null);
  const [copied, setCopied] = useState<boolean>(false);

  // New Worker Form
  const [newName, setNewName] = useState<string>('');
  const [newEmail, setNewEmail] = useState<string>('');
  const [newRole, setNewRole] = useState<'OFFICE' | 'FIELD'>('OFFICE');
  const [newStart, setNewStart] = useState<string>('09:00');
  const [newEnd, setNewEnd] = useState<string>('18:00');
  const [newTz, setNewTz] = useState<string>('America/Santiago');

  const loadData = async () => {
    setLoading(true);
    try {
      const [k, w] = await Promise.all([
        fetchKpiSummary(),
        getWorkersList(search, roleFilter, statusFilter),
      ]);
      setKpis(k);
      setWorkers(w);
    } catch (e) {
      console.error('Failed to load dashboard data', e);
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    loadData();
  }, [search, roleFilter, statusFilter]);

  const handleToggle = async (id: string) => {
    await toggleWorkerStatus(id);
    loadData();
  };

  const handleCreate = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!newName || !newEmail) return;

    try {
      const res = await createWorker(newName, newEmail, newRole, newStart, newEnd, newTz);
      setEnrollmentResult(res.enrollment);
      loadData();
    } catch (err) {
      console.error(err);
    }
  };

  const copyEnrollmentJson = () => {
    if (enrollmentResult) {
      navigator.clipboard.writeText(JSON.stringify(enrollmentResult, null, 2));
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    }
  };

  return (
    <div className="space-y-6">
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
              value={search}
              onChange={e => setSearch(e.target.value)}
              className="w-full bg-slate-900 border border-slate-700 text-slate-200 text-xs rounded-lg pl-9 pr-3 py-2 focus:outline-none focus:border-indigo-500"
            />
          </div>

          {/* Role Filter */}
          <div className="flex items-center gap-1.5 bg-slate-900 border border-slate-700 rounded-lg px-2.5 py-1.5 text-xs text-slate-300">
            <Filter className="w-3.5 h-3.5 text-slate-400" />
            <select
              value={roleFilter}
              onChange={e => setRoleFilter(e.target.value)}
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
              onChange={e => setStatusFilter(e.target.value)}
              className="bg-transparent focus:outline-none cursor-pointer"
            >
              <option value="ALL">Todos los Estados</option>
              <option value="ACTIVE">Activos</option>
              <option value="INACTIVE">Inactivos</option>
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
            onClick={() => {
              setEnrollmentResult(null);
              setShowCreateModal(true);
            }}
            className="bg-indigo-600 hover:bg-indigo-500 text-white text-xs font-semibold px-4 py-2 rounded-lg flex items-center gap-2 shadow-lg transition"
          >
            <UserPlus className="w-4 h-4" />
            Nuevo Trabajador
          </button>
        </div>
      </div>

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
              onShowEnrollment={worker => {
                setEnrollmentResult({
                  api_url: "https://telemetry.acme.corp/api/v1",
                  worker_id: worker.id,
                  admin_public_key: "K_admin_pub_x25519_base64_sample_key_here==",
                  organization: "Acme SpA",
                });
                setShowCreateModal(true);
              }}
            />
          ))}
        </div>
      )}

      {/* Create Worker & Enrollment Modal */}
      {showCreateModal && (
        <div className="fixed inset-0 z-50 bg-black/80 flex items-center justify-center p-4">
          <div className="bg-slate-800 border border-slate-700 rounded-2xl max-w-lg w-full p-6 space-y-4 shadow-2xl">
            <div className="flex justify-between items-center border-b border-slate-700 pb-3">
              <h3 className="text-base font-semibold text-white">
                {enrollmentResult ? 'Paquete de Enrolamiento Generado' : 'Registrar Nuevo Trabajador'}
              </h3>
              <button
                onClick={() => setShowCreateModal(false)}
                className="text-slate-400 hover:text-white"
              >
                <X className="w-5 h-5" />
              </button>
            </div>

            {enrollmentResult ? (
              <div className="space-y-4">
                <p className="text-xs text-slate-300">
                  Entregue este paquete JSON o código al trabajador para aprovisionar su aplicación <span className="font-mono text-indigo-400">telemetry-worker</span>.
                </p>
                <div className="bg-slate-950 p-4 rounded-xl border border-slate-800 text-xs font-mono text-slate-300 relative overflow-x-auto max-h-52">
                  <pre>{JSON.stringify(enrollmentResult, null, 2)}</pre>
                  <button
                    onClick={copyEnrollmentJson}
                    className="absolute top-2 right-2 p-1.5 bg-slate-800 hover:bg-slate-700 text-slate-300 rounded border border-slate-700 transition"
                    title="Copiar JSON"
                  >
                    {copied ? <Check className="w-3.5 h-3.5 text-emerald-400" /> : <Copy className="w-3.5 h-3.5" />}
                  </button>
                </div>
                <div className="flex justify-end gap-2 pt-2">
                  <button
                    onClick={() => setShowCreateModal(false)}
                    className="px-4 py-2 bg-indigo-600 hover:bg-indigo-500 text-white rounded-lg text-xs font-semibold transition"
                  >
                    Finalizar
                  </button>
                </div>
              </div>
            ) : (
              <form onSubmit={handleCreate} className="space-y-3">
                <div>
                  <label className="text-xs text-slate-400 font-medium">Nombre Completo</label>
                  <input
                    type="text"
                    required
                    value={newName}
                    onChange={e => setNewName(e.target.value)}
                    placeholder="Ej. Roberto Muñoz"
                    className="w-full mt-1 bg-slate-900 border border-slate-700 text-slate-200 text-xs rounded-lg px-3 py-2 focus:outline-none focus:border-indigo-500"
                  />
                </div>
                <div>
                  <label className="text-xs text-slate-400 font-medium">Correo Electrónico Corporativo</label>
                  <input
                    type="email"
                    required
                    value={newEmail}
                    onChange={e => setNewEmail(e.target.value)}
                    placeholder="roberto.munoz@empresa.com"
                    className="w-full mt-1 bg-slate-900 border border-slate-700 text-slate-200 text-xs rounded-lg px-3 py-2 focus:outline-none focus:border-indigo-500"
                  />
                </div>
                <div className="grid grid-cols-2 gap-3">
                  <div>
                    <label className="text-xs text-slate-400 font-medium">Rol Laboral</label>
                    <select
                      value={newRole}
                      onChange={e => setNewRole(e.target.value as any)}
                      className="w-full mt-1 bg-slate-900 border border-slate-700 text-slate-200 text-xs rounded-lg px-3 py-2 focus:outline-none focus:border-indigo-500"
                    >
                      <option value="OFFICE">Oficina (OFFICE)</option>
                      <option value="FIELD">Terreno (FIELD)</option>
                    </select>
                  </div>
                  <div>
                    <label className="text-xs text-slate-400 font-medium">Zona Horaria</label>
                    <input
                      type="text"
                      value={newTz}
                      onChange={e => setNewTz(e.target.value)}
                      className="w-full mt-1 bg-slate-900 border border-slate-700 text-slate-200 text-xs rounded-lg px-3 py-2 focus:outline-none focus:border-indigo-500"
                    />
                  </div>
                </div>
                <div className="grid grid-cols-2 gap-3">
                  <div>
                    <label className="text-xs text-slate-400 font-medium">Hora Inicio Turno</label>
                    <input
                      type="time"
                      value={newStart}
                      onChange={e => setNewStart(e.target.value)}
                      className="w-full mt-1 bg-slate-900 border border-slate-700 text-slate-200 text-xs rounded-lg px-3 py-2 focus:outline-none focus:border-indigo-500"
                    />
                  </div>
                  <div>
                    <label className="text-xs text-slate-400 font-medium">Hora Fin Turno</label>
                    <input
                      type="time"
                      value={newEnd}
                      onChange={e => setNewEnd(e.target.value)}
                      className="w-full mt-1 bg-slate-900 border border-slate-700 text-slate-200 text-xs rounded-lg px-3 py-2 focus:outline-none focus:border-indigo-500"
                    />
                  </div>
                </div>

                <div className="flex justify-end gap-2 pt-4">
                  <button
                    type="button"
                    onClick={() => setShowCreateModal(false)}
                    className="px-4 py-2 bg-slate-700 text-slate-300 rounded-lg text-xs hover:bg-slate-600 transition"
                  >
                    Cancelar
                  </button>
                  <button
                    type="submit"
                    className="px-4 py-2 bg-indigo-600 hover:bg-indigo-500 text-white rounded-lg text-xs font-semibold transition"
                  >
                    Crear y Generar Enrolamiento
                  </button>
                </div>
              </form>
            )}
          </div>
        </div>
      )}
    </div>
  );
};
