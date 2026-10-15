import React, { useState } from 'react';
import {
  X,
  Copy,
  Check,
  ShieldCheck,
  ShieldAlert,
  MapPin,
  Clock,
  Building2,
  FileEdit,
} from 'lucide-react';
import { AttendanceMark } from '../api/types';
import {
  formatR38DateTime,
  formatHashBlocks,
  MARK_TYPE_LABELS,
  MARK_SOURCE_LABELS,
  effectiveTs,
  deviceTimeDiffers,
} from '../lib/attendance';

interface MarkDetailDrawerProps {
  mark: AttendanceMark | null;
  onClose: () => void;
  onProposeCorrection?: (mark: AttendanceMark) => void;
  /** Worker's IANA time zone for display (defaults to America/Santiago). */
  timezone?: string;
}

export const MarkDetailDrawer: React.FC<MarkDetailDrawerProps> = ({
  mark,
  onClose,
  onProposeCorrection,
  timezone,
}) => {
  const [copiedHash, setCopiedHash] = useState(false);
  const [copiedPrevHash, setCopiedPrevHash] = useState(false);

  if (!mark) return null;

  const copyToClipboard = async (text: string, isPrev = false) => {
    try {
      await navigator.clipboard.writeText(text);
      if (isPrev) {
        setCopiedPrevHash(true);
        setTimeout(() => setCopiedPrevHash(false), 2000);
      } else {
        setCopiedHash(true);
        setTimeout(() => setCopiedHash(false), 2000);
      }
    } catch {
      // ignore
    }
  };

  const tz = timezone || 'America/Santiago';
  // v3.1 (RF-ADMIN-17-1): the time that counts; the hash keeps using the canonical fields.
  const effective = effectiveTs(mark);
  const showDeviceTime = deviceTimeDiffers(mark);
  const isHashValid = mark.hash_verified === true;
  const isHashInvalid = mark.hash_verified === false;

  return (
    <div className="fixed inset-0 z-50 bg-black/60 flex justify-end">
      <div className="bg-surface border-l border-default w-full max-w-lg h-full overflow-y-auto flex flex-col justify-between shadow-xl">
        <div className="p-6 space-y-6">
          {/* Header */}
          <div className="flex items-start justify-between border-b border-default pb-4">
            <div>
              <div className="flex items-center gap-2">
                <Clock className="w-5 h-5 text-accent" />
                <h3 className="text-base font-bold text-fg">Detalle de Marcación</h3>
              </div>
              <p className="text-xs text-muted mt-1 font-mono">
                ID: {mark.id}
              </p>
            </div>
            <button
              onClick={onClose}
              className="p-1 rounded-control text-muted hover:text-fg hover:bg-surface-subtle transition"
              title="Cerrar detalle"
            >
              <X className="w-5 h-5" />
            </button>
          </div>

          {/* Type & Official Timestamps */}
          <div className="bg-surface-subtle border border-default rounded-card p-4 space-y-3">
            <div className="flex justify-between items-center">
              <span className="text-xs text-muted">Tipo de marca:</span>
              <span className="text-xs font-semibold text-fg bg-surface px-2.5 py-1 rounded-badge border border-default">
                {MARK_TYPE_LABELS[mark.type] || mark.type}
              </span>
            </div>

            <div className="flex justify-between items-center">
              <span className="text-xs text-muted">Hora de la marca:</span>
              <div className="text-right">
                <span className="text-xs font-mono font-bold text-fg">
                  {formatR38DateTime(effective, tz)}
                </span>
                {(mark.offline || mark.source === 'CORRECTION') && (
                  <span className="block text-[10px] text-muted">
                    {mark.source === 'CORRECTION'
                      ? 'Hora fijada por la corrección'
                      : 'Hora del equipo al marcar sin conexión'}
                  </span>
                )}
              </div>
            </div>

            <div className="flex justify-between items-center">
              <span className="text-xs text-muted">Registrada por el servidor:</span>
              <div className="text-right">
                <span className="text-xs font-mono text-fg">
                  {formatR38DateTime(mark.server_ts, tz)}
                </span>
                <span className="block text-[10px] text-muted font-mono">{mark.server_ts}</span>
              </div>
            </div>

            {showDeviceTime && (
              <div className="flex justify-between items-center">
                <span className="text-xs text-muted">Hora del equipo:</span>
                <div className="text-right">
                  <span className="text-xs font-mono text-fg">
                    {formatR38DateTime(mark.device_ts, tz)}
                  </span>
                  <span className="block text-[10px] text-muted font-mono">{mark.device_ts}</span>
                </div>
              </div>
            )}

            <div className="flex justify-between items-center">
              <span className="text-xs text-muted">Transmisión:</span>
              <span
                className={`text-xs font-semibold px-2 py-0.5 rounded-badge border ${
                  mark.offline
                    ? 'bg-warning-subtle text-warning border-warning/30'
                    : 'bg-success-subtle text-success border-success/30'
                }`}
              >
                {mark.offline ? 'Sin conexión (offline)' : 'En línea'}
              </span>
            </div>

            <div className="flex justify-between items-center">
              <span className="text-xs text-muted">Origen:</span>
              <span className="text-xs text-fg font-medium">
                {MARK_SOURCE_LABELS[mark.source] || mark.source}
              </span>
            </div>

            <div className="flex justify-between items-center">
              <span className="text-xs text-muted">Identificación:</span>
              <span className="text-xs text-fg font-mono">
                {mark.identification || 'PIN en equipo vinculado'}
              </span>
            </div>
          </div>

          {/* Workplace & Location */}
          <div className="bg-surface-subtle border border-default rounded-card p-4 space-y-3">
            <h4 className="text-xs font-bold text-fg uppercase tracking-wider flex items-center gap-1.5">
              <Building2 className="w-3.5 h-3.5 text-muted" /> Lugar y Ubicación
            </h4>

            <div className="text-xs space-y-1">
              <p className="text-muted">Establecimiento:</p>
              <p className="font-semibold text-fg">
                {mark.workplace?.name || 'Lugar asignado'}
              </p>
              {mark.workplace?.address && (
                <p className="text-muted text-[11px]">{mark.workplace.address}</p>
              )}
            </div>

            {mark.location ? (
              <div className="p-2.5 bg-surface rounded-control border border-default text-xs space-y-1">
                <span className="text-muted flex items-center gap-1 font-medium">
                  <MapPin className="w-3.5 h-3.5 text-accent" /> Coordenadas al marcar:
                </span>
                <p className="font-mono text-fg">
                  Lat: {mark.location.lat.toFixed(6)}, Lon: {mark.location.lon.toFixed(6)}
                </p>
                <p className="text-[11px] text-muted">
                  Precisión: ±{mark.location.accuracy_m.toFixed(1)} m
                </p>
              </div>
            ) : (
              <p className="text-[11px] text-muted italic">
                Sin lectura de ubicación geográfica al momento de la marcación.
              </p>
            )}
          </div>

          {/* Cryptographic Chain Integrity (R38 art. 8, ATT-03) */}
          <div className="bg-surface-subtle border border-default rounded-card p-4 space-y-3">
            <div className="flex items-center justify-between">
              <h4 className="text-xs font-bold text-fg uppercase tracking-wider flex items-center gap-1.5">
                Integridad Criptográfica (SHA-256)
              </h4>
              {isHashValid && (
                <span className="flex items-center gap-1 text-[11px] font-semibold text-success bg-success-subtle px-2 py-0.5 rounded-badge border border-success/30">
                  <ShieldCheck className="w-3.5 h-3.5" /> Hash verificado
                </span>
              )}
              {isHashInvalid && (
                <span className="flex items-center gap-1 text-[11px] font-semibold text-danger bg-danger-subtle px-2 py-0.5 rounded-badge border border-danger/30">
                  <ShieldAlert className="w-3.5 h-3.5" /> Integridad: no coincide
                </span>
              )}
            </div>

            {/* Current mark hash */}
            <div className="space-y-1">
              <div className="flex items-center justify-between text-xs">
                <span className="text-muted">Hash de esta marca:</span>
                <button
                  onClick={() => copyToClipboard(mark.hash)}
                  className="text-xs text-accent hover:underline flex items-center gap-1"
                >
                  {copiedHash ? (
                    <>
                      <Check className="w-3 h-3 text-success" /> Copiado
                    </>
                  ) : (
                    <>
                      <Copy className="w-3 h-3" /> Copiar
                    </>
                  )}
                </button>
              </div>
              <div className="p-2.5 bg-surface border border-default rounded-control font-mono text-[11px] text-fg break-all select-all">
                {formatHashBlocks(mark.hash)}
              </div>
            </div>

            {/* Previous mark hash */}
            <div className="space-y-1">
              <div className="flex items-center justify-between text-xs">
                <span className="text-muted">Hash anterior (encadenado):</span>
                <button
                  onClick={() => copyToClipboard(mark.prev_hash, true)}
                  className="text-xs text-accent hover:underline flex items-center gap-1"
                >
                  {copiedPrevHash ? (
                    <>
                      <Check className="w-3 h-3 text-success" /> Copiado
                    </>
                  ) : (
                    <>
                      <Copy className="w-3 h-3" /> Copiar
                    </>
                  )}
                </button>
              </div>
              <div className="p-2.5 bg-surface border border-default rounded-control font-mono text-[11px] text-muted break-all select-all">
                {formatHashBlocks(mark.prev_hash)}
              </div>
            </div>
          </div>

          {/* Correction metadata if present */}
          {(mark.corrected_by || mark.superseded_by) && (
            <div className="bg-warning-subtle/40 border border-warning/40 rounded-card p-3 text-xs space-y-1 text-fg">
              <p className="font-semibold text-warning">Registro alterado por procedimiento legal</p>
              {mark.corrected_by && (
                <p className="text-[11px] text-muted">
                  Esta marca fue producida por la corrección: {mark.corrected_by}
                </p>
              )}
              {mark.superseded_by && (
                <p className="text-[11px] text-muted">
                  Esta marca original fue reemplazada por la consolidación: {mark.superseded_by}
                </p>
              )}
            </div>
          )}
        </div>

        {/* Footer Actions */}
        <div className="p-4 border-t border-default bg-surface flex items-center justify-between gap-3">
          <button
            onClick={onClose}
            className="px-4 py-2 bg-surface-subtle hover:bg-surface-subtle/80 text-fg text-xs font-medium rounded-control border border-default transition"
          >
            Cerrar
          </button>

          {onProposeCorrection && (
            <button
              onClick={() => {
                onClose();
                onProposeCorrection(mark);
              }}
              className="px-4 py-2 bg-accent hover:bg-accent-hover text-accent-fg text-xs font-semibold rounded-control flex items-center gap-1.5 transition"
            >
              <FileEdit className="w-3.5 h-3.5" />
              Proponer corrección sobre esta marca
            </button>
          )}
        </div>
      </div>
    </div>
  );
};
