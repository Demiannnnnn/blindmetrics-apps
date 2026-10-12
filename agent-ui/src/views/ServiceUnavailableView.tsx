import React from 'react';
import { AlertCircle, RefreshCw } from 'lucide-react';
import { useApp } from '../context/AppContext';

export const ServiceUnavailableView: React.FC = () => {
  const { retryService, serviceLoading, error } = useApp();

  return (
    <div className="h-full w-full flex flex-col items-center justify-center p-6 text-center select-none">
      <div className="w-12 h-12 rounded-full bg-red-100 dark:bg-red-950/60 flex items-center justify-center mb-4">
        <AlertCircle className="w-6 h-6 text-danger" />
      </div>

      <h1 className="text-lg font-semibold text-fg mb-2">
        Servicio no disponible
      </h1>

      <p className="text-sm text-muted max-w-sm mb-6">
        El servicio de fondo <code className="font-mono text-xs bg-surface-subtle px-1 py-0.5 rounded">telemetry-agent</code> no está respondiendo.
        Verifica que esté ejecutándose o presiona el botón para iniciarlo.
      </p>

      {error && (
        <div
          role="alert"
          aria-live="polite"
          className="mb-6 p-3 bg-red-50 dark:bg-red-950/40 border border-red-200 dark:border-red-800/60 rounded-card text-xs text-red-900 dark:text-red-200 max-w-sm text-left"
        >
          {error}
        </div>
      )}

      <button
        onClick={retryService}
        disabled={serviceLoading}
        className="h-9 px-4 rounded-control bg-accent text-accent-fg hover:bg-accent-hover font-medium text-sm flex items-center space-x-2 transition-colors disabled:opacity-50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent"
      >
        <RefreshCw className={`w-4 h-4 ${serviceLoading ? 'animate-spin' : ''}`} />
        <span>{serviceLoading ? 'Conectando…' : 'Reintentar conexión'}</span>
      </button>
    </div>
  );
};
