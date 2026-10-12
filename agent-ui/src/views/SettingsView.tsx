import React, { useState, useEffect } from 'react';
import { Settings, RefreshCw, Copy, Check, AlertTriangle } from 'lucide-react';
import * as bridge from '../api/tauri-bridge';
import type { DiagnosticsInfo } from '../api/types';
import { formatBytes, formatUptime } from '../lib/helpers';
import { useApp } from '../context/AppContext';
import { Modal } from '../components/Modal';

export const SettingsView: React.FC = () => {
  const { setEnrollmentStep } = useApp();
  const [diag, setDiag] = useState<DiagnosticsInfo | null>(null);
  const [loadingDiag, setLoadingDiag] = useState<boolean>(false);
  const [copiedId, setCopiedId] = useState<boolean>(false);
  const [copiedKey, setCopiedKey] = useState<boolean>(false);

  // Unlink dialog
  const [showUnlinkModal, setShowUnlinkModal] = useState<boolean>(false);
  const [unlinkConfirmText, setUnlinkConfirmText] = useState<string>('');
  const [unlinkLoading, setUnlinkLoading] = useState<boolean>(false);
  const [unlinkError, setUnlinkError] = useState<string | null>(null);

  const fetchDiagnostics = async () => {
    setLoadingDiag(true);
    try {
      const info = await bridge.getDiagnostics();
      setDiag(info);
    } catch (_e) {
      // ignore
    } finally {
      setLoadingDiag(false);
    }
  };

  useEffect(() => {
    fetchDiagnostics();
  }, []);

  const handleCopyId = async () => {
    if (!diag?.device_id_short) return;
    try {
      await navigator.clipboard.writeText(diag.device_id_short);
      setCopiedId(true);
      setTimeout(() => setCopiedId(false), 2000);
    } catch (_e) {}
  };

  const handleCopyKey = async () => {
    if (!diag?.key_fingerprint) return;
    try {
      await navigator.clipboard.writeText(diag.key_fingerprint);
      setCopiedKey(true);
      setTimeout(() => setCopiedKey(false), 2000);
    } catch (_e) {}
  };

  const handleUnlink = async () => {
    setUnlinkLoading(true);
    setUnlinkError(null);
    try {
      await bridge.unlinkDevice(unlinkConfirmText.trim());
      setShowUnlinkModal(false);
      setEnrollmentStep('link');
    } catch (err: unknown) {
      const msg = err && typeof err === 'object' && 'message' in err ? String(err.message) : 'Error al desvincular el equipo';
      setUnlinkError(msg);
    } finally {
      setUnlinkLoading(false);
    }
  };

  return (
    <div className="p-6 select-none max-w-lg space-y-6">
      <div className="flex items-center space-x-2">
        <Settings className="w-5 h-5 text-accent" />
        <h1 className="text-base font-semibold text-fg">Ajustes</h1>
      </div>

      {/* Diagnostics */}
      <div className="bg-surface border border-default rounded-card p-4 shadow-sm">
        <div className="flex items-center justify-between mb-3 border-b border-default pb-2">
          <h2 className="text-xs font-semibold text-fg">Diagnóstico del sistema</h2>
          <button
            type="button"
            onClick={fetchDiagnostics}
            disabled={loadingDiag}
            className="text-xs text-muted hover:text-fg flex items-center space-x-1 focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-accent rounded px-1"
          >
            <RefreshCw className={`w-3.5 h-3.5 ${loadingDiag ? 'animate-spin' : ''}`} />
            <span>Actualizar</span>
          </button>
        </div>

        {diag ? (
          <div className="text-xs space-y-2">
            <div className="flex justify-between py-1 border-b border-default/50">
              <span className="text-muted">Versión del servicio:</span>
              <span className="font-mono text-fg">{diag.service_version}</span>
            </div>
            <div className="flex justify-between py-1 border-b border-default/50">
              <span className="text-muted">Tiempo de actividad:</span>
              <span className="text-fg">{formatUptime(diag.uptime_seconds)}</span>
            </div>
            <div className="flex justify-between py-1 border-b border-default/50">
              <span className="text-muted">Servidor API:</span>
              <span className="font-mono text-[11px] text-fg truncate max-w-[200px]" title={diag.api_base_url}>
                {diag.api_base_url}
              </span>
            </div>
            <div className="flex justify-between py-1 border-b border-default/50">
              <span className="text-muted">Cola de telemetría:</span>
              <span className="text-fg">
                {diag.telemetry_queue_items} sobres ({formatBytes(diag.telemetry_queue_bytes)})
              </span>
            </div>
            <div className="flex items-center justify-between py-1 border-b border-default/50">
              <span className="text-muted">Identificador de equipo:</span>
              <div className="flex items-center space-x-1">
                <span className="font-mono text-fg">{diag.device_id_short}</span>
                <button
                  type="button"
                  onClick={handleCopyId}
                  className="p-1 text-muted hover:text-fg focus-visible:outline-none"
                  aria-label="Copiar identificador de equipo"
                >
                  {copiedId ? <Check className="w-3.5 h-3.5 text-emerald-600" /> : <Copy className="w-3.5 h-3.5" />}
                </button>
              </div>
            </div>
            <div className="flex items-center justify-between py-1 border-b border-default/50">
              <span className="text-muted">Huella de clave pública:</span>
              <div className="flex items-center space-x-1">
                <span className="font-mono text-[11px] text-fg">
                  {diag.key_fingerprint ? diag.key_fingerprint.slice(0, 16) : 'No fijada'}
                </span>
                {diag.key_fingerprint && (
                  <button
                    type="button"
                    onClick={handleCopyKey}
                    className="p-1 text-muted hover:text-fg focus-visible:outline-none"
                    aria-label="Copiar huella de clave"
                  >
                    {copiedKey ? <Check className="w-3.5 h-3.5 text-emerald-600" /> : <Copy className="w-3.5 h-3.5" />}
                  </button>
                )}
              </div>
            </div>
            <div className="flex justify-between py-1 border-b border-default/50">
              <span className="text-muted">Bandeja del sistema:</span>
              <span className={diag.tray_available ? 'text-emerald-700 dark:text-emerald-400 font-medium' : 'text-danger font-medium'}>
                {diag.tray_available ? 'Disponible' : 'No disponible'}
              </span>
            </div>
            <div className="flex justify-between py-1">
              <span className="text-muted">Permisos del SO:</span>
              <span className="text-fg">{diag.os_permissions}</span>
            </div>
          </div>
        ) : (
          <div className="text-xs text-muted py-3 text-center">Cargando diagnóstico…</div>
        )}
      </div>

      <div aria-live="polite" className="sr-only">
        {copiedId && 'Identificador copiado al portapapeles'}
        {copiedKey && 'Huella de clave copiada al portapapeles'}
      </div>

      {/* Shutdown service (RF-AGENT-11-7 / PROC-04) */}
      <div className="bg-surface border border-default rounded-card p-4 shadow-sm">
        <h2 className="text-xs font-semibold text-fg mb-1">Detener servicio</h2>
        <p className="text-xs text-muted leading-relaxed">
          Para cerrar completamente el servicio en segundo plano de Telemetry en este computador, usa la opción <span className="font-semibold text-fg">Salir</span> en el menú de la bandeja del sistema. Se te solicitará confirmación en esta ventana antes de cerrarlo.
        </p>
      </div>

      {/* Danger zone: Unlink */}
      <div className="bg-surface border border-danger/40 rounded-card p-4 shadow-sm">
        <div className="flex items-center space-x-1.5 mb-1 text-danger">
          <AlertTriangle className="w-4 h-4 shrink-0" />
          <h2 className="text-xs font-semibold">Zona de peligro: Desvincular</h2>
        </div>
        <p className="text-xs text-muted mb-3 leading-relaxed">
          Se borran tus credenciales locales y los datos de telemetría no enviados. Tus marcas siguen guardadas en el servidor.
        </p>
        <button
          type="button"
          onClick={() => {
            setUnlinkConfirmText('');
            setUnlinkError(null);
            setShowUnlinkModal(true);
          }}
          className="h-9 px-3 rounded-control bg-danger text-white hover:bg-danger-hover text-xs font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-danger"
        >
          Desvincular este equipo…
        </button>
      </div>

      {/* Modal: Unlink confirmation */}
      <Modal
        title="¿Desvincular este equipo?"
        isOpen={showUnlinkModal}
        onClose={() => setShowUnlinkModal(false)}
      >
        <div className="space-y-3">
          <p className="text-xs text-muted leading-relaxed">
            Para confirmar, escribe exactamente <span className="font-mono font-bold text-fg">DESVINCULAR</span> a continuación:
          </p>

          <input
            type="text"
            value={unlinkConfirmText}
            onChange={(e) => setUnlinkConfirmText(e.target.value)}
            placeholder="DESVINCULAR"
            spellCheck="false"
            className="w-full h-9 px-3 rounded-control border border-strong bg-surface text-fg font-mono text-center tracking-wider text-xs focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-danger"
          />

          {unlinkError && (
            <div role="alert" aria-live="polite" className="p-2 bg-red-50 dark:bg-red-950/40 border border-red-200 text-xs text-danger rounded">
              {unlinkError}
            </div>
          )}

          <div className="flex space-x-2 pt-2">
            <button
              type="button"
              onClick={handleUnlink}
              disabled={unlinkLoading || unlinkConfirmText.trim() !== 'DESVINCULAR'}
              className="flex-1 h-9 rounded-control bg-danger text-white hover:bg-danger-hover font-medium text-xs transition-colors disabled:opacity-50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-danger"
            >
              {unlinkLoading ? 'Desvinculando…' : 'Confirmar desvinculación'}
            </button>
            <button
              type="button"
              onClick={() => setShowUnlinkModal(false)}
              disabled={unlinkLoading}
              className="flex-1 h-9 rounded-control border border-default bg-surface text-fg hover:bg-surface-subtle font-medium text-xs transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent"
            >
              Cancelar
            </button>
          </div>
        </div>
      </Modal>
    </div>
  );
};
