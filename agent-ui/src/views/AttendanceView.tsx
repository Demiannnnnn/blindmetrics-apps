import React from 'react';
import { Clock } from 'lucide-react';

export const AttendanceView: React.FC = () => {
  return (
    <div className="p-6 select-none max-w-lg">
      <div className="flex items-center space-x-2 mb-4">
        <Clock className="w-5 h-5 text-accent" />
        <h1 className="text-base font-semibold text-fg">Asistencia</h1>
      </div>

      <div className="bg-surface border border-default rounded-card p-6 shadow-sm">
        <h2 className="text-sm font-semibold text-fg mb-1">
          Disponible próximamente
        </h2>
        <p className="text-xs text-muted leading-relaxed mb-4">
          El módulo legal de registro de inicio y fin de jornada (con verificación de PIN personal y emisión de comprobantes certificados según R38) estará disponible en la próxima versión.
        </p>

        <div className="p-3 bg-surface-subtle border border-default rounded-control text-xs text-muted flex items-center space-x-2">
          <span className="w-2 h-2 rounded-full bg-zinc-400" aria-hidden="true" />
          <span>Sin marcaciones registradas aún en este equipo.</span>
        </div>
      </div>
    </div>
  );
};
