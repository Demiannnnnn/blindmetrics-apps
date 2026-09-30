import React, { useState } from 'react';
import { KeyRound, ShieldAlert, FileKey, Server, Sparkles, LogOut, CheckCircle2 } from 'lucide-react';
import type { KeyInstalled, VaultStatus } from '../api/types';
import { keysRegisterOnServer, vaultCreateInitialKey } from '../api/tauri-bridge';
import { describeError } from '../lib/errors';
import { keyStateScreen, shortFingerprint } from '../lib/vault';
import {
  ExportTkbPanel,
  GenerateNewKeyPanel,
  ImportBackupPanel,
  RecoverFromServerPanel,
  ServerBackupPanel,
} from './VaultPanels';

interface KeyRecoveryWizardProps {
  status: VaultStatus;
  /** Re-reads the vault status (the parent shows the dashboard when the key is ready). */
  onResolved: () => void;
  onLogout: () => void;
}

type Option = 'A' | 'B' | 'C' | null;

/**
 * K1 (create) · K1b (register local key) · K2 (mismatch) · K3 (recover) — TASK-ADMIN-11 =
 * TASK-APP-02. Shown instead of the dashboard until this device holds the registered key.
 */
export const KeyRecoveryWizard: React.FC<KeyRecoveryWizardProps> = ({ status, onResolved, onLogout }) => {
  const screen = keyStateScreen(status.key_state);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [option, setOption] = useState<Option>(null);
  const [backupStep, setBackupStep] = useState<{ fingerprint: string; revoked: number } | null>(null);
  const [serverBackupDone, setServerBackupDone] = useState(false);
  const [fileBackupDone, setFileBackupDone] = useState(false);
  const [skipAck, setSkipAck] = useState(false);
  const [exportedCurrent, setExportedCurrent] = useState(false);
  const backupEnabled = !!status.server_backup?.key_backup_enabled;

  const run = async (action: () => Promise<void>) => {
    setBusy(true);
    setError(null);
    try {
      await action();
    } catch (err) {
      setError(describeError(err));
    } finally {
      setBusy(false);
    }
  };

  const toBackups = (installed: KeyInstalled) =>
    setBackupStep({ fingerprint: installed.public_key_fingerprint, revoked: installed.revoked_workers });

  const card = (active: boolean) =>
    `text-left p-4 rounded-xl border transition ${
      active ? 'border-indigo-500 bg-indigo-950/40' : 'border-slate-700 bg-slate-900/70 hover:border-slate-500'
    }`;

  const shell = (title: string, subtitle: string, children: React.ReactNode) => (
    <div className="min-h-screen bg-slate-950 text-slate-100 flex items-center justify-center p-6">
      <div className="max-w-3xl w-full bg-slate-900 border border-slate-800 rounded-2xl p-7 shadow-2xl space-y-5">
        <div className="flex items-start justify-between gap-4">
          <div>
            <h1 className="text-lg font-bold text-white flex items-center gap-2">
              <KeyRound className="w-5 h-5 text-indigo-400" />
              {title}
            </h1>
            <p className="text-xs text-slate-400 mt-1">{subtitle}</p>
          </div>
          <button onClick={onLogout} className="text-xs text-slate-400 hover:text-rose-300 flex items-center gap-1">
            <LogOut className="w-3.5 h-3.5" /> Salir
          </button>
        </div>
        {error && <p className="text-xs text-rose-300 bg-rose-950/40 border border-rose-800/60 rounded-lg p-2.5">{error}</p>}
        {children}
      </div>
    </div>
  );

  // Backups right after creating, registering or generating a key (K1 step 3).
  if (backupStep) {
    const anyBackup = serverBackupDone || fileBackupDone;
    return shell(
      'Respalda tu llave',
      `Llave ${shortFingerprint(backupStep.fingerprint)} lista en este equipo.`,
      <>
        {backupStep.revoked > 0 && (
          <p className="text-xs text-amber-300 bg-amber-950/30 border border-amber-800/50 rounded-lg p-2.5">
            Se desvincularon {backupStep.revoked} trabajador(es): genera un código nuevo para cada uno desde el tablero.
          </p>
        )}
        <p className="text-xs text-slate-300">
          Sin respaldo, perder este equipo significa perder el acceso a todos los datos. Crea al menos uno:
        </p>
        <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
          <ServerBackupPanel enabled={false} onDone={() => setServerBackupDone(true)} />
          <ExportTkbPanel verifyWithServer={false} onDone={() => setFileBackupDone(true)} />
        </div>
        {!anyBackup && (
          <label className="flex items-start gap-2 text-xs text-amber-300">
            <input type="checkbox" checked={skipAck} onChange={e => setSkipAck(e.target.checked)} className="mt-0.5" />
            Continuar sin respaldo (el tablero mostrará una alerta hasta que crees uno).
          </label>
        )}
        <div className="flex justify-end">
          <button
            disabled={!anyBackup && !skipAck}
            onClick={onResolved}
            className="px-4 py-2 bg-indigo-600 hover:bg-indigo-500 text-white text-xs font-semibold rounded-lg disabled:opacity-40"
          >
            Continuar al tablero
          </button>
        </div>
      </>,
    );
  }

  if (screen === 'CREATE') {
    return shell(
      'Crea la llave de tu organización',
      'Es la única forma de leer los datos de tus trabajadores. El servidor nunca la tiene.',
      <div className="space-y-4">
        <ul className="text-xs text-slate-300 list-disc pl-5 space-y-1">
          <li>La llave privada se genera y se guarda en el llavero de este sistema operativo.</li>
          <li>El servidor solo recibe la llave pública (para que los equipos de tus trabajadores cifren).</li>
          <li>A continuación crearás un respaldo protegido con una Master Password.</li>
        </ul>
        <button
          disabled={busy}
          onClick={() => run(async () => toBackups(await vaultCreateInitialKey()))}
          className="px-4 py-2 bg-indigo-600 hover:bg-indigo-500 text-white text-xs font-semibold rounded-lg disabled:opacity-50 flex items-center gap-1.5"
        >
          <Sparkles className="w-4 h-4" />
          {busy ? 'Generando…' : 'Crear llave'}
        </button>
      </div>,
    );
  }

  if (screen === 'REGISTER') {
    return shell(
      'Registra la llave de este equipo',
      `Este equipo tiene una llave (${shortFingerprint(status.local_fingerprint)}) y tu cuenta aún no tiene ninguna registrada.`,
      <button
        disabled={busy}
        onClick={() =>
          run(async () => {
            await keysRegisterOnServer();
            setBackupStep({ fingerprint: status.local_fingerprint ?? '', revoked: 0 });
          })
        }
        className="px-4 py-2 bg-indigo-600 hover:bg-indigo-500 text-white text-xs font-semibold rounded-lg disabled:opacity-50"
      >
        {busy ? 'Registrando…' : 'Registrar llave'}
      </button>,
    );
  }

  const replaceLocal = screen === 'MISMATCH';
  const options = (
    <>
      <div className="grid grid-cols-1 md:grid-cols-3 gap-3">
        <button onClick={() => setOption('A')} className={card(option === 'A')}>
          <FileKey className="w-5 h-5 text-sky-400 mb-2" />
          <p className="text-sm font-semibold text-white">A. Tengo archivo de respaldo (.tkb)</p>
          <p className="text-[11px] text-slate-400 mt-1">Importa el archivo y descífralo con tu Master Password.</p>
        </button>
        <button
          onClick={() => backupEnabled && setOption('B')}
          disabled={!backupEnabled}
          className={`${card(option === 'B')} disabled:opacity-40 disabled:cursor-not-allowed`}
          title={backupEnabled ? undefined : 'Tu cuenta no tiene un respaldo en el servidor'}
        >
          <Server className="w-5 h-5 text-sky-400 mb-2" />
          <p className="text-sm font-semibold text-white">B. Recuperar con Master Password</p>
          <p className="text-[11px] text-slate-400 mt-1">
            {backupEnabled ? 'Descarga tu respaldo cifrado del servidor.' : 'No disponible: tu cuenta no tiene respaldo en el servidor.'}
          </p>
        </button>
        <button onClick={() => setOption('C')} className={card(option === 'C')}>
          <ShieldAlert className="w-5 h-5 text-rose-400 mb-2" />
          <p className="text-sm font-semibold text-white">C. Generar nueva llave</p>
          <p className="text-[11px] text-slate-400 mt-1">Destructivo: datos históricos ilegibles y trabajadores desvinculados.</p>
        </button>
      </div>
      {option === 'A' && <ImportBackupPanel replaceLocal={replaceLocal} onDone={() => onResolved()} onCancel={() => setOption(null)} />}
      {option === 'B' && <RecoverFromServerPanel replaceLocal={replaceLocal} onDone={() => onResolved()} onCancel={() => setOption(null)} />}
      {option === 'C' && <GenerateNewKeyPanel onDone={toBackups} onCancel={() => setOption(null)} />}
    </>
  );

  if (screen === 'MISMATCH') {
    return shell(
      'La llave de este equipo no es la registrada',
      `Este equipo: ${shortFingerprint(status.local_fingerprint)} · Registrada: ${shortFingerprint(status.registered_fingerprint)}`,
      <div className="space-y-4">
        <p className="text-xs text-slate-300">
          La llave local podría descifrar datos antiguos. Antes de reemplazarla, guárdala en un archivo .tkb.
        </p>
        {exportedCurrent ? (
          <p className="text-xs text-emerald-300 flex items-center gap-1.5">
            <CheckCircle2 className="w-4 h-4" /> Llave actual exportada.
          </p>
        ) : (
          <ExportTkbPanel verifyWithServer={false} onDone={() => setExportedCurrent(true)} />
        )}
        {!exportedCurrent && (
          <label className="flex items-start gap-2 text-xs text-amber-300">
            <input type="checkbox" checked={exportedCurrent} onChange={e => setExportedCurrent(e.target.checked)} className="mt-0.5" />
            No necesito conservar la llave actual de este equipo.
          </label>
        )}
        {exportedCurrent && options}
      </div>,
    );
  }

  // RECOVER (K3): new device or reinstall.
  return shell(
    'Recupera tu llave en este equipo',
    `Tu cuenta tiene la llave ${shortFingerprint(status.registered_fingerprint)} registrada, pero este equipo no la tiene.`,
    <div className="space-y-4">{options}</div>,
  );
};
