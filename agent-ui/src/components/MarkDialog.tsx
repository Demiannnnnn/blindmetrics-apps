import React, { useEffect, useRef, useState } from 'react';
import { CheckCircle2, CloudOff } from 'lucide-react';
import * as bridge from '../api/tauri-bridge';
import type { MarkRecord, MarkType } from '../api/types';
import { Modal } from './Modal';
import { BUTTON_LINK, BUTTON_PRIMARY, BUTTON_SECONDARY, CopyButton, Notice, PinInput } from './ui';
import {
  HASH_MISMATCH_ADVICE,
  attendanceErrorMessage,
  effectiveTs,
  formatDateCl,
  formatTimeCl,
  groupHash,
  markStatusLabel,
  markTypeLabel,
  maskEmail,
  nextMarkLabel,
  remainingDelay,
} from '../lib/attendance';

type Phase = 'pin' | 'sending' | 'done';

/**
 * M — Mark dialog (ATT-01): the only way the window creates a mark. Needs the PIN typed here;
 * `Enter` confirms, `Esc` cancels, initial focus on the PIN. On success it turns into the receipt.
 */
export const MarkDialog: React.FC<{
  type: MarkType;
  onClose: () => void;
  onForgotPin: () => void;
  onMarked: () => void;
  timeZone?: string | null;
  personalEmail?: string | null;
}> = ({ type, onClose, onForgotPin, onMarked, timeZone, personalEmail }) => {
  const [phase, setPhase] = useState<Phase>('pin');
  const [pin, setPin] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [delay, setDelay] = useState<{ total: number; startedAt: number } | null>(null);
  const [now, setNow] = useState(() => Date.now());
  const [result, setResult] = useState<MarkRecord | null>(null);
  const mounted = useRef(true);

  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);

  // Countdown of the PIN delay returned by the service (RF-AGENT-12-6).
  useEffect(() => {
    if (!delay) return;
    const id = window.setInterval(() => setNow(Date.now()), 250);
    return () => window.clearInterval(id);
  }, [delay]);
  // Back to the PIN step (first render or after an error): focus the PIN field again.
  useEffect(() => {
    if (phase === 'pin') document.getElementById('mark-pin')?.focus();
  }, [phase]);

  const waitLeft = delay ? remainingDelay(delay.total, delay.startedAt, now) : 0;
  useEffect(() => {
    if (delay && waitLeft === 0) {
      setDelay(null);
      setError((prev) => (prev && prev.startsWith('PIN incorrecto') ? 'PIN incorrecto.' : prev));
    }
  }, [delay, waitLeft]);

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (phase !== 'pin' || waitLeft > 0 || pin.length === 0) return;
    const value = pin;
    setPin(''); // the PIN leaves React state as soon as it is sent (SEC-19)
    setPhase('sending');
    setError(null);
    try {
      const record = await bridge.markAttendance(type, value);
      if (!mounted.current) return;
      setResult(record);
      setPhase('done');
      onMarked();
    } catch (err) {
      if (!mounted.current) return;
      const cmd = bridge.toCommandError(err);
      setError(attendanceErrorMessage(cmd));
      if (cmd.retry_after_secs && cmd.retry_after_secs > 0) {
        setNow(Date.now());
        setDelay({ total: cmd.retry_after_secs, startedAt: Date.now() });
      }
      setPhase('pin');
    }
  };

  const liveError =
    error && waitLeft > 0 ? `PIN incorrecto. Espera ${waitLeft} s para reintentar.` : error;
  const title = phase === 'done' ? 'Comprobante de marca' : nextMarkLabel(type);

  return (
    <Modal title={title} isOpen onClose={onClose} size="md" closeOnEscape={phase !== 'sending'}>
      {phase !== 'done' || !result ? (
        <form onSubmit={submit} className="space-y-3">
          <p className="text-xs text-muted">Se registrará con la hora del servidor al recibirla.</p>
          <PinInput
            id="mark-pin"
            label="PIN de asistencia"
            value={pin}
            onChange={setPin}
            autoFocus
            disabled={phase === 'sending'}
            invalid={!!error}
            describedBy={liveError ? 'mark-pin-error' : undefined}
          />
          {liveError && (
            <p id="mark-pin-error" role="alert" className="text-xs text-danger tabular-nums">
              {liveError}
            </p>
          )}
          <div className="flex items-center gap-2 pt-1">
            <button
              type="submit"
              className={BUTTON_PRIMARY}
              disabled={phase === 'sending' || waitLeft > 0 || pin.length === 0}
            >
              {phase === 'sending' ? 'Registrando…' : waitLeft > 0 ? `Espera ${waitLeft} s` : 'Confirmar'}
            </button>
            <button type="button" className={BUTTON_SECONDARY} onClick={onClose} disabled={phase === 'sending'}>
              Cancelar
            </button>
            <button type="button" className={`${BUTTON_LINK} ml-auto`} onClick={onForgotPin} disabled={phase === 'sending'}>
              Olvidé mi PIN
            </button>
          </div>
        </form>
      ) : (
        <Receipt record={result} timeZone={timeZone} personalEmail={personalEmail} onClose={onClose} />
      )}
    </Modal>
  );
};

const Receipt: React.FC<{
  record: MarkRecord;
  timeZone?: string | null;
  personalEmail?: string | null;
  onClose: () => void;
}> = ({ record, timeZone, personalEmail, onClose }) => {
  const closeRef = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    closeRef.current?.focus();
  }, []);
  const m = record.server_mark;
  const masked = maskEmail(personalEmail);

  if (record.status === 'REJECTED') {
    return (
      <div className="space-y-3">
        <Notice tone="danger" role="alert">
          {markStatusLabel('REJECTED', record.rejection_reason).label}. La marca quedó guardada en este equipo
          como evidencia.
        </Notice>
        <button ref={closeRef} type="button" className={BUTTON_SECONDARY} onClick={onClose}>
          Cerrar
        </button>
      </div>
    );
  }

  if (!m) {
    // Stored offline: sent automatically later (ATT-04).
    return (
      <div className="space-y-3">
        <div className="flex items-start gap-2">
          <CloudOff className="w-5 h-5 text-warning shrink-0" aria-hidden="true" />
          <p className="text-sm text-fg">
            Sin conexión: la marca quedó guardada con la hora de tu equipo (
            <span className="tabular-nums">{formatTimeCl(record.device_ts, timeZone)}</span>) y se enviará
            automáticamente.
          </p>
        </div>
        <dl className="text-xs grid grid-cols-[auto_1fr] gap-x-3 gap-y-1">
          <dt className="text-muted">Tipo</dt>
          <dd>{markTypeLabel(record.type)}</dd>
          <dt className="text-muted">Fecha</dt>
          <dd className="tabular-nums">{formatDateCl(record.device_ts, timeZone)}</dd>
        </dl>
        <button ref={closeRef} type="button" className={BUTTON_SECONDARY} onClick={onClose}>
          Cerrar
        </button>
      </div>
    );
  }

  const ts = effectiveTs(m);
  return (
    <div className="space-y-3">
      <div className="flex items-center gap-2">
        <CheckCircle2 className="w-5 h-5 text-success shrink-0" aria-hidden="true" />
        <p className="text-sm font-medium">Marca registrada</p>
      </div>
      {record.status === 'HASH_MISMATCH' && (
        <Notice tone="danger" role="alert">
          Hash no coincide. {HASH_MISMATCH_ADVICE}
        </Notice>
      )}
      {record.duplicate_of && (
        <Notice tone="info">El servidor ya tenía registrada esta marca; este es su comprobante.</Notice>
      )}
      <dl className="text-xs grid grid-cols-[auto_1fr] gap-x-3 gap-y-1">
        <dt className="text-muted">Tipo</dt>
        <dd>{markTypeLabel(m.type)}</dd>
        <dt className="text-muted">Fecha</dt>
        <dd className="tabular-nums">{formatDateCl(ts, timeZone)}</dd>
        <dt className="text-muted">Hora</dt>
        <dd className="tabular-nums">{formatTimeCl(ts, timeZone)}</dd>
      </dl>
      <div>
        <p className="text-[11px] text-muted mb-1">Hash</p>
        <p className="font-mono text-xs bg-surface-subtle border border-default rounded-control p-2 break-all tabular-nums">
          {groupHash(m.hash)}
        </p>
      </div>
      <p className="text-xs text-muted">
        {masked ? `Te enviamos el comprobante a ${masked}` : 'Te enviamos el comprobante a tu correo personal.'}
      </p>
      <div className="flex gap-2">
        <CopyButton label="Copiar" text={m.hash} />
        <button ref={closeRef} type="button" className={`${BUTTON_SECONDARY} ml-auto`} onClick={onClose}>
          Cerrar
        </button>
      </div>
    </div>
  );
};
