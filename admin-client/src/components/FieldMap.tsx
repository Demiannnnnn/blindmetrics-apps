import React from 'react';
import { MapPin, ShieldAlert, Navigation } from 'lucide-react';

interface Waypoint {
  time: string;
  lat: number;
  lng: number;
  accuracy: number;
}

interface FieldMapProps {
  workerName: string;
  workerRole: 'OFFICE' | 'FIELD';
  workHours: { start: string; end: string };
}

const mockWaypoints: Waypoint[] = [
  { time: "08:45", lat: -33.4489, lng: -70.6693, accuracy: 12 },
  { time: "10:15", lat: -33.4520, lng: -70.6610, accuracy: 8 },
  { time: "12:30", lat: -33.4610, lng: -70.6550, accuracy: 15 },
  { time: "15:00", lat: -33.4580, lng: -70.6650, accuracy: 10 },
  { time: "17:15", lat: -33.4495, lng: -70.6690, accuracy: 6 },
];

export const FieldMap: React.FC<FieldMapProps> = ({ workerName, workerRole, workHours }) => {
  if (workerRole === 'OFFICE') {
    return (
      <div className="bg-slate-800/80 border border-slate-700/80 rounded-xl p-8 shadow-lg text-center space-y-3">
        <div className="w-12 h-12 rounded-full bg-slate-700/60 text-slate-400 flex items-center justify-center mx-auto">
          <ShieldAlert className="w-6 h-6 text-amber-400" />
        </div>
        <h3 className="text-base font-semibold text-white">Geolocalización No Disponible para Trabajadores de Oficina</h3>
        <p className="text-xs text-slate-400 max-w-lg mx-auto leading-relaxed">
          En cumplimiento con la <strong>Ley de Protección de Datos de Chile (19.628 / 21.719)</strong> y <strong>GDPR</strong>, los empleados con rol <span className="font-mono text-indigo-400">OFFICE</span> tienen permanentemente deshabilitada la recolección de ubicación geográfica tanto en el agente como en el servidor.
        </p>
      </div>
    );
  }

  return (
    <div className="bg-slate-800/80 border border-slate-700/80 rounded-xl p-5 shadow-lg space-y-5">
      <div className="flex flex-wrap justify-between items-center gap-3">
        <div>
          <h2 className="text-lg font-semibold text-white flex items-center gap-2">
            <Navigation className="w-5 h-5 text-amber-400" />
            Trazado de Rutas en Terreno (Trabajadores FIELD)
          </h2>
          <p className="text-xs text-slate-400">Ruta laboral descifrada para: <span className="text-amber-400 font-medium">{workerName}</span></p>
        </div>
        <div className="text-xs bg-amber-500/10 border border-amber-500/30 text-amber-300 px-3 py-1.5 rounded-lg">
          Horario Auditado: <strong>{workHours.start} - {workHours.end}</strong> (Fuera de horario: Suspensión automática)
        </div>
      </div>

      {/* Mock Map Canvas */}
      <div className="relative h-80 bg-slate-950 rounded-xl border border-slate-700/80 overflow-hidden flex flex-col justify-between p-4">
        {/* Simulated vector grid / map */}
        <div className="absolute inset-0 opacity-20 bg-[radial-gradient(#6366f1_1px,transparent_1px)] [background-size:16px_16px]"></div>

        <div className="relative z-10 flex justify-between items-start text-xs">
          <div className="bg-slate-900/80 backdrop-blur px-3 py-1.5 rounded-lg border border-slate-700 text-slate-300 font-mono">
            Sector Metropolitano: Santiago, Chile
          </div>
          <div className="bg-emerald-950/80 border border-emerald-800 text-emerald-300 px-2.5 py-1 rounded text-[11px] font-semibold">
            Geocerca Corporativa: 100% en Rango
          </div>
        </div>

        {/* Route Points */}
        <div className="relative z-10 flex justify-around items-center py-10">
          {mockWaypoints.map((wp, idx) => (
            <div key={idx} className="flex flex-col items-center group cursor-pointer">
              <div className="w-8 h-8 rounded-full bg-indigo-600 border-2 border-white text-white flex items-center justify-center shadow-lg group-hover:scale-125 transition-transform">
                <MapPin className="w-4 h-4" />
              </div>
              <span className="mt-2 text-[11px] font-mono text-slate-200 bg-slate-900/80 px-2 py-0.5 rounded border border-slate-700">{wp.time}</span>
              <span className="text-[9px] text-slate-400 font-mono mt-0.5">±{wp.accuracy}m</span>
            </div>
          ))}
        </div>

        <div className="relative z-10 text-[11px] text-slate-500 font-mono text-center">
          Leaflet / MapLibre Engine — Coordenadas cifradas con clave efímera
        </div>
      </div>
    </div>
  );
};
