import React, { useState, useEffect } from 'react';
import {
  Camera,
  Eye,
  EyeOff,
  Download,
  ZoomIn,
  AlertCircle,
  ChevronLeft,
  ChevronRight,
  Shield,
  X,
} from 'lucide-react';
import {
  TimeRangeFilter,
  getTimeRangeIso,
  fetchAndDecryptTelemetry,
  isDemoMode,
  AdminProfile,
} from '../api/tauri-bridge';

interface DecryptedScreenshotItem {
  id: string;
  timestamp: string;
  appContext: string;
  format: string;
  blobUrl: string;
  width?: number;
  height?: number;
}

interface ScreenshotGalleryProps {
  workerId?: string;
  workerName: string;
  adminProfile?: AdminProfile | null;
  timeRange: TimeRangeFilter;
}

const mockScreenshotsData = [
  { id: "shot-1", time: "09:15", appContext: "Visual Studio Code" },
  { id: "shot-2", time: "09:45", appContext: "Terminal" },
  { id: "shot-3", time: "10:15", appContext: "GitHub Enterprise" },
  { id: "shot-4", time: "10:45", appContext: "Slack Workspaces" },
  { id: "shot-5", time: "11:15", appContext: "Jira Software" },
  { id: "shot-6", time: "11:45", appContext: "Documentation" },
];

export const ScreenshotGallery: React.FC<ScreenshotGalleryProps> = ({
  workerId,
  workerName,
  adminProfile,
  timeRange,
}) => {
  // RF-ADMIN-05-3: Blur mode active by default to protect sensitive text
  const [privacyBlur, setPrivacyBlur] = useState<boolean>(true);
  const [loading, setLoading] = useState<boolean>(false);
  const [screenshots, setScreenshots] = useState<DecryptedScreenshotItem[]>([]);
  const [selectedShot, setSelectedShot] = useState<DecryptedScreenshotItem | null>(null);
  const [currentPage, setCurrentPage] = useState<number>(1);
  const pageSize = 12; // RF-ADMIN-05-3 & Section 5: Maximum 12 images per page

  useEffect(() => {
    console.debug("Dashboard context updated:", { workerId, range: timeRange });

    if (!workerId) {
      setScreenshots([]);
      return;
    }

    const { from, to } = getTimeRangeIso(timeRange);
    let isCancelled = false;
    setLoading(true);

    // Track created blob URLs so we can revoke them on cleanup (Security & RAM leak prevention)
    const createdBlobUrls: string[] = [];

    fetchAndDecryptTelemetry(workerId, 'productivity', from, to, 'SCREENSHOT')
      .then(records => {
        if (isCancelled) return;

        const shots: DecryptedScreenshotItem[] = [];

        for (const rec of records) {
          const items = Array.isArray(rec.payload?.items) ? rec.payload.items : [rec.payload];
          for (let i = 0; i < items.length; i++) {
            const item = items[i];
            if (!item || !item.image_b64) continue;

            try {
              // Decode base64 to binary Blob
              const byteCharacters = atob(item.image_b64);
              const byteNumbers = new Array(byteCharacters.length);
              for (let j = 0; j < byteCharacters.length; j++) {
                byteNumbers[j] = byteCharacters.charCodeAt(j);
              }
              const byteArray = new Uint8Array(byteNumbers);
              const mimeType = item.format === 'webp' ? 'image/webp' : 'image/jpeg';
              const blob = new Blob([byteArray], { type: mimeType });
              const blobUrl = URL.createObjectURL(blob);
              createdBlobUrls.push(blobUrl);

              const ts = item.ts || rec.client_timestamp || '';
              const dateObj = new Date(ts);
              const timeFormatted = isNaN(dateObj.getTime())
                ? ts
                : dateObj.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });

              shots.push({
                id: `${rec.id}-${i}`,
                timestamp: timeFormatted,
                appContext: item.app_context || item.window_title || 'Captura de Pantalla',
                format: item.format || 'jpeg',
                blobUrl,
                width: item.width,
                height: item.height,
              });
            } catch (convErr) {
              console.warn("Failed to parse screenshot blob:", convErr);
            }
          }
        }

        setScreenshots(shots);
        setCurrentPage(1);
      })
      .catch(err => {
        if (isCancelled) return;
        console.error("ScreenshotGallery fetch error:", err);
        setScreenshots([]);
      })
      .finally(() => {
        if (!isCancelled) setLoading(false);
      });

    return () => {
      isCancelled = true;
      // Memory cleanup: revoke all created object URLs
      for (const url of createdBlobUrls) {
        try {
          URL.revokeObjectURL(url);
        } catch (_) {}
      }
    };
  }, [workerId, timeRange.preset, timeRange.from, timeRange.to]);

  // Export screenshot with burned-in watermark (RF-ADMIN-05-3)
  const handleExportWatermarked = (shot: DecryptedScreenshotItem) => {
    const img = new Image();
    img.crossOrigin = 'anonymous';
    img.onload = () => {
      const canvas = document.createElement('canvas');
      canvas.width = img.width || 1920;
      canvas.height = img.height || 1080;
      const ctx = canvas.getContext('2d');
      if (!ctx) return;

      // Draw original image
      ctx.drawImage(img, 0, 0, canvas.width, canvas.height);

      // Draw watermark bar at the bottom
      const bannerHeight = 40;
      ctx.fillStyle = 'rgba(15, 23, 42, 0.85)';
      ctx.fillRect(0, canvas.height - bannerHeight, canvas.width, bannerHeight);

      // Watermark text
      ctx.fillStyle = '#f8fafc';
      ctx.font = '14px monospace';
      ctx.textBaseline = 'middle';

      const adminInfo = adminProfile ? `${adminProfile.name} (${adminProfile.email})` : 'Administrador';
      const watermarkText = `CONFIDENCIAL | Auditoría Telemetry: ${adminInfo} | Empleado: ${workerName} | ${shot.timestamp}`;
      ctx.fillText(watermarkText, 20, canvas.height - bannerHeight / 2);

      // Trigger download
      canvas.toBlob(blob => {
        if (!blob) return;
        const link = document.createElement('a');
        link.download = `telemetry-audit-${shot.id}-watermarked.png`;
        link.href = URL.createObjectURL(blob);
        link.click();
        setTimeout(() => URL.revokeObjectURL(link.href), 1000);
      }, 'image/png');
    };
    img.src = shot.blobUrl;
  };

  const totalPages = Math.ceil(screenshots.length / pageSize) || 1;
  const paginatedShots = screenshots.slice((currentPage - 1) * pageSize, currentPage * pageSize);

  return (
    <div className="bg-slate-800/80 border border-slate-700/80 rounded-xl p-5 shadow-lg space-y-5">
      {/* Header with Blur Mode Toggle */}
      <div className="flex flex-wrap justify-between items-center gap-3">
        <div>
          <h2 className="text-lg font-semibold text-white flex items-center gap-2">
            <Camera className="w-5 h-5 text-indigo-400" />
            Galería de Capturas Periódicas de Productividad
          </h2>
          <p className="text-xs text-slate-400">
            Capturas descargadas y descifradas localmente en RAM para:{' '}
            <span className="text-indigo-400 font-medium">{workerName}</span>
          </p>
        </div>

        <div className="flex items-center gap-2">
          {/* RF-ADMIN-05-3: Blur Toggle Button */}
          <button
            onClick={() => setPrivacyBlur(!privacyBlur)}
            className={`px-3 py-1.5 rounded-lg text-xs font-medium flex items-center gap-2 border transition ${
              privacyBlur
                ? 'bg-amber-500/20 text-amber-300 border-amber-500/40 hover:bg-amber-500/30'
                : 'bg-slate-700 text-slate-300 border-slate-600 hover:bg-slate-600'
            }`}
          >
            {privacyBlur ? <EyeOff className="w-4 h-4" /> : <Eye className="w-4 h-4" />}
            <span>{privacyBlur ? '🔒 Modo Blur Activo (Texto Oculto)' : '🔓 Texto Visible'}</span>
          </button>
        </div>
      </div>

      {loading ? (
        <div className="h-72 w-full flex flex-col items-center justify-center bg-slate-900/40 rounded-xl border border-slate-700/50">
          <div className="w-8 h-8 border-2 border-indigo-500 border-t-transparent rounded-full animate-spin"></div>
          <p className="text-xs text-slate-400 mt-3 font-mono">Descifrando capturas JPEG/WebP en memoria...</p>
        </div>
      ) : screenshots.length === 0 ? (
        // Empty State or Demo mode fallback
        isDemoMode ? (
          <div className="space-y-4">
            <div className="bg-indigo-950/40 border border-indigo-800/40 rounded-lg p-3 text-xs text-indigo-300 flex items-center gap-2">
              <Shield className="w-4 h-4 shrink-0 text-indigo-400" />
              <span>Modo Demostración: Simulando tarjetas de capturas de pantalla periódicas.</span>
            </div>
            <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4">
              {mockScreenshotsData.map(shot => (
                <div
                  key={shot.id}
                  className="bg-slate-900 border border-slate-700 rounded-xl overflow-hidden shadow-md flex flex-col justify-between"
                >
                  <div className="relative h-44 bg-slate-950 flex items-center justify-center overflow-hidden">
                    <div
                      className={`w-full h-full p-4 flex flex-col justify-between transition-all duration-300 ${
                        privacyBlur ? 'blur-md select-none' : ''
                      }`}
                      style={{ background: 'linear-gradient(135deg, #1e1b4b 0%, #0f172a 100%)' }}
                    >
                      <div className="flex justify-between items-center text-[10px] text-slate-400 border-b border-slate-800 pb-2">
                        <span>{shot.appContext}</span>
                        <span>{shot.time}</span>
                      </div>
                      <div className="text-center py-6 text-xs text-indigo-300 font-mono">
                        [Simulación de captura protegida]
                      </div>
                      <div className="text-[9px] text-slate-500 font-mono text-right">
                        Watermark: Telemetry Audit
                      </div>
                    </div>
                  </div>
                  <div className="p-3 bg-slate-800 text-xs flex justify-between items-center border-t border-slate-700/80">
                    <span className="font-semibold text-slate-200">{shot.appContext}</span>
                    <span className="text-slate-400 font-mono text-[11px]">{shot.time} hrs</span>
                  </div>
                </div>
              ))}
            </div>
          </div>
        ) : (
          <div className="bg-slate-900/50 border border-slate-700/60 rounded-xl p-12 text-center space-y-3">
            <div className="w-12 h-12 rounded-full bg-slate-800 text-slate-400 flex items-center justify-center mx-auto">
              <AlertCircle className="w-6 h-6 text-indigo-400" />
            </div>
            <h3 className="text-base font-semibold text-white">Sin Capturas en el Período</h3>
            <p className="text-xs text-slate-400 max-w-md mx-auto leading-relaxed">
              No se encontraron capturas de pantalla descifradas para <strong className="text-slate-200">{workerName}</strong> en el rango temporal seleccionado.
            </p>
          </div>
        )
      ) : (
        <div className="space-y-4">
          {/* Gallery Grid (Up to 12 items per page) */}
          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4">
            {paginatedShots.map(shot => (
              <div
                key={shot.id}
                className="group relative bg-slate-900 border border-slate-700 rounded-xl overflow-hidden shadow-md flex flex-col justify-between"
              >
                <div className="relative h-48 bg-slate-950 flex items-center justify-center overflow-hidden">
                  <img
                    src={shot.blobUrl}
                    alt={shot.appContext}
                    className={`w-full h-full object-cover transition duration-300 ${
                      privacyBlur ? 'blur-md hover:blur-none select-none' : ''
                    }`}
                  />

                  {/* Hover Overlay with Zoom and Watermarked Export */}
                  <div className="absolute inset-0 bg-slate-950/70 opacity-0 group-hover:opacity-100 transition-opacity flex items-center justify-center gap-3">
                    <button
                      onClick={() => setSelectedShot(shot)}
                      className="p-2.5 bg-indigo-600 text-white rounded-lg hover:bg-indigo-500 transition shadow-lg"
                      title="Inspeccionar en detalle"
                    >
                      <ZoomIn className="w-4 h-4" />
                    </button>
                    <button
                      onClick={() => handleExportWatermarked(shot)}
                      className="p-2.5 bg-slate-700 text-slate-200 rounded-lg hover:bg-slate-600 transition shadow-lg"
                      title="Exportar con marca de agua legal"
                    >
                      <Download className="w-4 h-4" />
                    </button>
                  </div>
                </div>

                <div className="p-3 bg-slate-800 text-xs flex justify-between items-center border-t border-slate-700/80">
                  <span className="font-semibold text-slate-200 truncate">{shot.appContext}</span>
                  <span className="text-slate-400 font-mono text-[11px] shrink-0 ml-2">
                    {shot.timestamp}
                  </span>
                </div>
              </div>
            ))}
          </div>

          {/* Pagination Controls */}
          {totalPages > 1 && (
            <div className="flex justify-between items-center pt-2 border-t border-slate-700/80 text-xs text-slate-400">
              <span>
                Mostrando {paginatedShots.length} de {screenshots.length} capturas
              </span>
              <div className="flex items-center gap-2">
                <button
                  disabled={currentPage === 1}
                  onClick={() => setCurrentPage(p => Math.max(1, p - 1))}
                  className="p-1.5 rounded-lg bg-slate-800 border border-slate-700 text-slate-300 hover:bg-slate-700 disabled:opacity-40 disabled:cursor-not-allowed transition"
                >
                  <ChevronLeft className="w-4 h-4" />
                </button>
                <span className="font-mono text-slate-200">
                  Página {currentPage} de {totalPages}
                </span>
                <button
                  disabled={currentPage === totalPages}
                  onClick={() => setCurrentPage(p => Math.min(totalPages, p + 1))}
                  className="p-1.5 rounded-lg bg-slate-800 border border-slate-700 text-slate-300 hover:bg-slate-700 disabled:opacity-40 disabled:cursor-not-allowed transition"
                >
                  <ChevronRight className="w-4 h-4" />
                </button>
              </div>
            </div>
          )}
        </div>
      )}

      {/* Modal for Zoomed Screenshot */}
      {selectedShot && (
        <div className="fixed inset-0 z-50 bg-black/85 backdrop-blur-sm flex items-center justify-center p-4">
          <div className="bg-slate-900 border border-slate-700 rounded-2xl max-w-4xl w-full p-5 space-y-4 shadow-2xl">
            <div className="flex justify-between items-center border-b border-slate-800 pb-3">
              <h3 className="text-sm font-semibold text-white flex items-center gap-2">
                <Camera className="w-4 h-4 text-indigo-400" />
                {selectedShot.appContext} — {selectedShot.timestamp}
              </h3>
              <button
                onClick={() => setSelectedShot(null)}
                className="text-slate-400 hover:text-white p-1 rounded-lg hover:bg-slate-800 transition"
              >
                <X className="w-5 h-5" />
              </button>
            </div>

            <div className="relative max-h-[65vh] overflow-hidden rounded-xl bg-slate-950 flex items-center justify-center border border-slate-800">
              <img
                src={selectedShot.blobUrl}
                alt="Vista ampliada"
                className={`max-h-[65vh] w-auto object-contain transition duration-200 ${
                  privacyBlur ? 'blur-md hover:blur-none' : ''
                }`}
              />
              <div className="absolute bottom-0 inset-x-0 bg-slate-950/80 backdrop-blur-sm p-2 text-center text-[11px] text-amber-300 font-mono border-t border-slate-800">
                Watermark de Auditoría: {workerName} — {selectedShot.timestamp} (Confidencial)
              </div>
            </div>

            <div className="flex justify-between items-center pt-2">
              <button
                onClick={() => setPrivacyBlur(!privacyBlur)}
                className="text-xs text-slate-300 flex items-center gap-1.5 hover:text-white transition"
              >
                {privacyBlur ? <Eye className="w-4 h-4 text-amber-400" /> : <EyeOff className="w-4 h-4" />}
                <span>{privacyBlur ? 'Revelar texto nítido' : 'Aplicar desenfoque'}</span>
              </button>
              <div className="flex gap-2">
                <button
                  onClick={() => handleExportWatermarked(selectedShot)}
                  className="px-3 py-1.5 bg-indigo-600 hover:bg-indigo-500 text-white rounded-lg text-xs font-medium flex items-center gap-1.5 transition shadow"
                >
                  <Download className="w-3.5 h-3.5" />
                  <span>Descargar con Marca de Agua</span>
                </button>
                <button
                  onClick={() => setSelectedShot(null)}
                  className="px-3 py-1.5 bg-slate-800 text-slate-300 rounded-lg text-xs hover:bg-slate-700 transition"
                >
                  Cerrar
                </button>
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
