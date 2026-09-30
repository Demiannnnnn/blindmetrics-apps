import React, { useState } from 'react';
import { AlertTriangle, CheckCircle2, Download, Upload, Server, KeyRound, Trash2 } from 'lucide-react';
import type { ExportedBackup, KeyInstalled } from '../api/types';
import {
  vaultDeleteServerBackup,
  vaultExportTkb,
  vaultGenerateNewKey,
  vaultImportBackup,
  vaultRecoverFromServer,
  vaultSetupServerBackup,
} from '../api/tauri-bridge';
import { describeError } from '../lib/errors';
import { backupFileIssue, masterPasswordIssue, NEW_KEY_PHRASE, newKeyConfirmationValid, shortFingerprint } from '../lib/vault';
import { MasterPasswordFields } from './MasterPasswordFields';

const panel = 'p-4 bg-slate-950/60 border border-slate-700/70 rounded-xl space-y-3';
const primary =
  'px-4 py-2 bg-indigo-600 hover:bg-indigo-500 text-white text-xs font-semibold rounded-lg disabled:opacity-50 flex items-center gap-1.5';
const secondary = 'px-3 py-2 bg-slate-800 hover:bg-slate-700 text-slate-300 text-xs rounded-lg';

function ErrorLine({ message }: { message: string | null }) {
  return message ? (
    <p className="text-xs text-rose-300 flex items-start gap-1.5">
      <AlertTriangle className="w-3.5 h-3.5 mt-0.5 shrink-0" />
      {message}
    </p>
  ) : null;
}

export const SAFE_KEEPING_WARNING =
  'Guarda este archivo en un lugar seguro y separado de este equipo (por ejemplo, un pendrive guardado bajo llave o un gestor de contraseñas). Quien tenga el archivo y tu Master Password puede leer los datos de tus trabajadores. Si pierdes ambos, los datos históricos son irrecuperables.';

/** RF-ADMIN-11-3: download a `.tkb` backup (TASK-APP-02 "Exportar respaldo"). */
export const ExportTkbPanel: React.FC<{
  /** The password is verified against the server backup instead of typed twice. */
  verifyWithServer: boolean;
  onDone?: (exported: ExportedBackup) => void;
  onCancel?: () => void;
}> = ({ verifyWithServer, onDone, onCancel }) => {
  const [password, setPassword] = useState('');
  const [confirmation, setConfirmation] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<ExportedBackup | null>(null);

  const invalid = verifyWithServer ? masterPasswordIssue(password) : masterPasswordIssue(password, confirmation);

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (invalid) return;
    setBusy(true);
    setError(null);
    try {
      const exported = await vaultExportTkb(password, verifyWithServer ? undefined : confirmation);
      setResult(exported);
      onDone?.(exported);
    } catch (err) {
      setError(describeError(err));
    } finally {
      setPassword('');
      setConfirmation('');
      setBusy(false);
    }
  };

  if (result) {
    return (
      <div className={panel}>
        <p className="text-xs text-emerald-300 flex items-center gap-1.5">
          <CheckCircle2 className="w-4 h-4" /> Respaldo guardado
        </p>
        <p className="text-xs text-slate-300 font-mono break-all select-all">{result.path}</p>
        <p className="text-[11px] text-slate-400">Llave {shortFingerprint(result.public_key_fingerprint)}</p>
        <p className="text-[11px] text-amber-300">{SAFE_KEEPING_WARNING}</p>
        {onCancel && (
          <button onClick={onCancel} className={secondary}>
            Cerrar
          </button>
        )}
      </div>
    );
  }

  return (
    <form onSubmit={submit} className={panel}>
      <h4 className="text-sm font-semibold text-white flex items-center gap-2">
        <Download className="w-4 h-4 text-indigo-400" /> Exportar respaldo (.tkb)
      </h4>
      <p className="text-xs text-slate-400">
        {verifyWithServer
          ? 'Ingresa tu Master Password: se verifica con tu respaldo del servidor y se usa para cifrar el archivo (Argon2id + AES-256-GCM, en este equipo).'
          : 'Elige una Master Password para cifrar el archivo (Argon2id + AES-256-GCM, en este equipo).'}
      </p>
      <MasterPasswordFields
        password={password}
        onPasswordChange={setPassword}
        confirmation={verifyWithServer ? undefined : confirmation}
        onConfirmationChange={setConfirmation}
        disabled={busy}
      />
      <ErrorLine message={error} />
      <div className="flex justify-end gap-2">
        {onCancel && (
          <button type="button" onClick={onCancel} className={secondary}>
            Cancelar
          </button>
        )}
        <button type="submit" disabled={busy || !!invalid} className={primary}>
          {busy ? 'Cifrando…' : 'Guardar archivo .tkb'}
        </button>
      </div>
    </form>
  );
};

/** RF-ADMIN-11-4: zero-knowledge backup on the server (option B on a new device). */
export const ServerBackupPanel: React.FC<{
  enabled: boolean;
  onDone?: () => void;
  onCancel?: () => void;
}> = ({ enabled, onDone, onCancel }) => {
  const [password, setPassword] = useState('');
  const [confirmation, setConfirmation] = useState('');
  const [replaceAck, setReplaceAck] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [done, setDone] = useState(false);
  const invalid = masterPasswordIssue(password, confirmation) || (enabled && !replaceAck ? 'confirm' : null);

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (invalid) return;
    setBusy(true);
    setError(null);
    try {
      await vaultSetupServerBackup(password, confirmation);
      setDone(true);
      onDone?.();
    } catch (err) {
      setError(describeError(err));
    } finally {
      setPassword('');
      setConfirmation('');
      setBusy(false);
    }
  };

  if (done) {
    return (
      <div className={panel}>
        <p className="text-xs text-emerald-300 flex items-center gap-1.5">
          <CheckCircle2 className="w-4 h-4" /> Respaldo cifrado guardado en el servidor
        </p>
        <p className="text-[11px] text-slate-400">
          En un equipo nuevo podrás recuperar la llave con tu Master Password. El servidor no puede descifrarlo.
        </p>
        {onCancel && (
          <button onClick={onCancel} className={secondary}>
            Cerrar
          </button>
        )}
      </div>
    );
  }

  return (
    <form onSubmit={submit} className={panel}>
      <h4 className="text-sm font-semibold text-white flex items-center gap-2">
        <Server className="w-4 h-4 text-indigo-400" /> {enabled ? 'Reemplazar respaldo en el servidor' : 'Respaldo cifrado en el servidor'}
      </h4>
      <p className="text-xs text-slate-400">
        La llave se cifra en este equipo con tu Master Password; el servidor solo guarda el resultado cifrado y nunca ve la
        contraseña.
      </p>
      <MasterPasswordFields
        password={password}
        onPasswordChange={setPassword}
        confirmation={confirmation}
        onConfirmationChange={setConfirmation}
        disabled={busy}
      />
      {enabled && (
        <label className="flex items-start gap-2 text-xs text-amber-300">
          <input type="checkbox" checked={replaceAck} onChange={e => setReplaceAck(e.target.checked)} className="mt-0.5" />
          Entiendo que la Master Password anterior deja de servir para recuperar la llave desde el servidor.
        </label>
      )}
      <ErrorLine message={error} />
      <div className="flex justify-end gap-2">
        {onCancel && (
          <button type="button" onClick={onCancel} className={secondary}>
            Cancelar
          </button>
        )}
        <button type="submit" disabled={busy || !!invalid} className={primary}>
          {busy ? 'Cifrando…' : 'Guardar en el servidor'}
        </button>
      </div>
    </form>
  );
};

/** Deletes the server backup (option B stops existing). */
export const DeleteServerBackupButton: React.FC<{ onDone: () => void }> = ({ onDone }) => {
  const [error, setError] = useState<string | null>(null);
  const remove = async () => {
    if (!confirm('¿Eliminar el respaldo del servidor? Ya no podrás recuperar la llave con tu Master Password en un equipo nuevo.')) return;
    try {
      await vaultDeleteServerBackup();
      onDone();
    } catch (err) {
      setError(describeError(err));
    }
  };
  return (
    <span className="inline-flex flex-col">
      <button onClick={remove} className="px-3 py-1.5 bg-rose-950/60 hover:bg-rose-900 text-rose-300 text-xs rounded-lg flex items-center gap-1.5">
        <Trash2 className="w-3.5 h-3.5" /> Eliminar respaldo del servidor
      </button>
      {error && <span className="text-[11px] text-rose-300 mt-1">{error}</span>}
    </span>
  );
};

/** A — "Tengo archivo de respaldo (.tkb)" (RF-ADMIN-11-5). */
export const ImportBackupPanel: React.FC<{
  replaceLocal: boolean;
  onDone: (installed: KeyInstalled) => void;
  onCancel?: () => void;
}> = ({ replaceLocal, onDone, onCancel }) => {
  const [file, setFile] = useState<File | null>(null);
  const [password, setPassword] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const fileIssue = backupFileIssue(file);

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!file || fileIssue || !password) return;
    setBusy(true);
    setError(null);
    try {
      const bytes = new Uint8Array(await file.arrayBuffer());
      onDone(await vaultImportBackup(bytes, password, replaceLocal));
    } catch (err) {
      setError(describeError(err));
    } finally {
      setPassword('');
      setBusy(false);
    }
  };

  return (
    <form onSubmit={submit} className={panel}>
      <h4 className="text-sm font-semibold text-white flex items-center gap-2">
        <Upload className="w-4 h-4 text-sky-400" /> Restaurar desde archivo
      </h4>
      <p className="text-xs text-slate-400">Archivo .tkb (o .vault de versiones anteriores) y la Master Password con que se creó.</p>
      <input
        type="file"
        accept=".tkb,.vault,.json"
        onChange={e => setFile(e.target.files?.[0] ?? null)}
        className="block w-full text-xs text-slate-400 file:mr-3 file:py-1.5 file:px-3 file:rounded-lg file:border-0 file:bg-slate-700 file:text-slate-200"
      />
      {file && fileIssue && <p className="text-[11px] text-rose-400">{fileIssue}</p>}
      <MasterPasswordFields password={password} onPasswordChange={setPassword} disabled={busy} />
      <ErrorLine message={error} />
      <div className="flex justify-end gap-2">
        {onCancel && (
          <button type="button" onClick={onCancel} className={secondary}>
            Cancelar
          </button>
        )}
        <button type="submit" disabled={busy || !file || !!fileIssue || !password} className={primary}>
          {busy ? 'Descifrando…' : replaceLocal ? 'Reemplazar llave local' : 'Restaurar llave'}
        </button>
      </div>
    </form>
  );
};

/** B — "Recuperar con Master Password" from the server backup (RF-ADMIN-11-6). */
export const RecoverFromServerPanel: React.FC<{
  replaceLocal: boolean;
  onDone: (installed: KeyInstalled) => void;
  onCancel?: () => void;
}> = ({ replaceLocal, onDone, onCancel }) => {
  const [password, setPassword] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!password) return;
    setBusy(true);
    setError(null);
    try {
      onDone(await vaultRecoverFromServer(password, replaceLocal));
    } catch (err) {
      setError(describeError(err));
    } finally {
      setPassword('');
      setBusy(false);
    }
  };

  return (
    <form onSubmit={submit} className={panel}>
      <h4 className="text-sm font-semibold text-white flex items-center gap-2">
        <Server className="w-4 h-4 text-sky-400" /> Recuperar con Master Password
      </h4>
      <p className="text-xs text-slate-400">
        Se descarga tu respaldo cifrado (queda registrado en la auditoría) y se descifra en este equipo.
      </p>
      <MasterPasswordFields password={password} onPasswordChange={setPassword} autoFocus disabled={busy} />
      <ErrorLine message={error} />
      <div className="flex justify-end gap-2">
        {onCancel && (
          <button type="button" onClick={onCancel} className={secondary}>
            Cancelar
          </button>
        )}
        <button type="submit" disabled={busy || !password} className={primary}>
          {busy ? 'Descifrando…' : 'Recuperar llave'}
        </button>
      </div>
    </form>
  );
};

/** C — "Generar nueva llave": destructive, double confirmation (RF-ADMIN-11-7, SEC-15). */
export const GenerateNewKeyPanel: React.FC<{
  onDone: (installed: KeyInstalled) => void;
  onCancel?: () => void;
}> = ({ onDone, onCancel }) => {
  const [ack, setAck] = useState(false);
  const [phrase, setPhrase] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const valid = newKeyConfirmationValid(ack, phrase);

  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!valid) return;
    setBusy(true);
    setError(null);
    try {
      onDone(await vaultGenerateNewKey(ack, phrase));
    } catch (err) {
      setError(describeError(err));
    } finally {
      setBusy(false);
    }
  };

  return (
    <form onSubmit={submit} className="p-4 bg-rose-950/30 border border-rose-800/60 rounded-xl space-y-3">
      <h4 className="text-sm font-semibold text-rose-200 flex items-center gap-2">
        <KeyRound className="w-4 h-4" /> Generar nueva llave (acción destructiva)
      </h4>
      <ul className="text-xs text-rose-200/90 list-disc pl-5 space-y-1">
        <li>Los datos históricos cifrados con la llave anterior serán ilegibles sin esa llave.</li>
        <li>Todos tus trabajadores quedan desvinculados y deberán vincularse de nuevo con un código nuevo.</li>
        <li>El respaldo del servidor de la llave anterior se elimina.</li>
      </ul>
      <label className="flex items-start gap-2 text-xs text-rose-100">
        <input type="checkbox" checked={ack} onChange={e => setAck(e.target.checked)} className="mt-0.5" />
        Entiendo que los datos históricos cifrados con la llave anterior serán ilegibles y que todos mis trabajadores
        quedarán desvinculados.
      </label>
      <label className="block text-xs text-rose-100">
        Escribe <span className="font-mono font-semibold">{NEW_KEY_PHRASE}</span> para confirmar:
        <input
          value={phrase}
          onChange={e => setPhrase(e.target.value)}
          className="mt-1 w-full bg-slate-950 border border-rose-800 rounded-lg px-3 py-2 text-xs text-white font-mono focus:outline-none focus:border-rose-500"
        />
      </label>
      <ErrorLine message={error} />
      <div className="flex justify-end gap-2">
        {onCancel && (
          <button type="button" onClick={onCancel} className={secondary}>
            Cancelar
          </button>
        )}
        <button
          type="submit"
          disabled={busy || !valid}
          className="px-4 py-2 bg-rose-600 hover:bg-rose-500 text-white text-xs font-semibold rounded-lg disabled:opacity-40"
        >
          {busy ? 'Generando…' : 'Generar nueva llave'}
        </button>
      </div>
    </form>
  );
};
