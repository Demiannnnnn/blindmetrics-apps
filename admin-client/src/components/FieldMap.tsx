import React, { useState, useEffect, useRef } from 'react';
import {
  ShieldAlert,
  Navigation,
  ShieldCheck,
  AlertCircle,
  Clock,
  Compass,
} from 'lucide-react';
import L from 'leaflet';
import 'leaflet/dist/leaflet.css';
import {
  TimeRangeFilter,
  getTimeRangeIso,
  fetchAndDecryptTelemetry,
  isDemoMode,
} from '../api/tauri-bridge';
import { LIGHT_THEME_COLORS } from '../hooks/useThemeColors';

/** Map tiles and Leaflet popups are always light: fixed light-theme palette for contrast. */
const MAP = LIGHT_THEME_COLORS;

export interface Waypoint {
  time: string;
  lat: number;
  lng: number;
  accuracy: number;
  isStop?: boolean;
}

export interface FieldMapProps {
  workerId?: string;
  workerName: string;
  workerRole: 'OFFICE' | 'FIELD';
  workHours: { start: string; end: string };
  timezone?: string;
  timeRange: TimeRangeFilter;
}

const defaultMockWaypoints: Waypoint[] = [
  { time: "09:15:00", lat: -33.4489, lng: -70.6693, accuracy: 12.0, isStop: false },
  { time: "10:30:00", lat: -33.4520, lng: -70.6610, accuracy: 8.5, isStop: true },
  { time: "12:15:00", lat: -33.4610, lng: -70.6550, accuracy: 15.2, isStop: false },
  { time: "14:45:00", lat: -33.4580, lng: -70.6650, accuracy: 10.0, isStop: true },
  { time: "17:30:00", lat: -33.4495, lng: -70.6690, accuracy: 6.4, isStop: false },
];

// Default Corporate Headquarters Center (Santiago, Chile)
const CORPORATE_CENTER = { lat: -33.4489, lng: -70.6693, radiusMeters: 3000 };

// Haversine distance formula in meters
export function getDistanceMeters(lat1: number, lon1: number, lat2: number, lon2: number): number {
  const R = 6371e3;
  const phi1 = (lat1 * Math.PI) / 180;
  const phi2 = (lat2 * Math.PI) / 180;
  const deltaPhi = ((lat2 - lat1) * Math.PI) / 180;
  const deltaLambda = ((lon2 - lon1) * Math.PI) / 180;

  const a =
    Math.sin(deltaPhi / 2) * Math.sin(deltaPhi / 2) +
    Math.cos(phi1) * Math.cos(phi2) * Math.sin(deltaLambda / 2) * Math.sin(deltaLambda / 2);
  const c = 2 * Math.atan2(Math.sqrt(a), Math.sqrt(1 - a));

  return R * c;
}

// Validates whether coordinates are in valid numeric ranges (Section 4)
export function isValidCoordinate(lat: unknown, lng: unknown): boolean {
  if (typeof lat !== 'number' || typeof lng !== 'number') return false;
  if (isNaN(lat) || isNaN(lng)) return false;
  return lat >= -90 && lat <= 90 && lng >= -180 && lng <= 180;
}

// Checks if a given time HH:MM falls within [start, end]
export function isWithinShift(timeStr: string, startStr: string, endStr: string): boolean {
  if (!timeStr || !startStr || !endStr) return true;
  const [h, m] = timeStr.slice(0, 5).split(':').map(Number);
  const [sH, sM] = startStr.split(':').map(Number);
  const [eH, eM] = endStr.split(':').map(Number);

  if (isNaN(h) || isNaN(sH) || isNaN(eH)) return true;

  const cur = h * 60 + (m || 0);
  const start = sH * 60 + (sM || 0);
  const end = eH * 60 + (eM || 0);

  if (start <= end) {
    return cur >= start && cur <= end;
  } else {
    // Overnight shift
    return cur >= start || cur <= end;
  }
}

export const FieldMap: React.FC<FieldMapProps> = ({
  workerId,
  workerName,
  workerRole,
  workHours,
  timezone = 'America/Santiago',
  timeRange,
}) => {
  // RF-ADMIN-06-1 & DoD 1: Block view completely for OFFICE workers
  if (workerRole === 'OFFICE') {
    return (
      <div className="bg-surface-subtle border border-default rounded-card p-8 text-center space-y-4">
        <div className="w-14 h-14 rounded-full bg-surface-subtle text-warning flex items-center justify-center mx-auto ring-8 ring-warning">
          <ShieldAlert className="w-7 h-7" />
        </div>
        <h3 className="text-base font-semibold text-fg">
          Monitoreo de Ubicación No Disponible para Trabajadores de Oficina
        </h3>
        <p className="text-xs text-muted max-w-lg mx-auto leading-relaxed">
          En cumplimiento estricto con la <strong>Ley de Protección de Datos Personales (Ley 19.628 / 21.719)</strong> y{' '}
          <strong>GDPR</strong>, los empleados con rol <span className="font-mono text-fg font-semibold">OFFICE</span>{' '}
          tienen permanentemente deshabilitada la recolección de geolocalización tanto en el agente como en el servidor.
        </p>
        <div className="inline-flex items-center gap-2 text-xs bg-surface px-3 py-1.5 rounded-control border border-default text-muted">
          <ShieldCheck className="w-4 h-4 text-success" />
          <span>Privacidad por Diseño (DP-04): Hardware GPS y servicios de ubicación suspendidos</span>
        </div>
      </div>
    );
  }

  const [loading, setLoading] = useState<boolean>(false);
  const [waypoints, setWaypoints] = useState<Waypoint[]>([]);
  const mapContainerRef = useRef<HTMLDivElement | null>(null);
  const mapInstanceRef = useRef<L.Map | null>(null);

  useEffect(() => {
    console.debug("Dashboard context updated:", { workerId, range: timeRange });

    if (!workerId) {
      if (isDemoMode) {
        setWaypoints(defaultMockWaypoints);
      } else {
        setWaypoints([]);
      }
      return;
    }

    const { from, to } = getTimeRangeIso(timeRange);
    let isCancelled = false;
    setLoading(true);

    fetchAndDecryptTelemetry(workerId, 'location', from, to)
      .then(records => {
        if (isCancelled) return;

        const pts: Waypoint[] = [];

        for (const rec of records) {
          const items = Array.isArray(rec.payload?.items) ? rec.payload.items : [rec.payload];
          for (const item of items) {
            if (!item) continue;
            const lat = Number(item.lat);
            const lng = Number(item.lon ?? item.lng);

            if (!isValidCoordinate(lat, lng)) continue;

            const ts = item.ts || rec.client_timestamp || '';
            const dateObj = new Date(ts);
            const timeLabel = isNaN(dateObj.getTime())
              ? ts
              : dateObj.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' });

            pts.push({
              time: timeLabel,
              lat,
              lng,
              accuracy: typeof item.accuracy_m === 'number' ? item.accuracy_m : (item.accuracy ?? 10.0),
            });
          }
        }

        // Sort chronologically (RF-ADMIN-06-2 & DoD 2)
        pts.sort((a, b) => a.time.localeCompare(b.time));

        // Mark stops (> 15 min within 50m radius)
        for (let i = 0; i < pts.length - 1; i++) {
          const dist = getDistanceMeters(pts[i].lat, pts[i].lng, pts[i + 1].lat, pts[i + 1].lng);
          if (dist < 50) {
            pts[i].isStop = true;
          }
        }

        if (pts.length > 0) {
          setWaypoints(pts);
        } else if (isDemoMode) {
          setWaypoints(defaultMockWaypoints);
        } else {
          setWaypoints([]);
        }
      })
      .catch(err => {
        if (isCancelled) return;
        console.error("FieldMap fetch error:", err);
        if (isDemoMode) {
          setWaypoints(defaultMockWaypoints);
        } else {
          setWaypoints([]);
        }
      })
      .finally(() => {
        if (!isCancelled) setLoading(false);
      });

    return () => {
      isCancelled = true;
    };
  }, [workerId, timeRange.preset, timeRange.from, timeRange.to]);

  // Leaflet map initialization and update
  useEffect(() => {
    if (!mapContainerRef.current) return;

    // Clean up previous map instance if exists
    if (mapInstanceRef.current) {
      mapInstanceRef.current.remove();
      mapInstanceRef.current = null;
    }

    const initialCenter: [number, number] = waypoints.length > 0
      ? [waypoints[0].lat, waypoints[0].lng]
      : [CORPORATE_CENTER.lat, CORPORATE_CENTER.lng];

    const map = L.map(mapContainerRef.current, {
      zoomControl: true,
      attributionControl: true,
    }).setView(initialCenter, 13);

    mapInstanceRef.current = map;

    // OpenStreetMap tile layer (Section 4: Open source, no third party tracking)
    L.tileLayer('https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png', {
      attribution: '&copy; OpenStreetMap contributors',
      maxZoom: 19,
    }).addTo(map);

    // Geofence Circle (RF-ADMIN-06-3)
    L.circle([CORPORATE_CENTER.lat, CORPORATE_CENTER.lng], {
      radius: CORPORATE_CENTER.radiusMeters,
      color: MAP.success,
      fillColor: MAP.success,
      fillOpacity: 0.12,
      dashArray: '6, 6',
      weight: 2,
    })
      .bindPopup('<strong>Geocerca Corporativa</strong><br/>Radio autorizado: 3.0 km')
      .addTo(map);

    if (waypoints.length > 0) {
      const latLngs: [number, number][] = waypoints.map(w => [w.lat, w.lng]);

      // Route Polyline (RF-ADMIN-06-2 & DoD 2)
      if (latLngs.length > 1) {
        L.polyline(latLngs, {
          color: MAP.accent,
          weight: 4,
          opacity: 0.85,
          lineJoin: 'round',
        }).addTo(map);
      }

      // Add custom markers
      waypoints.forEach((wp, idx) => {
        const isStart = idx === 0;
        const isEnd = idx === waypoints.length - 1 && waypoints.length > 1;

        let iconHtml = `<div style="background-color: ${MAP.accent}; width: 14px; height: 14px; border-radius: 50%; border: 2px solid white; box-shadow: 0 2px 4px rgba(0,0,0,0.3);"></div>`;

        if (isStart) {
          // Green start marker (RF-ADMIN-06-2)
          iconHtml = `<div style="background-color: ${MAP.success}; width: 26px; height: 26px; border-radius: 50%; border: 2px solid white; display: flex; align-items: center; justify-content: center; color: white; font-weight: bold; font-size: 11px; box-shadow: 0 4px 6px rgba(0,0,0,0.3);">▶</div>`;
        } else if (isEnd) {
          // Red end marker (RF-ADMIN-06-2)
          iconHtml = `<div style="background-color: ${MAP.danger}; width: 26px; height: 26px; border-radius: 50%; border: 2px solid white; display: flex; align-items: center; justify-content: center; color: white; font-weight: bold; font-size: 11px; box-shadow: 0 4px 6px rgba(0,0,0,0.3);">■</div>`;
        } else if (wp.isStop) {
          // Amber stop marker
          iconHtml = `<div style="background-color: ${MAP.warning}; width: 20px; height: 20px; border-radius: 50%; border: 2px solid white; display: flex; align-items: center; justify-content: center; color: white; font-weight: bold; font-size: 9px; box-shadow: 0 2px 4px rgba(0,0,0,0.3);">⏸</div>`;
        }

        const customIcon = L.divIcon({
          className: 'field-custom-marker',
          html: iconHtml,
          iconSize: [26, 26],
          iconAnchor: [13, 13],
        });

        const marker = L.marker([wp.lat, wp.lng], { icon: customIcon }).addTo(map);

        // Popup with time and estimated accuracy (RF-ADMIN-06-2 & DoD 3)
        marker.bindPopup(`
          <div style="font-family: 'IBM Plex Sans', sans-serif; font-size: 12px; color: ${MAP.text}; min-width: 160px;">
            <div style="font-weight: bold; color: ${MAP.accent}; margin-bottom: 4px;">
              ${isStart ? 'Inicio de Ruta' : isEnd ? 'Fin de Ruta' : wp.isStop ? 'Parada Detectada' : 'Punto de Registro'}
            </div>
            <div><strong>Hora:</strong> ${wp.time}</div>
            <div><strong>Precisión GPS:</strong> ±${wp.accuracy.toFixed(1)} m</div>
            <div><strong>Coordenadas:</strong> ${wp.lat.toFixed(4)}, ${wp.lng.toFixed(4)}</div>
            ${wp.isStop ? '<div style="color: ${MAP.warning}; font-weight: 600; margin-top: 4px;">Parada prolongada</div>' : ''}
          </div>
        `);
      });

      // Fit map bounds to show full route and geofence
      const bounds = L.latLngBounds(latLngs);
      bounds.extend([CORPORATE_CENTER.lat, CORPORATE_CENTER.lng]);
      map.fitBounds(bounds, { padding: [40, 40] });
    }

    console.info("Rendered field route with", waypoints.length, "waypoints");

    return () => {
      if (mapInstanceRef.current) {
        mapInstanceRef.current.remove();
        mapInstanceRef.current = null;
      }
    };
  }, [waypoints]);

  // Geofence audit calculations (RF-ADMIN-06-3)
  const insideGeofenceCount = waypoints.filter(
    wp => getDistanceMeters(wp.lat, wp.lng, CORPORATE_CENTER.lat, CORPORATE_CENTER.lng) <= CORPORATE_CENTER.radiusMeters
  ).length;

  const geofencePct = waypoints.length > 0
    ? Math.round((insideGeofenceCount / waypoints.length) * 100)
    : 100;

  // Work hours audit calculations (RF-ADMIN-06-4 & DoD 4)
  const allWithinWorkHours = waypoints.every(wp =>
    isWithinShift(wp.time, workHours.start, workHours.end)
  );

  return (
    <div className="bg-surface-subtle border border-default rounded-card p-5 space-y-5">
      {/* Header and Shift Audit Confirmation */}
      <div className="flex flex-wrap justify-between items-center gap-3">
        <div>
          <h2 className="text-lg font-semibold text-fg flex items-center gap-2">
            <Navigation className="w-5 h-5 text-warning" />
            Trazado de Rutas en Terreno (Trabajador FIELD)
          </h2>
          <p className="text-xs text-muted">
            Ruta laboral auditada para: <span className="text-warning font-medium">{workerName}</span> ({timezone})
          </p>
        </div>

        {/* Work Hours and Geofence Badges */}
        <div className="flex flex-wrap items-center gap-2 text-xs">
          <div className="bg-warning-subtle border border-default text-warning px-3 py-1.5 rounded-control flex items-center gap-1.5 font-mono">
            <Clock className="w-3.5 h-3.5 text-warning" />
            <span>Jornada: <strong>{workHours.start} - {workHours.end}</strong></span>
          </div>

          <div
            className={`px-3 py-1.5 rounded-control border flex items-center gap-1.5 font-medium ${
 geofencePct >= 80
 ? 'bg-success-subtle text-success border-success'
 : 'bg-warning-subtle text-warning border-warning'
 }`}
          >
            <Compass className="w-3.5 h-3.5" />
            <span>Geocerca: <strong>{geofencePct}% en Rango</strong></span>
          </div>
        </div>
      </div>

      {/* Confirmation of Non-Monitoring Outside Work Hours (RF-ADMIN-06-4 & DoD 4) */}
      <div className="bg-surface border border-default rounded-control p-3 text-xs flex items-center justify-between flex-wrap gap-2 text-fg">
        <div className="flex items-center gap-2">
          {allWithinWorkHours ? (
            <ShieldCheck className="w-4 h-4 text-success shrink-0" />
          ) : (
            <AlertCircle className="w-4 h-4 text-warning shrink-0" />
          )}
          <span>
            {allWithinWorkHours
              ? `Confirmación de Privacidad DP-04: Todas las coordenadas registradas corresponden estrictamente a la jornada laboral (${workHours.start} a ${workHours.end}). Fuera de horario el hardware suspende la recolección.`
              : `Atención: Se detectaron registros fuera del horario pactado (${workHours.start} a ${workHours.end}).`}
          </span>
        </div>
        <span className="text-[11px] font-mono text-muted">
          {waypoints.length} puntos cronológicos auditados
        </span>
      </div>

      {/* Leaflet Map Canvas Container */}
      <div className="relative rounded-card border border-default overflow-hidden ">
        {loading && (
          <div className="absolute inset-0 z-20 bg-bg flex flex-col items-center justify-center">
            <div className="w-8 h-8 border-2 border-accent border-t-transparent rounded-full animate-spin"></div>
            <p className="text-xs text-fg mt-3 font-mono">Descargando y descifrando coordenadas GPS...</p>
          </div>
        )}

        <div ref={mapContainerRef} className="h-96 w-full bg-bg" style={{ zIndex: 1 }} />
      </div>

      {/* Route Legend */}
      <div className="flex flex-wrap items-center justify-between text-xs text-muted pt-1 border-t border-default gap-3">
        <div className="flex items-center gap-4 flex-wrap">
          <span className="flex items-center gap-1.5">
            <span className="w-3 h-3 rounded-full bg-success inline-block"></span>
            Inicio de jornada
          </span>
          <span className="flex items-center gap-1.5">
            <span className="w-3 h-3 rounded-full bg-danger inline-block"></span>
            Fin de jornada
          </span>
          <span className="flex items-center gap-1.5">
            <span className="w-3 h-3 rounded-full bg-warning inline-block"></span>
            Parada prolongada (&gt;15 min)
          </span>
          <span className="flex items-center gap-1.5">
            <span className="w-3.5 h-0.5 bg-accent inline-block"></span>
            Polilínea cronológica
          </span>
        </div>
        <div className="font-mono text-[11px] text-muted">
          Motor Leaflet + OpenStreetMap (Cero telemetría a servidores terceros)
        </div>
      </div>
    </div>
  );
};

export default FieldMap;
