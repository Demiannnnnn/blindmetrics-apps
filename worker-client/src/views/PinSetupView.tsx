import React, { useState } from 'react';
import { KeyRound } from 'lucide-react';
import * as bridge from '../api/tauri-bridge';
import { useApp } from '../context/AppContext';
import { attendanceErrorMessage, newPinMessage } from '../lib/attendance';
import { NewPinFields } from '../components/NewPinFields';
import { BUTTON_PRIMARY, Notice } from '../components/ui';

/** P1 — Create PIN (SEC-19, SEC-20). Shown while `attendance_mode = NEEDS_PIN`; success → A. */
export const PinSetupView: React.FC = () => {
  const { snapshot, refreshSnapshot } = useApp();
  const [pin, setPin] = useState('');
  const [confirm, setConfirm] = useState('');
  const [showErrors, setShowErrors] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    setShowErrors(true);
    if (newPinMessage(pin, confirm)) return;
    setBusy(true);
    setError(null);
    const value = pin;
    // The PIN leaves React state as soon as it is sent (SEC-19).
    setPin('');
    setConfirm('');
    setShowErrors(false);
    try {
      await bridge.setPin(value);
      await refreshSnapshot();
    } catch (err) {
      setError(attendanceErrorMessage(bridge.toCommandError(err)));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="h-screen w-screen flex flex-col bg-bg text-fg">
      <header className="shrink-0 bg-ink text-ink-fg px-4 h-12 flex items-center gap-2">
        <span className="text-sm font-semibold truncate">{snapshot?.organization ?? 'Telemetry'}</span>
        {snapshot?.worker_name && <span className="text-xs text-ink-muted truncate">· {snapshot.worker_name}</span>}
      </header>
      <div className="flex-1 overflow-y-auto flex items-start justify-center p-6">
        <form onSubmit={submit} className="w-full max-w-sm bg-surface border border-default rounded-card p-6 space-y-4">
          <div className="flex items-center gap-2">
            <KeyRound className="w-5 h-5 text-accent" aria-hidden="true" />
            <h1 className="text-base font-semibold">Crea tu PIN de asistencia</h1>
          </div>
          <p className="text-xs text-muted leading-relaxed">
            Crea tu PIN de asistencia. Lo usarás para confirmar cada marca. Es personal: no lo compartas.
          </p>
          <NewPinFields
            idPrefix="p1"
            pin={pin}
            confirm={confirm}
            onPin={setPin}
            onConfirm={setConfirm}
            showErrors={showErrors}
            disabled={busy}
            autoFocus
          />
          {error && (
            <Notice tone="danger" role="alert">
              {error}
            </Notice>
          )}
          <button type="submit" className={`${BUTTON_PRIMARY} w-full`} disabled={busy}>
            {busy ? 'Guardando…' : 'Crear PIN'}
          </button>
        </form>
      </div>
    </div>
  );
};
