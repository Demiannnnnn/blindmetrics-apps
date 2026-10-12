import React from 'react';
import { FileText } from 'lucide-react';

export const RecordsView: React.FC = () => {
  return (
    <div className="p-6 select-none max-w-lg">
      <div className="flex items-center space-x-2 mb-4">
        <FileText className="w-5 h-5 text-accent" />
        <h1 className="text-base font-semibold text-fg">Registros</h1>
      </div>

      <div className="bg-surface border border-default rounded-card p-6 shadow-sm">
        <h2 className="text-sm font-semibold text-fg mb-1">
          Disponible próximamente
        </h2>
        <p className="text-xs text-muted leading-relaxed mb-4">
          La consulta histórica de marcaciones de hasta 5 años, comprobantes con hash y revisión de correcciones según la normativa R38 estará disponible en la próxima versión.
        </p>

        <div className="p-3 bg-surface-subtle border border-default rounded-control text-xs text-muted">
          No hay registros disponibles en el almacenamiento local.
        </div>
      </div>
    </div>
  );
};
