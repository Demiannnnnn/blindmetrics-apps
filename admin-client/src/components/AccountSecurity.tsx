import React, { useEffect, useState } from 'react';
import { Laptop, Pencil, Ban, CreditCard, RefreshCw, Check, X } from 'lucide-react';
import type { AdminDevice, SubscriptionStatus } from '../api/types';
import { devicesList, devicesRename, devicesRevoke, openBillingPortal, subscriptionStatus } from '../api/tauri-bridge';
import { describeError } from '../lib/errors';
import { displayUrl, isSafePortalUrl } from '../lib/subscription';
import { sameFingerprint, shortFingerprint } from '../lib/vault';

function relative(iso: string | null): string {
  if (!iso) return 'nunca';
  const minutes = Math.round((Date.now() - new Date(iso).getTime()) / 60000);
  if (minutes < 2) return 'ahora';
  if (minutes < 60) return `hace ${minutes} min`;
  const hours = Math.round(minutes / 60);
  if (hours < 48) return `hace ${hours} h`;
  return `hace ${Math.round(hours / 24)} días`;
}

const STATUS_LABELS: Record<string, string> = {
  TRIAL: 'Período de prueba',
  ACTIVE: 'Activa',
  SUSPENDED: 'Suspendida',
  CANCELLED: 'Cancelada',
};

/** Authorized devices (TASK-ADMIN-10) and subscription summary (TASK-ADMIN-12). */
export const AccountSecurity: React.FC<{
  registeredFingerprint: string | null | undefined;
  onCurrentDeviceRevoked: () => void;
}> = ({ registeredFingerprint, onCurrentDeviceRevoked }) => {
  const [devices, setDevices] = useState<AdminDevice[] | null>(null);
  const [subscription, setSubscription] = useState<SubscriptionStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [editing, setEditing] = useState<string | null>(null);
  const [draftName, setDraftName] = useState('');

  const load = async () => {
    setError(null);
    try {
      const [list, sub] = await Promise.all([devicesList(), subscriptionStatus()]);
      setDevices(list);
      setSubscription(sub);
    } catch (err) {
      setError(describeError(err));
    }
  };

  useEffect(() => {
    load();
  }, []);

  const rename = async (device: AdminDevice) => {
    try {
      await devicesRename(device.id, draftName);
      setEditing(null);
      load();
    } catch (err) {
      setError(describeError(err));
    }
  };

  const revoke = async (device: AdminDevice) => {
    const question = device.is_current
      ? 'Vas a revocar ESTE equipo: tu sesión terminará de inmediato. ¿Continuar?'
      : `¿Revocar "${device.device_name}"? Perderá la sesión de inmediato.`;
    if (!confirm(question)) return;
    try {
      const wasCurrent = await devicesRevoke(device.id);
      if (wasCurrent) {
        onCurrentDeviceRevoked();
        return;
      }
      load();
    } catch (err) {
      setError(describeError(err));
    }
  };

  const openPortal = async () => {
    try {
      await openBillingPortal();
    } catch (err) {
      setError(describeError(err));
    }
  };

  return (
    <div className="bg-surface-subtle border border-default rounded-card p-5 space-y-5">
      <div className="flex items-center justify-between">
        <h2 className="text-lg font-semibold text-fg flex items-center gap-2">
          <Laptop className="w-5 h-5 text-accent" /> Dispositivos autorizados y suscripción
        </h2>
        <button onClick={load} className="p-2 bg-surface-subtle hover:bg-strong rounded-control" title="Actualizar">
          <RefreshCw className="w-4 h-4 text-fg" />
        </button>
      </div>
      {error && <p className="text-xs text-danger">{error}</p>}

      <section className="space-y-2">
        <p className="text-xs text-muted">
          Si pierdes un equipo, revócalo aquí: su sesión termina de inmediato aunque tenga credenciales vigentes.
        </p>
        {devices === null ? (
          <p className="text-xs text-muted">Cargando…</p>
        ) : devices.length === 0 ? (
          <p className="text-xs text-muted">Sin dispositivos registrados.</p>
        ) : (
          <table className="w-full text-xs">
            <thead className="text-muted">
              <tr className="text-left">
                <th className="py-1 font-medium">Equipo</th>
                <th className="py-1 font-medium">Llave</th>
                <th className="py-1 font-medium">Último uso</th>
                <th className="py-1 font-medium">Estado</th>
                <th className="py-1" />
              </tr>
            </thead>
            <tbody className="text-fg">
              {devices.map(d => (
                <tr key={d.id} className="border-t border-default">
                  <td className="py-1.5 pr-2">
                    {editing === d.id ? (
                      <span className="flex items-center gap-1">
                        <input
                          value={draftName}
                          maxLength={255}
                          onChange={e => setDraftName(e.target.value)}
                          className="bg-surface border border-default rounded px-2 py-1 text-xs text-fg w-40"
                        />
                        <button onClick={() => rename(d)} className="p-1 hover:bg-surface-subtle rounded" title="Guardar">
                          <Check className="w-3.5 h-3.5 text-success" />
                        </button>
                        <button onClick={() => setEditing(null)} className="p-1 hover:bg-surface-subtle rounded" title="Cancelar">
                          <X className="w-3.5 h-3.5" />
                        </button>
                      </span>
                    ) : (
                      <span>
                        {d.device_name}
                        {d.is_current && <span className="ml-1.5 text-[10px] text-accent">(este equipo)</span>}
                        <span className="block text-[10px] text-muted font-mono">{d.device_fingerprint.slice(0, 13)}</span>
                      </span>
                    )}
                  </td>
                  <td className="py-1.5">
                    {!d.public_key_fingerprint ? (
                      <span className="text-muted">—</span>
                    ) : sameFingerprint(d.public_key_fingerprint, registeredFingerprint) ? (
                      <span className="text-success" title={d.public_key_fingerprint}>✅ {shortFingerprint(d.public_key_fingerprint).slice(0, 9)}</span>
                    ) : (
                      <span className="text-warning" title="Llave distinta a la registrada">⚠️ otra</span>
                    )}
                  </td>
                  <td className="py-1.5">{relative(d.last_seen_at)}</td>
                  <td className="py-1.5">
                    {d.is_active ? <span className="text-success">Activo</span> : <span className="text-danger">Revocado</span>}
                  </td>
                  <td className="py-1.5 text-right whitespace-nowrap">
                    {d.is_active && (
                      <>
                        <button
                          onClick={() => {
                            setEditing(d.id);
                            setDraftName(d.device_name);
                          }}
                          className="p-1.5 hover:bg-surface-subtle rounded"
                          title="Renombrar"
                        >
                          <Pencil className="w-3.5 h-3.5" />
                        </button>
                        <button onClick={() => revoke(d)} className="p-1.5 hover:bg-danger-subtle rounded text-danger" title="Revocar">
                          <Ban className="w-3.5 h-3.5" />
                        </button>
                      </>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        )}
      </section>

      <section className="pt-4 border-t border-default space-y-2">
        <h3 className="text-sm font-semibold text-fg flex items-center gap-2">
          <CreditCard className="w-4 h-4 text-accent" /> Suscripción
        </h3>
        {subscription ? (
          <div className="text-xs text-fg space-y-1">
            <p>
              Estado: <span className="font-semibold text-fg">{STATUS_LABELS[subscription.effective_status] ?? subscription.effective_status}</span>
              {subscription.effective_status === 'TRIAL' && subscription.trial_days_left !== null && (
                <span className="text-muted"> · {subscription.trial_days_left} día(s) restantes</span>
              )}
            </p>
            {subscription.expires_at && <p>Vence: {new Date(subscription.expires_at).toLocaleDateString()}</p>}
            {isSafePortalUrl(subscription.portal_url) ? (
              <button onClick={openPortal} className="mt-1 px-3 py-1.5 bg-accent hover:bg-accent-hover text-accent-fg rounded-control">
                Gestionar suscripción ({displayUrl(subscription.portal_url)})
              </button>
            ) : (
              <p className="text-muted">El servidor no informó el portal de facturación.</p>
            )}
          </div>
        ) : (
          <p className="text-xs text-muted">Cargando…</p>
        )}
      </section>
    </div>
  );
};
