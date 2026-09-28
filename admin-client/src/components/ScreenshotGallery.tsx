import React, { useState } from 'react';
import { Camera, Eye, EyeOff, Download, ZoomIn } from 'lucide-react';

interface MockScreenshot {
  id: string;
  time: string;
  appContext: string;
}

const mockScreenshots: MockScreenshot[] = [
  { id: "shot-1", time: "09:15", appContext: "Visual Studio Code" },
  { id: "shot-2", time: "09:45", appContext: "Terminal" },
  { id: "shot-3", time: "10:15", appContext: "GitHub Enterprise" },
  { id: "shot-4", time: "10:45", appContext: "Slack Workspaces" },
  { id: "shot-5", time: "11:15", appContext: "Jira Software" },
  { id: "shot-6", time: "11:45", appContext: "Documentation" },
];

export const ScreenshotGallery: React.FC<{ workerName: string }> = ({ workerName }) => {
  const [privacyBlur, setPrivacyBlur] = useState<boolean>(true);
  const [selectedShot, setSelectedShot] = useState<MockScreenshot | null>(null);

  return (
    <div className="bg-slate-800/80 border border-slate-700/80 rounded-xl p-5 shadow-lg space-y-5">
      <div className="flex flex-wrap justify-between items-center gap-3">
        <div>
          <h2 className="text-lg font-semibold text-white flex items-center gap-2">
            <Camera className="w-5 h-5 text-indigo-400" />
            Galería de Capturas Periódicas de Productividad
          </h2>
          <p className="text-xs text-slate-400">Capturas periódicas para: <span className="text-indigo-400 font-medium">{workerName}</span></p>
        </div>

        <div className="flex items-center gap-2">
          <button
            onClick={() => setPrivacyBlur(!privacyBlur)}
            className={`px-3 py-1.5 rounded-lg text-xs font-medium flex items-center gap-2 border transition ${
              privacyBlur
                ? 'bg-amber-500/20 text-amber-300 border-amber-500/40 hover:bg-amber-500/30'
                : 'bg-slate-700 text-slate-300 border-slate-600 hover:bg-slate-600'
            }`}
          >
            {privacyBlur ? <EyeOff className="w-4 h-4" /> : <Eye className="w-4 h-4" />}
            {privacyBlur ? '🔒 Modo Privacidad (Desenfoque Activo)' : '🔓 Modo Desenfocado Inactivo'}
          </button>
        </div>
      </div>

      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4">
        {mockScreenshots.map(shot => (
          <div key={shot.id} className="group relative bg-slate-900 border border-slate-700 rounded-xl overflow-hidden shadow-md flex flex-col justify-between">
            <div className="relative h-44 bg-slate-950 flex items-center justify-center overflow-hidden">
              {/* Mock desktop canvas */}
              <div
                className={`w-full h-full p-4 flex flex-col justify-between transition-all duration-300 ${
                  privacyBlur ? 'blur-sm select-none' : ''
                }`}
                style={{
                  background: 'linear-gradient(135deg, #1e1b4b 0%, #0f172a 100%)'
                }}
              >
                <div className="flex justify-between items-center text-[10px] text-slate-400 border-b border-slate-800 pb-2">
                  <span>{shot.appContext}</span>
                  <span>{shot.time}</span>
                </div>
                <div className="text-center py-6 text-xs text-indigo-300 font-mono">
                  [Contenido de pantalla capturado periódicamente]
                </div>
                <div className="text-[9px] text-slate-500 font-mono text-right">
                  Marca de Agua: Telemetry Security Audit
                </div>
              </div>

              {/* Hover actions */}
              <div className="absolute inset-0 bg-slate-900/60 opacity-0 group-hover:opacity-100 transition-opacity flex items-center justify-center gap-3">
                <button
                  onClick={() => setSelectedShot(shot)}
                  className="p-2 bg-indigo-600 text-white rounded-lg hover:bg-indigo-500 transition shadow"
                  title="Ampliar captura"
                >
                  <ZoomIn className="w-4 h-4" />
                </button>
                <button
                  className="p-2 bg-slate-700 text-slate-200 rounded-lg hover:bg-slate-600 transition shadow"
                  title="Exportar con marca de agua"
                >
                  <Download className="w-4 h-4" />
                </button>
              </div>
            </div>

            <div className="p-3 bg-slate-800 text-xs flex justify-between items-center border-t border-slate-700/80">
              <span className="font-semibold text-slate-200">{shot.appContext}</span>
              <span className="text-slate-400 font-mono text-[11px]">{shot.time} hrs</span>
            </div>
          </div>
        ))}
      </div>

      {/* Modal for Zoomed Screenshot */}
      {selectedShot && (
        <div className="fixed inset-0 z-50 bg-black/80 flex items-center justify-center p-4">
          <div className="bg-slate-800 border border-slate-700 rounded-2xl max-w-3xl w-full p-6 space-y-4">
            <div className="flex justify-between items-center">
              <h3 className="text-base font-semibold text-white">Inspección de Captura — {selectedShot.appContext} ({selectedShot.time})</h3>
              <button onClick={() => setSelectedShot(null)} className="text-slate-400 hover:text-white font-bold text-lg">&times;</button>
            </div>
            <div className="h-80 bg-slate-950 rounded-xl p-8 flex flex-col justify-between border border-slate-800 relative">
              <div className="text-slate-300 text-sm font-mono">[Vista en tamaño completo descifrada localmente en RAM]</div>
              <div className="text-xs text-amber-400">Protegido por Watermark de Auditoría: {workerName} — {selectedShot.time}</div>
            </div>
            <div className="flex justify-end gap-3">
              <button onClick={() => setSelectedShot(null)} className="px-4 py-2 bg-slate-700 text-slate-300 rounded-lg text-xs hover:bg-slate-600 transition">Cerrar</button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
