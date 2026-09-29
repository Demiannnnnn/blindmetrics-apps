import React, { useState, useEffect } from 'react';
import {
  ShieldCheck,
  Lock,
  FileText,
  UserCheck,
  Key,
  Download,
  Upload,
  AlertCircle,
  CheckCircle2,
} from 'lucide-react';
import {
  KeyStatus,
  keysStatus,
  adminExportBackup,
  adminImportBackup,
} from '../api/tauri-bridge';

export const ConsentSettings: React.FC = () => {
  const [keyStatus, setKeyStatus] = useState<KeyStatus | null>(null);
  const [loading, setLoading] = useState(false);
  const [feedback, setFeedback] = useState<{ type: 'success' | 'error'; message: string } | null>(
    null
  );

  // Export state
  const [exportPassphrase, setExportPassphrase] = useState('');
  const [showExportModal, setShowExportModal] = useState(false);

  // Import state
  const [importPassphrase, setImportPassphrase] = useState('');
  const [importFile, setImportFile] = useState<File | null>(null);
  const [showImportModal, setShowImportModal] = useState(false);

  const refreshKeyStatus = async () => {
    try {
      const status = await keysStatus();
      setKeyStatus(status);
    } catch (err) {
      console.warn('Failed to fetch keys status:', err);
    }
  };

  useEffect(() => {
    refreshKeyStatus();
  }, []);

  const handleExportBackup = async () => {
    if (exportPassphrase.length < 12) {
      setFeedback({
        type: 'error',
        message: 'La frase de paso debe tener al menos 12 caracteres (RF-ADMIN-01-3).',
      });
      return;
    }

    setLoading(true);
    setFeedback(null);
    try {
      const backupBytes = await adminExportBackup(exportPassphrase);
      const blob = new Blob([backupBytes as unknown as BlobPart], { type: 'application/json' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `telemetry-admin-master-key-${new Date().toISOString().split('T')[0]}.vault`;
      document.body.appendChild(a);
      a.click();
      document.body.removeChild(a);
      URL.revokeObjectURL(url);

      setFeedback({
        type: 'success',
        message: 'Respaldo cifrado exportado exitosamente con AES-256-GCM y PBKDF2.',
      });
      setExportPassphrase('');
      setShowExportModal(false);
    } catch (err: any) {
      setFeedback({
        type: 'error',
        message: `Error al exportar respaldo: ${err.message || err}`,
      });
    } finally {
      setLoading(false);
    }
  };

  const handleImportBackup = async () => {
    if (!importFile) {
      setFeedback({ type: 'error', message: 'Selecciona un archivo .vault para importar.' });
      return;
    }
    if (importPassphrase.length < 12) {
      setFeedback({
        type: 'error',
        message: 'La frase de paso debe tener al menos 12 caracteres.',
      });
      return;
    }

    setLoading(true);
    setFeedback(null);
    try {
      const arrayBuffer = await importFile.arrayBuffer();
      const uint8Array = new Uint8Array(arrayBuffer);
      await adminImportBackup(uint8Array, importPassphrase);

      setFeedback({
        type: 'success',
        message: 'Clave maestra restaurada e importada en el OS Keystore exitosamente.',
      });
      setImportPassphrase('');
      setImportFile(null);
      setShowImportModal(false);
      await refreshKeyStatus();
    } catch (err: any) {
      setFeedback({
        type: 'error',
        message: `Error al importar respaldo: ${err.message || 'Contraseña incorrecta o archivo corrupto'}`,
      });
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="space-y-6">
      {/* Privacy and Compliance Principles */}
      <div className="bg-slate-800/80 border border-slate-700/80 rounded-xl p-5 shadow-lg space-y-6">
        <div>
          <h2 className="text-lg font-semibold text-white flex items-center gap-2">
            <ShieldCheck className="w-5 h-5 text-indigo-400" />
            Políticas de Privacidad, Consentimientos & Cumplimiento Normativo
          </h2>
          <p className="text-xs text-slate-400">
            Parámetros corporativos auditables según GDPR y Ley Chilena 19.628 / 21.719
          </p>
        </div>

        <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
          <div className="p-4 bg-slate-900/50 rounded-xl border border-slate-700/60 space-y-2">
            <div className="flex items-center gap-2 text-indigo-400 text-sm font-semibold">
              <Lock className="w-4 h-4" />
              Zero-Knowledge Cryptographic Core
            </div>
            <p className="text-xs text-slate-300 leading-relaxed">
              El servidor en PostgreSQL almacena únicamente blobs binarios cifrados. La clave
              privada maestra reside exclusivamente en el OS Keystore de esta estación de trabajo.
            </p>
          </div>

          <div className="p-4 bg-slate-900/50 rounded-xl border border-slate-700/60 space-y-2">
            <div className="flex items-center gap-2 text-emerald-400 text-sm font-semibold">
              <UserCheck className="w-4 h-4" />
              Soberanía y Revocación en 1-Clic
            </div>
            <p className="text-xs text-slate-300 leading-relaxed">
              Los trabajadores conservan el derecho inalienable de desactivar cualquiera de sus
              consentimientos desde su interfaz egui en tiempo real.
            </p>
          </div>

          <div className="p-4 bg-slate-900/50 rounded-xl border border-slate-700/60 space-y-2">
            <div className="flex items-center gap-2 text-sky-400 text-sm font-semibold">
              <FileText className="w-4 h-4" />
              Minimización de Red & Archivos
            </div>
            <p className="text-xs text-slate-300 leading-relaxed">
              El agente jamás recolecta URLs completas, argumentos GET o cuerpos de peticiones HTTP.
              La auditoría de archivos monitorea únicamente metadatos (crear, renombrar, eliminar).
            </p>
          </div>

          <div className="p-4 bg-slate-900/50 rounded-xl border border-slate-700/60 space-y-2">
            <div className="flex items-center gap-2 text-amber-400 text-sm font-semibold">
              <ShieldCheck className="w-4 h-4" />
              Geolocalización FIELD Estricta
            </div>
            <p className="text-xs text-slate-300 leading-relaxed">
              El hardware GPS solo se activa si el trabajador es de campo y se encuentra dentro de su
              franja horaria pactada. Fuera de turno, se suspende automáticamente por software.
            </p>
          </div>
        </div>
      </div>

      {/* OS Keystore Vault & Master Key Backup (RF-ADMIN-01) */}
      <div className="bg-slate-800/80 border border-slate-700/80 rounded-xl p-5 shadow-lg space-y-5">
        <div className="flex items-center justify-between">
          <div>
            <h2 className="text-lg font-semibold text-white flex items-center gap-2">
              <Key className="w-5 h-5 text-indigo-400" />
              Bóveda del Sistema Operativo & Respaldo de Clave Maestra
            </h2>
            <p className="text-xs text-slate-400">
              Custodia local (Apple Keychain / Windows Credential Manager / Secret Service) y exportación cifrada
            </p>
          </div>

          <div className="flex items-center gap-2">
            <button
              onClick={() => {
                setShowExportModal(true);
                setShowImportModal(false);
                setFeedback(null);
              }}
              className="flex items-center gap-2 px-3 py-1.5 bg-indigo-600/80 hover:bg-indigo-600 text-white rounded-lg text-xs font-medium transition-colors"
            >
              <Download className="w-3.5 h-3.5" />
              Exportar Backup (.vault)
            </button>
            <button
              onClick={() => {
                setShowImportModal(true);
                setShowExportModal(false);
                setFeedback(null);
              }}
              className="flex items-center gap-2 px-3 py-1.5 bg-slate-700 hover:bg-slate-600 text-slate-200 rounded-lg text-xs font-medium transition-colors"
            >
              <Upload className="w-3.5 h-3.5" />
              Importar Backup
            </button>
          </div>
        </div>

        {feedback && (
          <div
            className={`p-3 rounded-lg border text-xs flex items-center gap-2 ${
              feedback.type === 'success'
                ? 'bg-emerald-950/40 border-emerald-500/50 text-emerald-300'
                : 'bg-rose-950/40 border-rose-500/50 text-rose-300'
            }`}
          >
            {feedback.type === 'success' ? (
              <CheckCircle2 className="w-4 h-4 shrink-0" />
            ) : (
              <AlertCircle className="w-4 h-4 shrink-0" />
            )}
            <span>{feedback.message}</span>
          </div>
        )}

        {/* Key Status Information */}
        <div className="p-4 bg-slate-900/60 rounded-xl border border-slate-700/60 grid grid-cols-1 md:grid-cols-3 gap-4">
          <div>
            <span className="text-xs text-slate-400 block mb-1">Estado de la Bóveda</span>
            {keyStatus?.has_local_key ? (
              <span className="inline-flex items-center gap-1.5 text-xs font-medium text-emerald-400 bg-emerald-950/50 px-2.5 py-1 rounded-md border border-emerald-500/30">
                <span className="w-2 h-2 rounded-full bg-emerald-400 animate-pulse" />
                K_admin_priv activa en OS Keystore
              </span>
            ) : (
              <span className="inline-flex items-center gap-1.5 text-xs font-medium text-amber-400 bg-amber-950/50 px-2.5 py-1 rounded-md border border-amber-500/30">
                <span className="w-2 h-2 rounded-full bg-amber-400" />
                Sin clave local detectada
              </span>
            )}
          </div>

          <div className="md:col-span-2">
            <span className="text-xs text-slate-400 block mb-1">Huella Criptográfica SHA-256 (Fingerprint)</span>
            <code className="text-xs font-mono text-indigo-300 bg-slate-950 px-2.5 py-1 rounded border border-slate-800 block truncate">
              {keyStatus?.local_fingerprint || 'No disponible'}
            </code>
          </div>
        </div>

        {/* Export Modal / Inline Section */}
        {showExportModal && (
          <div className="p-4 bg-indigo-950/30 border border-indigo-500/40 rounded-xl space-y-3">
            <h3 className="text-sm font-semibold text-white flex items-center gap-2">
              <Download className="w-4 h-4 text-indigo-400" />
              Exportar Respaldo Cifrado de K_admin_priv
            </h3>
            <p className="text-xs text-slate-300">
              La clave privada será cifrada con AES-256-GCM y una clave derivada mediante PBKDF2
              (100,000 iteraciones). Ingresa una frase de paso de al menos 12 caracteres:
            </p>
            <div className="flex gap-2">
              <input
                type="password"
                placeholder="Frase de paso segura (mínimo 12 caracteres)..."
                value={exportPassphrase}
                onChange={e => setExportPassphrase(e.target.value)}
                className="flex-1 bg-slate-900 border border-slate-700 rounded-lg px-3 py-1.5 text-xs text-white placeholder-slate-500 focus:outline-none focus:border-indigo-500"
              />
              <button
                onClick={handleExportBackup}
                disabled={loading || exportPassphrase.length < 12}
                className="px-4 py-1.5 bg-indigo-600 hover:bg-indigo-500 disabled:opacity-50 text-white rounded-lg text-xs font-medium"
              >
                {loading ? 'Cifrando...' : 'Descargar .vault'}
              </button>
              <button
                onClick={() => setShowExportModal(false)}
                className="px-3 py-1.5 bg-slate-700 hover:bg-slate-600 text-slate-300 rounded-lg text-xs"
              >
                Cancelar
              </button>
            </div>
          </div>
        )}

        {/* Import Modal / Inline Section */}
        {showImportModal && (
          <div className="p-4 bg-slate-900/80 border border-slate-700 rounded-xl space-y-3">
            <h3 className="text-sm font-semibold text-white flex items-center gap-2">
              <Upload className="w-4 h-4 text-sky-400" />
              Importar y Restaurar Respaldo (.vault)
            </h3>
            <p className="text-xs text-slate-300">
              Selecciona el archivo `.vault` previamente exportado e introduce su frase de paso
              para recuperar la clave privada en el OS Keystore:
            </p>
            <div className="space-y-2">
              <input
                type="file"
                accept=".vault,.json"
                onChange={e => {
                  if (e.target.files && e.target.files[0]) {
                    setImportFile(e.target.files[0]);
                  }
                }}
                className="block w-full text-xs text-slate-400 file:mr-4 file:py-1.5 file:px-3 file:rounded-lg file:border-0 file:text-xs file:font-semibold file:bg-slate-700 file:text-slate-200 hover:file:bg-slate-600 cursor-pointer"
              />
              <div className="flex gap-2">
                <input
                  type="password"
                  placeholder="Frase de paso del archivo .vault..."
                  value={importPassphrase}
                  onChange={e => setImportPassphrase(e.target.value)}
                  className="flex-1 bg-slate-950 border border-slate-700 rounded-lg px-3 py-1.5 text-xs text-white placeholder-slate-500 focus:outline-none focus:border-sky-500"
                />
                <button
                  onClick={handleImportBackup}
                  disabled={loading || !importFile || importPassphrase.length < 12}
                  className="px-4 py-1.5 bg-sky-600 hover:bg-sky-500 disabled:opacity-50 text-white rounded-lg text-xs font-medium"
                >
                  {loading ? 'Restaurando...' : 'Restaurar Bóveda'}
                </button>
                <button
                  onClick={() => setShowImportModal(false)}
                  className="px-3 py-1.5 bg-slate-700 hover:bg-slate-600 text-slate-300 rounded-lg text-xs"
                >
                  Cancelar
                </button>
              </div>
            </div>
          </div>
        )}
      </div>
    </div>
  );
};
