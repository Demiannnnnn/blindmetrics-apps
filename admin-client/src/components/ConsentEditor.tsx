import React from 'react';
import type { ConsentDescriptions, ConsentFlags, WorkMode } from '../api/types';
import {
  CONSENT_CATEGORIES,
  ConsentKey,
  DESCRIPTION_MAX,
  descriptionErrors,
} from './consentCategories';

export interface ConsentEditorProps {
  role: 'OFFICE' | 'FIELD';
  workMode?: WorkMode;
  flags: ConsentFlags;
  descriptions: ConsentDescriptions;
  locationCause?: string;
  onLocationCauseChange?: (cause: string) => void;
  onChange: (flags: ConsentFlags, descriptions: ConsentDescriptions) => void;
}

/**
 * "Data that may be collected" editor (W2 / W4). The admin defines categories and the
 * text the worker will read verbatim; the worker chooses which of them to accept on their
 * device (granular consent, DP-11).
 */
export const ConsentEditor: React.FC<ConsentEditorProps> = ({
  role,
  workMode = 'ON_SITE',
  flags,
  descriptions,
  locationCause = '',
  onLocationCauseChange,
  onChange,
}) => {
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
        const locationLocked = cat.key === 'location' && role === 'OFFICE';
        // DP-05: screenshots not allowed for REMOTE or HYBRID workers (R38 art. 55 a).
        const screenshotLocked =
          cat.key === 'productivity_screenshots' && workMode !== 'ON_SITE';
        const locked = locationLocked || screenshotLocked;
        const enabled = flags[cat.key] && !locked;

        let lockedHelp = '';
        if (locationLocked) lockedHelp = '(no disponible para OFFICE)';
        else if (screenshotLocked)
          lockedHelp = '(no disponible en teletrabajo o modalidad mixta)';

        return (
          <div key={cat.key} className="bg-surface border border-default rounded-control p-2.5">
            <label
              className={`flex items-center gap-2 text-xs ${
                locked ? 'text-muted cursor-not-allowed' : 'text-fg cursor-pointer'
              }`}
              title={
                locationLocked
                  ? 'La ubicación solo puede recolectarse para trabajadores FIELD (DP-04).'
                  : screenshotLocked
                    ? 'Capturas de pantalla no permitidas para teletrabajo o modalidad mixta (DP-05, R38 art. 55 a).'
                    : undefined
              }
            >
              <input
                type="checkbox"
                checked={enabled}
                disabled={locked}
                onChange={() => toggle(cat.key, cat.template)}
                className="accent-accent rounded-sm"
              />
              <span className="font-medium">{cat.label}</span>
              {locked && <span className="text-[10px] text-muted">{lockedHelp}</span>}
            </label>
            {enabled && (
              <div className="mt-2 space-y-2">
                <div>
                  <label className="text-[10px] text-muted block mb-0.5">
                    Descripción para el trabajador (10–500 caracteres):
                  </label>
                  <textarea
                    value={descriptions[cat.key] || ''}
                    maxLength={DESCRIPTION_MAX}
                    rows={2}
                    onChange={e =>
                      onChange(flags, { ...descriptions, [cat.key]: e.target.value })
                    }
                    className={`w-full bg-bg border text-fg text-xs rounded-control px-2.5 py-1.5 focus:outline-hidden ${
                      errors[cat.key] ? 'border-danger' : 'border-default focus:border-accent'
                    }`}
                  />
                  <div className="flex justify-between text-[10px] mt-0.5">
                    <span className="text-danger">{errors[cat.key] || ''}</span>
                    <span className="text-muted">
                      {(descriptions[cat.key] || '').trim().length}/{DESCRIPTION_MAX}
                    </span>
                  </div>
                </div>

                {cat.key === 'location' && onLocationCauseChange && (
                  <div className="p-2 bg-surface-subtle border border-default rounded-control space-y-1">
                    <label className="text-[10px] font-semibold text-fg block">
                      Causa de seguridad o legal (20–500 caracteres, R38 art. 53 e):
                    </label>
                    <textarea
                      value={locationCause}
                      maxLength={500}
                      rows={2}
                      placeholder="Declara la causa de seguridad o legal que se mostrará textual al trabajador (mínimo 20 caracteres)..."
                      onChange={e => onLocationCauseChange(e.target.value)}
                      className="w-full bg-surface border border-default text-fg text-xs rounded-control px-2.5 py-1.5 focus:outline-hidden focus:border-accent"
                    />
                    <div className="flex justify-between text-[10px]">
                      {locationCause.trim().length > 0 && locationCause.trim().length < 20 ? (
                        <span className="text-danger">Mínimo 20 caracteres requeridos</span>
                      ) : (
                        <span className="text-muted">Se muestra textual al trabajador</span>
                      )}
                      <span className="text-muted">{locationCause.trim().length}/500</span>
                    </div>
                  </div>
                )}
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
export const ConsentPreview: React.FC<ConsentPreviewProps> = ({
  organization,
  flags,
  descriptions,
}) => {
  const enabled = CONSENT_CATEGORIES.filter(c => flags[c.key]);
  if (enabled.length === 0) {
    return (
      <div className="bg-bg border border-default rounded-card p-3 text-xs text-muted italic">
        No se solicita telemetría: el trabajador no verá pantalla de consentimiento y solo utilizará la app para registrar asistencia.
      </div>
    );
  }
  return (
    <div className="bg-bg border border-default rounded-card p-3 text-xs text-fg space-y-2">
      <p className="font-semibold text-fg border-b border-default pb-1.5">
        {organization || 'Su organización'} — Telemetría del dispositivo
      </p>
      <p className="text-muted">
        Tu empleador solicita poder recolectar estos datos. Elige cuáles aceptas:
      </p>
      {enabled.map(c => (
        <div key={c.key}>
          <p className="text-fg font-medium">{c.label}</p>
          <p className="pl-5 text-muted italic">“{(descriptions[c.key] || '').trim()}”</p>
        </div>
      ))}
      <p className="text-muted pt-1 border-t border-default text-[11px]">
        Todas las casillas empiezan desmarcadas (DP-08). El trabajador puede aceptar solo algunas
        categorías, cambiar su elección o retirarla en cualquier momento.
      </p>
    </div>
  );
};
