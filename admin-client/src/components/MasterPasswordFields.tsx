import React from 'react';
import { KeyRound } from 'lucide-react';
import { masterPasswordIssue, passwordStrength } from '../lib/vault';

interface MasterPasswordFieldsProps {
  password: string;
  onPasswordChange: (value: string) => void;
  /** When defined, a confirmation field is shown (new Master Password). */
  confirmation?: string;
  onConfirmationChange?: (value: string) => void;
  autoFocus?: boolean;
  disabled?: boolean;
  label?: string;
}

const STRENGTH_COLORS: Record<string, string> = {
  débil: 'text-danger',
  aceptable: 'text-warning',
  fuerte: 'text-success',
};

/**
 * Master Password inputs (TASK-ADMIN-11). The password never leaves this device: it is sent
 * to the Rust core over IPC, where Argon2id derives the key. Parents clear the state after
 * every attempt.
 */
export const MasterPasswordFields: React.FC<MasterPasswordFieldsProps> = ({
  password,
  onPasswordChange,
  confirmation,
  onConfirmationChange,
  autoFocus,
  disabled,
  label = 'Master Password',
}) => {
  const isNew = confirmation !== undefined;
  const issue = isNew && password ? masterPasswordIssue(password, confirmation || undefined) : null;
  const strength = password ? passwordStrength(password) : null;
  const input =
    'w-full bg-bg border border-default rounded-control px-3 py-2 text-xs text-fg placeholder-muted focus:outline-none focus:border-accent';

  return (
    <div className="space-y-2">
      <label className="block text-xs font-medium text-fg">
        <span className="flex items-center gap-1.5 mb-1">
          <KeyRound className="w-3.5 h-3.5 text-muted" />
          {label}
        </span>
        <input
          type="password"
          autoComplete={isNew ? 'new-password' : 'current-password'}
          value={password}
          autoFocus={autoFocus}
          disabled={disabled}
          onChange={e => onPasswordChange(e.target.value)}
          placeholder="Mínimo 12 caracteres"
          className={input}
        />
      </label>
      {isNew && (
        <label className="block text-xs font-medium text-fg">
          <span className="block mb-1">Repite la Master Password</span>
          <input
            type="password"
            autoComplete="new-password"
            value={confirmation}
            disabled={disabled}
            onChange={e => onConfirmationChange?.(e.target.value)}
            className={input}
          />
        </label>
      )}
      {isNew && (
        <div className="flex justify-between text-[11px]">
          <span className="text-danger">{confirmation ? issue ?? '' : ''}</span>
          {strength && <span className={STRENGTH_COLORS[strength]}>Fortaleza: {strength}</span>}
        </div>
      )}
      {isNew && (
        <p className="text-[11px] text-muted">
          No es la contraseña de tu cuenta. Nadie puede recuperarla: si la olvidas y pierdes este equipo, los datos
          históricos quedan ilegibles.
        </p>
      )}
    </div>
  );
};
