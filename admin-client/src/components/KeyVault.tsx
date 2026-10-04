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
    `px-3 py-1.5 rounded-control text-xs font-medium flex items-center gap-1.5 transition ${
 active ? 'bg-accent text-accent-fg' : 'bg-surface-subtle hover:bg-strong text-fg'
 } disabled:opacity-40`;

  return (
    <div className="bg-surface-subtle border border-default rounded-card p-5 space-y-4">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-lg font-semibold text-fg flex items-center gap-2">
            <Key className="w-5 h-5 text-accent" /> Bóveda: llave maestra y respaldos
          </h2>
          <p className="text-xs text-muted">
            Argon2id + AES-256-GCM en este equipo. El servidor nunca ve tu Master Password ni la llave en claro.
          </p>
        </div>
        <button onClick={refresh} className="p-2 bg-surface-subtle hover:bg-strong rounded-control" title="Actualizar">
          <RefreshCw className="w-4 h-4 text-fg" />
        </button>
      </div>

      {error && <p className="text-xs text-danger">{error}</p>}
      {notice && (
        <p className="text-xs text-success flex items-center gap-1.5">
          <CheckCircle2 className="w-4 h-4" /> {notice}
        </p>
      )}

      {status && (
        <div className="grid grid-cols-1 md:grid-cols-3 gap-3 text-xs">
          <div className="p-3 bg-surface rounded-control border border-default">
            <p className="text-muted mb-1">Llave en este equipo</p>
            <p className="font-mono text-accent">{shortFingerprint(status.local_fingerprint)}</p>
            <p className={ready ? 'text-success mt-1' : 'text-warning mt-1'}>
              {ready ? 'Coincide con la registrada' : status.key_state === 'MISMATCH' ? 'Distinta a la registrada' : 'Sin llave válida'}
            </p>
          </div>
          <div className="p-3 bg-surface rounded-control border border-default">
            <p className="text-muted mb-1">Llave registrada en el servidor</p>
            <p className="font-mono text-accent">{shortFingerprint(status.registered_fingerprint)}</p>
          </div>
          <div className="p-3 bg-surface rounded-control border border-default">
            <p className="text-muted mb-1">Respaldo en el servidor</p>
            {backup?.key_backup_enabled ? (
              <p className="text-success">
                Activo{backup.updated_at ? ` · ${new Date(backup.updated_at).toLocaleDateString()}` : ''}
              </p>
            ) : (
              <p className="text-warning flex items-center gap-1">
                <AlertCircle className="w-3.5 h-3.5" /> No configurado
              </p>
            )}
          </div>
        </div>
      )}

      {status && !backup?.key_backup_enabled && ready && (
        <p className="text-xs text-danger bg-danger-subtle border border-danger rounded-control p-2.5">
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
          <p className="text-xs text-muted">Este equipo ya tiene la llave registrada; no necesitas importar un respaldo.</p>
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
