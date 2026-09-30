import React, { useState } from 'react';
import { Building } from 'lucide-react';
import type { AdminProfile } from '../api/types';
import { adminUpdateProfile } from '../api/tauri-bridge';
import { describeError } from '../lib/errors';

interface OrganizationPromptProps {
  initial?: string;
  onSaved: (profile: AdminProfile) => void;
  onCancel?: () => void;
}

/** O1 — organization name, required before creating workers (RF-ADMIN-09-5). */
export const OrganizationPrompt: React.FC<OrganizationPromptProps> = ({ initial = '', onSaved, onCancel }) => {
  const [organization, setOrganization] = useState(initial);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const trimmed = organization.trim();
  const valid = trimmed.length >= 2 && trimmed.length <= 255;

  const save = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!valid) return;
    setSaving(true);
    setError(null);
    try {
      onSaved(await adminUpdateProfile({ organization: trimmed }));
    } catch (err) {
      setError(describeError(err));
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 bg-black/80 flex items-center justify-center p-4">
      <form onSubmit={save} className="bg-slate-900 border border-slate-700 rounded-2xl max-w-md w-full p-6 space-y-4 shadow-2xl">
        <div className="flex items-center gap-2">
          <Building className="w-5 h-5 text-indigo-400" />
          <h3 className="text-base font-semibold text-white">Nombre de tu organización</h3>
        </div>
        <p className="text-xs text-slate-400">
          Se muestra a tus trabajadores en la pantalla de consentimiento, junto a lo que se recolecta. Es obligatorio antes de
          crear trabajadores.
        </p>
        <input
          autoFocus
          value={organization}
          maxLength={255}
          onChange={e => setOrganization(e.target.value)}
          placeholder="Ej. Constructora Los Andes SpA"
          className="w-full bg-slate-950 border border-slate-700 rounded-lg px-3 py-2 text-sm text-white focus:outline-none focus:border-indigo-500"
        />
        {!valid && trimmed.length > 0 && <p className="text-[11px] text-rose-400">Debe tener entre 2 y 255 caracteres.</p>}
        {error && <p className="text-xs text-rose-300">{error}</p>}
        <div className="flex justify-end gap-2">
          {onCancel && (
            <button type="button" onClick={onCancel} className="px-3 py-2 text-xs bg-slate-800 hover:bg-slate-700 text-slate-300 rounded-lg">
              Más tarde
            </button>
          )}
          <button
            type="submit"
            disabled={!valid || saving}
            className="px-4 py-2 text-xs font-semibold bg-indigo-600 hover:bg-indigo-500 text-white rounded-lg disabled:opacity-50"
          >
            {saving ? 'Guardando…' : 'Guardar'}
          </button>
        </div>
      </form>
    </div>
  );
};
