// Agreed schedule of the worker, read-only (TASK-AGENT-16). Loaded on demand; needs connection.
import React, { useState } from 'react';
import { CalendarDays, ChevronDown, ChevronUp } from 'lucide-react';
import * as bridge from '../api/tauri-bridge';
import type { MyScheduleView, WorkerSchedule } from '../api/types';
import { CARD, Notice, Skeleton } from './ui';
import { attendanceErrorMessage, nextScheduleHeading, scheduleFacts, scheduleWeekRows } from '../lib/attendance';

const ScheduleTable: React.FC<{ schedule: WorkerSchedule }> = ({ schedule }) => (
  <div className="space-y-2">
    <table className="w-full text-xs">
      <tbody className="divide-y divide-default">
        {scheduleWeekRows(schedule).map((row) => (
          <tr key={row.weekday}>
            <th scope="row" className="py-1 pr-3 text-left font-medium text-muted w-24">
              {row.name}
            </th>
            <td className={`py-1 tabular-nums ${row.works ? 'text-fg' : 'text-muted'}`}>
              {row.hours}
              {row.breakText && <span className="block text-muted">{row.breakText}</span>}
            </td>
          </tr>
        ))}
      </tbody>
    </table>
    <ul className="text-xs text-muted space-y-0.5">
      {scheduleFacts(schedule).map((fact) => (
        <li key={fact}>{fact}</li>
      ))}
    </ul>
  </div>
);

export const MyScheduleCard: React.FC = () => {
  const [open, setOpen] = useState(false);
  const [loading, setLoading] = useState(false);
  const [view, setView] = useState<MyScheduleView | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = async () => {
    setLoading(true);
    setError(null);
    try {
      setView(await bridge.attendanceSchedule());
    } catch (err) {
      const cmd = bridge.toCommandError(err);
      setError(cmd.code === 'OFFLINE' ? 'Tu jornada pactada se consulta con conexión.' : attendanceErrorMessage(cmd));
    } finally {
      setLoading(false);
    }
  };

  const toggle = () => {
    const next = !open;
    setOpen(next);
    if (next && !view && !loading) void load();
  };

  return (
    <section className={`${CARD} space-y-3`} aria-label="Mi jornada pactada">
      <button
        type="button"
        onClick={toggle}
        aria-expanded={open}
        className="w-full flex items-center justify-between text-sm font-semibold text-fg"
      >
        <span className="flex items-center gap-1.5">
          <CalendarDays className="w-4 h-4 text-muted" aria-hidden="true" />
          Mi jornada pactada
        </span>
        {open ? <ChevronUp className="w-4 h-4" aria-hidden="true" /> : <ChevronDown className="w-4 h-4" aria-hidden="true" />}
      </button>
      {open && (
        <div className="space-y-3">
          {loading && <Skeleton className="h-24 w-full" />}
          {error && (
            <Notice tone="warning" role="status">
              {error}{' '}
              <button type="button" className="underline" onClick={() => void load()}>
                Reintentar
              </button>
            </Notice>
          )}
          {view && !loading && (
            <>
              {view.current ? (
                <ScheduleTable schedule={view.current} />
              ) : (
                <p className="text-xs text-muted">Tu empleador aún no registra una jornada pactada.</p>
              )}
              {view.next && (
                <div className="space-y-2 pt-2 border-t border-default">
                  <p className="text-xs font-semibold text-fg">{nextScheduleHeading(view.next)}</p>
                  <ScheduleTable schedule={view.next} />
                </div>
              )}
            </>
          )}
        </div>
      )}
    </section>
  );
};
