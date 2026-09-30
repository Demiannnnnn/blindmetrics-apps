import React from 'react';
import type { ConsentDescriptions, ConsentFlags } from '../api/tauri-bridge';
import {
  CONSENT_CATEGORIES,
  ConsentKey,
  DESCRIPTION_MAX,
  descriptionErrors,
} from './consentCategories';

interface ConsentEditorProps {
  role: 'OFFICE' | 'FIELD';
  flags: ConsentFlags;
  descriptions: ConsentDescriptions;
  onChange: (flags: ConsentFlags, descriptions: ConsentDescriptions) => void;
}

/**
 * "Data that may be collected" editor (W2 / W4). The admin defines categories and the
 * text the worker will read verbatim; the worker chooses which of them to accept on their
 * device (granular consent, DP-11).
 */
export const ConsentEditor: React.FC<ConsentEditorProps> = ({ role, flags, descriptions, onChange }) => {
  const errors = descriptionErrors(flags, descriptions);

  const toggle = (key: ConsentKey, template: string) => {
    const enabled = !flags[key];
    onChange(
      { ...flags, [key]: enabled },
      { ...descriptions, [key]: enabled ? descriptions[key] || template : descriptions[key] },
    );
  };

  return (
    <div className="space-y-2">
      {CONSENT_CATEGORIES.map(cat => {
        // DP-04: location is only possible for FIELD workers.
        const locked = cat.key === 'location' && role === 'OFFICE';
        const enabled = flags[cat.key] && !locked;
        return (
          <div key={cat.key} className="bg-slate-900/70 border border-slate-700/70 rounded-lg p-2.5">
            <label
              className={`flex items-center gap-2 text-xs ${locked ? 'text-slate-500 cursor-not-allowed' : 'text-slate-200 cursor-pointer'}`}
              title={locked ? 'La ubicación solo puede recolectarse para trabajadores FIELD (DP-04).' : undefined}
            >
              <input
                type="checkbox"
                checked={enabled}
                disabled={locked}
                onChange={() => toggle(cat.key, cat.template)}
                className="accent-indigo-500"
              />
              <span className="font-medium">{cat.label}</span>
              {locked && <span className="text-[10px] text-slate-500">(no disponible para OFFICE)</span>}
            </label>
            {enabled && (
              <div className="mt-2">
                <textarea
                  value={descriptions[cat.key] || ''}
                  maxLength={DESCRIPTION_MAX}
                  rows={2}
                  onChange={e => onChange(flags, { ...descriptions, [cat.key]: e.target.value })}
                  className={`w-full bg-slate-950 border text-slate-200 text-xs rounded-lg px-2.5 py-1.5 focus:outline-none ${
                    errors[cat.key] ? 'border-rose-600' : 'border-slate-700 focus:border-indigo-500'
                  }`}
                />
                <div className="flex justify-between text-[10px] mt-0.5">
                  <span className="text-rose-400">{errors[cat.key] || ''}</span>
                  <span className="text-slate-500">
                    {(descriptions[cat.key] || '').trim().length}/{DESCRIPTION_MAX}
                  </span>
                </div>
              </div>
            )}
          </div>
        );
      })}
    </div>
  );
};

interface ConsentPreviewProps {
  organization: string;
  flags: ConsentFlags;
  descriptions: ConsentDescriptions;
}

/** Live preview of the worker's C1 consent screen (docs/worker-app-spec.md §C1). */
export const ConsentPreview: React.FC<ConsentPreviewProps> = ({ organization, flags, descriptions }) => {
  const enabled = CONSENT_CATEGORIES.filter(c => flags[c.key]);
  return (
    <div className="bg-slate-950 border border-slate-700 rounded-xl p-3 text-xs text-slate-300 space-y-2">
      <p className="font-semibold text-white border-b border-slate-800 pb-1.5">
        {organization || 'Su organización'} — Telemetría del dispositivo
      </p>
      {enabled.length === 0 ? (
        <p className="text-slate-500 italic">No se recolectará ningún dato. Active al menos una categoría.</p>
      ) : (
        <>
          <p className="text-slate-400">Tu empleador solicita poder recolectar estos datos. Elige cuáles aceptas:</p>
          {enabled.map(c => (
            <div key={c.key}>
              <p className="text-slate-200">☐ {c.label}</p>
              <p className="pl-5 text-slate-400 italic">“{(descriptions[c.key] || '').trim()}”</p>
            </div>
          ))}
          <p className="text-slate-500 pt-1 border-t border-slate-800">
            Todas las casillas empiezan desmarcadas. El trabajador puede aceptar solo algunas categorías, cambiar su elección
            o retirarla en cualquier momento.
          </p>
        </>
      )}
    </div>
  );
};
