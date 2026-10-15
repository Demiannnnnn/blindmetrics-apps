import React, { useMemo } from 'react';
import { Search, RefreshCw, FileSearch } from 'lucide-react';
import type { JourneyType, ReportKind, ReportPeriod } from '../api/types';
import { JOURNEY_TYPE_LABELS } from '../lib/schedule';
import {
  PERIOD_LABELS,
  filterWorkerOptions,
  rangeLimitLabel,
  type ReportFormState,
  type ReportWorkerOption,
} from '../lib/reports';

export interface WorkplaceOption {
  id: string;
  name: string;
}

interface ReportFiltersProps {
  report: ReportKind;
  value: ReportFormState;
  onChange: (next: ReportFormState) => void;
  workers: ReportWorkerOption[];
  workplaces: WorkplaceOption[];
  jobTitles: string[];
  optionsNote?: string | null;
  generating: boolean;
  error: string | null;
  onGenerate: () => void;
}

const inputClass =
  'w-full bg-surface border border-strong text-fg rounded-control px-2 py-1.5 text-xs focus:outline-hidden focus:border-accent';
const labelClass = 'block text-xs font-medium text-fg mb-1';

/**
 * R38 art. 25 filters (RF-ADMIN-16-3): all visible at once and combinable in any order —
 * workers (search by name or RUN + alphabetical multi-select), predefined period or range,
 * journey type, workplace, job title and hash.
 */
export const ReportFilters: React.FC<ReportFiltersProps> = ({
  report,
  value,
  onChange,
  workers,
  workplaces,
  jobTitles,
  optionsNote,
  generating,
  error,
  onGenerate,
}) => {
  const set = <K extends keyof ReportFormState>(key: K, v: ReportFormState[K]) => onChange({ ...value, [key]: v });
  const visibleWorkers = useMemo(() => filterWorkerOptions(workers, value.search), [workers, value.search]);
  const selected = new Set(value.selectedWorkerIds);

  const toggleWorker = (id: string) => {
    const next = selected.has(id)
      ? value.selectedWorkerIds.filter(w => w !== id)
      : [...value.selectedWorkerIds, id];
    set('selectedWorkerIds', next);
  };

  return (
    <form
      className="no-print bg-surface border border-default rounded-card p-4 space-y-4"
      onSubmit={e => {
        e.preventDefault();
        onGenerate();
      }}
    >
      <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-3">
        {/* Trabajadores */}
        <fieldset className="md:row-span-2 min-w-0">
          <legend className={labelClass}>Trabajadores</legend>
          <div className="relative mb-2">
            <Search className="w-3.5 h-3.5 text-muted absolute left-2 top-1/2 -translate-y-1/2" aria-hidden="true" />
            <input
              type="search"
              value={value.search}
              onChange={e => set('search', e.target.value)}
              placeholder="Buscar por nombre o RUN"
              aria-label="Buscar trabajador por nombre o RUN"
              maxLength={100}
              className={`${inputClass} pl-7`}
            />
          </div>
          <div
            className="max-h-44 overflow-y-auto border border-default rounded-control divide-y divide-default"
            role="group"
            aria-label="Selección de trabajadores"
          >
            {visibleWorkers.length === 0 ? (
              <p className="px-2 py-2 text-xs text-muted">
                {workers.length === 0
                  ? 'No hay trabajadores en la lista. Puedes buscar por nombre o RUN.'
                  : 'Ningún trabajador coincide con la búsqueda.'}
              </p>
            ) : (
              visibleWorkers.map(w => (
                <label key={w.id} className="flex items-start gap-2 px-2 py-1.5 text-xs cursor-pointer hover:bg-surface-subtle">
                  <input
                    type="checkbox"
                    checked={selected.has(w.id)}
                    onChange={() => toggleWorker(w.id)}
                    className="mt-0.5 accent-[var(--accent)]"
                  />
                  <span className="min-w-0">
                    <span className="text-fg">{w.name}</span>
                    {w.run && <span className="ml-1.5 font-mono text-muted">{w.run}</span>}
                  </span>
                </label>
              ))
            )}
          </div>
          <div className="flex items-center justify-between mt-1.5 text-[11px] text-muted">
            <span>
              {value.selectedWorkerIds.length === 0
                ? value.search.trim()
                  ? 'Sin selección: se buscará por el texto ingresado.'
                  : 'Sin selección se incluyen todos los trabajadores.'
                : `${value.selectedWorkerIds.length} seleccionado(s)`}
            </span>
            {value.selectedWorkerIds.length > 0 && (
              <button type="button" onClick={() => set('selectedWorkerIds', [])} className="text-accent hover:underline">
                Limpiar selección
              </button>
            )}
          </div>
          {optionsNote && <p className="mt-1 text-[11px] text-muted">{optionsNote}</p>}
        </fieldset>

        {/* Período o rango */}
        <fieldset className="min-w-0">
          <legend className={labelClass}>Período</legend>
          <div className="flex gap-3 text-xs mb-2">
            <label className="flex items-center gap-1.5 cursor-pointer">
              <input
                type="radio"
                name="report-mode"
                checked={value.mode === 'period'}
                onChange={() => set('mode', 'period')}
                className="accent-[var(--accent)]"
              />
              Predeterminado
            </label>
            <label className="flex items-center gap-1.5 cursor-pointer">
              <input
                type="radio"
                name="report-mode"
                checked={value.mode === 'range'}
                onChange={() => set('mode', 'range')}
                className="accent-[var(--accent)]"
              />
              Rango de fechas
            </label>
          </div>
          {value.mode === 'period' ? (
            <select
              value={value.period}
              onChange={e => set('period', e.target.value as ReportPeriod)}
              aria-label="Período predeterminado"
              className={inputClass}
            >
              {(Object.keys(PERIOD_LABELS) as ReportPeriod[]).map(p => (
                <option key={p} value={p}>
                  {PERIOD_LABELS[p]}
                </option>
              ))}
            </select>
          ) : (
            <div className="grid grid-cols-2 gap-2">
              <input
                type="date"
                value={value.from}
                onChange={e => set('from', e.target.value)}
                aria-label="Desde"
                className={inputClass}
              />
              <input
                type="date"
                value={value.to}
                onChange={e => set('to', e.target.value)}
                aria-label="Hasta"
                className={inputClass}
              />
              <p className="col-span-2 text-[11px] text-muted">Hasta {rangeLimitLabel(report)}.</p>
            </div>
          )}
        </fieldset>

        {/* Tipo de jornada */}
        <div className="min-w-0">
          <label className={labelClass} htmlFor="report-journey">
            Tipo de jornada
          </label>
          <select
            id="report-journey"
            value={value.journeyType}
            onChange={e => set('journeyType', e.target.value as JourneyType | '')}
            className={inputClass}
          >
            <option value="">Todas</option>
            {(Object.keys(JOURNEY_TYPE_LABELS) as JourneyType[]).map(j => (
              <option key={j} value={j}>
                {JOURNEY_TYPE_LABELS[j]}
              </option>
            ))}
          </select>
        </div>

        {/* Lugar */}
        <div className="min-w-0">
          <label className={labelClass} htmlFor="report-workplace">
            Lugar de trabajo
          </label>
          <select
            id="report-workplace"
            value={value.workplaceId}
            onChange={e => set('workplaceId', e.target.value)}
            className={inputClass}
          >
            <option value="">Todos</option>
            {workplaces.map(w => (
              <option key={w.id} value={w.id}>
                {w.name}
              </option>
            ))}
          </select>
        </div>

        {/* Cargo */}
        <div className="min-w-0">
          <label className={labelClass} htmlFor="report-job-title">
            Cargo
          </label>
          <input
            id="report-job-title"
            list="report-job-titles"
            value={value.jobTitle}
            onChange={e => set('jobTitle', e.target.value)}
            placeholder="Todos"
            maxLength={100}
            className={inputClass}
          />
          <datalist id="report-job-titles">
            {jobTitles.map(t => (
              <option key={t} value={t} />
            ))}
          </datalist>
        </div>

        {/* Hash */}
        <div className="min-w-0">
          <label className={labelClass} htmlFor="report-hash">
            Hash de una marca
          </label>
          <input
            id="report-hash"
            value={value.hash}
            onChange={e => set('hash', e.target.value)}
            placeholder="64 caracteres hexadecimales"
            maxLength={64}
            spellCheck={false}
            className={`${inputClass} font-mono`}
          />
        </div>
      </div>

      {error && (
        <p className="text-xs text-danger" role="alert">
          {error}
        </p>
      )}

      <div className="flex justify-end">
        <button
          type="submit"
          disabled={generating}
          className="px-4 py-2 bg-accent hover:bg-accent-hover text-accent-fg text-xs font-semibold rounded-control flex items-center gap-1.5 disabled:opacity-50"
        >
          {generating ? <RefreshCw className="w-3.5 h-3.5 animate-spin" /> : <FileSearch className="w-3.5 h-3.5" />}
          {generating ? 'Generando…' : 'Generar'}
        </button>
      </div>
    </form>
  );
};
