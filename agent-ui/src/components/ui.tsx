import React, { useState } from 'react';
import { Check, Copy } from 'lucide-react';
import type { Tone } from '../lib/attendance';
import { sanitizePinInput } from '../lib/attendance';

// Small shared pieces of the attendance screens. Colors only through design tokens.

export const BUTTON_PRIMARY =
  'h-9 px-4 rounded-control bg-accent text-accent-fg hover:bg-accent-hover font-medium text-sm transition-colors disabled:opacity-50 disabled:cursor-not-allowed focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent';
export const BUTTON_SECONDARY =
  'h-9 px-4 rounded-control border border-default bg-surface text-fg hover:bg-surface-subtle font-medium text-sm transition-colors disabled:opacity-50 disabled:cursor-not-allowed focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent';
export const BUTTON_DANGER =
  'h-9 px-4 rounded-control bg-danger text-white hover:bg-danger-hover font-medium text-sm transition-colors disabled:opacity-50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-danger';
export const BUTTON_LINK =
  'text-xs text-accent hover:text-accent-hover underline-offset-2 hover:underline focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent rounded-badge';
export const CARD = 'bg-surface border border-default rounded-card p-4';

const TONE_CLASSES: Record<Tone, string> = {
  success: 'bg-success-subtle text-success border-success/30',
  warning: 'bg-warning-subtle text-warning border-warning/30',
  danger: 'bg-danger-subtle text-danger border-danger/30',
  info: 'bg-info-subtle text-info border-info/30',
  muted: 'bg-surface-subtle text-muted border-default',
};

export function toneClasses(tone: Tone): string {
  return TONE_CLASSES[tone];
}

export const StatusBadge: React.FC<{ tone: Tone; children: React.ReactNode }> = ({ tone, children }) => (
  <span className={`inline-flex items-center px-1.5 py-0.5 rounded-badge border text-[11px] font-medium ${TONE_CLASSES[tone]}`}>
    {children}
  </span>
);

export const Notice: React.FC<{ tone: Tone; children: React.ReactNode; role?: 'alert' | 'status' }> = ({
  tone,
  children,
  role,
}) => (
  <div role={role} className={`p-2.5 rounded-control border text-xs leading-relaxed ${TONE_CLASSES[tone]}`}>
    {children}
  </div>
);

export const Skeleton: React.FC<{ className?: string }> = ({ className = '' }) => (
  <div className={`bg-surface-subtle rounded-control animate-pulse ${className}`} aria-hidden="true" />
);

/** Masked numeric PIN field (SEC-19): no autocomplete, digits only, keypad-friendly. */
export const PinInput: React.FC<{
  id: string;
  label: string;
  value: string;
  onChange: (value: string) => void;
  autoFocus?: boolean;
  disabled?: boolean;
  invalid?: boolean;
  describedBy?: string;
}> = ({ id, label, value, onChange, autoFocus, disabled, invalid, describedBy }) => (
  <div>
    <label htmlFor={id} className="block text-xs font-medium text-fg mb-1">
      {label}
    </label>
    <input
      id={id}
      type="password"
      inputMode="numeric"
      pattern="[0-9]*"
      autoComplete="off"
      autoCorrect="off"
      spellCheck={false}
      maxLength={8}
      value={value}
      autoFocus={autoFocus}
      disabled={disabled}
      aria-invalid={invalid || undefined}
      aria-describedby={describedBy}
      onChange={(e) => onChange(sanitizePinInput(e.target.value))}
      className="w-40 h-9 px-3 rounded-control border border-strong bg-surface text-fg font-mono tracking-[0.3em] text-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent disabled:opacity-50"
    />
  </div>
);

/** Copy-to-clipboard button with "Copiado" feedback announced to screen readers. */
export const CopyButton: React.FC<{ text: string; label: string; className?: string }> = ({
  text,
  label,
  className = '',
}) => {
  const [copied, setCopied] = useState(false);
  const [failed, setFailed] = useState(false);
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(text);
      setFailed(false);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      setFailed(true);
    }
  };
  return (
    <>
      <button
        type="button"
        onClick={copy}
        className={`inline-flex items-center gap-1 h-7 px-2 rounded-control border border-default bg-surface text-xs text-fg hover:bg-surface-subtle focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent ${className}`}
      >
        {copied ? <Check className="w-3.5 h-3.5 text-success" aria-hidden="true" /> : <Copy className="w-3.5 h-3.5" aria-hidden="true" />}
        <span>{copied ? 'Copiado' : label}</span>
      </button>
      <span aria-live="polite" className="sr-only">
        {copied ? 'Copiado al portapapeles' : failed ? 'No se pudo copiar' : ''}
      </span>
    </>
  );
};
