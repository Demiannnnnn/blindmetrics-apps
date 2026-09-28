import React from 'react';
import ReactECharts from 'echarts-for-react';
import { Cpu, HardDrive, Keyboard } from 'lucide-react';

interface MetricTimelinePoint {
  time: string;
  cpu: number;
  memoryMb: number;
  kpm: number;
  cpm: number;
  windowTitle: string;
}

interface ActivityChartProps {
  workerName: string;
  data?: MetricTimelinePoint[];
}

const defaultMockTimeline: MetricTimelinePoint[] = [
  { time: "09:00", cpu: 14.2, memoryMb: 4210, kpm: 45, cpm: 12, windowTitle: "Visual Studio Code — telemetry-apps" },
  { time: "09:30", cpu: 22.8, memoryMb: 4350, kpm: 72, cpm: 24, windowTitle: "Terminal — cargo test" },
  { time: "10:00", cpu: 35.1, memoryMb: 4890, kpm: 88, cpm: 31, windowTitle: "Google Chrome — GitHub Pull Request" },
  { time: "10:30", cpu: 18.4, memoryMb: 4620, kpm: 34, cpm: 15, windowTitle: "Slack — #dev-team" },
  { time: "11:00", cpu: 12.0, memoryMb: 4400, kpm: 52, cpm: 19, windowTitle: "Visual Studio Code — buffer.rs" },
  { time: "11:30", cpu: 28.5, memoryMb: 4780, kpm: 94, cpm: 38, windowTitle: "Terminal — git commit" },
  { time: "12:00", cpu: 15.3, memoryMb: 4510, kpm: 41, cpm: 14, windowTitle: "Jira Software — Sprints" },
];

export const ActivityChart: React.FC<ActivityChartProps> = ({ workerName, data = defaultMockTimeline }) => {
  const chartOption = {
    backgroundColor: 'transparent',
    tooltip: {
      trigger: 'axis',
      backgroundColor: '#1e293b',
      borderColor: '#334155',
      textStyle: { color: '#f8fafc' },
      axisPointer: { type: 'cross' }
    },
    legend: {
      data: ['Uso CPU (%)', 'Pulsaciones KPM', 'Memoria RAM (MB)'],
      textStyle: { color: '#94a3b8' },
      top: 0
    },
    grid: { left: '3%', right: '4%', bottom: '3%', containLabel: true },
    xAxis: {
      type: 'category',
      boundaryGap: false,
      data: data.map(d => d.time),
      axisLine: { lineStyle: { color: '#475569' } },
      axisLabel: { color: '#94a3b8' }
    },
    yAxis: [
      {
        type: 'value',
        name: 'CPU / KPM',
        position: 'left',
        axisLine: { lineStyle: { color: '#6366f1' } },
        axisLabel: { color: '#94a3b8' },
        splitLine: { lineStyle: { color: '#334155' } }
      },
      {
        type: 'value',
        name: 'RAM (MB)',
        position: 'right',
        axisLine: { lineStyle: { color: '#10b981' } },
        axisLabel: { color: '#94a3b8' },
        splitLine: { show: false }
      }
    ],
    series: [
      {
        name: 'Uso CPU (%)',
        type: 'line',
        smooth: true,
        data: data.map(d => d.cpu),
        itemStyle: { color: '#6366f1' },
        areaStyle: {
          color: {
            type: 'linear',
            x: 0, y: 0, x2: 0, y2: 1,
            colorStops: [
              { offset: 0, color: 'rgba(99, 102, 241, 0.4)' },
              { offset: 1, color: 'rgba(99, 102, 241, 0.0)' }
            ]
          }
        }
      },
      {
        name: 'Pulsaciones KPM',
        type: 'line',
        smooth: true,
        data: data.map(d => d.kpm),
        itemStyle: { color: '#38bdf8' }
      },
      {
        name: 'Memoria RAM (MB)',
        type: 'line',
        yAxisIndex: 1,
        smooth: true,
        data: data.map(d => d.memoryMb),
        itemStyle: { color: '#10b981' }
      }
    ]
  };

  return (
    <div className="bg-slate-800/80 border border-slate-700/80 rounded-xl p-5 shadow-lg space-y-5">
      <div className="flex flex-wrap justify-between items-center gap-3">
        <div>
          <h2 className="text-lg font-semibold text-white">Actividad de Sistema & Productividad</h2>
          <p className="text-xs text-slate-400">Analítica E2E descifrada en memoria para: <span className="text-indigo-400 font-medium">{workerName}</span></p>
        </div>
        <div className="flex gap-3 text-xs">
          <div className="bg-slate-900/60 px-3 py-1.5 rounded-lg border border-slate-700 flex items-center gap-2">
            <Cpu className="w-4 h-4 text-indigo-400" />
            <span className="text-slate-300">Promedio CPU: <strong>21.0%</strong></span>
          </div>
          <div className="bg-slate-900/60 px-3 py-1.5 rounded-lg border border-slate-700 flex items-center gap-2">
            <Keyboard className="w-4 h-4 text-sky-400" />
            <span className="text-slate-300">Promedio KPM: <strong>63 pul/min</strong></span>
          </div>
          <div className="bg-slate-900/60 px-3 py-1.5 rounded-lg border border-slate-700 flex items-center gap-2">
            <HardDrive className="w-4 h-4 text-emerald-400" />
            <span className="text-slate-300">RAM Muestreada: <strong>4.5 GB</strong></span>
          </div>
        </div>
      </div>

      <div className="h-80 w-full">
        <ReactECharts option={chartOption} style={{ height: '100%', width: '100%' }} />
      </div>

      {/* Foreground Window Titles Timeline */}
      <div className="border-t border-slate-700/80 pt-4">
        <h4 className="text-xs font-semibold text-slate-300 uppercase tracking-wider mb-3">Línea Temporal de Aplicaciones Activas</h4>
        <div className="space-y-2 max-h-40 overflow-y-auto pr-2">
          {data.map((pt, idx) => (
            <div key={idx} className="flex justify-between items-center text-xs bg-slate-900/40 p-2 rounded-lg border border-slate-700/40 hover:border-slate-600 transition">
              <span className="font-mono text-indigo-300 font-semibold">{pt.time}</span>
              <span className="text-slate-200 truncate mx-3 flex-1">{pt.windowTitle}</span>
              <span className="text-slate-400 font-mono text-[11px]">{pt.cpu.toFixed(1)}% CPU | {pt.kpm} KPM</span>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
};
