// The worker's own consent decisions, newest first (TASK-AGENT-17, spec §6 "Mis datos").
import React, { useState } from 'react';
import { ChevronDown, ChevronUp, History } from 'lucide-react';
import * as bridge from '../api/tauri-bridge';
import type { ConsentRecord } from '../api/types';
import { Notice, Skeleton } from './ui';
import { consentDecisionLabel, consentRecordSummary, consentRequestedSummary } from '../lib/helpers';
import { formatDateCl, formatTimeCl } from '../lib/attendance';

export const ConsentHistory: React.FC<{ timeZone?: string | null }> = ({ timeZone }) => {
  const [open, setOpen] = useState(false);
  const [loading, setLoading] = useState(false);
  const [records, setRecords] = useState<ConsentRecord[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = async () => {
    setLoading(true);
    setError(null);
    try {
      setRecords(await bridge.consentHistory());
    } catch (err) {
      const cmd = bridge.toCommandError(err);
      setError(cmd.code === 'OFFLINE' ? 'Tu historial se consulta con conexión.' : cmd.message);
    } finally {
      setLoading(false);
    }
  };

  const toggle = () => {
    const next = !open;
    setOpen(next);
    if (next && !records && !loading) void load();
  };

  return (
    <div className="space-y-3">
      <button
        type="button"
        onClick={toggle}
        aria-expanded={open}
        className="w-full flex items-center justify-between text-sm font-semibold text-fg"
      >
        <span className="flex items-center gap-1.5">
          <History className="w-4 h-4 text-muted" aria-hidden="true" />
          Historial de consentimiento
        </span>
        {open ? <ChevronUp className="w-4 h-4" aria-hidden="true" /> : <ChevronDown className="w-4 h-4" aria-hidden="true" />}
      </button>
      {open && (
        <div className="space-y-2">
          <p className="text-xs text-muted">
            Cada decisión sobre la telemetría queda registrada y no se puede modificar. Es tu evidencia.
          </p>
          {loading && <Skeleton className="h-16 w-full" />}
          {error && (
            <Notice tone="warning" role="status">
              {error}{' '}
              <button type="button" className="underline" onClick={() => void load()}>
                Reintentar
              </button>
            </Notice>
          )}
          {records && !loading && records.length === 0 && (
            <p className="text-xs text-muted">Aún no has tomado decisiones sobre la telemetría.</p>
          )}
          {records && !loading && records.length > 0 && (
            <ul className="divide-y divide-default text-xs">
              {records.map((r) => (
                <li key={r.id || `${r.created_at}-${r.decision}`} className="py-2 space-y-0.5">
                  <p className="text-fg">
                    <span className="font-medium">{consentDecisionLabel(r.decision)}</span>
                    <span className="text-muted tabular-nums">
                      {' '}
                      · {formatDateCl(r.created_at, timeZone)} {formatTimeCl(r.created_at, timeZone)} · versión {r.consent_version}
                    </span>
                  </p>
                  <p className="text-muted">{consentRequestedSummary(r.consent_flags)}</p>
                  <p className="text-fg">{consentRecordSummary(r)}</p>
                </li>
              ))}
            </ul>
          )}
        </div>
      )}
    </div>
  );
};
