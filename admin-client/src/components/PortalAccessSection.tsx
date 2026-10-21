import React, { useState } from 'react';
import { KeyRound } from 'lucide-react';
import {
  WorkerResponse,
  workersGet,
  workersGrantPortalAccess,
  workersRevokePortalAccess,
} from '../api/tauri-bridge';
import { describeError } from '../lib/errors';
import {
  PORTAL_ACCESS_DAYS,
  formatPortalAccessUntil,
  normalizePortalAccessEmail,
  portalAccessEmailError,
  portalAccessErrorMessage,
  portalAccessState,
} from '../lib/portalAccess';

interface PortalAccessSectionProps {
  worker: WorkerResponse;
  onUpdated: (worker: WorkerResponse, message: string) => void;
}

/** Ex-worker portal access on a REVOKED worker's detail (TASK-ADMIN-20, R38 art. 22.1). */
export const PortalAccessSection: React.FC<PortalAccessSectionProps> = ({ worker, onUpdated }) => {
  const [email, setEmail] = useState('');
  const [touched, setTouched] = useState(false);
  const [confirmRemove, setConfirmRemove] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const state = portalAccessState(worker.portal_access_until);
  const until = worker.portal_access_until
    ? formatPortalAccessUntil(worker.portal_access_until, worker.timezone)
    : null;
  const emailError = portalAccessEmailError(email);
  const tzLabel = worker.timezone || 'America/Santiago';

  const fail = (e: unknown) => setError(portalAccessErrorMessage(e) ?? describeError(e));

  const grant = async () => {
    setTouched(true);
    if (emailError) return;
    setBusy(true);
    setError(null);
    try {
      const updated = await workersGrantPortalAccess(worker.id, normalizePortalAccessEmail(email));
      setEmail('');
      setTouched(false);
      const shown = updated.portal_access_until
        ? formatPortalAccessUntil(updated.portal_access_until, updated.timezone)
        : null;
      onUpdated(updated, shown ? `Acceso al portal habilitado hasta el ${shown}.` : 'Acceso al portal habilitado.');
    } catch (e) {
      fail(e);
    } finally {
      setBusy(false);
    }
  };

  const remove = async () => {
    setBusy(true);
    setError(null);
    try {
      await workersRevokePortalAccess(worker.id);
      setConfirmRemove(false);
      let updated: WorkerResponse;
      try {
        updated = await workersGet(worker.id);
      } catch {
        updated = { ...worker, portal_access_until: null };
      }
      onUpdated(updated, 'Acceso al portal quitado.');
    } catch (e) {
      fail(e);
    } finally {
      setBusy(false);
    }
  };

  const btn =
    'px-3 py-1.5 rounded-control text-xs font-medium flex items-center gap-1.5 transition disabled:opacity-40';

  return (
    <section className="p-3 bg-surface border border-default rounded-control space-y-2 text-xs text-fg">
      <h4 className="text-xs font-semibold text-fg uppercase tracking-wide flex items-center gap-1.5">
        <KeyRound className="w-3.5 h-3.5" /> Acceso al portal (ex-trabajador)
      </h4>

      {state.enabled && until ? (
        <p>
          <span className="font-semibold text-success">Habilitado</span> hasta el{' '}
          <span className="font-mono">{until}</span> <span className="text-muted">({tzLabel})</span>.
        </p>
      ) : (
        <p>
          <span className="font-semibold text-muted">No habilitado</span>
          {until ? (
            <>
              {' '}
              · el último acceso venció el <span className="font-mono">{until}</span>
            </>
          ) : null}
          .
        </p>
      )}

      <p className="text-muted">
        La ley permite al ex-trabajador revisar sus registros de asistencia (R38 art. 22.1). Al habilitar
        un correo, podrá entrar al portal durante {PORTAL_ACCESS_DAYS} días y solo ver sus registros: no
        puede marcar ni cambiar nada. Si ya tenía un acceso, se reemplaza.
      </p>

      {error && (
        <div className="p-2 bg-danger-subtle border border-danger rounded-control text-danger">{error}</div>
      )}

      <div className="flex flex-wrap gap-2 items-start">
        <div className="flex-1 min-w-[220px]">
          <label htmlFor="portal-access-email" className="sr-only">
            Correo del ex-trabajador
          </label>
          <input
            id="portal-access-email"
            type="email"
            value={email}
            disabled={busy}
            placeholder="correo@ejemplo.cl"
            onChange={e => setEmail(e.target.value)}
            onBlur={() => setTouched(true)}
            className="w-full bg-surface-subtle border border-default text-fg text-xs rounded-control px-2.5 py-1.5 focus:outline-hidden focus:border-accent"
          />
          {touched && emailError && <p className="text-danger mt-1">{emailError}</p>}
        </div>
        <button
          disabled={busy || !!emailError}
          onClick={grant}
          className={`${btn} bg-accent hover:bg-accent-hover text-accent-fg font-semibold`}
        >
          {state.enabled ? 'Cambiar correo' : `Habilitar ${PORTAL_ACCESS_DAYS} días`}
        </button>
      </div>

      {state.enabled && !confirmRemove && (
        <button
          disabled={busy}
          onClick={() => setConfirmRemove(true)}
          className={`${btn} bg-danger-subtle text-danger border border-default`}
        >
          Quitar acceso
        </button>
      )}
      {state.enabled && confirmRemove && (
        <div className="p-2 bg-danger-subtle border border-danger rounded-control space-y-2">
          <p>¿Quitar el acceso al portal ahora? El ex-trabajador dejará de poder revisar sus registros hasta que le habilites un correo de nuevo.</p>
          <div className="flex gap-2">
            <button
              disabled={busy}
              onClick={remove}
              className={`${btn} bg-danger hover:bg-danger-hover text-accent-fg font-semibold`}
            >
              Confirmar
            </button>
            <button
              disabled={busy}
              onClick={() => setConfirmRemove(false)}
              className={`${btn} bg-surface text-fg border border-default`}
            >
              Cancelar
            </button>
          </div>
        </div>
      )}
    </section>
  );
};
