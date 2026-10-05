import React, { useState, useEffect } from 'react';
import ReactECharts from 'echarts-for-react';
import { useThemeColors } from '../hooks/useThemeColors';
import { Globe, ShieldCheck, AlertCircle, BarChart3, PieChart } from 'lucide-react';
import {
  TimeRangeFilter,
  getTimeRangeIso,
  fetchAndDecryptTelemetry,
  isDemoMode,
  validateFqdnPrivacy,
} from '../api/tauri-bridge';

interface DomainStats {
  fqdn: string;
  hits: number;
  category: string;
  isSanitizedValid: boolean;
}

interface NetworkExplorerProps {
  workerId?: string;
  workerName: string;
  timeRange: TimeRangeFilter;
}

const mockDomains: DomainStats[] = [
  { fqdn: "github.com", hits: 342, category: "Desarrollo", isSanitizedValid: true },
  { fqdn: "docs.rs", hits: 215, category: "Desarrollo", isSanitizedValid: true },
  { fqdn: "slack.com", hits: 180, category: "Comunicación", isSanitizedValid: true },
  { fqdn: "stackoverflow.com", hits: 98, category: "Desarrollo", isSanitizedValid: true },
  { fqdn: "notion.so", hits: 87, category: "Productividad", isSanitizedValid: true },
  { fqdn: "google.com", hits: 76, category: "Búsqueda", isSanitizedValid: true },
  { fqdn: "aws.amazon.com", hits: 54, category: "Infraestructura", isSanitizedValid: true },
];

function categorizeFqdn(fqdn: string, declaredCategory?: string | null): string {
  if (declaredCategory && declaredCategory.trim().length > 0) {
    return declaredCategory;
  }
  const f = fqdn.toLowerCase();
  if (f.includes('github') || f.includes('gitlab') || f.includes('docs.rs') || f.includes('crates.io') || f.includes('stackoverflow')) {
    return 'Desarrollo';
  }
  if (f.includes('slack') || f.includes('teams') || f.includes('zoom') || f.includes('meet.google') || f.includes('discord')) {
    return 'Comunicación';
  }
  if (f.includes('google') || f.includes('bing') || f.includes('duckduckgo')) {
    return 'Búsqueda';
  }
  if (f.includes('aws') || f.includes('azure') || f.includes('cloudflare') || f.includes('digitalocean')) {
    return 'Infraestructura';
  }
  if (f.includes('notion') || f.includes('jira') || f.includes('trello') || f.includes('confluence') || f.includes('asana')) {
    return 'Productividad';
  }
  return 'General';
}

export const NetworkExplorer: React.FC<NetworkExplorerProps> = ({
  workerId,
  workerName,
  timeRange,
}) => {
  const c = useThemeColors();
  const [loading, setLoading] = useState<boolean>(false);
  const [domains, setDomains] = useState<DomainStats[]>([]);
  const [allCompliant, setAllCompliant] = useState<boolean>(true);

  useEffect(() => {
    console.debug("Dashboard context updated:", { workerId, range: timeRange });

    if (!workerId) {
      if (isDemoMode) {
        setDomains(mockDomains);
        setAllCompliant(true);
      } else {
        setDomains([]);
      }
      return;
    }

    const { from, to } = getTimeRangeIso(timeRange);
    let isCancelled = false;
    setLoading(true);

    fetchAndDecryptTelemetry(workerId, 'network', from, to)
      .then(records => {
        if (isCancelled) return;

        const aggMap = new Map<string, { hits: number; category: string; valid: boolean }>();
        let compliantSoFar = true;

        for (const rec of records) {
          const items = Array.isArray(rec.payload?.items) ? rec.payload.items : [rec.payload];
          for (const item of items) {
            if (!item || !item.fqdn) continue;

            const check = validateFqdnPrivacy(item.fqdn);
            if (!check.isValid) {
              compliantSoFar = false;
            }

            const cleanHost = check.sanitized;
            if (!cleanHost) continue;

            const hits = typeof item.hits === 'number' ? item.hits : 1;
            const category = categorizeFqdn(cleanHost, item.category);

            const existing = aggMap.get(cleanHost);
            if (existing) {
              existing.hits += hits;
            } else {
              aggMap.set(cleanHost, {
                hits,
                category,
                valid: check.isValid,
              });
            }
          }
        }

        const domainList: DomainStats[] = Array.from(aggMap.entries())
          .map(([fqdn, data]) => ({
            fqdn,
            hits: data.hits,
            category: data.category,
            isSanitizedValid: data.valid,
          }))
          .sort((a, b) => b.hits - a.hits);

        if (domainList.length > 0) {
          setDomains(domainList);
          setAllCompliant(compliantSoFar);
        } else if (isDemoMode) {
          setDomains(mockDomains);
          setAllCompliant(true);
        } else {
          setDomains([]);
          setAllCompliant(true);
        }
      })
      .catch(err => {
        if (isCancelled) return;
        console.error("NetworkExplorer fetch error:", err);
        if (isDemoMode) {
          setDomains(mockDomains);
          setAllCompliant(true);
        } else {
          setDomains([]);
        }
      })
      .finally(() => {
        if (!isCancelled) setLoading(false);
      });

    return () => {
      isCancelled = true;
    };
  }, [workerId, timeRange.preset, timeRange.from, timeRange.to]);

  const top10 = domains.slice(0, 10);

  const pieOption = {
    backgroundColor: 'transparent',
    tooltip: { trigger: 'item', formatter: '{b}: {c} consultas ({d}%)' },
    series: [
      {
        name: 'Dominios FQDN',
        type: 'pie',
        radius: ['42%', '70%'],
        avoidLabelOverlap: false,
        itemStyle: { borderRadius: 6, borderColor: c.surface, borderWidth: 2 },
        label: { show: true, color: c.text, formatter: '{b}' },
        data: top10.map(d => ({ value: d.hits, name: d.fqdn })),
      },
    ],
  };

  const barOption = {
    backgroundColor: 'transparent',
    tooltip: { trigger: 'axis', axisPointer: { type: '' } },
    grid: { left: '3%', right: '5%', bottom: '3%', top: '5%', containLabel: true },
    xAxis: {
      type: 'value',
      axisLine: { lineStyle: { color: c.borderStrong } },
      axisLabel: { color: c.textMuted },
      splitLine: { lineStyle: { color: c.border } },
    },
    yAxis: {
      type: 'category',
      data: top10.slice(0, 6).map(d => d.fqdn).reverse(),
      axisLine: { lineStyle: { color: c.borderStrong } },
      axisLabel: { color: c.textMuted },
    },
    series: [
      {
        name: 'Visitas',
        type: 'bar',
        data: top10.slice(0, 6).map(d => d.hits).reverse(),
        itemStyle: {
          color: c.accent,
          borderRadius: [0, 4, 4, 0],
        },
      },
    ],
  };

  return (
    <div className="bg-surface-subtle border border-default rounded-card p-5 space-y-5">
      {/* Header and Compliance Verification Banner */}
      <div className="flex flex-wrap justify-between items-start gap-3">
        <div>
          <h2 className="text-lg font-semibold text-fg flex items-center gap-2">
            <Globe className="w-5 h-5 text-accent" />
            Navegación de Red (Solo Dominios FQDN)
          </h2>
          <p className="text-xs text-muted">
            Inspección de dominios para: <span className="text-accent font-medium">{workerName}</span>
          </p>
        </div>

        {/* RF-ADMIN-05-4 & DoD 4: Visual Confirmation of Network Minimization */}
        <div className="flex items-center gap-2">
          {allCompliant ? (
            <div className="bg-success-subtle border border-success text-success text-xs px-3 py-1.5 rounded-control flex items-center gap-1.5 font-medium ">
              <ShieldCheck className="w-4 h-4 text-success" />
              <span>Validación DP-02: 100% FQDN (Sin rutas ni parámetros GET)</span>
            </div>
          ) : (
            <div className="bg-warning-subtle border border-warning text-warning text-xs px-3 py-1.5 rounded-control flex items-center gap-1.5 font-medium ">
              <AlertCircle className="w-4 h-4 text-warning" />
              <span>Alerta: Se detectaron entradas no conformes que fueron sanitizadas</span>
            </div>
          )}
        </div>
      </div>

      {loading ? (
        <div className="h-72 w-full flex flex-col items-center justify-center bg-surface rounded-card border border-default">
          <div className="w-8 h-8 border-2 border-accent border-t-transparent rounded-full animate-spin"></div>
          <p className="text-xs text-muted mt-3 font-mono">Descargando y descifrando tráfico de red...</p>
        </div>
      ) : domains.length === 0 ? (
        <div className="bg-surface border border-default rounded-card p-12 text-center space-y-3">
          <div className="w-12 h-12 rounded-full bg-surface-subtle text-muted flex items-center justify-center mx-auto">
            <AlertCircle className="w-6 h-6 text-accent" />
          </div>
          <h3 className="text-base font-semibold text-fg">Sin Registros de Red en el Período</h3>
          <p className="text-xs text-muted max-w-md mx-auto leading-relaxed">
            No se registraron dominios FQDN para <strong className="text-fg">{workerName}</strong> en el rango temporal seleccionado.
          </p>
        </div>
      ) : (
        <div className="space-y-6">
          {/* Charts Row */}
          <div className="grid grid-cols-1 lg:grid-cols-2 gap-5 items-center">
            {/* Pie Chart */}
            <div className="bg-surface p-4 rounded-card border border-default">
              <div className="flex items-center gap-2 mb-2 text-xs font-semibold text-fg">
                <PieChart className="w-4 h-4 text-accent" />
                <span>Distribución de Dominios Principales</span>
              </div>
              <div className="h-64 w-full">
                <ReactECharts option={pieOption} style={{ height: '100%', width: '100%' }} />
              </div>
            </div>

            {/* Bar Chart */}
            <div className="bg-surface p-4 rounded-card border border-default">
              <div className="flex items-center gap-2 mb-2 text-xs font-semibold text-fg">
                <BarChart3 className="w-4 h-4 text-success" />
                <span>Top Dominios por Frecuencia de Consultas</span>
              </div>
              <div className="h-64 w-full">
                <ReactECharts option={barOption} style={{ height: '100%', width: '100%' }} />
              </div>
            </div>
          </div>

          {/* Detailed FQDN Frequency Table */}
          <div className="border-t border-default pt-4">
            <div className="flex justify-between items-center mb-3">
              <h4 className="text-xs font-semibold text-fg uppercase tracking-wider">
                Frecuencia y Categorización de Dominios Auditados
              </h4>
              <span className="text-[11px] text-muted font-mono">
                {domains.length} dominios canónicos identificados
              </span>
            </div>

            <div className="space-y-2 max-h-72 overflow-y-auto pr-2">
              {domains.map((item, idx) => (
                <div
                  key={idx}
                  className="flex justify-between items-center text-xs bg-surface p-2.5 rounded-control border border-default hover:border-strong transition"
                >
                  <div className="flex items-center gap-3">
                    <span className="font-mono text-fg font-medium">{item.fqdn}</span>
                    <span className="text-[10px] px-2 py-0.5 rounded bg-surface-subtle text-muted border border-default">
                      RFC 1035
                    </span>
                  </div>
                  <div className="flex items-center gap-3">
                    <span className="text-[11px] px-2.5 py-0.5 rounded-full bg-accent-subtle border border-accent text-accent font-medium">
                      {item.category}
                    </span>
                    <span className="font-semibold text-fg font-mono">
                      {item.hits} consultas
                    </span>
                  </div>
                </div>
              ))}
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
