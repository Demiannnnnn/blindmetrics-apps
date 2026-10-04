import React, { useState } from 'react';
import { AlertTriangle } from 'lucide-react';
import { WorkerItem, workersDelete } from '../api/tauri-bridge';

interface ConfirmDeleteWorkerModalProps {
  worker: WorkerItem;
  onClose: () => void;
  onDeleted: () => void;
}

/** RF-ADMIN-03-4: explicit confirmation (typed name) before the soft delete. */
export const ConfirmDeleteWorkerModal: React.FC<ConfirmDeleteWorkerModalProps> = ({ worker, onClose, onDeleted }) => {
  const [typed, setTyped] = useState('');
  const [deleting, setDeleting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const matches = typed.trim() === worker.name.trim();

  const handleDelete = async () => {
    setDeleting(true);
    setError(null);
    try {
      await workersDelete(worker.id);
      onDeleted();
    } catch (err: any) {
      setError(err?.message || String(err));
      setDeleting(false);
    }
  };

  return (
    <div className="fixed inset-0 z-50 bg-black/80 flex items-center justify-center p-4">
      <div className="bg-surface-subtle border border-danger rounded-card max-w-md w-full p-6 space-y-4 shadow-md">
        <div className="flex items-center gap-3">
          <div className="w-10 h-10 rounded-card bg-danger-subtle text-danger flex items-center justify-center">
            <AlertTriangle className="w-5 h-5" />
          </div>
          <h3 className="text-base font-semibold text-fg">Dar de baja a {worker.name}</h3>
        </div>
        <div className="text-xs text-fg space-y-1.5">
          <p>
            Las credenciales del trabajador se revocan de inmediato en el servidor y su equipo
            {worker.deviceHostname ? ` (${worker.deviceHostname})` : ''} deja de enviar telemetría.
          </p>
          <p className="text-muted">Escriba el nombre completo del trabajador para confirmar:</p>
        </div>
        <input
          type="text"
          value={typed}
          onChange={e => setTyped(e.target.value)}
          placeholder={worker.name}
          className="w-full bg-surface border border-default text-fg text-xs rounded-control px-3 py-2 focus:outline-none focus:border-danger"
        />
        {error && <div className="p-2.5 bg-danger-subtle border border-danger rounded-control text-xs text-danger">{error}</div>}
        <div className="flex justify-end gap-2">
          <button onClick={onClose} className="px-4 py-2 bg-surface-subtle text-fg rounded-control text-xs hover:bg-strong transition">
            Cancelar
          </button>
          <button
            onClick={handleDelete}
            disabled={!matches || deleting}
            className="px-4 py-2 bg-danger hover:bg-danger-hover text-accent-fg rounded-control text-xs font-semibold transition disabled:opacity-40"
          >
            {deleting ? 'Dando de baja…' : 'Dar de baja'}
          </button>
        </div>
      </div>
    </div>
  );
};
