import React from 'react';
import { AlertCircle, AlertTriangle, Info } from 'lucide-react';
import { useApp } from '../context/AppContext';
import { getRunModeBanner } from '../lib/helpers';

export const Banner: React.FC = () => {
  const { snapshot } = useApp();

  if (!snapshot) return null;

  // 1. Pending withdrawal has high priority
  if (snapshot.pending_withdrawal) {
    return (
      <div
        role="status"
        className="w-full bg-amber-50 dark:bg-amber-950/40 border-b border-amber-200 dark:border-amber-800/60 px-3 py-2 flex items-start space-x-2 text-xs"
      >
        <AlertTriangle className="w-4 h-4 text-amber-600 dark:text-amber-400 shrink-0 mt-0.5" />
        <div className="text-amber-900 dark:text-amber-200">
          <span className="font-semibold">Retiro pendiente de confirmación: </span>
          La recolección está detenida. Se notificará al servidor en la próxima sincronización.
        </div>
      </div>
    );
  }

  // 2. Banner based on run_mode
  const banner = getRunModeBanner(snapshot.run_mode, snapshot.last_error);
  if (!banner) return null;

  const getThemeClasses = () => {
    switch (banner.tone) {
      case 'danger':
        return {
          wrapper: 'bg-red-50 dark:bg-red-950/40 border-red-200 dark:border-red-800/60',
          text: 'text-red-900 dark:text-red-200',
          icon: <AlertCircle className="w-4 h-4 text-red-600 dark:text-red-400 shrink-0 mt-0.5" />,
        };
      case 'warning':
        return {
          wrapper: 'bg-amber-50 dark:bg-amber-950/40 border-amber-200 dark:border-amber-800/60',
          text: 'text-amber-900 dark:text-amber-200',
          icon: <AlertTriangle className="w-4 h-4 text-amber-600 dark:text-amber-400 shrink-0 mt-0.5" />,
        };
      case 'info':
      default:
        return {
          wrapper: 'bg-blue-50 dark:bg-blue-950/40 border-blue-200 dark:border-blue-800/60',
          text: 'text-blue-900 dark:text-blue-200',
          icon: <Info className="w-4 h-4 text-blue-600 dark:text-blue-400 shrink-0 mt-0.5" />,
        };
    }
  };

  const theme = getThemeClasses();

  return (
    <div
      role="status"
      className={`w-full border-b px-3 py-2 flex items-start space-x-2 text-xs ${theme.wrapper}`}
    >
      {theme.icon}
      <div className={theme.text}>
        <span className="font-semibold">{banner.title}: </span>
        {banner.message}
      </div>
    </div>
  );
};
