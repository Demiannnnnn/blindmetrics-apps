import React, { useEffect, useState } from 'react';
import { KeyRound } from 'lucide-react';
import * as bridge from '../api/tauri-bridge';
import { attendanceErrorMessage, maskEmail, newPinMessage, remainingDelay } from '../lib/attendance';
import { NewPinFields } from './NewPinFields';
import { BUTTON_LINK, BUTTON_PRIMARY, BUTTON_SECONDARY, CARD, Notice, PinInput } from './ui';

type Flow = 'idle' | 'change' | 'reset-request' | 'reset-code' | 'reset-new';

/**
 * Ajustes → PIN (RF-AGENT-13-2): *Cambiar PIN* (current + new twice) and *Olvidé mi PIN*
 * (request code → enter code → new PIN twice). PIN values are cleared from state after every action.
 */
export const PinSettings: React.FC<{
  personalEmail: string | null | undefined;
  startReset: boolean;
  onResetStarted: () => void;
}> = ({ personalEmail, startReset, onResetStarted }) => {
  const [flow, setFlow] = useState<Flow>('idle');
  const [current, setCurrent] = useState('');
  const [pin, setPin] = useState('');
  const [confirm, setConfirm] = useState('');
  const [code, setCode] = useState('');
  const [showErrors, setShowErrors] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [done, setDone] = useState<string | null>(null);
  const [delay, setDelay] = useState<{ total: number; startedAt: number } | null>(null);
  const [now, setNow] = useState(() => Date.now());

  const masked = maskEmail(personalEmail);

  useEffect(() => {
    if (startReset) {
      setFlow('reset-request');
      onResetStarted();
    }
  }, [startReset, onResetStarted]);

  useEffect(() => {
    if (!delay) return;
    const id = window.setInterval(() => setNow(Date.now()), 250);
    return () => window.clearInterval(id);
  }, [delay]);
  const waitLeft = delay ? remainingDelay(delay.total, delay.startedAt, now) : 0;

  const clearSecrets = () => {
    setCurrent('');
    setPin('');
    setConfirm('');
    setCode('');
    setShowErrors(false);
  };

  const start = (next: Flow) => {
    clearSecrets();
    setError(null);
    setDone(null);
    setFlow(next);
  };

  const fail = (err: unknown) => {
    const cmd = bridge.toCommandError(err);
    setError(attendanceErrorMessage(cmd));
    if (cmd.retry_after_secs && cmd.retry_after_secs > 0) {
      setNow(Date.now());
      setDelay({ total: cmd.retry_after_secs, startedAt: Date.now() });
    }
  };

  const submitChange = async (e: React.FormEvent) => {
    e.preventDefault();
    setShowErrors(true);
    if (current.length === 0 || newPinMessage(pin, confirm)) return;
    const [c, n] = [current, pin];
    clearSecrets();
    setBusy(true);
    setError(null);
    try {
      await bridge.changePin(c, n);
      setFlow('idle');
      setDone('Tu PIN fue cambiado.');
    } catch (err) {
      fail(err);
    } finally {
      setBusy(false);
    }
  };

  const requestCode = async () => {
    setBusy(true);
    setError(null);
    try {
      await bridge.requestPinReset();
      setFlow('reset-code');
    } catch (err) {
      fail(err);
    } finally {
      setBusy(false);
    }
  };

  const submitCode = async (e: React.FormEvent) => {
    e.preventDefault();
    if (code.trim().length === 0) return;
    const value = code.trim();
    setCode('');
    setBusy(true);
    setError(null);
    try {
      await bridge.verifyPinReset(value);
      setFlow('reset-new');
    } catch (err) {
      fail(err);
    } finally {
      setBusy(false);
    }
  };

  const submitNew = async (e: React.FormEvent) => {
    e.preventDefault();
    setShowErrors(true);
    if (newPinMessage(pin, confirm)) return;
    const value = pin;
    clearSecrets();
    setBusy(true);
    setError(null);
    try {
      await bridge.setPin(value);
      setFlow('idle');
      setDone('Tu nuevo PIN quedó guardado.');
    } catch (err) {
      fail(err);
    } finally {
      setBusy(false);
    }
  };

  const errorBox = error && (
    <Notice tone="danger" role="alert">
      {waitLeft > 0 && error.startsWith('PIN incorrecto') ? `PIN incorrecto. Espera ${waitLeft} s para reintentar.` : error}
    </Notice>
  );

  const cancel = (
    <button type="button" className={BUTTON_SECONDARY} disabled={busy} onClick={() => start('idle')}>
      Cancelar
    </button>
  );

  return (
    <section className={CARD} aria-label="PIN de asistencia">
      <div className="flex items-center gap-1.5 mb-2">
        <KeyRound className="w-4 h-4 text-muted" aria-hidden="true" />
        <h2 className="text-xs font-semibold text-fg">PIN de asistencia</h2>
      </div>

      {flow === 'idle' && (
        <div className="space-y-2">
          {done && (
            <Notice tone="success" role="status">
              {done}
            </Notice>
          )}
          {errorBox}
          <p className="text-xs text-muted">Tu PIN confirma cada marca. Es personal: no lo compartas.</p>
          <div className="flex flex-wrap items-center gap-3">
            <button type="button" className={BUTTON_SECONDARY} onClick={() => start('change')}>
              Cambiar PIN
            </button>
            <button type="button" className={BUTTON_LINK} onClick={() => start('reset-request')}>
              Olvidé mi PIN
            </button>
          </div>
        </div>
      )}

      {flow === 'change' && (
        <form onSubmit={submitChange} className="space-y-3">
          <PinInput id="pin-current" label="PIN actual" value={current} onChange={setCurrent} autoFocus disabled={busy} />
          <NewPinFields
            idPrefix="pin-change"
            pin={pin}
            confirm={confirm}
            onPin={setPin}
            onConfirm={setConfirm}
            showErrors={showErrors}
            disabled={busy}
          />
          {errorBox}
          <div className="flex gap-2">
            <button type="submit" className={BUTTON_PRIMARY} disabled={busy || waitLeft > 0}>
              {busy ? 'Guardando…' : waitLeft > 0 ? `Espera ${waitLeft} s` : 'Cambiar PIN'}
            </button>
            {cancel}
          </div>
        </form>
      )}

      {flow === 'reset-request' && (
        <div className="space-y-3">
          <p className="text-xs text-fg">
            {masked ? `Te enviaremos un código a ${masked}` : 'Te enviaremos un código a tu correo personal.'}
          </p>
          {errorBox}
          <div className="flex gap-2">
            <button type="button" className={BUTTON_PRIMARY} disabled={busy} onClick={requestCode}>
              {busy ? 'Enviando…' : 'Enviar código'}
            </button>
            {cancel}
          </div>
        </div>
      )}

      {flow === 'reset-code' && (
        <form onSubmit={submitCode} className="space-y-3">
          <p className="text-xs text-muted">Revisa tu correo. El código vence en 10 minutos.</p>
          <div>
            <label htmlFor="pin-reset-code" className="block text-xs font-medium text-fg mb-1">
              Código recibido
            </label>
            <input
              id="pin-reset-code"
              type="text"
              inputMode="numeric"
              autoComplete="one-time-code"
              spellCheck={false}
              value={code}
              autoFocus
              disabled={busy}
              onChange={(e) => setCode(e.target.value.replace(/\s/g, '').slice(0, 12))}
              className="w-40 h-9 px-3 rounded-control border border-strong bg-surface text-fg font-mono tracking-widest text-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent"
            />
          </div>
          {errorBox}
          <div className="flex flex-wrap items-center gap-2">
            <button type="submit" className={BUTTON_PRIMARY} disabled={busy || waitLeft > 0 || code.trim() === ''}>
              {busy ? 'Verificando…' : waitLeft > 0 ? `Espera ${waitLeft} s` : 'Verificar código'}
            </button>
            {cancel}
            <button type="button" className={`${BUTTON_LINK} ml-auto`} disabled={busy} onClick={requestCode}>
              Enviar otro código
            </button>
          </div>
        </form>
      )}

      {flow === 'reset-new' && (
        <form onSubmit={submitNew} className="space-y-3">
          <p className="text-xs text-muted">Código verificado. Crea tu nuevo PIN en los próximos 10 minutos.</p>
          <NewPinFields
            idPrefix="pin-reset"
            pin={pin}
            confirm={confirm}
            onPin={setPin}
            onConfirm={setConfirm}
            showErrors={showErrors}
            disabled={busy}
            autoFocus
          />
          {errorBox}
          <div className="flex gap-2">
            <button type="submit" className={BUTTON_PRIMARY} disabled={busy}>
              {busy ? 'Guardando…' : 'Guardar PIN'}
            </button>
            {cancel}
          </div>
        </form>
      )}
    </section>
  );
};
