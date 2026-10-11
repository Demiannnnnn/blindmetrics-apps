import React, { useMemo, useState } from 'react';
import { X } from 'lucide-react';
import {
  AdminProfile,
  CreatedWorker,
  ConsentDescriptions,
  ConsentFlags,
  CreateWorkerForm,
  createWorker,
} from '../api/tauri-bridge';
import { describeError, errorIs } from '../lib/errors';
import { OrganizationPrompt } from './OrganizationPrompt';
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
  /** Called after the organization prompt saved a new name (RF-ADMIN-09-5). */
  onProfileChanged?: (profile: AdminProfile) => void;
  /** Called when creating a worker fails because the employer profile is missing (RF-ADMIN-13-6). */
  onMissingEmployerProfile?: () => void;
}

/** W2 — New worker with admin-defined consent package (RF-ADMIN-08-2, RF-ADMIN-09-5/6). */
export const CreateWorkerModal: React.FC<CreateWorkerModalProps> = ({
  organization,
  onClose,
  onCreated,
  onProfileChanged,
  onMissingEmployerProfile,
}) => {
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
  const [pendingForm, setPendingForm] = useState<CreateWorkerForm | null>(null);
  const [currentOrganization, setCurrentOrganization] = useState(organization);

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
    const form: CreateWorkerForm = {
      name: name.trim(),
      email: email.trim(),
      role_type: role,
      work_hours_start: start,
      work_hours_end: end,
      timezone: timezone.trim(),
      consent_flags: effectiveFlags,
      consent_descriptions: normalizeDescriptions(effectiveFlags, descriptions),
      enrollment_ttl_hours: ttlHours,
    };
    await submitForm(form);
  };

  const submitForm = async (form: CreateWorkerForm) => {
    setSubmitting(true);
    setError(null);
    try {
      onCreated(await createWorker(form));
    } catch (err: any) {
      if (errorIs(err, 'ORGANIZATION_NAME_MISSING')) {
        // RF-ADMIN-09-5: ask for the organization, then retry with the same form.
        setPendingForm(form);
        return;
      }
      if (
        (errorIs(err, 'EMPLOYER_PROFILE_MISSING') || String(err).includes('EMPLOYER_PROFILE_MISSING')) &&
        onMissingEmployerProfile
      ) {
        onClose();
        onMissingEmployerProfile();
        return;
      }
      setError(describeError(err));
    } finally {
      setSubmitting(false);
    }
  };

  const input =
    'w-full mt-1 bg-surface border border-default text-fg text-xs rounded-control px-3 py-2 focus:outline-hidden focus:border-accent';

  if (pendingForm) {
    return (
      <OrganizationPrompt
        initial={currentOrganization}
        onSaved={profile => {
          setCurrentOrganization(profile.organization);
          onProfileChanged?.(profile);
          const form = pendingForm;
          setPendingForm(null);
          submitForm(form);
        }}
        onCancel={() => {
          setPendingForm(null);
          setError('Indica el nombre de tu organización para crear trabajadores.');
        }}
      />
    );
  }

  return (
    <div className="fixed inset-0 z-50 bg-black/80 flex items-center justify-center p-4">
      <div className="bg-surface-subtle border border-default rounded-card max-w-4xl w-full p-6 shadow-md max-h-[92vh] overflow-y-auto">
        <div className="flex justify-between items-center border-b border-default pb-3 mb-4">
          <h3 className="text-base font-semibold text-fg">Registrar Nuevo Trabajador</h3>
          <button onClick={onClose} className="text-muted hover:text-fg">
            <X className="w-5 h-5" />
          </button>
        </div>

        <form onSubmit={handleSubmit} className="grid grid-cols-1 lg:grid-cols-2 gap-5">
          <div className="space-y-3">
            <div>
              <label className="text-xs text-muted font-medium">Nombre Completo</label>
              <input type="text" required value={name} onChange={e => setName(e.target.value)} className={input} />
            </div>
            <div>
              <label className="text-xs text-muted font-medium">Correo Electrónico Corporativo</label>
              <input type="email" required value={email} onChange={e => setEmail(e.target.value)} className={input} />
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
                <label className="text-xs text-muted font-medium">Inicio Turno</label>
                <input type="time" value={start} onChange={e => setStart(e.target.value)} className={input} />
              </div>
              <div>
                <label className="text-xs text-muted font-medium">Fin Turno</label>
                <input type="time" value={end} onChange={e => setEnd(e.target.value)} className={input} />
              </div>
              <div>
                <label className="text-xs text-muted font-medium">Vigencia código (h)</label>
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
              <p className="text-xs text-muted font-medium mb-1.5">Datos a recolectar</p>
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
            <p className="text-xs text-muted font-medium">Vista previa de lo que verá el trabajador</p>
            <ConsentPreview organization={currentOrganization} flags={effectiveFlags} descriptions={descriptions} />

            {error && (
              <div className="p-2.5 bg-danger-subtle border border-danger rounded-control text-xs text-danger">{error}</div>
            )}

            <div className="flex justify-end gap-2 pt-2">
              <button
                type="button"
                onClick={onClose}
                className="px-4 py-2 bg-surface-subtle text-fg rounded-control text-xs hover:bg-strong transition"
              >
                Cancelar
              </button>
              <button
                type="submit"
                disabled={submitting}
                className="px-4 py-2 bg-accent hover:bg-accent-hover text-accent-fg rounded-control text-xs font-semibold transition disabled:opacity-50"
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
