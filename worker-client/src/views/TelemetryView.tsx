import React, { useState } from 'react';
import { Activity, ShieldOff, Check, X, ArrowLeft } from 'lucide-react';
import { useApp } from '../context/AppContext';
import type { ConsentFlags } from '../api/types';
import { countSelectedCategories, getCategoryStateDetails } from '../lib/helpers';
import * as bridge from '../api/tauri-bridge';
import { Modal } from '../components/Modal';

const CATEGORY_NAMES: Record<keyof ConsentFlags, string> = {
  system_activity: 'Actividad del sistema',
  network_activity: 'Actividad de red (FQDN)',
  productivity_basic: 'Productividad básica (teclado y clics)',
  productivity_screenshots: 'Capturas de pantalla',
  file_activity: 'Actividad de archivos',
  location: 'Ubicación geográfica',
};

export const TelemetryView: React.FC = () => {
  const { snapshot, refreshSnapshot, telemetryMode, setTelemetryMode } = useApp();
  const [loading, setLoading] = useState<boolean>(false);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);

  // DP-08: Checkboxes start ALL UNCHECKED (false) by default
  const [selectedFlags, setSelectedFlags] = useState<ConsentFlags>({
    system_activity: false,
    network_activity: false,
    productivity_basic: false,
    productivity_screenshots: false,
    file_activity: false,
    location: false,
  });

  const offer = snapshot?.consent_offer;
  const isDeclined = snapshot?.run_mode === 'DECLINED';
  const isNotRequested = !offer || Object.values(offer.requested_flags).every((v) => !v);

  const handleToggle = (key: keyof ConsentFlags) => {
    setSelectedFlags((prev) => ({
      ...prev,
      [key]: !prev[key],
    }));
  };

  const handleKeepPrevious = () => {
    if (offer?.previous_selection) {
      // Intersect previous with requested
      const intersected: ConsentFlags = {
        system_activity: !!(offer.previous_selection.system_activity && offer.requested_flags.system_activity),
        network_activity: !!(offer.previous_selection.network_activity && offer.requested_flags.network_activity),
        productivity_basic: !!(offer.previous_selection.productivity_basic && offer.requested_flags.productivity_basic),
        productivity_screenshots: !!(offer.previous_selection.productivity_screenshots && offer.requested_flags.productivity_screenshots),
        file_activity: !!(offer.previous_selection.file_activity && offer.requested_flags.file_activity),
        location: !!(offer.previous_selection.location && offer.requested_flags.location),
      };
      setSelectedFlags(intersected);
    }
  };

  const handleAcceptSelection = async () => {
    setLoading(true);
    setErrorMsg(null);
    try {
      await bridge.submitConsent(selectedFlags);
      await refreshSnapshot();
      setTelemetryMode('view');
    } catch (err: unknown) {
      const msg = err && typeof err === 'object' && 'message' in err ? String(err.message) : 'Error al registrar consentimiento';
      setErrorMsg(msg);
    } finally {
      setLoading(false);
    }
  };

  const handleRejectAll = async () => {
    setLoading(true);
    setErrorMsg(null);
    try {
      await bridge.submitConsent(null);
      await refreshSnapshot();
      setTelemetryMode('view');
    } catch (err: unknown) {
      const msg = err && typeof err === 'object' && 'message' in err ? String(err.message) : 'Error al rechazar consentimiento';
      setErrorMsg(msg);
    } finally {
      setLoading(false);
    }
  };

  const handleConfirmWithdraw = async () => {
    setLoading(true);
    setErrorMsg(null);
    try {
      await bridge.withdrawConsent();
      await refreshSnapshot();
      setTelemetryMode('view');
    } catch (err: unknown) {
      const msg = err && typeof err === 'object' && 'message' in err ? String(err.message) : 'Error al retirar consentimiento';
      setErrorMsg(msg);
    } finally {
      setLoading(false);
    }
  };

  // Screen W: Withdraw confirmation dialog (DP-10: <= 2 clicks)
  const renderWithdrawModal = () => (
    <Modal
      title="¿Retirar consentimiento de telemetría?"
      isOpen={telemetryMode === 'withdraw'}
      onClose={() => setTelemetryMode('view')}
    >
      <div className="space-y-4">
        <p className="text-xs text-muted leading-relaxed">
          Se detendrá la recolección de inmediato y se eliminarán los datos locales no enviados (DP-09).
          Tu registro de asistencia no se verá afectado (ATT-07).
        </p>

        {errorMsg && (
          <div role="alert" aria-live="polite" className="p-2 bg-red-50 dark:bg-red-950/40 border border-red-200 text-xs text-danger rounded">
            {errorMsg}
          </div>
        )}

        <div className="flex space-x-2 pt-2">
          <button
            type="button"
            onClick={handleConfirmWithdraw}
            disabled={loading}
            className="flex-1 h-9 rounded-control bg-danger text-white hover:bg-danger-hover font-medium text-xs transition-colors disabled:opacity-50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-danger"
          >
            {loading ? 'Retirando…' : 'Confirmar retiro'}
          </button>
          <button
            type="button"
            onClick={() => setTelemetryMode('view')}
            disabled={loading}
            className="flex-1 h-9 rounded-control border border-default bg-surface text-fg hover:bg-surface-subtle font-medium text-xs transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent"
          >
            Cancelar
          </button>
        </div>
      </div>
    </Modal>
  );

  // Screen C1: Consent review / modify
  if (telemetryMode === 'consent') {
    const selectedCount = countSelectedCategories(selectedFlags);
    const requestedEntries = (Object.keys(CATEGORY_NAMES) as Array<keyof ConsentFlags>).filter(
      (cat) => offer?.requested_flags[cat]
    );

    return (
      <div className="p-6 select-none max-w-lg">
        <div className="flex items-center justify-between mb-4">
          <div className="flex items-center space-x-2">
            <button
              onClick={() => setTelemetryMode('view')}
              className="p-1 rounded text-muted hover:text-fg focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent"
              aria-label="Volver a telemetría"
            >
              <ArrowLeft className="w-4 h-4" />
            </button>
            <h1 className="text-base font-semibold text-fg">Solicitud de telemetría</h1>
          </div>
          {offer?.is_update && offer.previous_selection && (
            <button
              onClick={handleKeepPrevious}
              className="text-xs text-accent hover:underline focus-visible:outline-none"
            >
              Mantener selección anterior
            </button>
          )}
        </div>

        <p className="text-xs text-muted mb-4 leading-relaxed">
          Selecciona las categorías que autorizas recolectar en este equipo. Cada casilla inicia desmarcada.
        </p>

        {errorMsg && (
          <div role="alert" aria-live="polite" className="p-2.5 mb-4 bg-red-50 dark:bg-red-950/40 border border-red-200 rounded-control text-xs text-danger">
            {errorMsg}
          </div>
        )}

        <div className="space-y-3 mb-6">
          {requestedEntries.map((key) => {
            const isChecked = selectedFlags[key];
            const desc = offer?.descriptions[key];

            return (
              <label
                key={key}
                className={`block p-3 rounded-card border transition-colors cursor-pointer ${
                  isChecked
                    ? 'border-accent bg-accent-subtle/30'
                    : 'border-default bg-surface hover:bg-surface-subtle/60'
                }`}
              >
                <div className="flex items-start space-x-3">
                  <input
                    type="checkbox"
                    checked={isChecked}
                    onChange={() => handleToggle(key)}
                    className="mt-0.5 h-4 w-4 rounded border-strong text-accent focus-visible:ring-accent accent-accent cursor-pointer"
                  />
                  <div className="min-w-0 flex-1">
                    <span className="text-xs font-semibold text-fg block">
                      {CATEGORY_NAMES[key]}
                    </span>
                    {desc && (
                      <p className="text-[11px] text-muted mt-1 leading-normal">
                        {desc}
                      </p>
                    )}
                  </div>
                </div>
              </label>
            );
          })}
        </div>

        <div className="flex items-center space-x-2 pt-2 border-t border-default">
          <button
            type="button"
            onClick={handleAcceptSelection}
            disabled={loading || selectedCount === 0}
            className="flex-1 h-9 rounded-control bg-accent text-accent-fg hover:bg-accent-hover font-medium text-xs transition-colors disabled:opacity-50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent flex items-center justify-center space-x-1"
          >
            <Check className="w-4 h-4 mr-1" />
            <span>Aceptar selección ({selectedCount})</span>
          </button>

          <button
            type="button"
            onClick={handleRejectAll}
            disabled={loading}
            className="h-9 px-3 rounded-control border border-default bg-surface text-fg hover:bg-surface-subtle font-medium text-xs transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent flex items-center justify-center space-x-1"
          >
            <X className="w-3.5 h-3.5 mr-1 text-muted" />
            <span>Rechazar todo</span>
          </button>
        </div>
      </div>
    );
  }

  // Normal Telemetry View
  const categoryStates = snapshot?.category_states;

  return (
    <div className="p-6 select-none max-w-lg">
      <div className="flex items-center justify-between mb-4">
        <div className="flex items-center space-x-2">
          <Activity className="w-5 h-5 text-accent" />
          <h1 className="text-base font-semibold text-fg">Telemetría</h1>
        </div>
      </div>

      {renderWithdrawModal()}

      {isNotRequested ? (
        <div className="bg-surface border border-default rounded-card p-6 shadow-sm text-center">
          <p className="text-sm font-medium text-fg mb-1">
            Tu empleador no solicita telemetría de este equipo.
          </p>
          <p className="text-xs text-muted">
            No se recolecta ningún dato de actividad ni rendimiento en esta máquina.
          </p>
        </div>
      ) : isDeclined ? (
        <div className="bg-surface border border-default rounded-card p-6 shadow-sm text-center">
          <div className="w-10 h-10 rounded-full bg-zinc-100 dark:bg-zinc-800 flex items-center justify-center mx-auto mb-3">
            <ShieldOff className="w-5 h-5 text-muted" />
          </div>
          <p className="text-sm font-medium text-fg mb-1">
            Rechazaste la telemetría
          </p>
          <p className="text-xs text-muted mb-4">
            No se recolecta ningún dato en este computador. Tu asistencia opera con total normalidad.
          </p>
          <button
            type="button"
            onClick={() => setTelemetryMode('consent')}
            className="h-9 px-4 rounded-control bg-accent text-accent-fg hover:bg-accent-hover font-medium text-xs transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent"
          >
            Revisar solicitud
          </button>
        </div>
      ) : !snapshot?.accepted_selection ? (
        <div className="bg-surface border border-default rounded-card p-6 shadow-sm text-center">
          <p className="text-sm font-semibold text-fg mb-1">
            Tu empleador solicita datos de este equipo
          </p>
          <p className="text-xs text-muted mb-4">
            Puedes revisar las categorías solicitadas y elegir cuáles deseas autorizar o rechazar.
          </p>
          <button
            type="button"
            onClick={() => setTelemetryMode('consent')}
            className="h-9 px-4 rounded-control bg-accent text-accent-fg hover:bg-accent-hover font-medium text-xs transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent"
          >
            Revisar solicitud
          </button>
        </div>
      ) : (
        <div className="space-y-4">
          <div className="bg-surface border border-default rounded-card p-4 shadow-sm space-y-3">
            <div className="text-xs font-semibold text-fg border-b border-default pb-2">
              Categorías de telemetría
            </div>

            <div className="space-y-2.5">
              {(Object.keys(CATEGORY_NAMES) as Array<keyof ConsentFlags>).map((key) => {
                const state = categoryStates ? categoryStates[key] : 'NOT_REQUESTED';
                const { label, tone } = getCategoryStateDetails(state);
                const desc = offer?.descriptions[key];

                const toneBadgeClass =
                  tone === 'active'
                    ? 'bg-emerald-100 text-emerald-800 dark:bg-emerald-950/60 dark:text-emerald-300'
                    : tone === 'warning'
                    ? 'bg-amber-100 text-amber-800 dark:bg-amber-950/60 dark:text-amber-300'
                    : tone === 'danger'
                    ? 'bg-red-100 text-red-800 dark:bg-red-950/60 dark:text-red-300'
                    : 'bg-zinc-100 text-zinc-700 dark:bg-zinc-800 dark:text-zinc-300';

                return (
                  <div key={key} className="p-2.5 rounded border border-default/70 bg-surface-subtle/30">
                    <div className="flex items-center justify-between mb-1">
                      <span className="text-xs font-medium text-fg">
                        {CATEGORY_NAMES[key]}
                      </span>
                      <span className={`text-[10px] px-1.5 py-0.5 rounded-badge font-semibold ${toneBadgeClass}`}>
                        {label}
                      </span>
                    </div>
                    {desc && (
                      <p className="text-[11px] text-muted leading-tight">
                        {desc}
                      </p>
                    )}
                  </div>
                );
              })}
            </div>
          </div>

          <div className="flex items-center space-x-2 pt-1">
            <button
              type="button"
              onClick={() => {
                if (snapshot?.accepted_selection) {
                  setSelectedFlags({ ...snapshot.accepted_selection });
                }
                setTelemetryMode('consent');
              }}
              className="flex-1 h-9 rounded-control border border-default bg-surface text-fg hover:bg-surface-subtle font-medium text-xs transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent"
            >
              Cambiar mi selección
            </button>

            <button
              type="button"
              onClick={() => setTelemetryMode('withdraw')}
              className="h-9 px-3 rounded-control border border-danger/40 text-danger hover:bg-red-50 dark:hover:bg-red-950/30 font-medium text-xs transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-danger"
            >
              Retirar consentimiento
            </button>
          </div>
        </div>
      )}
    </div>
  );
};
