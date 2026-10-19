import React from 'react';
import { useApp } from '../context/AppContext';

export const Header: React.FC = () => {
  const { snapshot } = useApp();

  const org = snapshot?.organization || 'Mi Empresa';
  const worker = snapshot?.worker_name || 'Trabajador';
  const runMode = snapshot?.run_mode || 'UNENROLLED';

  const getStatusLight = () => {
    switch (runMode) {
      case 'COLLECTING':
        return {
          colorClass: 'bg-emerald-500',
          text: 'Recolectando',
          textClass: 'text-emerald-700 dark:text-emerald-400',
        };
      case 'PAUSED':
        return {
          colorClass: 'bg-amber-500',
          text: 'Pausado',
          textClass: 'text-amber-700 dark:text-amber-400',
        };
      case 'EMPLOYER_SUSPENDED':
        return {
          colorClass: 'bg-amber-500',
          text: 'Suspendido',
          textClass: 'text-amber-700 dark:text-amber-400',
        };
      case 'HALTED':
        return {
          colorClass: 'bg-red-500',
          text: 'Detenido',
          textClass: 'text-red-700 dark:text-red-400',
        };
      default:
        return {
          colorClass: 'bg-zinc-400',
          text: 'Inactivo',
          textClass: 'text-muted',
        };
    }
  };

  const status = getStatusLight();

  return (
    <header className="h-12 border-b border-default bg-surface px-4 flex items-center justify-between shrink-0 select-none">
      <div className="flex items-center space-x-2 min-w-0">
        <span className="font-semibold text-fg text-sm truncate">{org}</span>
        <span className="text-muted text-xs">·</span>
        <span className="text-muted text-xs truncate">{worker}</span>
      </div>

      <div className="flex items-center space-x-2 shrink-0">
        <span
          className={`inline-block w-2 h-2 rounded-full ${status.colorClass}`}
          aria-hidden="true"
        />
        <span className={`text-xs font-medium ${status.textClass}`}>
          {status.text}
        </span>
      </div>
    </header>
  );
};
