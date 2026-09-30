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
    <div className="bg-slate-800/80 border border-slate-700/80 rounded-xl p-5 shadow-lg space-y-5">
      <div className="flex items-center justify-between">
        <h2 className="text-lg font-semibold text-white flex items-center gap-2">
          <Laptop className="w-5 h-5 text-indigo-400" /> Dispositivos autorizados y suscripción
        </h2>
        <button onClick={load} className="p-2 bg-slate-700 hover:bg-slate-600 rounded-lg" title="Actualizar">
          <RefreshCw className="w-4 h-4 text-slate-300" />
        </button>
      </div>
      {error && <p className="text-xs text-rose-300">{error}</p>}

      <section className="space-y-2">
        <p className="text-xs text-slate-400">
          Si pierdes un equipo, revócalo aquí: su sesión termina de inmediato aunque tenga credenciales vigentes.
        </p>
        {devices === null ? (
          <p className="text-xs text-slate-400">Cargando…</p>
        ) : devices.length === 0 ? (
          <p className="text-xs text-slate-400">Sin dispositivos registrados.</p>
        ) : (
          <table className="w-full text-xs">
            <thead className="text-slate-400">
              <tr className="text-left">
                <th className="py-1 font-medium">Equipo</th>
                <th className="py-1 font-medium">Llave</th>
                <th className="py-1 font-medium">Último uso</th>
                <th className="py-1 font-medium">Estado</th>
                <th className="py-1" />
              </tr>
            </thead>
            <tbody className="text-slate-300">
              {devices.map(d => (
                <tr key={d.id} className="border-t border-slate-700/60">
                  <td className="py-1.5 pr-2">
                    {editing === d.id ? (
                      <span className="flex items-center gap-1">
                        <input
                          value={draftName}
                          maxLength={255}
                          onChange={e => setDraftName(e.target.value)}
                          className="bg-slate-900 border border-slate-700 rounded px-2 py-1 text-xs text-white w-40"
                        />
                        <button onClick={() => rename(d)} className="p-1 hover:bg-slate-700 rounded" title="Guardar">
                          <Check className="w-3.5 h-3.5 text-emerald-400" />
                        </button>
                        <button onClick={() => setEditing(null)} className="p-1 hover:bg-slate-700 rounded" title="Cancelar">
                          <X className="w-3.5 h-3.5" />
                        </button>
                      </span>
                    ) : (
                      <span>
                        {d.device_name}
                        {d.is_current && <span className="ml-1.5 text-[10px] text-indigo-300">(este equipo)</span>}
                        <span className="block text-[10px] text-slate-500 font-mono">{d.device_fingerprint.slice(0, 13)}</span>
                      </span>
                    )}
                  </td>
                  <td className="py-1.5">
                    {!d.public_key_fingerprint ? (
                      <span className="text-slate-500">—</span>
                    ) : sameFingerprint(d.public_key_fingerprint, registeredFingerprint) ? (
                      <span className="text-emerald-400" title={d.public_key_fingerprint}>✅ {shortFingerprint(d.public_key_fingerprint).slice(0, 9)}</span>
                    ) : (
                      <span className="text-amber-300" title="Llave distinta a la registrada">⚠️ otra</span>
                    )}
                  </td>
                  <td className="py-1.5">{relative(d.last_seen_at)}</td>
                  <td className="py-1.5">
                    {d.is_active ? <span className="text-emerald-400">Activo</span> : <span className="text-rose-300">Revocado</span>}
                  </td>
                  <td className="py-1.5 text-right whitespace-nowrap">
                    {d.is_active && (
                      <>
                        <button
                          onClick={() => {
                            setEditing(d.id);
                            setDraftName(d.device_name);
                          }}
                          className="p-1.5 hover:bg-slate-700 rounded"
                          title="Renombrar"
                        >
                          <Pencil className="w-3.5 h-3.5" />
                        </button>
                        <button onClick={() => revoke(d)} className="p-1.5 hover:bg-rose-900/60 rounded text-rose-300" title="Revocar">
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

      <section className="pt-4 border-t border-slate-700/60 space-y-2">
        <h3 className="text-sm font-semibold text-white flex items-center gap-2">
          <CreditCard className="w-4 h-4 text-indigo-400" /> Suscripción
        </h3>
        {subscription ? (
          <div className="text-xs text-slate-300 space-y-1">
            <p>
              Estado: <span className="font-semibold text-white">{STATUS_LABELS[subscription.effective_status] ?? subscription.effective_status}</span>
              {subscription.effective_status === 'TRIAL' && subscription.trial_days_left !== null && (
                <span className="text-slate-400"> · {subscription.trial_days_left} día(s) restantes</span>
              )}
            </p>
            {subscription.expires_at && <p>Vence: {new Date(subscription.expires_at).toLocaleDateString()}</p>}
            {isSafePortalUrl(subscription.portal_url) ? (
              <button onClick={openPortal} className="mt-1 px-3 py-1.5 bg-indigo-600 hover:bg-indigo-500 text-white rounded-lg">
                Gestionar suscripción ({displayUrl(subscription.portal_url)})
              </button>
            ) : (
              <p className="text-slate-500">El servidor no informó el portal de facturación.</p>
            )}
          </div>
        ) : (
          <p className="text-xs text-slate-400">Cargando…</p>
        )}
      </section>
    </div>
  );
};
