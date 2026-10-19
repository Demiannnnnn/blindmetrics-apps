import React from 'react';
import { AlertTriangle, ArrowLeft } from 'lucide-react';
import { useApp } from '../context/AppContext';

export const TamperingAlertView: React.FC = () => {
  const { setEnrollmentStep } = useApp();

  return (
    <div className="h-full w-full flex flex-col items-center justify-center p-6 select-none">
      <div className="w-full max-w-sm bg-surface border border-danger/40 rounded-card p-6 shadow-sm text-center">
        <div className="w-12 h-12 rounded-full bg-red-100 dark:bg-red-950/60 flex items-center justify-center mx-auto mb-4">
          <AlertTriangle className="w-6 h-6 text-danger" />
        </div>

        <h1 className="text-base font-semibold text-danger mb-2">
          Alerta de seguridad
        </h1>

        <p className="text-xs text-muted mb-6 leading-relaxed">
          Los datos de verificación no coincidieron con los registrados por tu empleador.
          Por precaución criptográfica, se eliminaron las credenciales temporales.
        </p>

        <button
          type="button"
          onClick={() => setEnrollmentStep('link')}
          className="w-full h-9 rounded-control bg-accent text-accent-fg hover:bg-accent-hover font-medium text-sm flex items-center justify-center space-x-2 transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent"
        >
          <ArrowLeft className="w-4 h-4" />
          <span>Volver al inicio</span>
        </button>
      </div>
    </div>
  );
};
