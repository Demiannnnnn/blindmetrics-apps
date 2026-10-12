import React, { useState } from 'react';
import { User, Download, Trash2, CheckCircle2 } from 'lucide-react';
import * as bridge from '../api/tauri-bridge';
import type { DataDeletionDto, DataRequestDto } from '../api/types';
import { Modal } from '../components/Modal';

export const MyDataView: React.FC = () => {
  const [loading, setLoading] = useState<boolean>(false);
  const [dataResult, setDataResult] = useState<DataRequestDto | null>(null);
  const [deletionResult, setDeletionResult] = useState<DataDeletionDto | null>(null);
  const [showDeletionConfirm, setShowDeletionConfirm] = useState<boolean>(false);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);

  const handleRequestCopy = async () => {
    setLoading(true);
    setErrorMsg(null);
    setDataResult(null);
    try {
      const res = await bridge.requestData();
      setDataResult(res);
    } catch (err: unknown) {
      const msg = err && typeof err === 'object' && 'message' in err ? String(err.message) : 'Error al solicitar copia de datos';
      setErrorMsg(msg);
    } finally {
      setLoading(false);
    }
  };

  const handleRequestDeletion = async () => {
    setLoading(true);
    setErrorMsg(null);
    setDeletionResult(null);
    setShowDeletionConfirm(false);
    try {
      const res = await bridge.requestDeletion();
      setDeletionResult(res);
    } catch (err: unknown) {
      const msg = err && typeof err === 'object' && 'message' in err ? String(err.message) : 'Error al solicitar eliminación de datos';
      setErrorMsg(msg);
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="p-6 select-none max-w-lg">
      <div className="flex items-center space-x-2 mb-4">
        <User className="w-5 h-5 text-accent" />
        <h1 className="text-base font-semibold text-fg">Mis datos</h1>
      </div>

      <div className="space-y-4">
        {errorMsg && (
          <div role="alert" aria-live="polite" className="p-2.5 bg-red-50 dark:bg-red-950/40 border border-red-200 rounded text-xs text-danger">
            {errorMsg}
          </div>
        )}

        <div className="bg-surface border border-default rounded-card p-4 shadow-sm">
          <h2 className="text-xs font-semibold text-fg mb-1">
            Derecho de acceso y portabilidad
          </h2>
          <p className="text-xs text-muted mb-3 leading-relaxed">
            Puedes solicitar una copia de todos los datos personales y de telemetría recopilados sobre tu actividad.
          </p>

          {dataResult ? (
            <div className="p-3 bg-emerald-50 dark:bg-emerald-950/40 border border-emerald-200 dark:border-emerald-800/60 rounded text-xs text-emerald-900 dark:text-emerald-200 space-y-1 mb-2">
              <div className="flex items-center space-x-1.5 font-medium">
                <CheckCircle2 className="w-4 h-4 text-emerald-600 shrink-0" />
                <span>Solicitud de copia registrada</span>
              </div>
              <div className="text-[11px] font-mono">ID: {dataResult.request_id}</div>
              <div className="text-[11px]">Estado: {dataResult.status}</div>
              {dataResult.estimated_completion && (
                <div className="text-[11px]">Plazo estimado: {dataResult.estimated_completion}</div>
              )}
            </div>
          ) : (
            <button
              type="button"
              onClick={handleRequestCopy}
              disabled={loading}
              className="h-9 px-3 rounded-control border border-default bg-surface hover:bg-surface-subtle font-medium text-xs text-fg flex items-center space-x-1.5 transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent"
            >
              <Download className="w-3.5 h-3.5" />
              <span>Solicitar copia de mis datos</span>
            </button>
          )}
        </div>

        <div className="bg-surface border border-default rounded-card p-4 shadow-sm">
          <h2 className="text-xs font-semibold text-fg mb-1">
            Derecho de supresión y eliminación
          </h2>
          <p className="text-xs text-muted mb-3 leading-relaxed">
            Puedes solicitar la eliminación de los registros de telemetría que no estén sujetos a conservación legal.
          </p>

          {deletionResult ? (
            <div className="p-3 bg-emerald-50 dark:bg-emerald-950/40 border border-emerald-200 dark:border-emerald-800/60 rounded text-xs text-emerald-900 dark:text-emerald-200 space-y-1 mb-2">
              <div className="flex items-center space-x-1.5 font-medium">
                <CheckCircle2 className="w-4 h-4 text-emerald-600 shrink-0" />
                <span>Solicitud de supresión registrada</span>
              </div>
              <div className="text-[11px] font-mono">ID: {deletionResult.request_id}</div>
              <div className="text-[11px]">Estado: {deletionResult.status}</div>
              {deletionResult.message && <div className="text-[11px]">{deletionResult.message}</div>}
            </div>
          ) : (
            <button
              type="button"
              onClick={() => setShowDeletionConfirm(true)}
              disabled={loading}
              className="h-9 px-3 rounded-control border border-danger/40 text-danger hover:bg-red-50 dark:hover:bg-red-950/30 font-medium text-xs flex items-center space-x-1.5 transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-danger"
            >
              <Trash2 className="w-3.5 h-3.5" />
              <span>Solicitar eliminación de datos</span>
            </button>
          )}
        </div>

        <div className="p-3 bg-surface-subtle border border-default rounded-control text-xs text-muted leading-relaxed">
          <span className="font-semibold text-fg">Nota legal: </span>
          Los registros de asistencia se conservan 5 años por obligación legal (R38 art. 22.1) y no se eliminan antes.
        </div>
      </div>

      <Modal
        title="¿Solicitar eliminación de datos?"
        isOpen={showDeletionConfirm}
        onClose={() => setShowDeletionConfirm(false)}
      >
        <div className="space-y-4">
          <p className="text-xs text-muted leading-relaxed">
            Se enviará la solicitud al administrador. La telemetría recopilada será suprimida conforme al procedimiento legal.
          </p>
          <div className="flex space-x-2 pt-2">
            <button
              type="button"
              onClick={handleRequestDeletion}
              disabled={loading}
              className="flex-1 h-9 rounded-control bg-danger text-white hover:bg-danger-hover font-medium text-xs transition-colors focus-visible:outline-none"
            >
              {loading ? 'Enviando…' : 'Confirmar solicitud'}
            </button>
            <button
              type="button"
              onClick={() => setShowDeletionConfirm(false)}
              disabled={loading}
              className="flex-1 h-9 rounded-control border border-default bg-surface text-fg hover:bg-surface-subtle font-medium text-xs transition-colors focus-visible:outline-none"
            >
              Cancelar
            </button>
          </div>
        </div>
      </Modal>
    </div>
  );
};
