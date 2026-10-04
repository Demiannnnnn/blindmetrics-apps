import React, { useMemo, useState } from 'react';
import { X } from 'lucide-react';
import { UpdateWorkerForm, WorkerItem, WorkerResponse, workersUpdate } from '../api/tauri-bridge';
import { ianaTimezones, isValidTimezone, workHoursError } from './consentCategories';

interface WorkerEditModalProps {
  worker: WorkerItem;
  onClose: () => void;
  onSaved: (updated: WorkerResponse) => void;
}

/** Edit identity/schedule fields (`PUT /workers/:id`, RF-ADMIN-03). Consent is edited separately. */
export const WorkerEditModal: React.FC<WorkerEditModalProps> = ({ worker, onClose, onSaved }) => {
  const [name, setName] = useState(worker.name);
  const [role, setRole] = useState<'OFFICE' | 'FIELD'>(worker.role);
  const [start, setStart] = useState(worker.workHoursStart);
  const [end, setEnd] = useState(worker.workHoursEnd);
  const [timezone, setTimezone] = useState(worker.timezone);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const timezones = useMemo(ianaTimezones, []);

  // DP-04: a worker with location requested cannot become OFFICE until consent is edited.
  const locationRequested = worker.raw.requested_consent_flags.location;

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!name.trim()) return setError('El nombre es obligatorio.');
    const hours = workHoursError(start, end);
    if (hours) return setError(hours);
    if (!isValidTimezone(timezone)) return setError('Zona horaria IANA inválida (p. ej. America/Santiago).');
    if (role === 'OFFICE' && locationRequested) {
      return setError('Desactive primero la ubicación en el consentimiento: no se permite para OFFICE (DP-04).');
    }

    // Send only what changed.
    const form: UpdateWorkerForm = {};
    if (name.trim() !== worker.name) form.name = name.trim();
    if (role !== worker.role) form.role_type = role;
    if (start !== worker.workHoursStart || end !== worker.workHoursEnd) {
      form.work_hours_start = start;
      form.work_hours_end = end;
    }
    if (timezone.trim() !== worker.timezone) form.timezone = timezone.trim();
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
    'w-full mt-1 bg-surface border border-default text-fg text-xs rounded-control px-3 py-2 focus:outline-none focus:border-accent';

  return (
    <div className="fixed inset-0 z-50 bg-black/80 flex items-center justify-center p-4">
      <div className="bg-surface-subtle border border-default rounded-card max-w-lg w-full p-6 space-y-4 shadow-md">
        <div className="flex justify-between items-center border-b border-default pb-3">
          <div>
            <h3 className="text-base font-semibold text-fg">Editar Trabajador</h3>
            <p className="text-xs text-muted font-mono">{worker.email}</p>
          </div>
          <button onClick={onClose} className="text-muted hover:text-fg">
            <X className="w-5 h-5" />
          </button>
        </div>

        <form onSubmit={handleSubmit} className="space-y-3">
          <div>
            <label className="text-xs text-muted font-medium">Nombre Completo</label>
            <input type="text" value={name} onChange={e => setName(e.target.value)} className={input} />
          </div>
          <div className="grid grid-cols-2 gap-3">
            <div>
              <label className="text-xs text-muted font-medium">Rol Laboral</label>
              <select value={role} onChange={e => setRole(e.target.value as 'OFFICE' | 'FIELD')} className={input}>
                <option value="OFFICE">Oficina (OFFICE)</option>
                <option value="FIELD">Terreno (FIELD)</option>
              </select>
            </div>
            <div>
              <label className="text-xs text-muted font-medium">Zona Horaria (IANA)</label>
              <input list="iana-timezones-edit" value={timezone} onChange={e => setTimezone(e.target.value)} className={input} />
              <datalist id="iana-timezones-edit">
                {timezones.map(tz => (
                  <option key={tz} value={tz} />
                ))}
              </datalist>
            </div>
          </div>
          <div className="grid grid-cols-2 gap-3">
            <div>
              <label className="text-xs text-muted font-medium">Inicio Turno</label>
              <input type="time" value={start} onChange={e => setStart(e.target.value)} className={input} />
            </div>
            <div>
              <label className="text-xs text-muted font-medium">Fin Turno</label>
              <input type="time" value={end} onChange={e => setEnd(e.target.value)} className={input} />
            </div>
          </div>

          {error && <div className="p-2.5 bg-danger-subtle border border-danger rounded-control text-xs text-danger">{error}</div>}

          <div className="flex justify-end gap-2 pt-2">
            <button type="button" onClick={onClose} className="px-4 py-2 bg-surface-subtle text-fg rounded-control text-xs hover:bg-strong transition">
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
