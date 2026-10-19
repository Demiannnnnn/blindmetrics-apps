import React, { useCallback, useEffect, useState } from 'react';
import { ChevronDown, ChevronUp, Clock, MapPin } from 'lucide-react';
import * as bridge from '../api/tauri-bridge';
import type { MarkType, TodayView } from '../api/types';
import { useApp } from '../context/AppContext';
import { MarkDialog } from '../components/MarkDialog';
import { MarkDetail } from '../components/MarkDetail';
import { BUTTON_PRIMARY, BUTTON_SECONDARY, CARD, Notice, Skeleton, StatusBadge } from '../components/ui';
import {
  HASH_MISMATCH_ADVICE,
  attendanceErrorMessage,
  dayStateLabel,
  formatDayHeadingCl,
  formatTimeCl,
  hasClockSkew,
  markStatusLabel,
  markTypeLabel,
  nextMarkLabel,
  otherMarks,
  primaryMark,
  serverNowMs,
  shortClock,
  shortHash,
  todayRows,
  type MarkRow,
} from '../lib/attendance';

const EXEMPT_NOTICE =
  'Tu contrato no está sujeto a control de jornada (art. 22 del Código del Trabajo). No necesitas marcar.';

/** Server clock: device time + measured offset, refreshed every second (ATT-10). */
function useServerClock(offsetSecs: number | null | undefined): number {
  const [now, setNow] = useState(() => serverNowMs(Date.now(), offsetSecs));
  useEffect(() => {
    const tick = () => setNow(serverNowMs(Date.now(), offsetSecs));
    tick();
    // Re-render a few times per second so the seconds digit never lags behind.
    const id = window.setInterval(tick, 250);
    return () => window.clearInterval(id);
  }, [offsetSecs]);
  return now;
}

const ClockBlock: React.FC<{ today: TodayView; timeZone?: string | null }> = ({ today, timeZone }) => {
  const now = useServerClock(today.server_time_offset_secs);
  return (
    <div>
      <p className="text-sm text-fg">{formatDayHeadingCl(now, timeZone)}</p>
      <p className="flex items-baseline gap-2">
        <span className="text-3xl font-semibold tabular-nums tracking-tight" aria-live="off">
          {formatTimeCl(now, timeZone)}
        </span>
        <span className="text-[11px] text-muted">
          {today.server_time_offset_secs === null ? '(hora de tu equipo)' : '(hora del servidor)'}
        </span>
      </p>
      {hasClockSkew(today.server_time_offset_secs) && (
        <p className="text-[11px] text-muted mt-0.5">
          La hora de tu equipo difiere de la del servidor; se usa la del servidor.
        </p>
      )}
    </div>
  );
};

const MarkRowItem: React.FC<{ row: MarkRow; timeZone?: string | null; onOpen: () => void }> = ({
  row,
  timeZone,
  onOpen,
}) => {
  const status = markStatusLabel(row.status, row.rejectionReason);
  return (
    <li>
      <button
        type="button"
        onClick={onOpen}
        className="w-full text-left px-2 py-2 hover:bg-surface-subtle rounded-control focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent"
      >
        <div className="flex flex-wrap items-center gap-x-2 gap-y-1 text-xs">
          <span className="font-mono tabular-nums text-fg">{formatTimeCl(row.ts, timeZone)}</span>
          <span className="text-fg">{markTypeLabel(row.type)}</span>
          <StatusBadge tone={status.tone}>{status.label}</StatusBadge>
          {row.corrected && <StatusBadge tone="info">Corregida</StatusBadge>}
          {row.superseded && <StatusBadge tone="muted">Reemplazada</StatusBadge>}
          {row.hash && <span className="font-mono text-[11px] text-muted ml-auto">{shortHash(row.hash)}</span>}
        </div>
        {row.status === 'HASH_MISMATCH' && <p className="text-[11px] text-danger mt-1">{HASH_MISMATCH_ADVICE}</p>}
      </button>
    </li>
  );
};

/** A — Asistencia (home), spec §3. */
export const AttendanceView: React.FC = () => {
  const { snapshot, refreshSnapshot, setCurrentView, setSettingsIntent } = useApp();
  const [fetched, setFetched] = useState<TodayView | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [showOthers, setShowOthers] = useState(false);
  const [dialogType, setDialogType] = useState<MarkType | null>(null);
  const [detail, setDetail] = useState<MarkRow | null>(null);

  const today = snapshot?.attendance_today ?? fetched;
  const timeZone = today?.server?.timezone ?? snapshot?.timezone ?? null;
  const exempt = snapshot?.attendance_mode === 'EXEMPT';

  const loadToday = useCallback(async () => {
    setLoading(true);
    setLoadError(null);
    try {
      setFetched(await bridge.attendanceToday());
    } catch (err) {
      setLoadError(attendanceErrorMessage(bridge.toCommandError(err)));
    } finally {
      setLoading(false);
    }
  }, []);

  // No snapshot data yet: skeleton, ask the service once, then explain the error (no invented data).
  useEffect(() => {
    if (!exempt && snapshot && !snapshot.attendance_today && !fetched && !loading && !loadError) {
      loadToday();
    }
  }, [exempt, snapshot, fetched, loading, loadError, loadToday]);

  const header = (
    <div className="flex items-center gap-2 mb-4">
      <Clock className="w-5 h-5 text-accent" aria-hidden="true" />
      <h1 className="text-base font-semibold text-fg">Asistencia</h1>
    </div>
  );

  if (exempt) {
    return (
      <div className="p-6 max-w-lg">
        {header}
        <div className={CARD}>
          <p className="text-sm text-fg leading-relaxed">{EXEMPT_NOTICE}</p>
        </div>
      </div>
    );
  }

  if (!today) {
    return (
      <div className="p-6 max-w-lg">
        {header}
        <div className={`${CARD} space-y-3`}>
          {loadError ? (
            <>
              <Notice tone="danger" role="alert">
                No pudimos cargar tu asistencia de hoy. {loadError}
              </Notice>
              <button type="button" className={BUTTON_SECONDARY} onClick={loadToday} disabled={loading}>
                {loading ? 'Cargando…' : 'Reintentar'}
              </button>
            </>
          ) : (
            <>
              <Skeleton className="h-4 w-32" />
              <Skeleton className="h-8 w-40" />
              <Skeleton className="h-3 w-56" />
              <Skeleton className="h-9 w-48" />
            </>
          )}
        </div>
      </div>
    );
  }

  const rows = todayRows(today);
  const next = primaryMark(today);
  const schedule = today.server?.schedule_today ?? null;

  return (
    <div className="p-6 max-w-lg space-y-4">
      {header}

      <section className={`${CARD} space-y-3`} aria-label="Estado de hoy">
        <ClockBlock today={today} timeZone={timeZone} />
        <dl className="text-xs space-y-0.5">
          <div className="flex gap-1">
            <dt className="text-muted">Estado:</dt>
            <dd className="text-fg tabular-nums">{dayStateLabel(today, timeZone)}</dd>
          </div>
          {today.server ? (
            schedule && (schedule.start || schedule.end) ? (
              <>
                <div className="flex gap-1">
                  <dt className="text-muted">Jornada de hoy:</dt>
                  <dd className="text-fg tabular-nums">
                    {shortClock(schedule.start)} – {shortClock(schedule.end)}
                  </dd>
                </div>
                {(schedule.break_start || schedule.break_end) && (
                  <div className="flex gap-1">
                    <dt className="text-muted">Colación:</dt>
                    <dd className="text-fg tabular-nums">
                      {shortClock(schedule.break_start)} – {shortClock(schedule.break_end)}
                    </dd>
                  </div>
                )}
              </>
            ) : (
              <p className="text-fg">Hoy no tienes jornada pactada. Si trabajas, marca igual.</p>
            )
          ) : (
            <p className="text-muted">Tu jornada de hoy no está disponible sin conexión.</p>
          )}
        </dl>
        {today.source === 'LOCAL' && <StatusBadge tone="muted">Datos locales (sin conexión)</StatusBadge>}

        {/* ATT-02: never disabled by telemetry, subscription or banners; only while a mark is in progress. */}
        <div className="pt-1 space-y-2">
          <button
            type="button"
            className={`${BUTTON_PRIMARY} w-full h-10`}
            disabled={dialogType !== null}
            onClick={() => setDialogType(next)}
          >
            {nextMarkLabel(next)}
          </button>
          <button
            type="button"
            className="text-xs text-muted hover:text-fg inline-flex items-center gap-1 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent rounded-badge"
            aria-expanded={showOthers}
            aria-controls="other-marks"
            onClick={() => setShowOthers((v) => !v)}
          >
            Otras marcas
            {showOthers ? <ChevronUp className="w-3.5 h-3.5" aria-hidden="true" /> : <ChevronDown className="w-3.5 h-3.5" aria-hidden="true" />}
          </button>
          {showOthers && (
            <div id="other-marks" className="grid grid-cols-1 gap-1.5">
              {otherMarks(next).map((t) => (
                <button
                  key={t}
                  type="button"
                  className={`${BUTTON_SECONDARY} w-full`}
                  disabled={dialogType !== null}
                  onClick={() => setDialogType(t)}
                >
                  {nextMarkLabel(t)}
                </button>
              ))}
            </div>
          )}
        </div>
      </section>

      <section className={CARD} aria-label="Marcas de hoy">
        <h2 className="text-xs font-semibold text-fg mb-2">Marcas de hoy</h2>
        {rows.length === 0 ? (
          <p className="text-xs text-muted">Aún no tienes marcas hoy.</p>
        ) : (
          <ul className="divide-y divide-default -mx-2">
            {rows.map((r) => (
              <MarkRowItem key={r.key} row={r} timeZone={timeZone} onOpen={() => setDetail(r)} />
            ))}
          </ul>
        )}
      </section>

      <p className="text-xs text-muted flex items-center gap-1.5">
        <MapPin className="w-3.5 h-3.5" aria-hidden="true" />
        {snapshot?.location_at_mark
          ? 'Ubicación: se registra tu ubicación solo en el momento de marcar.'
          : 'Ubicación: no se registra al marcar.'}
      </p>

      {dialogType && (
        <MarkDialog
          type={dialogType}
          timeZone={timeZone}
          personalEmail={snapshot?.personal_email}
          onClose={() => setDialogType(null)}
          onMarked={() => {
            refreshSnapshot();
          }}
          onForgotPin={() => {
            setDialogType(null);
            setSettingsIntent('PIN_RESET');
            setCurrentView('settings');
          }}
        />
      )}
      <MarkDetail
        row={detail}
        onClose={() => setDetail(null)}
        timeZone={timeZone}
        organization={snapshot?.organization}
        worker={snapshot?.worker_name}
        source={today.source}
      />
    </div>
  );
};
