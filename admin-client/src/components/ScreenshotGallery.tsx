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
      ctx.fillStyle = 'rgba(10, 10, 10, 0.85)';
      ctx.fillRect(0, canvas.height - bannerHeight, canvas.width, bannerHeight);

      // Watermark text
      ctx.fillStyle = '#fafafa';
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
    <div className="bg-surface-subtle border border-default rounded-card p-5 space-y-5">
      {/* Header with Blur Mode Toggle */}
      <div className="flex flex-wrap justify-between items-center gap-3">
        <div>
          <h2 className="text-lg font-semibold text-fg flex items-center gap-2">
            <Camera className="w-5 h-5 text-accent" />
            Galería de Capturas Periódicas de Productividad
          </h2>
          <p className="text-xs text-muted">
            Capturas descargadas y descifradas localmente en RAM para:{' '}
            <span className="text-accent font-medium">{workerName}</span>
          </p>
        </div>

        <div className="flex items-center gap-2">
          {/* RF-ADMIN-05-3: Blur Toggle Button */}
          <button
            onClick={() => setPrivacyBlur(!privacyBlur)}
            className={`px-3 py-1.5 rounded-control text-xs font-medium flex items-center gap-2 border transition ${
 privacyBlur
 ? 'bg-warning-subtle text-warning border-default hover:bg-warning-subtle'
 : 'bg-surface-subtle text-fg border-strong hover:bg-strong'
 }`}
          >
            {privacyBlur ? <EyeOff className="w-4 h-4" /> : <Eye className="w-4 h-4" />}
            <span>{privacyBlur ? 'Difuminado activo (texto oculto)' : 'Texto visible'}</span>
          </button>
        </div>
      </div>

      {loading ? (
        <div className="h-72 w-full flex flex-col items-center justify-center bg-surface rounded-card border border-default">
          <div className="w-8 h-8 border-2 border-accent border-t-transparent rounded-full animate-spin"></div>
          <p className="text-xs text-muted mt-3 font-mono">Descifrando capturas JPEG/WebP en memoria...</p>
        </div>
      ) : screenshots.length === 0 ? (
        // Empty State or Demo mode fallback
        isDemoMode ? (
          <div className="space-y-4">
            <div className="bg-accent-subtle border border-default rounded-control p-3 text-xs text-accent flex items-center gap-2">
              <Shield className="w-4 h-4 shrink-0 text-accent" />
              <span>Modo Demostración: Simulando tarjetas de capturas de pantalla periódicas.</span>
            </div>
            <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4">
              {mockScreenshotsData.map(shot => (
                <div
                  key={shot.id}
                  className="bg-surface border border-default rounded-card overflow-hidden flex flex-col justify-between"
                >
                  <div className="relative h-44 bg-bg flex items-center justify-center overflow-hidden">
                    <div
                      className={`w-full h-full p-4 flex flex-col justify-between transition-all duration-300 ${
                        privacyBlur ? 'blur-md select-none' : ''
                      }`}
                      style={{ background: '#141414' }}
                    >
                      <div className="flex justify-between items-center text-[10px] text-muted border-b border-default pb-2">
                        <span>{shot.appContext}</span>
                        <span>{shot.time}</span>
                      </div>
                      <div className="text-center py-6 text-xs text-muted font-mono">
                        [Simulación de captura protegida]
                      </div>
                      <div className="text-[9px] text-muted font-mono text-right">
                        Watermark: Telemetry Audit
                      </div>
                    </div>
                  </div>
                  <div className="p-3 bg-surface-subtle text-xs flex justify-between items-center border-t border-default">
                    <span className="font-semibold text-fg">{shot.appContext}</span>
                    <span className="text-muted font-mono text-[11px]">{shot.time} hrs</span>
                  </div>
                </div>
              ))}
            </div>
          </div>
        ) : (
          <div className="bg-surface border border-default rounded-card p-12 text-center space-y-3">
            <div className="w-12 h-12 rounded-full bg-surface-subtle text-muted flex items-center justify-center mx-auto">
              <AlertCircle className="w-6 h-6 text-accent" />
            </div>
            <h3 className="text-base font-semibold text-fg">Sin Capturas en el Período</h3>
            <p className="text-xs text-muted max-w-md mx-auto leading-relaxed">
              No se encontraron capturas de pantalla descifradas para <strong className="text-fg">{workerName}</strong> en el rango temporal seleccionado.
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
                className="group relative bg-surface border border-default rounded-card overflow-hidden flex flex-col justify-between"
              >
                <div className="relative h-48 bg-bg flex items-center justify-center overflow-hidden">
                  <img
                    src={shot.blobUrl}
                    alt={shot.appContext}
                    className={`w-full h-full object-cover transition duration-300 ${
                      privacyBlur ? 'blur-md hover:blur-none select-none' : ''
                    }`}
                  />

                  {/* Hover Overlay with Zoom and Watermarked Export */}
                  <div className="absolute inset-0 bg-bg opacity-0 group-hover:opacity-100 transition-opacity flex items-center justify-center gap-3">
                    <button
                      onClick={() => setSelectedShot(shot)}
                      className="p-2.5 bg-accent text-accent-fg rounded-control hover:bg-accent-hover transition"
                      title="Inspeccionar en detalle"
                    >
                      <ZoomIn className="w-4 h-4" />
                    </button>
                    <button
                      onClick={() => handleExportWatermarked(shot)}
                      className="p-2.5 bg-surface-subtle text-fg rounded-control hover:bg-strong transition"
                      title="Exportar con marca de agua legal"
                    >
                      <Download className="w-4 h-4" />
                    </button>
                  </div>
                </div>

                <div className="p-3 bg-surface-subtle text-xs flex justify-between items-center border-t border-default">
                  <span className="font-semibold text-fg truncate">{shot.appContext}</span>
                  <span className="text-muted font-mono text-[11px] shrink-0 ml-2">
                    {shot.timestamp}
                  </span>
                </div>
              </div>
            ))}
          </div>

          {/* Pagination Controls */}
          {totalPages > 1 && (
            <div className="flex justify-between items-center pt-2 border-t border-default text-xs text-muted">
              <span>
                Mostrando {paginatedShots.length} de {screenshots.length} capturas
              </span>
              <div className="flex items-center gap-2">
                <button
                  disabled={currentPage === 1}
                  onClick={() => setCurrentPage(p => Math.max(1, p - 1))}
                  className="p-1.5 rounded-control bg-surface-subtle border border-default text-fg hover:bg-surface-subtle disabled:opacity-40 disabled:cursor-not-allowed transition"
                >
                  <ChevronLeft className="w-4 h-4" />
                </button>
                <span className="font-mono text-fg">
                  Página {currentPage} de {totalPages}
                </span>
                <button
                  disabled={currentPage === totalPages}
                  onClick={() => setCurrentPage(p => Math.min(totalPages, p + 1))}
                  className="p-1.5 rounded-control bg-surface-subtle border border-default text-fg hover:bg-surface-subtle disabled:opacity-40 disabled:cursor-not-allowed transition"
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
        <div className="fixed inset-0 z-50 bg-black/85 flex items-center justify-center p-4">
          <div className="bg-surface border border-default rounded-card max-w-4xl w-full p-5 space-y-4 ">
            <div className="flex justify-between items-center border-b border-default pb-3">
              <h3 className="text-sm font-semibold text-fg flex items-center gap-2">
                <Camera className="w-4 h-4 text-accent" />
                {selectedShot.appContext} — {selectedShot.timestamp}
              </h3>
              <button
                onClick={() => setSelectedShot(null)}
                className="text-muted hover:text-fg p-1 rounded-control hover:bg-surface-subtle transition"
              >
                <X className="w-5 h-5" />
              </button>
            </div>

            <div className="relative max-h-[65vh] overflow-hidden rounded-card bg-bg flex items-center justify-center border border-default">
              <img
                src={selectedShot.blobUrl}
                alt="Vista ampliada"
                className={`max-h-[65vh] w-auto object-contain transition duration-200 ${
                  privacyBlur ? 'blur-md hover:blur-none' : ''
                }`}
              />
              <div className="absolute bottom-0 inset-x-0 bg-bg p-2 text-center text-[11px] text-warning font-mono border-t border-default">
                Watermark de Auditoría: {workerName} — {selectedShot.timestamp} (Confidencial)
              </div>
            </div>

            <div className="flex justify-between items-center pt-2">
              <button
                onClick={() => setPrivacyBlur(!privacyBlur)}
                className="text-xs text-fg flex items-center gap-1.5 hover:text-fg transition"
              >
                {privacyBlur ? <Eye className="w-4 h-4 text-warning" /> : <EyeOff className="w-4 h-4" />}
                <span>{privacyBlur ? 'Revelar texto nítido' : 'Aplicar desenfoque'}</span>
              </button>
              <div className="flex gap-2">
                <button
                  onClick={() => handleExportWatermarked(selectedShot)}
                  className="px-3 py-1.5 bg-accent hover:bg-accent-hover text-accent-fg rounded-control text-xs font-medium flex items-center gap-1.5 transition"
                >
                  <Download className="w-3.5 h-3.5" />
                  <span>Descargar con Marca de Agua</span>
                </button>
                <button
                  onClick={() => setSelectedShot(null)}
                  className="px-3 py-1.5 bg-surface-subtle text-fg rounded-control text-xs hover:bg-surface-subtle transition"
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
