import React, { useState, useEffect } from 'react';
import ReactECharts from 'echarts-for-react';
import { useThemeColors, withAlpha } from '../hooks/useThemeColors';
import { Cpu, HardDrive, Keyboard, Usb, AlertCircle } from 'lucide-react';
import {
  TimeRangeFilter,
  getTimeRangeIso,
  fetchAndDecryptTelemetry,
  isDemoMode,
  sanitizeHtmlText,
} from '../api/tauri-bridge';

interface MetricTimelinePoint {
  time: string;
  cpu: number;
  memoryMb: number;
  kpm: number;
  cpm: number;
  windowTitle: string;
  idleSeconds?: number;
}

interface UsbEventItem {
  time: string;
  action: string;
  name: string;
  vendorId?: string;
  productId?: string;
}

interface ActivityChartProps {
  workerId?: string;
  workerName: string;
  timeRange: TimeRangeFilter;
}

const defaultMockTimeline: MetricTimelinePoint[] = [
  { time: "09:00", cpu: 14.2, memoryMb: 4210, kpm: 45, cpm: 12, windowTitle: "Visual Studio Code — telemetry-apps", idleSeconds: 5 },
  { time: "09:30", cpu: 22.8, memoryMb: 4350, kpm: 72, cpm: 24, windowTitle: "Terminal — cargo test", idleSeconds: 0 },
  { time: "10:00", cpu: 35.1, memoryMb: 4890, kpm: 88, cpm: 31, windowTitle: "Google Chrome — GitHub Pull Request", idleSeconds: 15 },
  { time: "10:30", cpu: 18.4, memoryMb: 4620, kpm: 34, cpm: 15, windowTitle: "Slack — #dev-team", idleSeconds: 20 },
  { time: "11:00", cpu: 12.0, memoryMb: 4400, kpm: 52, cpm: 19, windowTitle: "Visual Studio Code — buffer.rs", idleSeconds: 2 },
  { time: "11:30", cpu: 28.5, memoryMb: 4780, kpm: 94, cpm: 38, windowTitle: "Terminal — git commit", idleSeconds: 0 },
  { time: "12:00", cpu: 15.3, memoryMb: 4510, kpm: 41, cpm: 14, windowTitle: "Jira Software — Sprints", idleSeconds: 45 },
];

export const ActivityChart: React.FC<ActivityChartProps> = ({
  workerId,
  workerName,
  timeRange,
}) => {
  const c = useThemeColors();
  const [loading, setLoading] = useState<boolean>(false);
  const [dataPoints, setDataPoints] = useState<MetricTimelinePoint[]>([]);
  const [usbEvents, setUsbEvents] = useState<UsbEventItem[]>([]);
  const [fetchError, setFetchError] = useState<string | null>(null);

  useEffect(() => {
    console.debug("Dashboard context updated:", { workerId, range: timeRange });

    if (!workerId) {
      if (isDemoMode) {
        setDataPoints(defaultMockTimeline);
      } else {
        setDataPoints([]);
      }
      return;
    }

    const { from, to } = getTimeRangeIso(timeRange);
    let isCancelled = false;
    setLoading(true);
    setFetchError(null);

    Promise.all([
      fetchAndDecryptTelemetry(workerId, 'system', from, to).catch(err => {
        console.warn("Failed to fetch system telemetry:", err);
        return [];
      }),
      fetchAndDecryptTelemetry(workerId, 'productivity', from, to, 'BASIC').catch(err => {
        console.warn("Failed to fetch productivity basic telemetry:", err);
        return [];
      }),
    ])
      .then(([systemRecords, prodRecords]) => {
        if (isCancelled) return;

        // Build productivity basic lookup by timestamp minute (YYYY-MM-DDTHH:mm)
        const prodByMinute = new Map<string, { kpm: number; cpm: number }>();
        for (const rec of prodRecords) {
          const items = Array.isArray(rec.payload?.items) ? rec.payload.items : [rec.payload];
          for (const item of items) {
            if (!item) continue;
            const minKey = (item.minute || item.ts || rec.client_timestamp || '').slice(0, 16);
            if (minKey) {
              prodByMinute.set(minKey, {
                kpm: item.keystrokes ?? item.kpm ?? 0,
                cpm: item.clicks ?? item.cpm ?? 0,
              });
            }
          }
        }

        const points: MetricTimelinePoint[] = [];
        const events: UsbEventItem[] = [];

        for (const rec of systemRecords) {
          const items = Array.isArray(rec.payload?.items) ? rec.payload.items : [rec.payload];
          for (const item of items) {
            if (!item) continue;
            const tsStr = item.ts || rec.client_timestamp || '';
            const dateObj = new Date(tsStr);
            const timeLabel = isNaN(dateObj.getTime())
              ? tsStr
              : dateObj.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });

            const minKey = tsStr.slice(0, 16);
            const prod = prodByMinute.get(minKey);

            points.push({
              time: timeLabel,
              cpu: typeof item.cpu_pct === 'number' ? Math.round(item.cpu_pct * 10) / 10 : (item.cpu_usage ?? 0),
              memoryMb: typeof item.mem_used_mb === 'number' ? Math.round(item.mem_used_mb) : (item.ram_usage ?? 0),
              kpm: prod?.kpm ?? (item.keystrokes_per_minute ?? 0),
              cpm: prod?.cpm ?? (item.clicks_per_minute ?? 0),
              windowTitle: item.window_title || item.foreground_app || item.active_window || 'Sin título de ventana',
              idleSeconds: item.idle_seconds ?? undefined,
            });

            if (Array.isArray(item.usb_events)) {
              for (const ev of item.usb_events) {
                events.push({
                  time: timeLabel,
                  action: ev.action || 'DESCONOCIDO',
                  name: ev.name || 'Periférico USB',
                  vendorId: ev.vendor_id,
                  productId: ev.product_id,
                });
              }
            }
          }
        }

        if (points.length > 0) {
          setDataPoints(points);
          setUsbEvents(events);
        } else if (isDemoMode) {
          setDataPoints(defaultMockTimeline);
          setUsbEvents([]);
        } else {
          setDataPoints([]);
          setUsbEvents([]);
        }
      })
      .catch(err => {
        if (isCancelled) return;
        console.error("ActivityChart fetch error:", err);
        setFetchError(String(err));
        if (isDemoMode) {
          setDataPoints(defaultMockTimeline);
        } else {
          setDataPoints([]);
        }
      })
      .finally(() => {
        if (!isCancelled) setLoading(false);
      });

    return () => {
      isCancelled = true;
    };
  }, [workerId, timeRange.preset, timeRange.from, timeRange.to]);

  const avgCpu = dataPoints.length > 0
    ? (dataPoints.reduce((acc, d) => acc + d.cpu, 0) / dataPoints.length).toFixed(1)
    : '0.0';

  const avgKpm = dataPoints.length > 0
    ? Math.round(dataPoints.reduce((acc, d) => acc + d.kpm, 0) / dataPoints.length)
    : 0;

  const maxRam = dataPoints.length > 0
    ? Math.max(...dataPoints.map(d => d.memoryMb))
    : 0;

  const chartOption = {
    backgroundColor: 'transparent',
    tooltip: {
      trigger: 'axis',
      backgroundColor: c.surface,
      borderColor: c.border,
      textStyle: { color: c.text },
      axisPointer: { type: 'cross' },
    },
    legend: {
      data: ['Uso CPU (%)', 'Pulsaciones KPM', 'Memoria RAM (MB)'],
      textStyle: { color: c.textMuted },
      top: 0,
    },
    grid: { left: '3%', right: '4%', bottom: '3%', containLabel: true },
    xAxis: {
      type: 'category',
      boundaryGap: false,
      data: dataPoints.map(d => d.time),
      axisLine: { lineStyle: { color: c.borderStrong } },
      axisLabel: { color: c.textMuted },
    },
    yAxis: [
      {
        type: 'value',
        name: 'CPU / KPM',
        position: 'left',
        axisLine: { lineStyle: { color: c.accent } },
        axisLabel: { color: c.textMuted },
        splitLine: { lineStyle: { color: c.border } },
      },
      {
        type: 'value',
        name: 'RAM (MB)',
        position: 'right',
        axisLine: { lineStyle: { color: c.success } },
        axisLabel: { color: c.textMuted },
        splitLine: { show: false },
      },
    ],
    series: [
      {
        name: 'Uso CPU (%)',
        type: 'line',
        smooth: true,
        sampling: 'lttb', // RF-ADMIN-05-2: LTTB sampling for smooth rendering
        data: dataPoints.map(d => d.cpu),
        itemStyle: { color: c.accent },
        areaStyle: {
          color: {
            type: 'linear',
            x: 0,
            y: 0,
            x2: 0,
            y2: 1,
            colorStops: [
              { offset: 0, color: withAlpha(c.accent, 0.25) },
              { offset: 1, color: withAlpha(c.accent, 0) },
            ],
          },
        },
      },
      {
        name: 'Pulsaciones KPM',
        type: 'line',
        smooth: true,
        sampling: 'lttb', // RF-ADMIN-05-2: LTTB sampling
        data: dataPoints.map(d => d.kpm),
        itemStyle: { color: c.info },
      },
      {
        name: 'Memoria RAM (MB)',
        type: 'line',
        yAxisIndex: 1,
        smooth: true,
        sampling: 'lttb', // RF-ADMIN-05-2: LTTB sampling
        data: dataPoints.map(d => d.memoryMb),
        itemStyle: { color: c.success },
      },
    ],
  };

  return (
    <div className="bg-surface-subtle border border-default rounded-card p-5 space-y-5">
      {/* Header & KPI Summary */}
      <div className="flex flex-wrap justify-between items-center gap-3">
        <div>
          <h2 className="text-lg font-semibold text-fg">Actividad de Sistema & Productividad</h2>
          <p className="text-xs text-muted">
            Analítica E2E descifrada en memoria para: <span className="text-accent font-medium">{workerName}</span>
          </p>
        </div>
        <div className="flex flex-wrap gap-2 text-xs">
          <div className="bg-surface px-3 py-1.5 rounded-control border border-default flex items-center gap-2">
            <Cpu className="w-4 h-4 text-accent" />
            <span className="text-fg">Promedio CPU: <strong>{avgCpu}%</strong></span>
          </div>
          <div className="bg-surface px-3 py-1.5 rounded-control border border-default flex items-center gap-2">
            <Keyboard className="w-4 h-4 text-info" />
            <span className="text-fg">Promedio KPM: <strong>{avgKpm} pul/min</strong></span>
          </div>
          <div className="bg-surface px-3 py-1.5 rounded-control border border-default flex items-center gap-2">
            <HardDrive className="w-4 h-4 text-success" />
            <span className="text-fg">Pico RAM: <strong>{maxRam} MB</strong></span>
          </div>
        </div>
      </div>

      {/* Main Chart / Loading / Empty State */}
      {loading ? (
        <div className="h-80 w-full flex flex-col items-center justify-center bg-surface rounded-card border border-default">
          <div className="w-8 h-8 border-2 border-accent border-t-transparent rounded-full animate-spin"></div>
          <p className="text-xs text-muted mt-3 font-mono">Descargando y descifrando telemetría en RAM...</p>
        </div>
      ) : dataPoints.length === 0 ? (
        <div className="bg-surface border border-default rounded-card p-12 text-center space-y-3">
          <div className="w-12 h-12 rounded-full bg-surface-subtle text-muted flex items-center justify-center mx-auto">
            <AlertCircle className="w-6 h-6 text-warning" />
          </div>
          <h3 className="text-base font-semibold text-fg">Sin Registros en el Período</h3>
          <p className="text-xs text-muted max-w-md mx-auto leading-relaxed">
            {fetchError ? (
              <span className="text-danger font-mono">Error al consultar API: {fetchError}</span>
            ) : (
              <>No se encontraron muestras descifradas de actividad de sistema para <strong className="text-fg">{workerName}</strong> en el rango temporal seleccionado.</>
            )}
          </p>
        </div>
      ) : (
        <div className="h-80 w-full">
          <ReactECharts option={chartOption} style={{ height: '100%', width: '100%' }} />
        </div>
      )}

      {/* Foreground Window Titles Timeline */}
      {dataPoints.length > 0 && (
        <div className="border-t border-default pt-4">
          <div className="flex items-center justify-between mb-3">
            <h4 className="text-xs font-semibold text-fg uppercase tracking-wider">
              Línea Temporal de Aplicaciones Activas
            </h4>
            <span className="text-[11px] text-muted font-mono">
              {dataPoints.length} muestras analizadas (Muestreo LTTB)
            </span>
          </div>
          <div className="space-y-2 max-h-48 overflow-y-auto pr-2">
            {dataPoints.map((pt, idx) => (
              <div
                key={idx}
                className="flex justify-between items-center text-xs bg-surface p-2.5 rounded-control border border-default hover:border-strong transition"
              >
                <span className="font-mono text-fg font-medium tabular-nums shrink-0">{pt.time}</span>
                <span
                  className="text-fg truncate mx-3 flex-1"
                  title={pt.windowTitle}
                  dangerouslySetInnerHTML={{ __html: sanitizeHtmlText(pt.windowTitle) }}
                />
                <span className="text-muted font-mono text-[11px] shrink-0">
                  {pt.cpu.toFixed(1)}% CPU | {pt.kpm} KPM {pt.idleSeconds !== undefined ? `| ${pt.idleSeconds}s idle` : ''}
                </span>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* USB Connection Events Log */}
      {usbEvents.length > 0 && (
        <div className="border-t border-default pt-4">
          <div className="flex items-center gap-2 mb-3">
            <Usb className="w-4 h-4 text-success" />
            <h4 className="text-xs font-semibold text-fg uppercase tracking-wider">
              Registro de Conexión / Desconexión USB
            </h4>
          </div>
          <div className="space-y-2 max-h-36 overflow-y-auto pr-2">
            {usbEvents.map((ev, idx) => (
              <div
                key={idx}
                className="flex justify-between items-center text-xs bg-surface p-2 rounded-control border border-default"
              >
                <div className="flex items-center gap-2">
                  <span
                    className={`text-[10px] font-bold px-2 py-0.5 rounded ${
 ev.action === 'CONNECTED'
 ? 'bg-success-subtle text-success border border-default'
 : 'bg-danger-subtle text-danger border border-default'
 }`}
                  >
                    {ev.action}
                  </span>
                  <span className="text-fg">{ev.name}</span>
                  {(ev.vendorId || ev.productId) && (
                    <span className="text-[10px] text-muted font-mono">
                      (VID:{ev.vendorId || 'N/A'} PID:{ev.productId || 'N/A'})
                    </span>
                  )}
                </div>
                <span className="text-muted font-mono text-[11px]">{ev.time}</span>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
};
