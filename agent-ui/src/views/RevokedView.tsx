import React from 'react';
import { UserX, RefreshCw } from 'lucide-react';
import { useApp } from '../context/AppContext';

export const RevokedView: React.FC = () => {
  const { setEnrollmentStep } = useApp();

  return (
    <div className="h-full w-full flex flex-col items-center justify-center p-6 select-none">
      <div className="w-full max-w-sm bg-surface border border-default rounded-card p-6 shadow-sm text-center">
        <div className="w-12 h-12 rounded-full bg-zinc-100 dark:bg-zinc-800 flex items-center justify-center mx-auto mb-4">
          <UserX className="w-6 h-6 text-muted" />
        </div>

        <h1 className="text-base font-semibold text-fg mb-2">
          Equipo desvinculado
        </h1>

        <p className="text-xs text-muted mb-6 leading-relaxed">
          Tu equipo no se encuentra vinculado a una organización activa o el acceso fue revocado.
        </p>

        <button
          type="button"
          onClick={() => setEnrollmentStep('link')}
          className="w-full h-9 rounded-control bg-accent text-accent-fg hover:bg-accent-hover font-medium text-sm flex items-center justify-center space-x-2 transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent"
        >
          <RefreshCw className="w-4 h-4" />
          <span>Vincular nuevamente</span>
        </button>
      </div>
    </div>
  );
};
