import React, { useState, useRef } from 'react';
import { KeyRound, QrCode, Upload, CheckCircle2 } from 'lucide-react';
import { useApp } from '../context/AppContext';
import {
  normalizeCode,
  qrImageFileIssue,
  isEnrollmentTampered,
  extractCodeFromQrPayload,
} from '../lib/helpers';
import * as bridge from '../api/tauri-bridge';

export const EnrollmentView: React.FC = () => {
  const { setEnrollmentStep, refreshSnapshot } = useApp();
  const [mode, setMode] = useState<'code' | 'qr'>('code');
  const [code, setCode] = useState<string>('');
  const [qrPayload, setQrPayload] = useState<string>('');
  const [loading, setLoading] = useState<boolean>(false);
  const [decodingImage, setDecodingImage] = useState<boolean>(false);
  const [successNotice, setSuccessNotice] = useState<string | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  const fileInputRef = useRef<HTMLInputElement>(null);

  const handleCodeChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    setCode(normalizeCode(e.target.value));
  };

  const handleImageFile = async (file: File) => {
    setErrorMessage(null);
    setSuccessNotice(null);
    const issue = qrImageFileIssue(file);
    if (issue) {
      setErrorMessage(issue);
      return;
    }

    setDecodingImage(true);
    try {
      const buffer = await file.arrayBuffer();
      const bytes = new Uint8Array(buffer);
      const decoded = await bridge.decodeEnrollmentQrImage(bytes);
      setQrPayload(decoded);
      const codeFound = extractCodeFromQrPayload(decoded);
      setSuccessNotice(
        codeFound
          ? `Código QR leído desde la imagen (${codeFound}). Listo para vincular.`
          : 'Código QR leído desde la imagen. Listo para vincular.'
      );
    } catch (err: unknown) {
      const msg =
        err && typeof err === 'object' && 'message' in err
          ? String((err as { message: unknown }).message)
          : 'No se pudo decodificar el código QR de la imagen.';
      setErrorMessage(msg);
    } finally {
      setDecodingImage(false);
    }
  };

  const handlePaste = async (e: React.ClipboardEvent) => {
    if (mode !== 'qr') return;
    const items = e.clipboardData?.items;
    if (!items) return;

    for (let i = 0; i < items.length; i++) {
      const item = items[i];
      if (item.type.startsWith('image/')) {
        const file = item.getAsFile();
        if (file) {
          e.preventDefault();
          await handleImageFile(file);
          return;
        }
      }
    }
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setErrorMessage(null);
    setLoading(true);

    try {
      const codeArg = mode === 'code' ? code.trim() : undefined;
      const qrArg = mode === 'qr' ? qrPayload.trim() : undefined;

      if (mode === 'code' && (!codeArg || codeArg.length < 14)) {
        setErrorMessage('Ingresa un código válido de 12 caracteres (XXXX-XXXX-XXXX).');
        setLoading(false);
        return;
      }

      if (mode === 'qr' && !qrArg) {
        setErrorMessage('Ingresa o pega el contenido del QR de vinculación.');
        setLoading(false);
        return;
      }

      const res = await bridge.enrollWorker(qrArg, codeArg);

      if (res.status === 'NEEDS_SAFETY_CODE') {
        setEnrollmentStep('verify', res.safety_code);
      } else {
        await refreshSnapshot();
      }
    } catch (err: unknown) {
      if (isEnrollmentTampered(err)) {
        setEnrollmentStep('tamper');
        return;
      }
      const msg =
        err && typeof err === 'object' && 'message' in err
          ? String((err as { message: unknown }).message)
          : 'Error al intentar vincular el computador';
      setErrorMessage(msg);
    } finally {
      setLoading(false);
    }
  };

  return (
    <div
      className="h-full w-full flex flex-col items-center justify-center p-6 select-none"
      onPaste={handlePaste}
    >
      <div className="w-full max-w-sm bg-surface border border-default rounded-card p-6 shadow-sm">
        <h1 className="text-base font-semibold text-fg mb-1">
          Vincula este computador con tu empleador
        </h1>
        <p className="text-xs text-muted mb-4">
          Tu empleador te entregó un código o un QR. Si no lo tienes, pídeselo.
        </p>

        <div className="flex border-b border-default mb-4">
          <button
            type="button"
            onClick={() => {
              setMode('code');
              setErrorMessage(null);
              setSuccessNotice(null);
            }}
            className={`pb-2 text-xs font-medium mr-4 flex items-center space-x-1.5 transition-colors focus-visible:outline-none ${
              mode === 'code'
                ? 'border-b-2 border-accent text-accent'
                : 'text-muted hover:text-fg'
            }`}
          >
            <KeyRound className="w-3.5 h-3.5" />
            <span>Código</span>
          </button>
          <button
            type="button"
            onClick={() => {
              setMode('qr');
              setErrorMessage(null);
              setSuccessNotice(null);
            }}
            className={`pb-2 text-xs font-medium flex items-center space-x-1.5 transition-colors focus-visible:outline-none ${
              mode === 'qr'
                ? 'border-b-2 border-accent text-accent'
                : 'text-muted hover:text-fg'
            }`}
          >
            <QrCode className="w-3.5 h-3.5" />
            <span>Payload QR</span>
          </button>
        </div>

        <form onSubmit={handleSubmit} className="space-y-4">
          {mode === 'code' ? (
            <div>
              <label
                htmlFor="enrollment-code"
                className="block text-xs font-medium text-fg mb-1"
              >
                Código de vinculación
              </label>
              <input
                id="enrollment-code"
                type="text"
                value={code}
                onChange={handleCodeChange}
                placeholder="A68K-2PZS-PC4H"
                autoComplete="off"
                spellCheck="false"
                maxLength={14}
                className="w-full h-9 px-3 rounded-control border border-strong bg-surface text-fg font-mono text-center tracking-wider text-sm focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent"
              />
              <p className="text-[11px] text-muted mt-1">
                Formato de 12 símbolos entregado por tu administrador.
              </p>
            </div>
          ) : (
            <div className="space-y-3">
              <input
                ref={fileInputRef}
                type="file"
                accept="image/png,image/jpeg,image/webp"
                onChange={async (e) => {
                  const files = e.target.files;
                  if (files && files.length > 0) {
                    await handleImageFile(files[0]);
                  }
                  e.target.value = '';
                }}
                className="sr-only"
                tabIndex={-1}
              />

              <button
                type="button"
                disabled={loading || decodingImage}
                onClick={() => fileInputRef.current?.click()}
                className="w-full h-10 px-3 rounded-control border border-dashed border-strong bg-surface-subtle hover:bg-surface text-xs text-muted hover:text-fg transition-colors disabled:opacity-50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent flex items-center justify-center space-x-2"
              >
                <Upload className="w-4 h-4 text-muted shrink-0" />
                <span>
                  {decodingImage
                    ? 'Leyendo imagen QR…'
                    : 'Seleccionar imagen o pegar (Ctrl+V / Cmd+V)'}
                </span>
              </button>

              {successNotice && (
                <div
                  role="status"
                  aria-live="polite"
                  className="p-2.5 bg-green-50 dark:bg-green-950/40 border border-green-200 dark:border-green-800/60 rounded-control text-xs text-green-900 dark:text-green-200 flex items-center space-x-2"
                >
                  <CheckCircle2 className="w-4 h-4 text-green-600 dark:text-green-400 shrink-0" />
                  <span>{successNotice}</span>
                </div>
              )}

              <div>
                <label
                  htmlFor="qr-payload"
                  className="block text-xs font-medium text-fg mb-1"
                >
                  Texto o base64 del QR
                </label>
                <textarea
                  id="qr-payload"
                  rows={3}
                  value={qrPayload}
                  onChange={(e) => {
                    setQrPayload(e.target.value);
                    setSuccessNotice(null);
                  }}
                  placeholder="Pega aquí el contenido del QR..."
                  className="w-full p-2.5 rounded-control border border-strong bg-surface text-fg font-mono text-xs focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent resize-none"
                />
              </div>
            </div>
          )}

          {errorMessage && (
            <div
              role="alert"
              aria-live="polite"
              className="p-2.5 bg-red-50 dark:bg-red-950/40 border border-red-200 dark:border-red-800/60 rounded-control text-xs text-red-900 dark:text-red-200"
            >
              {errorMessage}
            </div>
          )}

          <button
            type="submit"
            disabled={loading || decodingImage}
            className="w-full h-9 rounded-control bg-accent text-accent-fg hover:bg-accent-hover font-medium text-sm transition-colors disabled:opacity-50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent flex items-center justify-center"
          >
            {loading ? 'Vinculando…' : 'Vincular computador'}
          </button>
        </form>
      </div>
    </div>
  );
};
