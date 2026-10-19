import React from 'react';
import { AlertCircle, AlertTriangle, Info } from 'lucide-react';
import { useApp } from '../context/AppContext';
import { selectBanner, type BannerAction } from '../lib/attendance';

const TONES = {
  danger: { wrapper: 'bg-danger-subtle border-danger/30 text-danger', Icon: AlertCircle },
  warning: { wrapper: 'bg-warning-subtle border-warning/30 text-warning', Icon: AlertTriangle },
  info: { wrapper: 'bg-info-subtle border-info/30 text-info', Icon: Info },
} as const;

/** One banner at a time, by the priority of spec §1, with its action (RF-AGENT-13-7). */
export const Banner: React.FC = () => {
  const { snapshot, setCurrentView, setTelemetryMode, setTermsOverlay } = useApp();
  const banner = selectBanner(snapshot);
  if (!banner) return null;

  const run = (action: BannerAction) => {
    switch (action) {
      case 'DIAGNOSTICS':
        setCurrentView('settings');
        break;
      case 'ATTENDANCE':
        setCurrentView('attendance');
        break;
      case 'RECORDS':
        setCurrentView('records');
        break;
      case 'TERMS':
        setTermsOverlay('update');
        break;
      case 'TELEMETRY':
        setTelemetryMode('view');
        setCurrentView('telemetry');
        break;
    }
  };

  const { wrapper, Icon } = TONES[banner.tone];
  return (
    <div role="status" className={`w-full border-b px-3 py-2 flex items-start gap-2 text-xs ${wrapper}`}>
      <Icon className="w-4 h-4 shrink-0 mt-0.5" aria-hidden="true" />
      <p className="flex-1 text-fg leading-relaxed">{banner.message}</p>
      <button
        type="button"
        onClick={() => run(banner.action)}
        className="shrink-0 font-medium underline underline-offset-2 hover:no-underline focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent rounded-badge"
      >
        {banner.actionLabel}
      </button>
    </div>
  );
};
