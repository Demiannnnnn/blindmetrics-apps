import React, { useEffect, useState } from 'react';
import { Key, AlertCircle, CheckCircle2, Download, Server, Upload, ShieldAlert, RefreshCw } from 'lucide-react';
import type { VaultStatus } from '../api/types';
import { vaultStatus } from '../api/tauri-bridge';
import { describeError } from '../lib/errors';
import { shortFingerprint } from '../lib/vault';
import {
  DeleteServerBackupButton,
  ExportTkbPanel,
  GenerateNewKeyPanel,
  ImportBackupPanel,
  ServerBackupPanel,
} from './VaultPanels';

type Panel = 'export' | 'server' | 'import' | 'generate' | null;

/** "Bóveda": key status, `.tkb` export, server backup, import and rotation (TASK-ADMIN-11). */
export const KeyVault: React.FC<{ onKeyChanged?: () => void }> = ({ onKeyChanged }) => {
  const [status, setStatus] = useState<VaultStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [panel, setPanel] = useState<Panel>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const refresh = async () => {
    setError(null);
    try {
      setStatus(await vaultStatus());
    } catch (err) {
      setError(describeError(err));
    }
  };

  useEffect(() => {
    refresh();
  }, []);

  const backup = status?.server_backup;
  const ready = status?.key_state === 'OK';
  const button = (active: boolean) =>
    `px-3 py-1.5 rounded-lg text-xs font-medium flex items-center gap-1.5 transition ${
      active ? 'bg-indigo-600 text-white' : 'bg-slate-700 hover:bg-slate-600 text-slate-200'
    } disabled:opacity-40`;

  return (
    <div className="bg-slate-800/80 border border-slate-700/80 rounded-xl p-5 shadow-lg space-y-4">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-lg font-semibold text-white flex items-center gap-2">
            <Key className="w-5 h-5 text-indigo-400" /> Bóveda: llave maestra y respaldos
          </h2>
          <p className="text-xs text-slate-400">
            Argon2id + AES-256-GCM en este equipo. El servidor nunca ve tu Master Password ni la llave en claro.
          </p>
        </div>
        <button onClick={refresh} className="p-2 bg-slate-700 hover:bg-slate-600 rounded-lg" title="Actualizar">
          <RefreshCw className="w-4 h-4 text-slate-300" />
        </button>
      </div>

      {error && <p className="text-xs text-rose-300">{error}</p>}
      {notice && (
        <p className="text-xs text-emerald-300 flex items-center gap-1.5">
          <CheckCircle2 className="w-4 h-4" /> {notice}
        </p>
      )}

      {status && (
        <div className="grid grid-cols-1 md:grid-cols-3 gap-3 text-xs">
          <div className="p-3 bg-slate-900/60 rounded-lg border border-slate-700/60">
            <p className="text-slate-400 mb-1">Llave en este equipo</p>
            <p className="font-mono text-indigo-300">{shortFingerprint(status.local_fingerprint)}</p>
            <p className={ready ? 'text-emerald-400 mt-1' : 'text-amber-300 mt-1'}>
              {ready ? 'Coincide con la registrada' : status.key_state === 'MISMATCH' ? 'Distinta a la registrada' : 'Sin llave válida'}
            </p>
          </div>
          <div className="p-3 bg-slate-900/60 rounded-lg border border-slate-700/60">
            <p className="text-slate-400 mb-1">Llave registrada en el servidor</p>
            <p className="font-mono text-indigo-300">{shortFingerprint(status.registered_fingerprint)}</p>
          </div>
          <div className="p-3 bg-slate-900/60 rounded-lg border border-slate-700/60">
            <p className="text-slate-400 mb-1">Respaldo en el servidor</p>
            {backup?.key_backup_enabled ? (
              <p className="text-emerald-400">
                Activo{backup.updated_at ? ` · ${new Date(backup.updated_at).toLocaleDateString()}` : ''}
              </p>
            ) : (
              <p className="text-amber-300 flex items-center gap-1">
                <AlertCircle className="w-3.5 h-3.5" /> No configurado
              </p>
            )}
          </div>
        </div>
      )}

      {status && !backup?.key_backup_enabled && ready && (
        <p className="text-xs text-rose-300 bg-rose-950/40 border border-rose-800/60 rounded-lg p-2.5">
          Sin respaldo en el servidor: si pierdes este equipo y no tienes tu archivo .tkb, los datos históricos serán
          irrecuperables. Configura al menos un respaldo.
        </p>
      )}

      <div className="flex flex-wrap gap-2">
        <button disabled={!status?.has_local_key} onClick={() => setPanel(panel === 'export' ? null : 'export')} className={button(panel === 'export')}>
          <Download className="w-3.5 h-3.5" /> Exportar respaldo (.tkb)
        </button>
        <button disabled={!ready} onClick={() => setPanel(panel === 'server' ? null : 'server')} className={button(panel === 'server')}>
          <Server className="w-3.5 h-3.5" /> {backup?.key_backup_enabled ? 'Reemplazar respaldo del servidor' : 'Respaldar en el servidor'}
        </button>
        <button onClick={() => setPanel(panel === 'import' ? null : 'import')} className={button(panel === 'import')}>
          <Upload className="w-3.5 h-3.5" /> Importar respaldo
        </button>
        <button onClick={() => setPanel(panel === 'generate' ? null : 'generate')} className={button(panel === 'generate')}>
          <ShieldAlert className="w-3.5 h-3.5" /> Generar nueva llave
        </button>
        {backup?.key_backup_enabled && (
          <DeleteServerBackupButton
            onDone={() => {
              setNotice('Respaldo del servidor eliminado.');
              refresh();
            }}
          />
        )}
      </div>

      {panel === 'export' && (
        <ExportTkbPanel
          verifyWithServer={ready && !!backup?.key_backup_enabled}
          onCancel={() => setPanel(null)}
        />
      )}
      {panel === 'server' && (
        <ServerBackupPanel
          enabled={!!backup?.key_backup_enabled}
          onDone={refresh}
          onCancel={() => setPanel(null)}
        />
      )}
      {panel === 'import' &&
        (ready ? (
          <p className="text-xs text-slate-400">Este equipo ya tiene la llave registrada; no necesitas importar un respaldo.</p>
        ) : (
          <ImportBackupPanel
            replaceLocal={status?.key_state === 'MISMATCH'}
            onDone={() => {
              setPanel(null);
              setNotice('Llave restaurada en este equipo.');
              refresh();
              onKeyChanged?.();
            }}
            onCancel={() => setPanel(null)}
          />
        ))}
      {panel === 'generate' && (
        <GenerateNewKeyPanel
          onDone={installed => {
            setPanel(null);
            setNotice(
              `Nueva llave ${shortFingerprint(installed.public_key_fingerprint)} activa. ${installed.revoked_workers} trabajador(es) desvinculados. Crea un respaldo ahora.`,
            );
            refresh();
            onKeyChanged?.();
          }}
          onCancel={() => setPanel(null)}
        />
      )}
    </div>
  );
};
