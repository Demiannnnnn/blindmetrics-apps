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
    <div className="px-6 py-2 bg-amber-500/10 border-b border-amber-500/30 text-xs text-amber-200 flex items-center gap-3">
      <Clock className="w-3.5 h-3.5" />
      <span>{trialBannerText(daysLeft)}</span>
      <button onClick={open} className="underline font-semibold hover:text-white">
        Suscribirse
      </button>
      {error && <span className="text-rose-300">{error}</span>}
    </div>
  );
};
