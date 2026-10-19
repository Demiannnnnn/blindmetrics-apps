import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { FileText } from 'lucide-react';
import * as bridge from '../api/tauri-bridge';
import type { Correction, DataSource, DaySummary, MarkRecord, ServerMark } from '../api/types';
import { useApp } from '../context/AppContext';
import { MarkDetail } from '../components/MarkDetail';
import { Modal } from '../components/Modal';
import { BUTTON_DANGER, BUTTON_SECONDARY, CARD, Notice, Skeleton, StatusBadge } from '../components/ui';
import {
  MAX_SUMMARY_DAYS,
  absenceLabel,
  attendanceErrorMessage,
  correctionStatusLabel,
  countdownLabel,
  customRangeError,
  daysBetween,
  describeAnomaly,
  effectiveTs,
  formatCalendarDate,
  formatDateCl,
  formatTimeCl,
  groupRowsByDay,
  localDateKey,
  markStatusLabel,
  markTypeLabel,
  mergeMarks,
  opposeUntilLabel,
  rangeForPreset,
  shortClock,
  sortCorrections,
  type DateRange,
  type MarkRow,
  type RangePreset,
} from '../lib/attendance';

const PRESETS: Array<{ id: RangePreset; label: string }> = [
  { id: 'THIS_WEEK', label: 'Esta semana' },
  { id: 'LAST_WEEK', label: 'Semana pasada' },
  { id: 'THIS_MONTH', label: 'Este mes' },
  { id: 'LAST_MONTH', label: 'Mes pasado' },
  { id: 'CUSTOM', label: 'Personalizado' },
];

const KIND_LABELS: Record<string, string> = {
  MODIFY: 'Modificar una marca',
  ADD: 'Agregar una marca olvidada',
  FIX: 'Ajuste',
};

interface RecordsData {
  source: DataSource;
  days: DaySummary[] | null;
  marks: ServerMark[];
  local: MarkRecord[];
  page: number;
  totalPages: number;
}

/** Day of the list: server summary when available, plus the rows of that day. */
interface DayItem {
  date: string;
  summary: DaySummary | null;
  rows: MarkRow[];
}

function buildDays(data: RecordsData, timeZone: string | null): DayItem[] {
  if (data.days) {
    const unsent = groupRowsByDay(mergeMarks([], data.local), timeZone);
    const unsentByDate = new Map(unsent.map((d) => [d.date, d.rows]));
    const items: DayItem[] = data.days.map((s) => {
      const rows = mergeMarks(s.marks, []);
      const extra = unsentByDate.get(s.date) ?? [];
      unsentByDate.delete(s.date);
      return { date: s.date, summary: s, rows: [...rows, ...extra].sort((a, b) => Date.parse(a.ts) - Date.parse(b.ts)) };
    });
    for (const [date, rows] of unsentByDate) items.push({ date, summary: null, rows });
    return items.sort((a, b) => (a.date < b.date ? 1 : a.date > b.date ? -1 : 0));
  }
  return groupRowsByDay(mergeMarks(data.marks, data.local), timeZone).map((d) => ({
    date: d.date,
    summary: null,
    rows: d.rows,
  }));
}

const DayCard: React.FC<{ item: DayItem; timeZone: string | null; onOpen: (r: MarkRow) => void }> = ({
  item,
  timeZone,
  onOpen,
}) => {
  const s = item.summary;
  const absence = s ? absenceLabel(s.absence, item.rows.length > 0, !!s.agreed) : null;
  return (
    <li className="py-3">
      <div className="flex flex-wrap items-baseline gap-x-2 gap-y-1">
        <h3 className="text-xs font-semibold text-fg tabular-nums">{formatCalendarDate(item.date)}</h3>
        {s?.agreed && (
          <span className="text-[11px] text-muted tabular-nums">
            Pactada {shortClock(s.agreed.start)} – {shortClock(s.agreed.end)}
            {s.agreed.break ? ` · Colación ${s.agreed.break.split('-').map(shortClock).join(' – ')}` : ''}
          </span>
        )}
        {s?.altered && <StatusBadge tone="info">Corregida</StatusBadge>}
        {absence && <StatusBadge tone="muted">{absence}</StatusBadge>}
      </div>
      {s?.absence_note && <p className="text-[11px] text-muted mt-0.5">{s.absence_note}</p>}
      {item.rows.length > 0 && (
        <ul className="mt-1.5 flex flex-wrap gap-1.5">
          {item.rows.map((r) => {
            const st = markStatusLabel(r.status, r.rejectionReason);
            return (
              <li key={r.key}>
                <button
                  type="button"
                  onClick={() => onOpen(r)}
                  className="inline-flex items-center gap-1.5 h-7 px-2 rounded-control border border-default bg-surface text-[11px] hover:bg-surface-subtle focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent"
                  aria-label={`${markTypeLabel(r.type)} ${formatTimeCl(r.ts, timeZone)}, ${st.label}`}
                >
                  <span className="font-mono tabular-nums">{formatTimeCl(r.ts, timeZone)}</span>
                  <span>{markTypeLabel(r.type)}</span>
                  {r.status !== 'CONFIRMED' && <StatusBadge tone={st.tone}>{st.label}</StatusBadge>}
                  {r.corrected && <StatusBadge tone="info">Corregida</StatusBadge>}
                </button>
              </li>
            );
          })}
        </ul>
      )}
      {s && (s.missing || s.extra || s.anomalies.length > 0) && (
        <div className="mt-1.5 text-[11px] space-y-0.5">
          {s.missing && (
            <p>
              <span className="text-muted">Tiempo faltante: </span>
              <span className="font-mono tabular-nums">{s.missing}</span>
            </p>
          )}
          {s.extra && (
            <p>
              <span className="text-muted">Tiempo extra: </span>
              <span className="font-mono tabular-nums">{s.extra}</span>
            </p>
          )}
          {s.anomalies.length > 0 && <p className="text-warning">{s.anomalies.map(describeAnomaly).join(' · ')}</p>}
        </div>
      )}
    </li>
  );
};

const CorrectionCard: React.FC<{
  c: Correction;
  timeZone: string | null;
  now: number;
  busy: boolean;
  onAccept?: () => void;
  onOppose?: () => void;
}> = ({ c, timeZone, now, busy, onAccept, onOppose }) => {
  const original = c.original_mark;
  const status = correctionStatusLabel(c.status);
  const pending = c.status === 'PENDING';
  return (
    <li className="py-3 space-y-1.5">
      <div className="flex flex-wrap items-center gap-2">
        <span className="text-xs font-semibold text-fg">{KIND_LABELS[c.kind] ?? 'Corrección'}</span>
        <span className="text-xs text-muted">· {markTypeLabel(c.type)}</span>
        <StatusBadge tone={status.tone}>{status.label}</StatusBadge>
      </div>
      <dl className="grid grid-cols-[auto_1fr] gap-x-3 gap-y-0.5 text-xs">
        <dt className="text-muted">Original</dt>
        <dd className="tabular-nums">
          {original
            ? `${markTypeLabel(original.type)} ${formatDateCl(effectiveTs(original), timeZone)} ${formatTimeCl(effectiveTs(original), timeZone)}`
            : 'Sin marca original (marca olvidada)'}
        </dd>
        <dt className="text-muted">Propuesta</dt>
        <dd className="tabular-nums">
          {markTypeLabel(c.type)} {formatDateCl(c.proposed_ts, timeZone)} {formatTimeCl(c.proposed_ts, timeZone)}
        </dd>
        <dt className="text-muted">Motivo</dt>
        <dd className="select-text">{c.reason}</dd>
      </dl>
      {pending && (
        <>
          <p className="text-[11px] text-muted tabular-nums">
            {[opposeUntilLabel(c.respond_before, timeZone), countdownLabel(c.respond_before, now)]
              .filter(Boolean)
              .join(' · ')}
          </p>
          <div className="flex gap-2 pt-0.5">
            <button type="button" className={BUTTON_SECONDARY} disabled={busy} onClick={onAccept}>
              Aceptar
            </button>
            <button type="button" className={BUTTON_SECONDARY} disabled={busy} onClick={onOppose}>
              Oponerme
            </button>
          </div>
        </>
      )}
      {!pending && c.responded_at && (
        <p className="text-[11px] text-muted tabular-nums">
          Respondida el {formatDateCl(c.responded_at, timeZone)} {formatTimeCl(c.responded_at, timeZone)}
        </p>
      )}
    </li>
  );
};

/** Registros (spec §4): days of a range, mark detail and corrections (ATT-05, ATT-06, ATT-09). */
export const RecordsView: React.FC = () => {
  const { snapshot, refreshSnapshot } = useApp();
  const timeZone = snapshot?.attendance_today?.server?.timezone ?? snapshot?.timezone ?? null;
  const [now, setNow] = useState(() => Date.now());
  const todayKey = localDateKey(now, timeZone);

  const [preset, setPreset] = useState<RangePreset>('THIS_WEEK');
  const [custom, setCustom] = useState<DateRange>({ from: '', to: '' });
  const [data, setData] = useState<RecordsData | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [detail, setDetail] = useState<{ row: MarkRow; source: DataSource } | null>(null);

  const [corrections, setCorrections] = useState<Correction[] | null>(null);
  const [corrError, setCorrError] = useState<string | null>(null);
  const [corrBusy, setCorrBusy] = useState(false);
  const [opposing, setOpposing] = useState<Correction | null>(null);

  useEffect(() => {
    const id = window.setInterval(() => setNow(Date.now()), 30_000);
    return () => window.clearInterval(id);
  }, []);

  const range: DateRange | null = useMemo(
    () => (preset === 'CUSTOM' ? custom : rangeForPreset(preset, todayKey)),
    [preset, custom, todayKey]
  );
  const rangeError = preset === 'CUSTOM' && range ? customRangeError(range, todayKey) : null;

  const load = useCallback(
    async (r: DateRange, page = 1, previous: RecordsData | null = null) => {
      setLoading(true);
      setError(null);
      try {
        const marksPage = await bridge.attendanceMarks(r.from, r.to, page);
        let days: DaySummary[] | null = previous?.days ?? null;
        if (page === 1 && marksPage.source === 'SERVER' && daysBetween(r.from, r.to) < MAX_SUMMARY_DAYS) {
          try {
            days = await bridge.attendanceDays(r.from, r.to);
          } catch {
            days = null; // summaries unavailable: the marks are still listed (no invented totals)
          }
        }
        setData({
          source: marksPage.source,
          days: marksPage.source === 'SERVER' ? days : null,
          marks: [...(previous?.marks ?? []), ...marksPage.marks],
          local: page === 1 ? marksPage.local : previous?.local ?? [],
          page,
          totalPages: marksPage.pagination?.total_pages ?? 1,
        });
      } catch (err) {
        setError(attendanceErrorMessage(bridge.toCommandError(err)));
      } finally {
        setLoading(false);
      }
    },
    []
  );

  const loadCorrections = useCallback(async () => {
    setCorrError(null);
    try {
      setCorrections(await bridge.listCorrections());
    } catch (err) {
      const cmd = bridge.toCommandError(err);
      setCorrError(cmd.code === 'OFFLINE' ? 'Las correcciones se consultan con conexión.' : attendanceErrorMessage(cmd));
    }
  }, []);

  useEffect(() => {
    loadCorrections();
  }, [loadCorrections, snapshot?.pending_corrections]);

  useEffect(() => {
    if (range && !rangeError && range.from && range.to) load(range);
  }, [range, rangeError, load]);

  const respond = async (c: Correction, accept: boolean) => {
    setCorrBusy(true);
    setCorrError(null);
    try {
      await bridge.respondCorrection(c.id, accept);
      setOpposing(null);
      await Promise.all([loadCorrections(), refreshSnapshot()]);
      if (range && !rangeError) load(range);
    } catch (err) {
      setCorrError(attendanceErrorMessage(bridge.toCommandError(err)));
      setOpposing(null);
      loadCorrections();
    } finally {
      setCorrBusy(false);
    }
  };

  const sorted = corrections ? sortCorrections(corrections) : null;
  const days = data ? buildDays(data, timeZone) : [];

  return (
    <div className="p-6 max-w-lg space-y-4">
      <div className="flex items-center gap-2">
        <FileText className="w-5 h-5 text-accent" aria-hidden="true" />
        <h1 className="text-base font-semibold text-fg">Registros</h1>
      </div>

      {/* Corrections first: pending ones need an answer within 48 h (ATT-05). */}
      <section className={CARD} aria-label="Correcciones">
        <h2 className="text-xs font-semibold text-fg">Correcciones</h2>
        {corrError && (
          <div className="mt-2">
            <Notice tone="warning" role="alert">
              {corrError}
            </Notice>
          </div>
        )}
        {!sorted && !corrError ? (
          <div className="mt-2 space-y-2">
            <Skeleton className="h-3 w-3/4" />
            <Skeleton className="h-3 w-1/2" />
          </div>
        ) : sorted && sorted.pending.length + sorted.history.length === 0 ? (
          <p className="text-xs text-muted mt-1">Tu empleador no ha propuesto correcciones.</p>
        ) : sorted ? (
          <>
            {sorted.pending.length > 0 && (
              <ul className="divide-y divide-default">
                {sorted.pending.map((c) => (
                  <CorrectionCard
                    key={c.id}
                    c={c}
                    timeZone={timeZone}
                    now={now}
                    busy={corrBusy}
                    onAccept={() => respond(c, true)}
                    onOppose={() => setOpposing(c)}
                  />
                ))}
              </ul>
            )}
            {sorted.history.length > 0 && (
              <>
                <h3 className="text-[11px] font-medium text-muted mt-3">Historial</h3>
                <ul className="divide-y divide-default">
                  {sorted.history.map((c) => (
                    <CorrectionCard key={c.id} c={c} timeZone={timeZone} now={now} busy={corrBusy} />
                  ))}
                </ul>
              </>
            )}
          </>
        ) : null}
      </section>

      <section className={CARD} aria-label="Marcas y jornadas">
        <div className="flex flex-wrap gap-1.5" role="group" aria-label="Rango">
          {PRESETS.map((p) => (
            <button
              key={p.id}
              type="button"
              aria-pressed={preset === p.id}
              onClick={() => setPreset(p.id)}
              className={`h-7 px-2.5 rounded-control border text-xs focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent ${
                preset === p.id ? 'border-accent text-accent bg-accent-subtle' : 'border-default text-fg hover:bg-surface-subtle'
              }`}
            >
              {p.label}
            </button>
          ))}
        </div>
        {preset === 'CUSTOM' && (
          <div className="mt-3 flex flex-wrap items-end gap-2">
            <label className="text-xs">
              <span className="block text-muted mb-1">Desde</span>
              <input
                type="date"
                value={custom.from}
                max={todayKey}
                onChange={(e) => setCustom((c) => ({ ...c, from: e.target.value }))}
                className="h-8 px-2 rounded-control border border-strong bg-surface text-fg text-xs"
              />
            </label>
            <label className="text-xs">
              <span className="block text-muted mb-1">Hasta</span>
              <input
                type="date"
                value={custom.to}
                max={todayKey}
                onChange={(e) => setCustom((c) => ({ ...c, to: e.target.value }))}
                className="h-8 px-2 rounded-control border border-strong bg-surface text-fg text-xs"
              />
            </label>
            {rangeError && <p className="text-xs text-danger w-full">{rangeError}</p>}
          </div>
        )}

        <div className="mt-3">
          {data?.source === 'LOCAL' && (
            <div className="mb-2">
              <StatusBadge tone="muted">Datos locales (sin conexión)</StatusBadge>
            </div>
          )}
          {data && data.source === 'SERVER' && !data.days && range && daysBetween(range.from, range.to) >= MAX_SUMMARY_DAYS && (
            <p className="text-[11px] text-muted mb-2">
              El resumen de cada día (tiempo faltante y extra) se muestra para rangos de hasta {MAX_SUMMARY_DAYS} días.
            </p>
          )}
          {error ? (
            <div className="space-y-2">
              <Notice tone="danger" role="alert">
                {error}
              </Notice>
              {range && (
                <button type="button" className={BUTTON_SECONDARY} onClick={() => load(range)}>
                  Reintentar
                </button>
              )}
            </div>
          ) : loading && !data ? (
            <div className="space-y-2">
              <Skeleton className="h-4 w-1/2" />
              <Skeleton className="h-6 w-full" />
              <Skeleton className="h-4 w-1/2" />
            </div>
          ) : data && days.length === 0 ? (
            <p className="text-xs text-muted">No hay marcas en este rango.</p>
          ) : (
            <ul className="divide-y divide-default">
              {days.map((d) => (
                <DayCard
                  key={d.date}
                  item={d}
                  timeZone={timeZone}
                  onOpen={(row) => setDetail({ row, source: data?.source ?? 'SERVER' })}
                />
              ))}
            </ul>
          )}
          {data && !data.days && data.page < data.totalPages && range && (
            <button
              type="button"
              className={`${BUTTON_SECONDARY} mt-2`}
              disabled={loading}
              onClick={() => load(range, data.page + 1, data)}
            >
              {loading ? 'Cargando…' : 'Cargar más marcas'}
            </button>
          )}
        </div>
      </section>

      <MarkDetail
        row={detail?.row ?? null}
        source={detail?.source}
        onClose={() => setDetail(null)}
        timeZone={timeZone}
        organization={snapshot?.organization}
        worker={snapshot?.worker_name}
      />

      <Modal title="¿Oponerte a esta corrección?" isOpen={opposing !== null} onClose={() => setOpposing(null)} closeOnEscape={!corrBusy}>
        <div className="space-y-3">
          <p className="text-xs text-muted">Si te opones, se mantiene tu marca original.</p>
          <div className="flex gap-2">
            <button type="button" className={BUTTON_SECONDARY} disabled={corrBusy} onClick={() => setOpposing(null)}>
              Cancelar
            </button>
            <button
              type="button"
              className={BUTTON_DANGER}
              disabled={corrBusy}
              onClick={() => opposing && respond(opposing, false)}
            >
              {corrBusy ? 'Enviando…' : 'Oponerme'}
            </button>
          </div>
        </div>
      </Modal>
    </div>
  );
};
