import React, { useEffect, useMemo, useState } from 'react';
import { X } from 'lucide-react';
import {
  UpdateWorkerForm,
  WorkerItem,
  WorkerResponse,
  WorkMode,
  Workplace,
  workplacesList,
  workersUpdate,
} from '../api/tauri-bridge';
import { ianaTimezones, isValidTimezone } from './consentCategories';
import { formatRut, parseRut } from '../lib/rut';

interface WorkerEditModalProps {
  worker: WorkerItem;
  onClose: () => void;
  onSaved: (updated: WorkerResponse) => void;
}

/** Edit worker legal/contract data (`PUT /workers/:id`, RF-ADMIN-14-6). */
export const WorkerEditModal: React.FC<WorkerEditModalProps> = ({ worker, onClose, onSaved }) => {
  const [name, setName] = useState(worker.name);
  const [run, setRun] = useState(worker.run);
  const [personalEmail, setPersonalEmail] = useState(worker.personalEmail);
  const [jobTitle, setJobTitle] = useState(worker.jobTitle);
  const [workplaceId, setWorkplaceId] = useState(worker.workplace?.id || '');
  const [workplaces, setWorkplaces] = useState<Workplace[]>([]);
  const [workMode, setWorkMode] = useState<WorkMode>(worker.workMode);
  const [role, setRole] = useState<'OFFICE' | 'FIELD'>(worker.role);
  const [art22Exempt, setArt22Exempt] = useState(worker.art22Exempt);
  const [locationCause, setLocationCause] = useState('');
  const [timezone, setTimezone] = useState(worker.timezone || 'America/Santiago');

  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const timezones = useMemo(ianaTimezones, []);

  useEffect(() => {
    workplacesList()
      .then(setWorkplaces)
      .catch(e => console.warn('Failed to load workplaces', e));
  }, []);

  // DP-04: a worker with location requested cannot become OFFICE until consent is edited.
  const locationRequested = worker.raw.requested_consent_flags.location;

  const handleRunBlur = () => {
    if (!run.trim()) return;
    try {
      setRun(formatRut(run));
    } catch {
      // keep raw if parse fails
    }
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!name.trim()) return setError('El nombre es obligatorio.');
    if (run.trim()) {
      try {
        parseRut(run);
      } catch (err) {
        return setError(err instanceof Error ? err.message : 'RUN inválido');
      }
    }
    if (personalEmail.trim() && !/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(personalEmail.trim())) {
      return setError('Ingrese un correo personal válido.');
    }
    if (timezone && !isValidTimezone(timezone)) {
      return setError('Zona horaria IANA inválida (p. ej. America/Santiago).');
    }
    if (role === 'OFFICE' && locationRequested) {
      return setError(
        'Desactive primero la ubicación en el consentimiento: no se permite para OFFICE (DP-04).',
      );
    }
    if (workMode !== 'ON_SITE' && worker.raw.requested_consent_flags.productivity_screenshots) {
      return setError(
        'Desactive primero las capturas de pantalla en el consentimiento: no se permiten para teletrabajo o modalidad mixta (DP-05, R38 art. 55 a).',
      );
    }

    const form: UpdateWorkerForm = {};
    if (name.trim() !== worker.name) form.name = name.trim();
    if (run.trim() && run.trim() !== worker.run) form.run = formatRut(run);
    if (personalEmail.trim() && personalEmail.trim() !== worker.personalEmail) {
      form.personal_email = personalEmail.trim();
    }
    if (jobTitle.trim() !== worker.jobTitle) form.job_title = jobTitle.trim();
    if (workplaceId && workplaceId !== worker.workplace?.id) form.workplace_id = workplaceId;
    if (workMode !== worker.workMode) form.work_mode = workMode;
    if (role !== worker.role) form.role_type = role;
    if (art22Exempt !== worker.art22Exempt) form.art22_exempt = art22Exempt;
    if (locationCause.trim()) form.location_cause = locationCause.trim();
    if (timezone.trim() !== (worker.timezone || '')) form.timezone = timezone.trim();

    if (Object.keys(form).length === 0) return onClose();

    setSaving(true);
    setError(null);
    try {
      onSaved(await workersUpdate(worker.id, form));
    } catch (err: any) {
      setError(err?.message || String(err));
    } finally {
      setSaving(false);
    }
  };

  const input =
    'w-full mt-1 bg-surface border border-default text-fg text-xs rounded-control px-3 py-2 focus:outline-hidden focus:border-accent';

  return (
    <div className="fixed inset-0 z-50 bg-black/80 flex items-center justify-center p-4">
      <div className="bg-surface-subtle border border-default rounded-card max-w-lg w-full p-6 space-y-4 shadow-md max-h-[90vh] overflow-y-auto">
        <div className="flex justify-between items-center border-b border-default pb-3">
          <div>
            <h3 className="text-base font-semibold text-fg">Editar Datos del Trabajador</h3>
            <p className="text-xs text-muted font-mono">{worker.personalEmail || worker.email}</p>
          </div>
          <button onClick={onClose} className="text-muted hover:text-fg">
            <X className="w-5 h-5" />
          </button>
        </div>

        <form onSubmit={handleSubmit} className="space-y-3">
          <div>
            <label className="text-xs text-muted font-medium">Nombre Completo</label>
            <input
              type="text"
              value={name}
              onChange={e => setName(e.target.value)}
              className={input}
            />
          </div>

          <div className="grid grid-cols-2 gap-3">
            <div>
              <label className="text-xs text-muted font-medium">RUN (RUT)</label>
              <input
                type="text"
                value={run}
                onChange={e => setRun(e.target.value)}
                onBlur={handleRunBlur}
                className={input}
              />
            </div>
            <div>
              <label className="text-xs text-muted font-medium">Cargo</label>
              <input
                type="text"
                value={jobTitle}
                onChange={e => setJobTitle(e.target.value)}
                className={input}
              />
            </div>
          </div>

          <div>
            <label className="text-xs text-muted font-medium">Correo Personal</label>
            <input
              type="email"
              value={personalEmail}
              onChange={e => setPersonalEmail(e.target.value)}
              className={input}
            />
          </div>

          <div className="grid grid-cols-2 gap-3">
            <div>
              <label className="text-xs text-muted font-medium">Lugar de Trabajo</label>
              <select
                value={workplaceId}
                onChange={e => setWorkplaceId(e.target.value)}
                className={input}
              >
                {workplaces.map(w => (
                  <option key={w.id} value={w.id}>
                    {w.name}
                  </option>
                ))}
              </select>
            </div>
            <div>
              <label className="text-xs text-muted font-medium">Modalidad</label>
              <select
                value={workMode}
                onChange={e => setWorkMode(e.target.value as WorkMode)}
                className={input}
              >
                <option value="ON_SITE">Presencial (ON_SITE)</option>
                <option value="REMOTE">Teletrabajo (REMOTE)</option>
                <option value="HYBRID">Mixta (HYBRID)</option>
              </select>
            </div>
          </div>

          <div className="grid grid-cols-2 gap-3">
            <div>
              <label className="text-xs text-muted font-medium">Rol Telemetría</label>
              <select
                value={role}
                onChange={e => setRole(e.target.value as 'OFFICE' | 'FIELD')}
                className={input}
              >
                <option value="OFFICE">Oficina (OFFICE)</option>
                <option value="FIELD">Terreno (FIELD)</option>
              </select>
            </div>
            <div>
              <label className="text-xs text-muted font-medium">Zona Horaria (IANA)</label>
              <input
                list="iana-timezones-edit"
                value={timezone}
                onChange={e => setTimezone(e.target.value)}
                className={input}
              />
              <datalist id="iana-timezones-edit">
                {timezones.map(tz => (
                  <option key={tz} value={tz} />
                ))}
              </datalist>
            </div>
          </div>

          {role === 'FIELD' && locationRequested && (
            <div>
              <label className="text-xs text-muted font-medium">Causa de seguridad o legal para ubicación (DP-04)</label>
              <input
                type="text"
                value={locationCause}
                onChange={e => setLocationCause(e.target.value)}
                placeholder="Motivo legal o de seguridad (20 a 500 caracteres)"
                className={input}
              />
            </div>
          )}

          <div className="pt-2 border-t border-default">
            <label className="flex items-center gap-2 text-xs text-fg cursor-pointer">
              <input
                type="checkbox"
                checked={art22Exempt}
                onChange={e => setArt22Exempt(e.target.checked)}
                className="rounded-sm accent-accent"
              />
              <span>Excluido de limitación de jornada (art. 22 inc. 2°)</span>
            </label>
          </div>

          {error && (
            <div className="p-2.5 bg-danger-subtle border border-danger rounded-control text-xs text-danger">
              {error}
            </div>
          )}

          <div className="flex justify-end gap-2 pt-2 border-t border-default">
            <button
              type="button"
              onClick={onClose}
              className="px-4 py-2 bg-surface-subtle text-fg rounded-control text-xs hover:bg-strong transition"
            >
              Cancelar
            </button>
            <button
              type="submit"
              disabled={saving}
              className="px-4 py-2 bg-accent hover:bg-accent-hover text-accent-fg rounded-control text-xs font-semibold transition disabled:opacity-50"
            >
              {saving ? 'Guardando…' : 'Guardar cambios'}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
};
