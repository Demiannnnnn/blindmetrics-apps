import React, { useEffect, useState } from 'react';
import {
  Building2,
  MapPin,
  Bell,
  AlertOctagon,
  Plus,
  Pencil,
  Trash2,
  CheckCircle2,
  AlertTriangle,
  X,
} from 'lucide-react';
import {
  employerGet,
  employerUpdate,
  workplacesList,
  workplacesCreate,
  workplacesUpdate,
  workplacesDelete,
  attendanceSettingsGet,
  attendanceSettingsUpdate,
  incidentsList,
  incidentsCreate,
  EmployerProfile as IEmployerProfile,
  Workplace,
  WorkplaceForm,
  AttendanceSettings,
  TechnicalIncident,
  CreateIncidentForm,
} from '../api/tauri-bridge';
import { formatRut, validateRut } from '../lib/rut';
import { describeError } from '../lib/errors';
import { CHILEAN_REGIONS, formatAddress } from '../lib/employer';

export { CHILEAN_REGIONS, formatAddress };

export const EmployerProfile: React.FC<{ onProfileUpdated?: () => void }> = ({ onProfileUpdated }) => {
  // Employer form state
  const [profile, setProfile] = useState<IEmployerProfile>({
    kind: 'LEGAL_ENTITY',
    legal_name: '',
    rut: '',
    address: {
      street: '',
      number: '',
      floor: '',
      office: '',
      commune: '',
      city: '',
      region: 'Región Metropolitana de Santiago',
    },
  });
  const [loadingProfile, setLoadingProfile] = useState(true);
  const [savingProfile, setSavingProfile] = useState(false);
  const [profileMessage, setProfileMessage] = useState<{ type: 'success' | 'error'; text: string } | null>(null);

  // Workplaces state
  const [workplaces, setWorkplaces] = useState<Workplace[]>([]);
  const [loadingWorkplaces, setLoadingWorkplaces] = useState(true);
  const [editingWorkplace, setEditingWorkplace] = useState<Workplace | null>(null);
  const [isCreatingWorkplace, setIsCreatingWorkplace] = useState(false);
  const [deletingWorkplace, setDeletingWorkplace] = useState<Workplace | null>(null);
  const [workplaceForm, setWorkplaceForm] = useState<WorkplaceForm>({
    name: '',
    address: {
      street: '',
      number: '',
      floor: '',
      office: '',
      commune: '',
      city: '',
      region: 'Región Metropolitana de Santiago',
    },
  });
  const [workplaceModalError, setWorkplaceModalError] = useState<string | null>(null);
  const [workplaceModalSaving, setWorkplaceModalSaving] = useState(false);

  // Settings state
  const [settings, setSettings] = useState<AttendanceSettings>({ missing_mark_alerts: true });
  const [updatingSettings, setUpdatingSettings] = useState(false);
  const [settingsFeedback, setSettingsFeedback] = useState<string | null>(null);

  // Incidents state
  const [incidents, setIncidents] = useState<TechnicalIncident[]>([]);
  const [loadingIncidents, setLoadingIncidents] = useState(true);
  const [isRegisteringIncident, setIsRegisteringIncident] = useState(false);
  const [incidentForm, setIncidentForm] = useState<CreateIncidentForm>({
    started_at: '',
    ended_at: '',
    description: '',
  });
  const [incidentModalError, setIncidentModalError] = useState<string | null>(null);
  const [incidentModalSaving, setIncidentModalSaving] = useState(false);

  useEffect(() => {
    let isMounted = true;

    employerGet()
      .then(p => {
        if (!isMounted) return;
        if (p) {
          setProfile({
            kind: p.kind,
            legal_name: p.legal_name,
            rut: p.rut,
            address: {
              street: p.address.street || '',
              number: p.address.number || '',
              floor: p.address.floor || '',
              office: p.address.office || '',
              commune: p.address.commune || '',
              city: p.address.city || '',
              region: p.address.region || 'Región Metropolitana de Santiago',
            },
          });
        }
        setLoadingProfile(false);
      })
      .catch(() => {
        if (isMounted) setLoadingProfile(false);
      });

    workplacesList()
      .then(list => {
        if (!isMounted) return;
        setWorkplaces(list);
        setLoadingWorkplaces(false);
      })
      .catch(() => {
        if (isMounted) setLoadingWorkplaces(false);
      });

    attendanceSettingsGet()
      .then(s => {
        if (!isMounted) return;
        setSettings(s);
      })
      .catch(() => {});

    incidentsList()
      .then(list => {
        if (!isMounted) return;
        setIncidents(list);
        setLoadingIncidents(false);
      })
      .catch(() => {
        if (isMounted) setLoadingIncidents(false);
      });

    return () => {
      isMounted = false;
    };
  }, []);

  // --- Handlers: Employer Profile ---
  const handleProfileRutBlur = () => {
    if (profile.rut.trim()) {
      const formatted = formatRut(profile.rut);
      setProfile(prev => ({ ...prev, rut: formatted }));
    }
  };

  const handleSaveProfile = async (e: React.FormEvent) => {
    e.preventDefault();
    setProfileMessage(null);

    // Validations
    if (!profile.legal_name.trim() || profile.legal_name.trim().length < 2) {
      setProfileMessage({ type: 'error', text: 'La razón social o nombre debe tener al menos 2 caracteres.' });
      return;
    }
    if (!validateRut(profile.rut)) {
      setProfileMessage({ type: 'error', text: 'El RUT ingresado no es válido (dígito verificador incorrecto).' });
      return;
    }
    if (!profile.address.street.trim() || profile.address.street.trim().length < 2) {
      setProfileMessage({ type: 'error', text: 'La calle de la dirección debe tener al menos 2 caracteres.' });
      return;
    }
    if (!profile.address.number.trim()) {
      setProfileMessage({ type: 'error', text: 'Debe ingresar el número de la dirección.' });
      return;
    }
    if (!profile.address.commune.trim()) {
      setProfileMessage({ type: 'error', text: 'Debe ingresar la comuna.' });
      return;
    }
    if (!profile.address.city.trim()) {
      setProfileMessage({ type: 'error', text: 'Debe ingresar la ciudad.' });
      return;
    }

    setSavingProfile(true);
    try {
      const saved = await employerUpdate(profile);
      setProfile(saved);
      setProfileMessage({ type: 'success', text: 'Datos del empleador actualizados correctamente.' });
      if (onProfileUpdated) onProfileUpdated();
    } catch (err) {
      setProfileMessage({ type: 'error', text: describeError(err, 'Error al guardar los datos del empleador.') });
    } finally {
      setSavingProfile(false);
    }
  };

  // --- Handlers: Workplaces ---
  const openCreateWorkplaceModal = () => {
    setWorkplaceForm({
      name: '',
      address: {
        street: profile.address.street || '',
        number: profile.address.number || '',
        floor: profile.address.floor || '',
        office: profile.address.office || '',
        commune: profile.address.commune || '',
        city: profile.address.city || '',
        region: profile.address.region || 'Región Metropolitana de Santiago',
      },
    });
    setWorkplaceModalError(null);
    setIsCreatingWorkplace(true);
  };

  const openEditWorkplaceModal = (wp: Workplace) => {
    setEditingWorkplace(wp);
    setWorkplaceForm({
      name: wp.name,
      address: {
        street: wp.address.street || '',
        number: wp.address.number || '',
        floor: wp.address.floor || '',
        office: wp.address.office || '',
        commune: wp.address.commune || '',
        city: wp.address.city || '',
        region: wp.address.region || 'Región Metropolitana de Santiago',
      },
    });
    setWorkplaceModalError(null);
  };

  const handleSaveWorkplace = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!workplaceForm.name.trim()) {
      setWorkplaceModalError('El nombre del lugar de trabajo es obligatorio.');
      return;
    }
    if (!workplaceForm.address.street.trim() || !workplaceForm.address.number.trim()) {
      setWorkplaceModalError('Debe ingresar la calle y número de la dirección.');
      return;
    }
    if (!workplaceForm.address.commune.trim() || !workplaceForm.address.city.trim()) {
      setWorkplaceModalError('Debe ingresar la comuna y ciudad.');
      return;
    }

    setWorkplaceModalSaving(true);
    setWorkplaceModalError(null);
    try {
      if (editingWorkplace) {
        const updated = await workplacesUpdate(editingWorkplace.id, workplaceForm);
        setWorkplaces(prev => prev.map(w => (w.id === updated.id ? updated : w)));
        setEditingWorkplace(null);
      } else {
        const created = await workplacesCreate(workplaceForm);
        setWorkplaces(prev => [...prev, created]);
        setIsCreatingWorkplace(false);
      }
    } catch (err) {
      setWorkplaceModalError(describeError(err, 'Error al guardar el lugar de trabajo.'));
    } finally {
      setWorkplaceModalSaving(false);
    }
  };

  const handleDeleteWorkplace = async () => {
    if (!deletingWorkplace) return;
    try {
      await workplacesDelete(deletingWorkplace.id);
      setWorkplaces(prev => prev.filter(w => w.id !== deletingWorkplace.id));
      setDeletingWorkplace(null);
    } catch (err) {
      alert(describeError(err, 'No se pudo eliminar el lugar de trabajo.'));
    }
  };

  // --- Handlers: Settings ---
  const handleToggleAlerts = async () => {
    const newVal = !settings.missing_mark_alerts;
    setUpdatingSettings(true);
    setSettingsFeedback(null);
    try {
      const updated = await attendanceSettingsUpdate(newVal);
      setSettings(updated);
      setSettingsFeedback('Configuración guardada.');
      setTimeout(() => setSettingsFeedback(null), 3000);
    } catch (err) {
      alert(describeError(err, 'Error al actualizar configuración de alertas.'));
    } finally {
      setUpdatingSettings(false);
    }
  };

  // --- Handlers: Technical Incidents ---
  const handleSaveIncident = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!incidentForm.started_at) {
      setIncidentModalError('Debe ingresar la fecha y hora de inicio del incidente.');
      return;
    }
    if (!incidentForm.description.trim() || incidentForm.description.trim().length < 5) {
      setIncidentModalError('La descripción debe tener al menos 5 caracteres.');
      return;
    }

    setIncidentModalSaving(true);
    setIncidentModalError(null);
    try {
      const payload: CreateIncidentForm = {
        started_at: new Date(incidentForm.started_at).toISOString(),
        ended_at: incidentForm.ended_at ? new Date(incidentForm.ended_at).toISOString() : null,
        description: incidentForm.description.trim(),
      };
      const created = await incidentsCreate(payload);
      setIncidents(prev => [created, ...prev]);
      setIsRegisteringIncident(false);
      setIncidentForm({ started_at: '', ended_at: '', description: '' });
    } catch (err) {
      setIncidentModalError(describeError(err, 'Error al registrar el incidente técnico.'));
    } finally {
      setIncidentModalSaving(false);
    }
  };

  return (
    <div className="space-y-6 max-w-5xl">
      {/* Header */}
      <div>
        <h1 className="text-xl font-bold text-fg flex items-center gap-2">
          <Building2 className="w-6 h-6 text-accent" />
          Empresa & Parámetros Legales
        </h1>
        <p className="text-xs text-muted mt-0.5">
          Razón social, RUT, lugares de trabajo y parámetros obligatorios según Resolución Exenta N°38 (R38).
        </p>
      </div>

      {/* Card 1: Datos del empleador */}
      <div className="bg-surface border border-default rounded-card p-5 space-y-4 shadow-xs">
        <div className="border-b border-default pb-3 flex items-center justify-between">
          <div>
            <h2 className="text-sm font-semibold text-fg flex items-center gap-2">
              <Building2 className="w-4 h-4 text-accent" />
              Datos Legales del Empleador
            </h2>
            <p className="text-xs text-muted mt-0.5">
              Identificación jurídica obligatoria para emisión de comprobantes y reportes legales de asistencia.
            </p>
          </div>
        </div>

        {profileMessage && (
          <div
            className={`p-3 rounded-control border text-xs flex items-center gap-2 ${
              profileMessage.type === 'success'
                ? 'bg-success-subtle border-default text-success'
                : 'bg-danger-subtle border-default text-danger'
            }`}
          >
            {profileMessage.type === 'success' ? (
              <CheckCircle2 className="w-4 h-4 shrink-0" />
            ) : (
              <AlertTriangle className="w-4 h-4 shrink-0" />
            )}
            <span>{profileMessage.text}</span>
          </div>
        )}

        {loadingProfile ? (
          <div className="py-8 flex items-center justify-center gap-2 text-xs text-muted">
            <div className="w-4 h-4 border-2 border-accent border-t-transparent rounded-full animate-spin" />
            <span>Cargando datos del empleador...</span>
          </div>
        ) : (
          <form onSubmit={handleSaveProfile} className="space-y-4">
            <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
              <div>
                <label className="block text-xs font-medium text-fg mb-1">Tipo de persona</label>
                <select
                  value={profile.kind}
                  onChange={e => setProfile(prev => ({ ...prev, kind: e.target.value as any }))}
                  className="w-full bg-surface-subtle border border-default text-fg text-xs rounded-control px-3 py-2 focus:outline-hidden focus:border-accent"
                >
                  <option value="LEGAL_ENTITY">Persona Jurídica (SpA, SA, Ltda)</option>
                  <option value="NATURAL_PERSON">Persona Natural</option>
                </select>
              </div>

              <div>
                <label className="block text-xs font-medium text-fg mb-1">
                  {profile.kind === 'LEGAL_ENTITY' ? 'Razón Social' : 'Nombre Completo'}
                </label>
                <input
                  type="text"
                  value={profile.legal_name}
                  onChange={e => setProfile(prev => ({ ...prev, legal_name: e.target.value }))}
                  placeholder="Ej. Acme Servicios SpA"
                  className="w-full bg-surface-subtle border border-default text-fg text-xs rounded-control px-3 py-2 focus:outline-hidden focus:border-accent"
                />
              </div>

              <div>
                <label className="block text-xs font-medium text-fg mb-1">RUT del Empleador</label>
                <input
                  type="text"
                  value={profile.rut}
                  onChange={e => setProfile(prev => ({ ...prev, rut: e.target.value }))}
                  onBlur={handleProfileRutBlur}
                  placeholder="76.123.456-7"
                  className="w-full bg-surface-subtle border border-default text-fg text-xs rounded-control px-3 py-2 focus:outline-hidden focus:border-accent font-mono"
                />
              </div>
            </div>

            <div className="pt-2 border-t border-default">
              <h3 className="text-xs font-semibold text-fg mb-3 flex items-center gap-1.5">
                <MapPin className="w-3.5 h-3.5 text-accent" />
                Dirección de Casa Matriz
              </h3>

              <div className="grid grid-cols-1 sm:grid-cols-2 md:grid-cols-4 gap-3">
                <div className="sm:col-span-2">
                  <label className="block text-xs font-medium text-fg mb-1">Calle</label>
                  <input
                    type="text"
                    value={profile.address.street}
                    onChange={e =>
                      setProfile(prev => ({
                        ...prev,
                        address: { ...prev.address, street: e.target.value },
                      }))
                    }
                    placeholder="Av. Apoquindo"
                    className="w-full bg-surface-subtle border border-default text-fg text-xs rounded-control px-3 py-2 focus:outline-hidden focus:border-accent"
                  />
                </div>

                <div>
                  <label className="block text-xs font-medium text-fg mb-1">Número</label>
                  <input
                    type="text"
                    value={profile.address.number}
                    onChange={e =>
                      setProfile(prev => ({
                        ...prev,
                        address: { ...prev.address, number: e.target.value },
                      }))
                    }
                    placeholder="1234"
                    className="w-full bg-surface-subtle border border-default text-fg text-xs rounded-control px-3 py-2 focus:outline-hidden focus:border-accent"
                  />
                </div>

                <div className="grid grid-cols-2 gap-2">
                  <div>
                    <label className="block text-xs font-medium text-fg mb-1">Piso</label>
                    <input
                      type="text"
                      value={profile.address.floor || ''}
                      onChange={e =>
                        setProfile(prev => ({
                          ...prev,
                          address: { ...prev.address, floor: e.target.value },
                        }))
                      }
                      placeholder="5"
                      className="w-full bg-surface-subtle border border-default text-fg text-xs rounded-control px-2 py-2 focus:outline-hidden focus:border-accent"
                    />
                  </div>
                  <div>
                    <label className="block text-xs font-medium text-fg mb-1">Oficina</label>
                    <input
                      type="text"
                      value={profile.address.office || ''}
                      onChange={e =>
                        setProfile(prev => ({
                          ...prev,
                          address: { ...prev.address, office: e.target.value },
                        }))
                      }
                      placeholder="501"
                      className="w-full bg-surface-subtle border border-default text-fg text-xs rounded-control px-2 py-2 focus:outline-hidden focus:border-accent"
                    />
                  </div>
                </div>

                <div>
                  <label className="block text-xs font-medium text-fg mb-1">Comuna</label>
                  <input
                    type="text"
                    value={profile.address.commune}
                    onChange={e =>
                      setProfile(prev => ({
                        ...prev,
                        address: { ...prev.address, commune: e.target.value },
                      }))
                    }
                    placeholder="Las Condes"
                    className="w-full bg-surface-subtle border border-default text-fg text-xs rounded-control px-3 py-2 focus:outline-hidden focus:border-accent"
                  />
                </div>

                <div>
                  <label className="block text-xs font-medium text-fg mb-1">Ciudad</label>
                  <input
                    type="text"
                    value={profile.address.city}
                    onChange={e =>
                      setProfile(prev => ({
                        ...prev,
                        address: { ...prev.address, city: e.target.value },
                      }))
                    }
                    placeholder="Santiago"
                    className="w-full bg-surface-subtle border border-default text-fg text-xs rounded-control px-3 py-2 focus:outline-hidden focus:border-accent"
                  />
                </div>

                <div className="sm:col-span-2">
                  <label className="block text-xs font-medium text-fg mb-1">Región</label>
                  <select
                    value={profile.address.region}
                    onChange={e =>
                      setProfile(prev => ({
                        ...prev,
                        address: { ...prev.address, region: e.target.value },
                      }))
                    }
                    className="w-full bg-surface-subtle border border-default text-fg text-xs rounded-control px-3 py-2 focus:outline-hidden focus:border-accent"
                  >
                    {CHILEAN_REGIONS.map(reg => (
                      <option key={reg} value={reg}>
                        {reg}
                      </option>
                    ))}
                  </select>
                </div>
              </div>
            </div>

            <div className="flex justify-end pt-2">
              <button
                type="submit"
                disabled={savingProfile}
                className="px-4 py-2 bg-accent hover:bg-accent-hover text-accent-fg text-xs font-semibold rounded-control transition-colors flex items-center gap-1.5 cursor-pointer disabled:opacity-50"
              >
                {savingProfile ? (
                  <div className="w-3.5 h-3.5 border-2 border-accent-fg border-t-transparent rounded-full animate-spin" />
                ) : null}
                <span>Guardar datos de la empresa</span>
              </button>
            </div>
          </form>
        )}
      </div>

      {/* Card 2: Lugares de trabajo */}
      <div className="bg-surface border border-default rounded-card p-5 space-y-4 shadow-xs">
        <div className="border-b border-default pb-3 flex items-center justify-between">
          <div>
            <h2 className="text-sm font-semibold text-fg flex items-center gap-2">
              <MapPin className="w-4 h-4 text-accent" />
              Lugares de Trabajo
            </h2>
            <p className="text-xs text-muted mt-0.5">
              Establecimientos, sucursales y faenas donde los trabajadores prestan servicios presenciales o mixtos.
            </p>
          </div>
          <button
            type="button"
            onClick={openCreateWorkplaceModal}
            className="px-3 py-1.5 bg-accent hover:bg-accent-hover text-accent-fg text-xs font-semibold rounded-control transition-colors flex items-center gap-1 cursor-pointer"
          >
            <Plus className="w-3.5 h-3.5" />
            <span>Agregar lugar</span>
          </button>
        </div>

        {loadingWorkplaces ? (
          <div className="py-6 flex items-center justify-center gap-2 text-xs text-muted">
            <div className="w-4 h-4 border-2 border-accent border-t-transparent rounded-full animate-spin" />
            <span>Cargando lugares de trabajo...</span>
          </div>
        ) : workplaces.length === 0 ? (
          <div className="py-8 text-center text-xs text-muted space-y-1">
            <p>No hay lugares de trabajo registrados.</p>
            <p className="text-muted/80">
              Agrega al menos un lugar (ej. Casa Matriz o Sucursal) para asignarlo a tus trabajadores.
            </p>
          </div>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full text-left text-xs">
              <thead>
                <tr className="border-b border-default text-muted font-medium">
                  <th className="py-2.5 px-3">Nombre</th>
                  <th className="py-2.5 px-3">Dirección</th>
                  <th className="py-2.5 px-3 text-center">Trabajadores Asignados</th>
                  <th className="py-2.5 px-3 text-right">Acciones</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-default">
                {workplaces.map(wp => {
                  const inUse = (wp.assigned_workers_count ?? 0) > 0;
                  return (
                    <tr key={wp.id} className="hover:bg-surface-subtle/50 transition-colors">
                      <td className="py-2.5 px-3 font-medium text-fg">{wp.name}</td>
                      <td className="py-2.5 px-3 text-muted">{formatAddress(wp.address)}</td>
                      <td className="py-2.5 px-3 text-center">
                        <span className="inline-block px-2 py-0.5 rounded-full bg-surface-subtle border border-default text-xs font-mono">
                          {wp.assigned_workers_count ?? 0}
                        </span>
                      </td>
                      <td className="py-2.5 px-3 text-right">
                        <div className="flex items-center justify-end gap-1.5">
                          <button
                            type="button"
                            onClick={() => openEditWorkplaceModal(wp)}
                            className="p-1 rounded-control text-muted hover:text-fg hover:bg-surface-subtle transition-colors cursor-pointer"
                            title="Editar lugar de trabajo"
                          >
                            <Pencil className="w-3.5 h-3.5" />
                          </button>
                          <button
                            type="button"
                            onClick={() => setDeletingWorkplace(wp)}
                            disabled={inUse}
                            className={`p-1 rounded-control transition-colors ${
                              inUse
                                ? 'text-muted/40 cursor-not-allowed'
                                : 'text-danger hover:bg-danger-subtle cursor-pointer'
                            }`}
                            title={
                              inUse
                                ? 'No se puede eliminar: tiene trabajadores asignados'
                                : 'Eliminar lugar de trabajo'
                            }
                          >
                            <Trash2 className="w-3.5 h-3.5" />
                          </button>
                        </div>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        )}
      </div>

      {/* Card 3: Alertas de marcación (R38 art. 45.1) */}
      <div className="bg-surface border border-default rounded-card p-5 space-y-4 shadow-xs">
        <div className="flex items-start justify-between gap-4">
          <div className="space-y-1">
            <h2 className="text-sm font-semibold text-fg flex items-center gap-2">
              <Bell className="w-4 h-4 text-accent" />
              Alertas por Marcas Faltantes (R38 art. 45.1)
            </h2>
            <p className="text-xs text-muted leading-relaxed max-w-2xl">
              Conforme al art. 45.1 de la Resolución Exenta N°38 de la Dirección del Trabajo, el sistema genera
              alertas por correo electrónico transcurridos <strong>30 minutos</strong> desde la hora de inicio o término
              pactada sin que se haya registrado la marcación respectiva. La alerta se remite al trabajador con copia al
              empleador.
            </p>
          </div>

          <div className="flex items-center gap-3 shrink-0">
            {settingsFeedback && <span className="text-xs text-success font-medium">{settingsFeedback}</span>}
            <button
              type="button"
              onClick={handleToggleAlerts}
              disabled={updatingSettings}
              className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors cursor-pointer ${
                settings.missing_mark_alerts ? 'bg-accent' : 'bg-surface-subtle border border-default'
              }`}
              role="switch"
              aria-checked={settings.missing_mark_alerts}
              aria-label="Alertas por marcas faltantes"
            >
              <span
                className={`inline-block h-4 w-4 transform rounded-full bg-white transition-transform ${
                  settings.missing_mark_alerts ? 'translate-x-6' : 'translate-x-1'
                }`}
              />
            </button>
          </div>
        </div>
      </div>

      {/* Card 4: Incidentes técnicos (R38 art. 27, 28) */}
      <div className="bg-surface border border-default rounded-card p-5 space-y-4 shadow-xs">
        <div className="border-b border-default pb-3 flex items-center justify-between">
          <div>
            <h2 className="text-sm font-semibold text-fg flex items-center gap-2">
              <AlertOctagon className="w-4 h-4 text-accent" />
              Registro de Incidentes Técnicos (R38)
            </h2>
            <p className="text-xs text-muted mt-0.5">
              Bitácora auditable de fallas del sistema o conectividad que justifiquen indisponibilidad del servicio de
              marcado.
            </p>
          </div>
          <button
            type="button"
            onClick={() => {
              setIncidentForm({
                started_at: new Date().toISOString().slice(0, 16),
                ended_at: '',
                description: '',
              });
              setIncidentModalError(null);
              setIsRegisteringIncident(true);
            }}
            className="px-3 py-1.5 bg-accent hover:bg-accent-hover text-accent-fg text-xs font-semibold rounded-control transition-colors flex items-center gap-1 cursor-pointer"
          >
            <Plus className="w-3.5 h-3.5" />
            <span>Registrar incidente</span>
          </button>
        </div>

        {loadingIncidents ? (
          <div className="py-6 flex items-center justify-center gap-2 text-xs text-muted">
            <div className="w-4 h-4 border-2 border-accent border-t-transparent rounded-full animate-spin" />
            <span>Cargando incidentes técnicos...</span>
          </div>
        ) : incidents.length === 0 ? (
          <div className="py-6 text-center text-xs text-muted">
            No hay incidentes técnicos registrados en el sistema.
          </div>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full text-left text-xs">
              <thead>
                <tr className="border-b border-default text-muted font-medium">
                  <th className="py-2.5 px-3">Inicio</th>
                  <th className="py-2.5 px-3">Término</th>
                  <th className="py-2.5 px-3">Descripción</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-default">
                {incidents.map(inc => (
                  <tr key={inc.id} className="hover:bg-surface-subtle/50 transition-colors">
                    <td className="py-2.5 px-3 font-mono text-fg whitespace-nowrap">
                      {new Date(inc.started_at).toLocaleString('es-CL')}
                    </td>
                    <td className="py-2.5 px-3 font-mono text-muted whitespace-nowrap">
                      {inc.ended_at ? new Date(inc.ended_at).toLocaleString('es-CL') : 'En curso'}
                    </td>
                    <td className="py-2.5 px-3 text-fg leading-relaxed">{inc.description}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>

      {/* Modal: Crear / Editar Lugar de Trabajo */}
      {(isCreatingWorkplace || editingWorkplace) && (
        <div className="fixed inset-0 z-50 bg-black/60 flex items-center justify-center p-4">
          <div className="bg-surface border border-default rounded-card max-w-lg w-full p-6 space-y-4 shadow-xl">
            <div className="flex items-center justify-between border-b border-default pb-3">
              <h3 className="text-sm font-semibold text-fg">
                {editingWorkplace ? 'Editar Lugar de Trabajo' : 'Nuevo Lugar de Trabajo'}
              </h3>
              <button
                type="button"
                onClick={() => {
                  setIsCreatingWorkplace(false);
                  setEditingWorkplace(null);
                }}
                className="text-muted hover:text-fg p-1 rounded-control cursor-pointer"
              >
                <X className="w-4 h-4" />
              </button>
            </div>

            {workplaceModalError && (
              <div className="p-2.5 bg-danger-subtle border border-default rounded-control text-xs text-danger flex items-center gap-2">
                <AlertTriangle className="w-4 h-4 shrink-0" />
                <span>{workplaceModalError}</span>
              </div>
            )}

            <form onSubmit={handleSaveWorkplace} className="space-y-3">
              <div>
                <label className="block text-xs font-medium text-fg mb-1">Nombre del lugar</label>
                <input
                  type="text"
                  value={workplaceForm.name}
                  onChange={e => setWorkplaceForm(prev => ({ ...prev, name: e.target.value }))}
                  placeholder="Ej. Casa Matriz o Sucursal Las Condes"
                  className="w-full bg-surface-subtle border border-default text-fg text-xs rounded-control px-3 py-2 focus:outline-hidden focus:border-accent"
                />
              </div>

              <div className="grid grid-cols-3 gap-2">
                <div className="col-span-2">
                  <label className="block text-xs font-medium text-fg mb-1">Calle</label>
                  <input
                    type="text"
                    value={workplaceForm.address.street}
                    onChange={e =>
                      setWorkplaceForm(prev => ({
                        ...prev,
                        address: { ...prev.address, street: e.target.value },
                      }))
                    }
                    placeholder="Av. Providencia"
                    className="w-full bg-surface-subtle border border-default text-fg text-xs rounded-control px-3 py-2 focus:outline-hidden focus:border-accent"
                  />
                </div>
                <div>
                  <label className="block text-xs font-medium text-fg mb-1">Número</label>
                  <input
                    type="text"
                    value={workplaceForm.address.number}
                    onChange={e =>
                      setWorkplaceForm(prev => ({
                        ...prev,
                        address: { ...prev.address, number: e.target.value },
                      }))
                    }
                    placeholder="1234"
                    className="w-full bg-surface-subtle border border-default text-fg text-xs rounded-control px-3 py-2 focus:outline-hidden focus:border-accent"
                  />
                </div>
              </div>

              <div className="grid grid-cols-2 gap-2">
                <div>
                  <label className="block text-xs font-medium text-fg mb-1">Piso (opcional)</label>
                  <input
                    type="text"
                    value={workplaceForm.address.floor || ''}
                    onChange={e =>
                      setWorkplaceForm(prev => ({
                        ...prev,
                        address: { ...prev.address, floor: e.target.value },
                      }))
                    }
                    placeholder="2"
                    className="w-full bg-surface-subtle border border-default text-fg text-xs rounded-control px-3 py-2 focus:outline-hidden focus:border-accent"
                  />
                </div>
                <div>
                  <label className="block text-xs font-medium text-fg mb-1">Oficina (opcional)</label>
                  <input
                    type="text"
                    value={workplaceForm.address.office || ''}
                    onChange={e =>
                      setWorkplaceForm(prev => ({
                        ...prev,
                        address: { ...prev.address, office: e.target.value },
                      }))
                    }
                    placeholder="201"
                    className="w-full bg-surface-subtle border border-default text-fg text-xs rounded-control px-3 py-2 focus:outline-hidden focus:border-accent"
                  />
                </div>
              </div>

              <div className="grid grid-cols-2 gap-2">
                <div>
                  <label className="block text-xs font-medium text-fg mb-1">Comuna</label>
                  <input
                    type="text"
                    value={workplaceForm.address.commune}
                    onChange={e =>
                      setWorkplaceForm(prev => ({
                        ...prev,
                        address: { ...prev.address, commune: e.target.value },
                      }))
                    }
                    placeholder="Providencia"
                    className="w-full bg-surface-subtle border border-default text-fg text-xs rounded-control px-3 py-2 focus:outline-hidden focus:border-accent"
                  />
                </div>
                <div>
                  <label className="block text-xs font-medium text-fg mb-1">Ciudad</label>
                  <input
                    type="text"
                    value={workplaceForm.address.city}
                    onChange={e =>
                      setWorkplaceForm(prev => ({
                        ...prev,
                        address: { ...prev.address, city: e.target.value },
                      }))
                    }
                    placeholder="Santiago"
                    className="w-full bg-surface-subtle border border-default text-fg text-xs rounded-control px-3 py-2 focus:outline-hidden focus:border-accent"
                  />
                </div>
              </div>

              <div>
                <label className="block text-xs font-medium text-fg mb-1">Región</label>
                <select
                  value={workplaceForm.address.region}
                  onChange={e =>
                    setWorkplaceForm(prev => ({
                      ...prev,
                      address: { ...prev.address, region: e.target.value },
                    }))
                  }
                  className="w-full bg-surface-subtle border border-default text-fg text-xs rounded-control px-3 py-2 focus:outline-hidden focus:border-accent"
                >
                  {CHILEAN_REGIONS.map(reg => (
                    <option key={reg} value={reg}>
                      {reg}
                    </option>
                  ))}
                </select>
              </div>

              <div className="flex justify-end gap-2 pt-3 border-t border-default">
                <button
                  type="button"
                  onClick={() => {
                    setIsCreatingWorkplace(false);
                    setEditingWorkplace(null);
                  }}
                  className="px-3 py-1.5 border border-default text-muted hover:text-fg text-xs rounded-control transition-colors cursor-pointer"
                >
                  Cancelar
                </button>
                <button
                  type="submit"
                  disabled={workplaceModalSaving}
                  className="px-4 py-1.5 bg-accent hover:bg-accent-hover text-accent-fg text-xs font-semibold rounded-control transition-colors flex items-center gap-1 cursor-pointer disabled:opacity-50"
                >
                  {workplaceModalSaving ? (
                    <div className="w-3.5 h-3.5 border-2 border-accent-fg border-t-transparent rounded-full animate-spin" />
                  ) : null}
                  <span>Guardar</span>
                </button>
              </div>
            </form>
          </div>
        </div>
      )}

      {/* Modal: Confirmar Eliminar Lugar de Trabajo */}
      {deletingWorkplace && (
        <div className="fixed inset-0 z-50 bg-black/60 flex items-center justify-center p-4">
          <div className="bg-surface border border-default rounded-card max-w-sm w-full p-5 space-y-4 shadow-xl">
            <div className="flex items-center gap-2.5 text-danger">
              <AlertTriangle className="w-5 h-5 shrink-0" />
              <h3 className="text-sm font-semibold">¿Eliminar lugar de trabajo?</h3>
            </div>
            <p className="text-xs text-muted leading-relaxed">
              ¿Estás seguro de que deseas eliminar el lugar de trabajo{' '}
              <strong className="text-fg">{deletingWorkplace.name}</strong>? Esta acción no se puede deshacer.
            </p>
            <div className="flex justify-end gap-2 pt-2 border-t border-default">
              <button
                type="button"
                onClick={() => setDeletingWorkplace(null)}
                className="px-3 py-1.5 border border-default text-muted hover:text-fg text-xs rounded-control transition-colors cursor-pointer"
              >
                Cancelar
              </button>
              <button
                type="button"
                onClick={handleDeleteWorkplace}
                className="px-3 py-1.5 bg-danger hover:bg-danger/90 text-white text-xs font-semibold rounded-control transition-colors cursor-pointer"
              >
                Eliminar
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Modal: Registrar Incidente Técnico */}
      {isRegisteringIncident && (
        <div className="fixed inset-0 z-50 bg-black/60 flex items-center justify-center p-4">
          <div className="bg-surface border border-default rounded-card max-w-md w-full p-6 space-y-4 shadow-xl">
            <div className="flex items-center justify-between border-b border-default pb-3">
              <h3 className="text-sm font-semibold text-fg">Registrar Incidente Técnico (R38)</h3>
              <button
                type="button"
                onClick={() => setIsRegisteringIncident(false)}
                className="text-muted hover:text-fg p-1 rounded-control cursor-pointer"
              >
                <X className="w-4 h-4" />
              </button>
            </div>

            {incidentModalError && (
              <div className="p-2.5 bg-danger-subtle border border-default rounded-control text-xs text-danger flex items-center gap-2">
                <AlertTriangle className="w-4 h-4 shrink-0" />
                <span>{incidentModalError}</span>
              </div>
            )}

            <form onSubmit={handleSaveIncident} className="space-y-3">
              <div>
                <label className="block text-xs font-medium text-fg mb-1">Fecha y hora de inicio</label>
                <input
                  type="datetime-local"
                  value={incidentForm.started_at}
                  onChange={e => setIncidentForm(prev => ({ ...prev, started_at: e.target.value }))}
                  className="w-full bg-surface-subtle border border-default text-fg text-xs rounded-control px-3 py-2 focus:outline-hidden focus:border-accent"
                />
              </div>

              <div>
                <label className="block text-xs font-medium text-fg mb-1">
                  Fecha y hora de término (opcional, en blanco si sigue en curso)
                </label>
                <input
                  type="datetime-local"
                  value={incidentForm.ended_at || ''}
                  onChange={e => setIncidentForm(prev => ({ ...prev, ended_at: e.target.value }))}
                  className="w-full bg-surface-subtle border border-default text-fg text-xs rounded-control px-3 py-2 focus:outline-hidden focus:border-accent"
                />
              </div>

              <div>
                <label className="block text-xs font-medium text-fg mb-1">Descripción del incidente</label>
                <textarea
                  rows={3}
                  value={incidentForm.description}
                  onChange={e => setIncidentForm(prev => ({ ...prev, description: e.target.value }))}
                  placeholder="Detalle de la falla técnica, corte de energía o conectividad..."
                  className="w-full bg-surface-subtle border border-default text-fg text-xs rounded-control px-3 py-2 focus:outline-hidden focus:border-accent"
                />
              </div>

              <div className="flex justify-end gap-2 pt-3 border-t border-default">
                <button
                  type="button"
                  onClick={() => setIsRegisteringIncident(false)}
                  className="px-3 py-1.5 border border-default text-muted hover:text-fg text-xs rounded-control transition-colors cursor-pointer"
                >
                  Cancelar
                </button>
                <button
                  type="submit"
                  disabled={incidentModalSaving}
                  className="px-4 py-1.5 bg-accent hover:bg-accent-hover text-accent-fg text-xs font-semibold rounded-control transition-colors flex items-center gap-1 cursor-pointer disabled:opacity-50"
                >
                  {incidentModalSaving ? (
                    <div className="w-3.5 h-3.5 border-2 border-accent-fg border-t-transparent rounded-full animate-spin" />
                  ) : null}
                  <span>Registrar</span>
                </button>
              </div>
            </form>
          </div>
        </div>
      )}
    </div>
  );
};
