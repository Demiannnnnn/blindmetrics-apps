import React, { useMemo } from 'react';
import type {
  JourneyType,
  ScheduleDay,
  ScheduleInput,
  ScheduleRequestedBy,
  WorkMode,
} from '../api/types';
import {
  DAILY_MAX_HOURS,
  JOURNEY_TYPE_LABELS,
  WEEKDAY_NAMES,
  calculateDayHours,
  calculateWeeklyTotalHours,
  daysToGrid,
  formatMinutes,
  getDefaultWeekDays,
  getScheduleWarnings,
  getWeeklyLegalMaxHours,
  gridToDays,
  serverWarningLabel,
  validateDisconnection,
} from '../lib/schedule';
import { AlertTriangle, Clock, Info } from 'lucide-react';

export interface ScheduleEditorProps {
  value: ScheduleInput;
  onChange: (value: ScheduleInput) => void;
  workMode: WorkMode;
  serverWarnings?: string[];
  disabled?: boolean;
}

export const ScheduleEditor: React.FC<ScheduleEditorProps> = ({
  value,
  onChange,
  workMode,
  serverWarnings,
  disabled = false,
}) => {
  const grid = useMemo(() => daysToGrid(value.days || []), [value.days]);

  const updateField = <K extends keyof ScheduleInput>(field: K, val: ScheduleInput[K]) => {
    onChange({ ...value, [field]: val });
  };

  const handleDayChange = (
    weekday: number,
    patch: Partial<{
      works: boolean;
      start: string;
      end: string;
      break_start: string;
      break_end: string;
    }>,
  ) => {
    const updatedGrid = grid.map(r => (r.weekday === weekday ? { ...r, ...patch } : r));
    const newDays = gridToDays(updatedGrid);
    updateField('days', newDays);
  };

  const handleApplyDefaultWeek = () => {
    const defaultDays = getDefaultWeekDays();
    updateField('days', defaultDays);
  };

  const handleClearDays = () => {
    updateField('days', []);
  };

  const weeklyHours = useMemo(
    () => calculateWeeklyTotalHours(value.days || [], value.break_counts_as_work),
    [value.days, value.break_counts_as_work],
  );

  const legalWeeklyMax = useMemo(
    () => getWeeklyLegalMaxHours(value.effective_from),
    [value.effective_from],
  );

  const localWarnings = useMemo(
    () => getScheduleWarnings(value.days || [], value.break_counts_as_work, value.effective_from),
    [value.days, value.break_counts_as_work, value.effective_from],
  );

  const disconnectionValidation = useMemo(() => {
    if (workMode === 'ON_SITE' && !value.disconnection) return { valid: true, hours: 0 };
    return validateDisconnection(value.disconnection);
  }, [workMode, value.disconnection]);

  const inputClass =
    'bg-surface border border-default text-fg text-xs rounded-control px-2 py-1.5 focus:outline-hidden focus:border-accent disabled:opacity-50';

  return (
    <div className="space-y-4">
      {/* 1. Journey type & Exceptional resolution */}
      <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
        <div>
          <label className="text-xs text-muted font-medium block mb-1">Tipo de Jornada</label>
          <select
            value={value.journey_type}
            disabled={disabled}
            onChange={e => updateField('journey_type', e.target.value as JourneyType)}
            className={`w-full ${inputClass}`}
          >
            {(Object.keys(JOURNEY_TYPE_LABELS) as JourneyType[]).map(jt => (
              <option key={jt} value={jt}>
                {JOURNEY_TYPE_LABELS[jt]}
              </option>
            ))}
          </select>
        </div>

        <div>
          <label className="text-xs text-muted font-medium block mb-1">Vigente desde</label>
          <input
            type="date"
            required
            value={value.effective_from}
            disabled={disabled}
            onChange={e => updateField('effective_from', e.target.value)}
            className={`w-full ${inputClass}`}
          />
        </div>
      </div>

      {value.journey_type === 'EXCEPTIONAL' && (
        <div className="p-3 bg-surface border border-default rounded-card space-y-2">
          <p className="text-xs font-semibold text-fg flex items-center gap-1.5">
            <Info className="w-3.5 h-3.5 text-accent" />
            Resolución de la Dirección del Trabajo (R38 art. 25.1 e)
          </p>
          <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
            <div>
              <label className="text-[11px] text-muted block mb-0.5">Número de Resolución</label>
              <input
                type="text"
                placeholder="Ej. Res. Ex. N° 1234/2025"
                value={value.exceptional_resolution?.number || ''}
                disabled={disabled}
                onChange={e =>
                  updateField('exceptional_resolution', {
                    number: e.target.value,
                    date: value.exceptional_resolution?.date || '',
                  })
                }
                className={`w-full ${inputClass}`}
              />
            </div>
            <div>
              <label className="text-[11px] text-muted block mb-0.5">Fecha de Resolución</label>
              <input
                type="date"
                value={value.exceptional_resolution?.date || ''}
                disabled={disabled}
                onChange={e =>
                  updateField('exceptional_resolution', {
                    number: value.exceptional_resolution?.number || '',
                    date: e.target.value,
                  })
                }
                className={`w-full ${inputClass}`}
              />
            </div>
          </div>
        </div>
      )}

      {/* 2. Options and action buttons */}
      <div className="flex flex-wrap items-center justify-between gap-2 pt-1 border-t border-default">
        <label className="flex items-center gap-2 text-xs text-fg cursor-pointer">
          <input
            type="checkbox"
            checked={value.break_counts_as_work}
            disabled={disabled}
            onChange={e => updateField('break_counts_as_work', e.target.checked)}
            className="rounded-sm accent-accent"
          />
          <span>La colación se imputa a la jornada laboral</span>
        </label>

        <div className="flex items-center gap-2">
          <button
            type="button"
            disabled={disabled}
            onClick={handleApplyDefaultWeek}
            className="px-2.5 py-1 bg-surface hover:bg-surface-subtle text-fg border border-default rounded-control text-[11px] transition"
            title="Aplica Lunes a Viernes 09:00 a 18:00 con colación 13:00 a 14:00"
          >
            Aplicar Lun–Vie 09:00–18:00
          </button>
          <button
            type="button"
            disabled={disabled}
            onClick={handleClearDays}
            className="px-2.5 py-1 bg-surface-subtle hover:bg-surface text-muted text-[11px] rounded-control transition"
          >
            Vaciar días
          </button>
        </div>
      </div>

      {/* 3. Monday–Sunday schedule grid */}
      <div className="border border-default rounded-card overflow-hidden">
        <div className="overflow-x-auto">
          <table className="w-full text-left text-xs border-collapse">
            <thead>
              <tr className="bg-surface-subtle border-b border-default text-muted font-medium text-[11px]">
                <th className="py-2 px-3 w-28">Día</th>
                <th className="py-2 px-2 w-16 text-center">Trabaja</th>
                <th className="py-2 px-2">Entrada</th>
                <th className="py-2 px-2">Salida</th>
                <th className="py-2 px-2">Colación Inicio</th>
                <th className="py-2 px-2">Colación Fin</th>
                <th className="py-2 px-3 text-right w-20">Total Día</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-default bg-surface">
              {grid.map(row => {
                const dayObj: ScheduleDay = {
                  weekday: row.weekday,
                  start: row.start,
                  end: row.end,
                  break_start: row.break_start || undefined,
                  break_end: row.break_end || undefined,
                };
                const dayHours = row.works
                  ? calculateDayHours(dayObj, value.break_counts_as_work)
                  : 0;
                const isHeavyDay = dayHours > DAILY_MAX_HOURS;

                return (
                  <tr
                    key={row.weekday}
                    className={`hover:bg-surface-subtle/50 transition-colors ${
                      row.works ? 'text-fg' : 'text-muted bg-surface-subtle/20'
                    }`}
                  >
                    <td className="py-2 px-3 font-medium whitespace-nowrap">
                      {WEEKDAY_NAMES[row.weekday]}
                    </td>
                    <td className="py-2 px-2 text-center">
                      <input
                        type="checkbox"
                        checked={row.works}
                        disabled={disabled}
                        onChange={e =>
                          handleDayChange(row.weekday, {
                            works: e.target.checked,
                            start: e.target.checked && !row.start ? '09:00' : row.start,
                            end: e.target.checked && !row.end ? '18:00' : row.end,
                            break_start:
                              e.target.checked && !row.break_start ? '13:00' : row.break_start,
                            break_end:
                              e.target.checked && !row.break_end ? '14:00' : row.break_end,
                          })
                        }
                        className="rounded-sm accent-accent"
                      />
                    </td>
                    <td className="py-2 px-2">
                      <input
                        type="time"
                        value={row.start}
                        disabled={disabled || !row.works}
                        onChange={e => handleDayChange(row.weekday, { start: e.target.value })}
                        className={`w-24 ${inputClass} ${!row.works ? 'opacity-40' : ''}`}
                      />
                    </td>
                    <td className="py-2 px-2">
                      <input
                        type="time"
                        value={row.end}
                        disabled={disabled || !row.works}
                        onChange={e => handleDayChange(row.weekday, { end: e.target.value })}
                        className={`w-24 ${inputClass} ${!row.works ? 'opacity-40' : ''}`}
                      />
                    </td>
                    <td className="py-2 px-2">
                      <input
                        type="time"
                        value={row.break_start}
                        disabled={disabled || !row.works}
                        onChange={e =>
                          handleDayChange(row.weekday, { break_start: e.target.value })
                        }
                        className={`w-24 ${inputClass} ${!row.works ? 'opacity-40' : ''}`}
                      />
                    </td>
                    <td className="py-2 px-2">
                      <input
                        type="time"
                        value={row.break_end}
                        disabled={disabled || !row.works}
                        onChange={e => handleDayChange(row.weekday, { break_end: e.target.value })}
                        className={`w-24 ${inputClass} ${!row.works ? 'opacity-40' : ''}`}
                      />
                    </td>
                    <td className="py-2 px-3 text-right font-mono font-medium">
                      {row.works ? (
                        <span className={isHeavyDay ? 'text-warning font-semibold' : ''}>
                          {formatMinutes(dayHours * 60)}
                        </span>
                      ) : (
                        <span className="text-muted">—</span>
                      )}
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>

        {/* Weekly summary bar */}
        <div className="bg-surface-subtle p-3 border-t border-default flex items-center justify-between text-xs">
          <div className="flex items-center gap-2">
            <Clock className="w-4 h-4 text-muted" />
            <span className="text-fg font-medium">
              Total semanal pactado: <span className="font-bold">{weeklyHours.toFixed(1)} h</span>
            </span>
            <span className="text-muted text-[11px]">
              (Máximo legal vigente: {legalWeeklyMax} h)
            </span>
          </div>
          {weeklyHours > legalWeeklyMax && (
            <span className="text-warning text-xs font-semibold flex items-center gap-1">
              <AlertTriangle className="w-3.5 h-3.5" />
              Supera máximo legal
            </span>
          )}
        </div>
      </div>

      {/* 4. Disconnection Window (REMOTE / HYBRID) */}
      {(workMode !== 'ON_SITE' || value.disconnection) && (
        <div className="p-3 bg-surface border border-default rounded-card space-y-2">
          <div className="flex items-center justify-between">
            <label className="text-xs font-semibold text-fg flex items-center gap-1.5">
              <Clock className="w-3.5 h-3.5 text-accent" />
              Franja de Desconexión Digital (CT art. 152 quáter J)
            </label>
            <span className="text-[11px] text-muted">
              Mínimo obligatorio: 12 h continuas
            </span>
          </div>

          <div className="grid grid-cols-1 sm:grid-cols-2 gap-3 items-center">
            <div>
              <label className="text-[11px] text-muted block mb-0.5">Inicio Desconexión</label>
              <input
                type="time"
                value={value.disconnection?.start || '20:00'}
                disabled={disabled}
                onChange={e =>
                  updateField('disconnection', {
                    start: e.target.value,
                    end: value.disconnection?.end || '08:00',
                  })
                }
                className={`w-full ${inputClass}`}
              />
            </div>
            <div>
              <label className="text-[11px] text-muted block mb-0.5">Término Desconexión</label>
              <input
                type="time"
                value={value.disconnection?.end || '08:00'}
                disabled={disabled}
                onChange={e =>
                  updateField('disconnection', {
                    start: value.disconnection?.start || '20:00',
                    end: e.target.value,
                  })
                }
                className={`w-full ${inputClass}`}
              />
            </div>
          </div>

          {disconnectionValidation.hours > 0 && (
            <div className="text-[11px]">
              {disconnectionValidation.valid ? (
                <span className="text-success font-medium">
                  Franja válida: {formatMinutes(disconnectionValidation.hours * 60)} continuas.
                </span>
              ) : (
                <span className="text-danger font-medium flex items-center gap-1">
                  <AlertTriangle className="w-3 h-3" />
                  {disconnectionValidation.error}
                </span>
              )}
            </div>
          )}
        </div>
      )}

      {/* 5. Metadata: Location at mark, Requested By, Note */}
      <div className="grid grid-cols-1 sm:grid-cols-2 gap-3 pt-1 border-t border-default">
        <div>
          <label className="text-xs text-muted font-medium block mb-1">Solicitado por</label>
          <select
            value={value.requested_by}
            disabled={disabled}
            onChange={e =>
              updateField('requested_by', e.target.value as ScheduleRequestedBy)
            }
            className={`w-full ${inputClass}`}
          >
            <option value="EMPLOYER">Empleador</option>
            <option value="WORKER">Trabajador</option>
          </select>
        </div>

        <div>
          <label className="text-xs text-muted font-medium block mb-1">Nota (opcional)</label>
          <input
            type="text"
            placeholder="Ej. Cambio de turno acordado en anexo"
            maxLength={500}
            value={value.note || ''}
            disabled={disabled}
            onChange={e => updateField('note', e.target.value)}
            className={`w-full ${inputClass}`}
          />
        </div>
      </div>

      <div>
        <label className="flex items-start gap-2 text-xs text-fg cursor-pointer">
          <input
            type="checkbox"
            checked={value.location_at_mark}
            disabled={disabled}
            onChange={e => updateField('location_at_mark', e.target.checked)}
            className="mt-0.5 rounded-sm accent-accent"
          />
          <div>
            <span className="font-medium">Registrar ubicación geográfica al marcar</span>
            <p className="text-[11px] text-muted">
              Requiere consentimiento escrito del trabajador en su contrato o anexo (R38 art. 57.1).
              Se registra una única lectura al presionar el botón de marcación.
            </p>
          </div>
        </label>
      </div>

      {/* 6. Non-blocking Legal Warnings (ATT-11) */}
      {(localWarnings.length > 0 || (serverWarnings && serverWarnings.length > 0)) && (
        <div className="p-3 bg-warning-subtle border border-warning rounded-card text-xs text-warning space-y-1">
          <div className="flex items-center gap-1.5 font-semibold">
            <AlertTriangle className="w-4 h-4 text-warning" />
            <span>Advertencias legales de jornada (no bloqueantes, ATT-11):</span>
          </div>
          <ul className="list-disc list-inside space-y-0.5 pl-1 text-[11px]">
            {localWarnings.map((w, i) => (
              <li key={`local-${i}`}>{w}</li>
            ))}
            {serverWarnings?.map((sw, i) => (
              <li key={`server-${i}`}>{serverWarningLabel(sw)}</li>
            ))}
          </ul>
        </div>
      )}
    </div>
  );
};
