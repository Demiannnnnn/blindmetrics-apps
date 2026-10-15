import React, { useState } from 'react';
import { newPinMessage } from '../lib/attendance';
import { PinInput } from './ui';

/**
 * Two "new PIN" fields with the informative policy mirror (SEC-20). The service applies the real
 * rule and answers `PIN_POLICY_*`. Values live only in the parent's state and are cleared by it
 * after each action.
 */
export const NewPinFields: React.FC<{
  idPrefix: string;
  pin: string;
  confirm: string;
  onPin: (v: string) => void;
  onConfirm: (v: string) => void;
  showErrors: boolean;
  disabled?: boolean;
  autoFocus?: boolean;
}> = ({ idPrefix, pin, confirm, onPin, onConfirm, showErrors, disabled, autoFocus }) => {
  const [touched, setTouched] = useState(false);
  const problem = newPinMessage(pin, confirm);
  const visible = (showErrors || (touched && confirm.length > 0)) && problem;
  const errorId = `${idPrefix}-error`;
  return (
    <div className="space-y-3">
      <PinInput
        id={`${idPrefix}-new`}
        label="PIN nuevo (6 a 8 dígitos)"
        value={pin}
        onChange={onPin}
        disabled={disabled}
        autoFocus={autoFocus}
        invalid={!!visible}
        describedBy={visible ? errorId : undefined}
      />
      <PinInput
        id={`${idPrefix}-confirm`}
        label="Repite el PIN nuevo"
        value={confirm}
        onChange={(v) => {
          setTouched(true);
          onConfirm(v);
        }}
        disabled={disabled}
        invalid={!!visible}
        describedBy={visible ? errorId : undefined}
      />
      {visible ? (
        <p id={errorId} className="text-xs text-danger">
          {problem}
        </p>
      ) : (
        <p className="text-[11px] text-muted">No uses números repetidos ni seguidos como 123456.</p>
      )}
    </div>
  );
};
