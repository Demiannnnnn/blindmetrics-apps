import React, { useEffect, useMemo, useState } from 'react';
import { X } from 'lucide-react';
import {
  AdminProfile,
  CreatedWorker,
  ConsentDescriptions,
  ConsentFlags,
  CreateWorkerForm,
  ScheduleInput,
  WorkMode,
  Workplace,
  createWorker,
  workplacesList,
} from '../api/tauri-bridge';
import { describeError, errorIs } from '../lib/errors';
import { OrganizationPrompt } from './OrganizationPrompt';
import { ConsentEditor, ConsentPreview } from './ConsentEditor';
import { ScheduleEditor } from './ScheduleEditor';
import {
  NO_CONSENT,
  NO_DESCRIPTIONS,
  descriptionErrors,
  ianaTimezones,
  isValidTimezone,
  normalizeDescriptions,
} from './consentCategories';
import { formatRut, parseRut } from '../lib/rut';
import { validateScheduleInput } from '../lib/schedule';

interface CreateWorkerModalProps {
  organization: string;
  onClose: () => void;
  onCreated: (created: CreatedWorker) => void;
  /** Called after the organization prompt saved a new name (RF-ADMIN-09-5). */
  onProfileChanged?: (profile: AdminProfile) => void;
  /** Called when creating a worker fails because the employer profile is missing (RF-ADMIN-13-6). */
  onMissingEmployerProfile?: () => void;
}

/** W2 — New worker (RF-ADMIN-14-2, 5 sections in spec order). */
export const CreateWorkerModal: React.FC<CreateWorkerModalProps> = ({
  organization,
  onClose,
  onCreated,
  onProfileChanged,
  onMissingEmployerProfile,
}) => {
  // 1. Identificación
  const [name, setName] = useState('');
  const [run, setRun] = useState('');
  const [personalEmail, setPersonalEmail] = useState('');
  const [jobTitle, setJobTitle] = useState('');

  // 2. Lugar y modalidad
  const [workplaces, setWorkplaces] = useState<Workplace[]>([]);
  const [workplaceId, setWorkplaceId] = useState('');
  const [workMode, setWorkMode] = useState<WorkMode>('ON_SITE');
  const [art22Exempt, setArt22Exempt] = useState(false);
  const [role, setRole] = useState<'OFFICE' | 'FIELD'>('OFFICE');

  // 3. Jornada pactada
  const todayStr = useMemo(() => new Date().toISOString().slice(0, 10), []);
  const [schedule, setSchedule] = useState<ScheduleInput>({
    journey_type: 'FIXED',
    exceptional_resolution: null,
    days: [],
    break_counts_as_work: false,
    disconnection: null,
    location_at_mark: false,
    effective_from: todayStr,
    requested_by: 'EMPLOYER',
    note: null,
  });

  // 4. Telemetría (opcional)
  const [flags, setFlags] = useState<ConsentFlags>(NO_CONSENT);
  const [descriptions, setDescriptions] = useState<ConsentDescriptions>(NO_DESCRIPTIONS);
  const [locationCause, setLocationCause] = useState('');

  // 5. Código de vinculación
  const [ttlHours, setTtlHours] = useState(24);
  const [timezone, setTimezone] = useState(
    () => Intl.DateTimeFormat().resolvedOptions().timeZone || 'America/Santiago',
  );

  // States
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [pendingForm, setPendingForm] = useState<CreateWorkerForm | null>(null);
  const [currentOrganization, setCurrentOrganization] = useState(organization);
  const [loadingWorkplaces, setLoadingWorkplaces] = useState(true);

  const timezones = useMemo(ianaTimezones, []);

  useEffect(() => {
    workplacesList()
      .then(list => {
        setWorkplaces(list);
        if (list.length > 0) {
          setWorkplaceId(list[0].id);
        }
      })
      .catch(e => console.warn('Failed to load workplaces', e))
      .finally(() => setLoadingWorkplaces(false));
  }, []);

  // Update schedule disconnection default if switching to remote/hybrid
  useEffect(() => {
    if (workMode !== 'ON_SITE' && !schedule.disconnection) {
      setSchedule(s => ({
        ...s,
        disconnection: { start: '20:00', end: '08:00' },
      }));
    }
  }, [workMode, schedule.disconnection]);

  // DP-04 / DP-05: Location dropped for OFFICE; screenshots dropped for REMOTE/HYBRID
  const effectiveFlags: ConsentFlags = useMemo(() => {
    let f = { ...flags };
    if (role === 'OFFICE') f.location = false;
    if (workMode !== 'ON_SITE') f.productivity_screenshots = false;
    return f;
  }, [flags, role, workMode]);

  const handleRunBlur = () => {
    if (!run.trim()) return;
    try {
      const formatted = formatRut(run);
      setRun(formatted);
    } catch {
      // Keep user input if parsing fails
    }
  };

  const validate = (): string | null => {
    // 1. Identificación
    if (!name.trim() || name.trim().length < 2 || name.trim().length > 255) {
      return 'El nombre completo debe tener entre 2 y 255 caracteres.';
    }
    if (!run.trim()) {
      return 'El RUN del trabajador es obligatorio.';
    }
    try {
      parseRut(run);
    } catch (err) {
      return err instanceof Error ? err.message : 'El RUN ingresado es inválido (módulo 11).';
    }
    if (!personalEmail.trim() || !/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(personalEmail.trim())) {
      return 'Se requiere un correo personal válido para el envío de comprobantes.';
    }
    if (!jobTitle.trim() || jobTitle.trim().length < 2 || jobTitle.trim().length > 100) {
      return 'El cargo debe tener entre 2 y 100 caracteres.';
    }

    // 2. Lugar y modalidad
    if (!workplaceId) {
      return 'Selecciona un lugar de trabajo.';
    }

    // 3. Jornada pactada
    if (!art22Exempt) {
      const schedErrors = validateScheduleInput(schedule, workMode);
      if (schedErrors.length > 0) {
        return schedErrors[0];
      }
    }

    // 4. Telemetría
    if (effectiveFlags.location && role === 'FIELD') {
      const causeLen = locationCause.trim().length;
      if (causeLen < 20 || causeLen > 500) {
        return 'Se requiere declarar una causa de seguridad o legal (20 a 500 caracteres) para solicitar ubicación (R38 art. 53 e).';
      }
    }
    const descErrs = descriptionErrors(effectiveFlags, descriptions);
    if (Object.keys(descErrs).length > 0) {
      return Object.values(descErrs)[0];
    }

    // 5. Código
    if (!Number.isInteger(ttlHours) || ttlHours < 1 || ttlHours > 72) {
      return 'La vigencia del código debe estar entre 1 y 72 horas.';
    }
    if (timezone && !isValidTimezone(timezone)) {
      return 'Zona horaria IANA inválida (p. ej. America/Santiago).';
    }

    return null;
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    const err = validate();
    if (err) {
      setError(err);
      return;
    }

    const form: CreateWorkerForm = {
      name: name.trim(),
      run: formatRut(run),
      personal_email: personalEmail.trim(),
      job_title: jobTitle.trim(),
      workplace_id: workplaceId,
      work_mode: workMode,
      art22_exempt: art22Exempt,
      schedule: art22Exempt ? null : schedule,
      role_type: role,
      consent_flags: effectiveFlags,
      consent_descriptions: normalizeDescriptions(effectiveFlags, descriptions),
      location_cause: effectiveFlags.location ? locationCause.trim() : null,
      timezone: timezone.trim() || undefined,
      enrollment_ttl_hours: ttlHours,
    };

    await submitForm(form);
  };

  const submitForm = async (form: CreateWorkerForm) => {
    setSubmitting(true);
    setError(null);
    try {
      const created = await createWorker(form);
      onCreated(created);
    } catch (err: any) {
      if (errorIs(err, 'ORGANIZATION_NAME_MISSING')) {
        setPendingForm(form);
        return;
      }
      if (
        (errorIs(err, 'EMPLOYER_PROFILE_MISSING') ||
          String(err).includes('EMPLOYER_PROFILE_MISSING')) &&
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

  const inputClass =
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

  const hasAnyTelemetry = Object.values(effectiveFlags).some(Boolean);

  return (
    <div className="fixed inset-0 z-50 bg-black/80 flex items-center justify-center p-4">
      <div className="bg-surface-subtle border border-default rounded-card max-w-4xl w-full p-6 shadow-md max-h-[94vh] overflow-y-auto">
        <div className="flex justify-between items-center border-b border-default pb-3 mb-4">
          <div>
            <h3 className="text-base font-semibold text-fg">Registrar Nuevo Trabajador</h3>
            <p className="text-xs text-muted">
              Datos legales, jornada pactada y telemetría opcional (Resolución Exenta N° 38/2024)
            </p>
          </div>
          <button onClick={onClose} className="text-muted hover:text-fg">
            <X className="w-5 h-5" />
          </button>
        </div>

        <form onSubmit={handleSubmit} className="space-y-6">
          {/* SECCIÓN 1: Identificación */}
          <section className="bg-surface border border-default rounded-card p-4 space-y-3">
            <h4 className="text-xs font-semibold text-accent uppercase tracking-wider">
              1. Identificación del Trabajador
            </h4>
            <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
              <div>
                <label className="text-xs text-muted font-medium">Nombre Completo</label>
                <input
                  type="text"
                  required
                  placeholder="Ej. Juan Pérez González"
                  value={name}
                  onChange={e => setName(e.target.value)}
                  className={inputClass}
                />
              </div>

              <div>
                <label className="text-xs text-muted font-medium">RUN (RUT personal)</label>
                <input
                  type="text"
                  required
                  placeholder="12.345.678-5"
                  value={run}
                  onChange={e => setRun(e.target.value)}
                  onBlur={handleRunBlur}
                  className={inputClass}
                />
              </div>
            </div>

            <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
              <div>
                <label className="text-xs text-muted font-medium">Correo Electrónico Personal</label>
                <input
                  type="email"
                  required
                  placeholder="juan.perez@gmail.com"
                  value={personalEmail}
                  onChange={e => setPersonalEmail(e.target.value)}
                  className={inputClass}
                />
                <p className="text-[11px] text-muted mt-0.5">
                  Aquí llegan los comprobantes de cada marca. Debe ser un correo que controle el
                  trabajador (R38 art. 12 e).
                </p>
              </div>

              <div>
                <label className="text-xs text-muted font-medium">Cargo</label>
                <input
                  type="text"
                  required
                  placeholder="Ej. Desarrollador, Técnico, Vendedor"
                  value={jobTitle}
                  onChange={e => setJobTitle(e.target.value)}
                  className={inputClass}
                />
              </div>
            </div>
          </section>

          {/* SECCIÓN 2: Lugar y modalidad */}
          <section className="bg-surface border border-default rounded-card p-4 space-y-3">
            <h4 className="text-xs font-semibold text-accent uppercase tracking-wider">
              2. Lugar y Modalidad de Prestación de Servicios
            </h4>
            <div className="grid grid-cols-1 sm:grid-cols-3 gap-3">
              <div>
                <label className="text-xs text-muted font-medium">Lugar de Trabajo</label>
                {loadingWorkplaces ? (
                  <p className="text-xs text-muted mt-2">Cargando lugares...</p>
                ) : workplaces.length === 0 ? (
                  <div className="mt-1">
                    <p className="text-xs text-warning">
                      No hay lugares registrados.{' '}
                      {onMissingEmployerProfile && (
                        <button
                          type="button"
                          onClick={() => {
                            onClose();
                            onMissingEmployerProfile();
                          }}
                          className="underline text-accent"
                        >
                          Crear lugar en Empresa
                        </button>
                      )}
                    </p>
                  </div>
                ) : (
                  <select
                    value={workplaceId}
                    onChange={e => setWorkplaceId(e.target.value)}
                    className={inputClass}
                  >
                    {workplaces.map(w => (
                      <option key={w.id} value={w.id}>
                        {w.name}
                      </option>
                    ))}
                  </select>
                )}
              </div>

              <div>
                <label className="text-xs text-muted font-medium">Modalidad de Trabajo</label>
                <select
                  value={workMode}
                  onChange={e => setWorkMode(e.target.value as WorkMode)}
                  className={inputClass}
                >
                  <option value="ON_SITE">Presencial (ON_SITE)</option>
                  <option value="REMOTE">Teletrabajo (REMOTE)</option>
                  <option value="HYBRID">Mixta (HYBRID)</option>
                </select>
              </div>

              <div>
                <label className="text-xs text-muted font-medium">Rol para Telemetría</label>
                <select
                  value={role}
                  onChange={e => setRole(e.target.value as 'OFFICE' | 'FIELD')}
                  className={inputClass}
                >
                  <option value="OFFICE">Oficina (OFFICE)</option>
                  <option value="FIELD">Terreno (FIELD)</option>
                </select>
              </div>
            </div>

            <div className="pt-2 border-t border-default">
              <label className="flex items-start gap-2 text-xs text-fg cursor-pointer">
                <input
                  type="checkbox"
                  checked={art22Exempt}
                  onChange={e => setArt22Exempt(e.target.checked)}
                  className="mt-0.5 rounded-sm accent-accent"
                />
                <div>
                  <span className="font-semibold">
                    Excluido de limitación de jornada (art. 22 inc. 2° del Código del Trabajo)
                  </span>
                  <p className="text-[11px] text-muted">
                    Los trabajadores del art. 22 no tienen registro de asistencia ni jornada pactada.
                    La app del trabajador mostrará que está excluido de control horario (ATT-12).
                  </p>
                </div>
              </label>
            </div>
          </section>

          {/* SECCIÓN 3: Jornada pactada */}
          {!art22Exempt && (
            <section className="bg-surface border border-default rounded-card p-4 space-y-3">
              <h4 className="text-xs font-semibold text-accent uppercase tracking-wider">
                3. Jornada Pactada
              </h4>
              <ScheduleEditor
                value={schedule}
                onChange={setSchedule}
                workMode={workMode}
              />
            </section>
          )}

          {/* SECCIÓN 4: Telemetría (opcional) */}
          <section className="bg-surface border border-default rounded-card p-4 space-y-3">
            <div className="flex items-center justify-between">
              <h4 className="text-xs font-semibold text-accent uppercase tracking-wider">
                4. Telemetría Opcional
              </h4>
              <span className="text-[11px] text-muted">
                Puedes no solicitar telemetría: el trabajador igual podrá marcar asistencia.
              </span>
            </div>

            <ConsentEditor
              role={role}
              workMode={workMode}
              flags={effectiveFlags}
              descriptions={descriptions}
              locationCause={locationCause}
              onLocationCauseChange={setLocationCause}
              onChange={(f, d) => {
                setFlags(f);
                setDescriptions(d);
              }}
            />

            {hasAnyTelemetry && (
              <div className="pt-3 border-t border-default">
                <p className="text-xs font-medium text-muted mb-2">
                  Vista previa de la solicitud de telemetría (se muestra al trabajador):
                </p>
                <ConsentPreview
                  organization={currentOrganization}
                  flags={effectiveFlags}
                  descriptions={descriptions}
                />
              </div>
            )}
          </section>

          {/* SECCIÓN 5: Código de vinculación */}
          <section className="bg-surface border border-default rounded-card p-4 space-y-3">
            <h4 className="text-xs font-semibold text-accent uppercase tracking-wider">
              5. Código de Vinculación y Parámetros
            </h4>
            <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
              <div>
                <label className="text-xs text-muted font-medium">
                  Vigencia del código de vinculación (horas)
                </label>
                <input
                  type="number"
                  min={1}
                  max={72}
                  value={ttlHours}
                  onChange={e => setTtlHours(Number(e.target.value))}
                  className={inputClass}
                />
                <p className="text-[11px] text-muted mt-0.5">
                  El trabajador tendrá este plazo para vincular su equipo antes de que el código
                  caduque (1 a 72 h, por defecto 24).
                </p>
              </div>

              <div>
                <label className="text-xs text-muted font-medium">Zona Horaria (IANA)</label>
                <input
                  list="iana-timezones"
                  value={timezone}
                  onChange={e => setTimezone(e.target.value)}
                  className={inputClass}
                />
                <datalist id="iana-timezones">
                  {timezones.map(tz => (
                    <option key={tz} value={tz} />
                  ))}
                </datalist>
              </div>
            </div>
          </section>

          {error && (
            <div className="p-3 bg-danger-subtle border border-danger rounded-card text-xs text-danger">
              {error}
            </div>
          )}

          <div className="flex justify-end gap-2 pt-2 border-t border-default">
            <button
              type="button"
              onClick={onClose}
              className="px-4 py-2 bg-surface hover:bg-surface-subtle text-fg border border-default rounded-control text-xs transition"
            >
              Cancelar
            </button>
            <button
              type="submit"
              disabled={submitting}
              className="px-5 py-2 bg-accent hover:bg-accent-hover text-accent-fg rounded-control text-xs font-semibold transition disabled:opacity-50"
            >
              {submitting ? 'Creando trabajador…' : 'Crear y Generar Enrolamiento'}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
};
