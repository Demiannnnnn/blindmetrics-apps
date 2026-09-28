import React from 'react';
import ReactECharts from 'echarts-for-react';
import { Globe, ShieldCheck } from 'lucide-react';

interface DomainStats {
  fqdn: string;
  hits: number;
  category: string;
}

const mockDomains: DomainStats[] = [
  { fqdn: "github.com", hits: 342, category: "Desarrollo" },
  { fqdn: "docs.rs", hits: 215, category: "Desarrollo" },
  { fqdn: "slack.com", hits: 180, category: "Comunicación" },
  { fqdn: "stackoverflow.com", hits: 98, category: "Desarrollo" },
  { fqdn: "notion.so", hits: 87, category: "Productividad" },
  { fqdn: "google.com", hits: 76, category: "Búsqueda" },
  { fqdn: "aws.amazon.com", hits: 54, category: "Infraestructura" },
];

export const NetworkExplorer: React.FC<{ workerName: string }> = ({ workerName }) => {
  const chartOption = {
    backgroundColor: 'transparent',
    tooltip: { trigger: 'item', formatter: '{b}: {c} consultas ({d}%)' },
    series: [
      {
        name: 'Dominios FQDN',
        type: 'pie',
        radius: ['45%', '70%'],
        avoidLabelOverlap: false,
        itemStyle: { borderRadius: 8, borderColor: '#1e293b', borderWidth: 2 },
        label: { show: true, color: '#cbd5e1', formatter: '{b}' },
        data: mockDomains.map(d => ({ value: d.hits, name: d.fqdn })),
      }
    ]
  };

  return (
    <div className="bg-slate-800/80 border border-slate-700/80 rounded-xl p-5 shadow-lg space-y-5">
      <div className="flex justify-between items-start">
        <div>
          <h2 className="text-lg font-semibold text-white flex items-center gap-2">
            <Globe className="w-5 h-5 text-indigo-400" />
            Navegación de Red (Solo Dominios FQDN)
          </h2>
          <p className="text-xs text-slate-400">Inspección de dominios para: <span className="text-indigo-400 font-medium">{workerName}</span></p>
        </div>
        <div className="bg-emerald-950/60 border border-emerald-800/40 text-emerald-300 text-xs px-3 py-1.5 rounded-lg flex items-center gap-1.5">
          <ShieldCheck className="w-4 h-4 text-emerald-400" />
          <span>Cumplimiento GDPR: Sin rutas ni queries</span>
        </div>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-5 items-center">
        <div className="h-72 w-full">
          <ReactECharts option={chartOption} style={{ height: '100%', width: '100%' }} />
        </div>

        <div className="space-y-2 max-h-72 overflow-y-auto pr-2">
          <h4 className="text-xs font-semibold text-slate-400 uppercase tracking-wider mb-2">Tabla de Frecuencia de Dominios</h4>
          {mockDomains.map((item, idx) => (
            <div key={idx} className="flex justify-between items-center text-xs bg-slate-900/40 p-2.5 rounded-lg border border-slate-700/50 hover:border-slate-600 transition">
              <span className="font-mono text-slate-200">{item.fqdn}</span>
              <div className="flex items-center gap-3">
                <span className="text-[11px] px-2 py-0.5 rounded bg-slate-700/60 text-slate-300 font-medium">{item.category}</span>
                <span className="font-semibold text-indigo-300 font-mono">{item.hits} visitas</span>
              </div>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
};
