import React, { useMemo, useState } from 'react';
import { X } from 'lucide-react';
import {
  CreatedWorker,
  ConsentDescriptions,
  ConsentFlags,
  createWorker,
} from '../api/tauri-bridge';
import { ConsentEditor, ConsentPreview } from './ConsentEditor';
import {
  NO_CONSENT,
  NO_DESCRIPTIONS,
  descriptionErrors,
  ianaTimezones,
  isValidTimezone,
  normalizeDescriptions,
  workHoursError,
} from './consentCategories';

interface CreateWorkerModalProps {
  organization: string;
  onClose: () => void;
  onCreated: (created: CreatedWorker) => void;
}

/** W2 — New worker with admin-defined consent package (RF-ADMIN-08-2). */
export const CreateWorkerModal: React.FC<CreateWorkerModalProps> = ({ organization, onClose, onCreated }) => {
  const [name, setName] = useState('');
  const [email, setEmail] = useState('');
  const [role, setRole] = useState<'OFFICE' | 'FIELD'>('OFFICE');
  const [start, setStart] = useState('09:00');
  const [end, setEnd] = useState('18:00');
  const [timezone, setTimezone] = useState(
    () => Intl.DateTimeFormat().resolvedOptions().timeZone || 'America/Santiago',
  );
  const [ttlHours, setTtlHours] = useState(24);
  const [flags, setFlags] = useState<ConsentFlags>(NO_CONSENT);
  const [descriptions, setDescriptions] = useState<ConsentDescriptions>(NO_DESCRIPTIONS);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const timezones = useMemo(ianaTimezones, []);
  // DP-04: switching to OFFICE drops location.
  const effectiveFlags: ConsentFlags = role === 'OFFICE' ? { ...flags, location: false } : flags;

  const validationError = (): string | null => {
    if (!name.trim()) return 'El nombre es obligatorio.';
    if (!/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email.trim())) return 'Ingrese un correo válido.';
    const hours = workHoursError(start, end);
    if (hours) return hours;
    if (!isValidTimezone(timezone)) return 'Zona horaria IANA inválida (p. ej. America/Santiago).';
    if (!Number.isInteger(ttlHours) || ttlHours < 1 || ttlHours > 72) {
      return 'La vigencia del código debe estar entre 1 y 72 horas.';
    }
    if (!Object.values(effectiveFlags).some(Boolean)) return 'Active al menos una categoría de datos.';
    if (Object.keys(descriptionErrors(effectiveFlags, descriptions)).length > 0) {
      return 'Revise las descripciones de las categorías activas.';
    }
    return null;
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    const invalid = validationError();
    if (invalid) {
      setError(invalid);
      return;
    }
    setSubmitting(true);
    setError(null);
    try {
      const created = await createWorker({
        name: name.trim(),
        email: email.trim(),
        role_type: role,
        work_hours_start: start,
        work_hours_end: end,
        timezone: timezone.trim(),
        consent_flags: effectiveFlags,
        consent_descriptions: normalizeDescriptions(effectiveFlags, descriptions),
        enrollment_ttl_hours: ttlHours,
      });
      onCreated(created);
    } catch (err: any) {
      setError(err?.message || String(err));
    } finally {
      setSubmitting(false);
    }
  };

  const input =
    'w-full mt-1 bg-slate-900 border border-slate-700 text-slate-200 text-xs rounded-lg px-3 py-2 focus:outline-none focus:border-indigo-500';

  return (
    <div className="fixed inset-0 z-50 bg-black/80 flex items-center justify-center p-4">
      <div className="bg-slate-800 border border-slate-700 rounded-2xl max-w-4xl w-full p-6 shadow-2xl max-h-[92vh] overflow-y-auto">
        <div className="flex justify-between items-center border-b border-slate-700 pb-3 mb-4">
          <h3 className="text-base font-semibold text-white">Registrar Nuevo Trabajador</h3>
          <button onClick={onClose} className="text-slate-400 hover:text-white">
            <X className="w-5 h-5" />
          </button>
        </div>

        <form onSubmit={handleSubmit} className="grid grid-cols-1 lg:grid-cols-2 gap-5">
          <div className="space-y-3">
            <div>
              <label className="text-xs text-slate-400 font-medium">Nombre Completo</label>
              <input type="text" required value={name} onChange={e => setName(e.target.value)} className={input} />
            </div>
            <div>
              <label className="text-xs text-slate-400 font-medium">Correo Electrónico Corporativo</label>
              <input type="email" required value={email} onChange={e => setEmail(e.target.value)} className={input} />
            </div>
            <div className="grid grid-cols-2 gap-3">
              <div>
                <label className="text-xs text-slate-400 font-medium">Rol Laboral</label>
                <select value={role} onChange={e => setRole(e.target.value as 'OFFICE' | 'FIELD')} className={input}>
                  <option value="OFFICE">Oficina (OFFICE)</option>
                  <option value="FIELD">Terreno (FIELD)</option>
                </select>
              </div>
              <div>
                <label className="text-xs text-slate-400 font-medium">Zona Horaria (IANA)</label>
                <input
                  list="iana-timezones"
                  value={timezone}
                  onChange={e => setTimezone(e.target.value)}
                  className={input}
                />
                <datalist id="iana-timezones">
                  {timezones.map(tz => (
                    <option key={tz} value={tz} />
                  ))}
                </datalist>
              </div>
            </div>
            <div className="grid grid-cols-3 gap-3">
              <div>
                <label className="text-xs text-slate-400 font-medium">Inicio Turno</label>
                <input type="time" value={start} onChange={e => setStart(e.target.value)} className={input} />
              </div>
              <div>
                <label className="text-xs text-slate-400 font-medium">Fin Turno</label>
                <input type="time" value={end} onChange={e => setEnd(e.target.value)} className={input} />
              </div>
              <div>
                <label className="text-xs text-slate-400 font-medium">Vigencia código (h)</label>
                <input
                  type="number"
                  min={1}
                  max={72}
                  value={ttlHours}
                  onChange={e => setTtlHours(Number(e.target.value))}
                  className={input}
                />
              </div>
            </div>

            <div>
              <p className="text-xs text-slate-400 font-medium mb-1.5">Datos a recolectar</p>
              <ConsentEditor
                role={role}
                flags={effectiveFlags}
                descriptions={descriptions}
                onChange={(f, d) => {
                  setFlags(f);
                  setDescriptions(d);
                }}
              />
            </div>
          </div>

          <div className="space-y-3">
            <p className="text-xs text-slate-400 font-medium">Vista previa de lo que verá el trabajador</p>
            <ConsentPreview organization={organization} flags={effectiveFlags} descriptions={descriptions} />

            {error && (
              <div className="p-2.5 bg-rose-950/50 border border-rose-800/60 rounded-lg text-xs text-rose-300">{error}</div>
            )}

            <div className="flex justify-end gap-2 pt-2">
              <button
                type="button"
                onClick={onClose}
                className="px-4 py-2 bg-slate-700 text-slate-300 rounded-lg text-xs hover:bg-slate-600 transition"
              >
                Cancelar
              </button>
              <button
                type="submit"
                disabled={submitting}
                className="px-4 py-2 bg-indigo-600 hover:bg-indigo-500 text-white rounded-lg text-xs font-semibold transition disabled:opacity-50"
              >
                {submitting ? 'Creando…' : 'Crear y Generar Enrolamiento'}
              </button>
            </div>
          </div>
        </form>
      </div>
    </div>
  );
};
