import React from 'react';
import { Modal } from './Modal';
import { CopyButton, Notice, StatusBadge, BUTTON_SECONDARY } from './ui';
import {
  HASH_MISMATCH_ADVICE,
  formatDateCl,
  formatTimeCl,
  groupHash,
  markSourceLabel,
  markStatusLabel,
  markTypeLabel,
  receiptText,
  type MarkRow,
} from '../lib/attendance';

const Field: React.FC<{ label: string; children: React.ReactNode; mono?: boolean }> = ({ label, children, mono }) => (
  <div className="py-1.5 border-b border-default last:border-b-0">
    <dt className="text-[11px] text-muted">{label}</dt>
    <dd className={`text-xs text-fg ${mono ? 'font-mono break-all tabular-nums' : ''}`}>{children}</dd>
  </div>
);

/** Every field of a mark's receipt (spec §4 "Mark detail"), with *Copiar comprobante*. */
export const MarkDetail: React.FC<{
  row: MarkRow | null;
  onClose: () => void;
  timeZone?: string | null;
  organization?: string | null;
  worker?: string | null;
  source?: 'SERVER' | 'LOCAL';
}> = ({ row, onClose, timeZone, organization, worker, source }) => {
  if (!row) return null;
  const status = markStatusLabel(row.status, row.rejectionReason);
  const m = row.server;
  return (
    <Modal title={`Marca: ${markTypeLabel(row.type)}`} isOpen onClose={onClose} size="md">
      <div className="space-y-3">
        <div className="flex flex-wrap items-center gap-1.5">
          <StatusBadge tone={status.tone}>{status.label}</StatusBadge>
          {row.corrected && <StatusBadge tone="info">Corregida</StatusBadge>}
          {row.superseded && <StatusBadge tone="muted">Reemplazada por una corrección</StatusBadge>}
          {source === 'LOCAL' && <StatusBadge tone="muted">Datos locales (sin conexión)</StatusBadge>}
        </div>
        {row.status === 'HASH_MISMATCH' && <Notice tone="danger">{HASH_MISMATCH_ADVICE}</Notice>}
        <dl>
          <Field label="Tipo">{markTypeLabel(row.type)}</Field>
          <Field label="Fecha">
            <span className="tabular-nums">{formatDateCl(row.ts, timeZone)}</span>
          </Field>
          <Field label="Hora">
            <span className="tabular-nums">{formatTimeCl(row.ts, timeZone)}</span>
          </Field>
          {m ? (
            <>
              <Field label="Registrada por el servidor">
                <span className="tabular-nums">
                  {formatDateCl(m.server_ts, timeZone)} {formatTimeCl(m.server_ts, timeZone)}
                </span>
              </Field>
              <Field label="Hora del equipo">
                <span className="tabular-nums">
                  {formatDateCl(m.device_ts, timeZone)} {formatTimeCl(m.device_ts, timeZone)}
                </span>
              </Field>
              <Field label="Sin conexión">{m.offline ? 'Sí, se envió después' : 'No'}</Field>
              <Field label="Origen">{markSourceLabel(m.source)}</Field>
              {m.identification && (
                <Field label="Identificación">
                  {m.identification === 'DEVICE_PIN' ? 'Equipo vinculado y PIN' : m.identification}
                </Field>
              )}
              <Field label="Lugar de trabajo">
                {m.workplace?.name
                  ? `${m.workplace.name}${m.workplace.address ? ` — ${m.workplace.address}` : ''}`
                  : 'No informado'}
              </Field>
              <Field label="Ubicación">
                {m.location
                  ? `${m.location.lat}, ${m.location.lon} (±${m.location.accuracy_m} m)`
                  : 'No se registró'}
              </Field>
              <Field label="Hash" mono>
                {groupHash(m.hash)}
              </Field>
              <Field label="Hash anterior" mono>
                {groupHash(m.prev_hash)}
              </Field>
            </>
          ) : (
            <Field label="Estado del envío">
              Guardada en este equipo con la hora del equipo. Aún no tiene datos del servidor.
            </Field>
          )}
        </dl>
        <div className="flex flex-wrap gap-2 pt-1">
          <CopyButton
            label="Copiar comprobante"
            text={receiptText(row, timeZone, { organization, worker })}
          />
          <button type="button" className={`${BUTTON_SECONDARY} ml-auto`} onClick={onClose}>
            Cerrar
          </button>
        </div>
      </div>
    </Modal>
  );
};
