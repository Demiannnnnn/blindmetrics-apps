import React, { useState } from 'react';
import { Clock } from 'lucide-react';
import { openBillingPortal } from '../api/tauri-bridge';
import { describeError } from '../lib/errors';
import { trialBannerText } from '../lib/subscription';

/** RF-ADMIN-12-6: shown while the trial has 7 days or less. */
export const TrialBanner: React.FC<{ daysLeft: number }> = ({ daysLeft }) => {
  const [error, setError] = useState<string | null>(null);
  const open = async () => {
    try {
      await openBillingPortal();
    } catch (e) {
      setError(describeError(e));
    }
  };
  return (
    <div className="px-6 py-2 bg-warning-subtle border-b border-default text-xs text-warning flex items-center gap-3">
      <Clock className="w-3.5 h-3.5" />
      <span>{trialBannerText(daysLeft)}</span>
      <button onClick={open} className="underline font-semibold hover:text-fg">
        Suscribirse
      </button>
      {error && <span className="text-danger">{error}</span>}
    </div>
  );
};
