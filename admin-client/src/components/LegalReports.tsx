import React, { useEffect, useMemo, useState } from 'react';
import { FileSpreadsheet, FileText, FileType, Printer, FolderOpen, X, Mail, Info, RefreshCw } from 'lucide-react';
import type { ExportFormat, LegalReport, ReportFilters as ReportFiltersDto, ReportKind } from '../api/types';
import {
  attendanceBoard,
  attendanceExportReport,
  attendancePrintReport,
  attendanceReport,
  getWorkersList,
  openDownloadsFolder,
  workplacesList,
} from '../api/tauri-bridge';
import { describeError } from '../lib/errors';
import { formatR38DateTime } from '../lib/attendance';
import {
  ALTERED_LEGEND,
  ALTERED_MARK,
  DAILY_REPORT_INFO,
  EXPORT_LABELS,
  REPORT_MENU,
  buildReportFilters,
  hasAlteredCells,
  initialReportForm,
  savedToastText,
  uniqueJobTitles,
  validateReportFilters,
  type ReportFormState,
  type ReportMenuId,
  type ReportWorkerOption,
} from '../lib/reports';
import { ReportFilters, type WorkplaceOption } from './ReportFilters';
import { ReportTable } from './ReportTable';

interface LegalReportsProps {
  /** Subscription blocked (BILL-04): only GET /attendance/* is reachable, no console data. */
  readOnly?: boolean;
}

interface Generated {
  kind: ReportKind;
  filters: ReportFiltersDto;
  data: LegalReport;
}

interface Toast {
  text: string;
  showFolder: boolean;
}

type BusyAction = ExportFormat | 'print' | null;

const EXPORT_ICONS: Record<ExportFormat, React.ReactNode> = {
  xlsx: <FileSpreadsheet className="w-3.5 h-3.5" />,
  pdf: <FileText className="w-3.5 h-3.5" />,
  docx: <FileType className="w-3.5 h-3.5" />,
};

const secondaryButton =
  'px-3 py-2 bg-surface border border-default hover:bg-surface-subtle text-fg text-xs rounded-control flex items-center gap-1.5 disabled:opacity-50';

/**
 * AS3 — Reportes legales (R38 arts. 25, 27, 28; TASK-ADMIN-16). The server computes and exports
 * every report; this view filters, previews the server JSON verbatim and saves the server files.
 * Read-only: no correction action is offered here.
 */
export const LegalReports: React.FC<LegalReportsProps> = ({ readOnly = false }) => {
  const [selected, setSelected] = useState<ReportMenuId>('attendance');
  const [form, setForm] = useState<ReportFormState>(initialReportForm);
  const [workers, setWorkers] = useState<ReportWorkerOption[]>([]);
  const [workplaces, setWorkplaces] = useState<WorkplaceOption[]>([]);
  const [optionsNote, setOptionsNote] = useState<string | null>(null);
  const [generated, setGenerated] = useState<Generated | null>(null);
  const [generating, setGenerating] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState<BusyAction>(null);
  const [actionError, setActionError] = useState<string | null>(null);
  const [toast, setToast] = useState<Toast | null>(null);

  // Worker and workplace options. While blocked only the attendance board is reachable (BILL-04).
  useEffect(() => {
    let cancelled = false;
    const fromBoard = async () => {
      const board = await attendanceBoard();
      const options: ReportWorkerOption[] = board.map(b => ({
        id: b.worker_id,
        name: b.name,
        run: b.run ?? '',
        workplaceId: b.workplace?.id ?? null,
        workplaceName: b.workplace?.name ?? null,
      }));
      const places = new Map<string, string>();
      for (const b of board) if (b.workplace) places.set(b.workplace.id, b.workplace.name);
      if (cancelled) return;
      setWorkers(options);
      setWorkplaces([...places].map(([id, name]) => ({ id, name })));
      setOptionsNote('La lista muestra los trabajadores activos; busca por nombre o RUN para encontrar a otros.');
    };
    const load = async () => {
      try {
        if (readOnly) {
          await fromBoard();
          return;
        }
        const list = await getWorkersList();
        if (cancelled) return;
        setWorkers(
          list.map(w => ({
            id: w.id,
            name: w.name,
            run: w.run,
            workplaceId: w.workplace?.id ?? null,
            workplaceName: w.workplace?.name ?? null,
            jobTitle: w.jobTitle,
          })),
        );
        setOptionsNote(
          list.length >= 100 ? 'Se muestran los primeros 100 trabajadores; busca por nombre o RUN para encontrar a otros.' : null,
        );
        try {
          const places = await workplacesList();
          if (!cancelled) setWorkplaces(places.map(p => ({ id: p.id, name: p.name })));
        } catch {
          // Fall back to the workplaces of the listed workers.
          const fallback = new Map<string, string>();
          for (const w of list) if (w.workplace) fallback.set(w.workplace.id, w.workplace.name);
          if (!cancelled) setWorkplaces([...fallback].map(([id, name]) => ({ id, name })));
        }
      } catch {
        try {
          await fromBoard();
        } catch {
          if (!cancelled) setOptionsNote('No se pudo cargar la lista de trabajadores; puedes buscar por nombre o RUN.');
        }
      }
    };
    load();
    return () => {
      cancelled = true;
    };
  }, [readOnly]);

  const jobTitles = useMemo(() => uniqueJobTitles(workers), [workers]);
  const menuEntry = REPORT_MENU.find(r => r.id === selected) ?? REPORT_MENU[0];
  const reportKind: ReportKind | null = selected === 'daily' ? null : selected;

  const selectReport = (id: ReportMenuId) => {
    setSelected(id);
    setGenerated(null);
    setError(null);
    setActionError(null);
  };

  const generate = async () => {
    if (!reportKind) return;
    const filters = buildReportFilters(form);
    const invalid = validateReportFilters(reportKind, filters);
    if (invalid) {
      setError(invalid);
      return;
    }
    setGenerating(true);
    setError(null);
    setActionError(null);
    try {
      const data = await attendanceReport(reportKind, filters);
      setGenerated({ kind: reportKind, filters, data });
    } catch (e) {
      setGenerated(null);
      setError(describeError(e, 'No se pudo generar el reporte.'));
    } finally {
      setGenerating(false);
    }
  };

  // Exports use the filters of the preview, so the file matches what is on screen (art. 28 a).
  const exportAs = async (format: ExportFormat) => {
    if (!generated) return;
    setBusy(format);
    setActionError(null);
    try {
      const path = await attendanceExportReport(generated.kind, generated.filters, format);
      setToast({ text: savedToastText(path), showFolder: true });
    } catch (e) {
      setActionError(describeError(e, 'No se pudo descargar el reporte.'));
    } finally {
      setBusy(null);
    }
  };

  const print = async () => {
    if (!generated) return;
    setBusy('print');
    setActionError(null);
    try {
      const result = await attendancePrintReport(generated.kind, generated.filters);
      setToast({
        text: result.opened
          ? `${savedToastText(result.path)}. Imprímelo desde el visor de PDF.`
          : `${savedToastText(result.path)}. No se pudo abrir el visor de PDF; ábrelo desde la carpeta.`,
        showFolder: true,
      });
    } catch (e) {
      setActionError(describeError(e, 'No se pudo preparar el PDF para imprimir.'));
    } finally {
      setBusy(null);
    }
  };

  const openFolder = async () => {
    try {
      await openDownloadsFolder();
    } catch (e) {
      setActionError(describeError(e, 'No se pudo abrir la carpeta de descargas.'));
    }
  };

  const data = generated?.data ?? null;
  const showAlteredLegend =
    !!data && data.sections.some(s => hasAlteredCells(s.rows) || hasAlteredCells(s.totals));

  return (
    <div className="space-y-5">
      <div className="no-print">
        <h2 className="text-base font-semibold text-fg">Reportes legales</h2>
        <p className="text-xs text-muted mt-0.5">
          Reportes del registro de asistencia según la Res. Ex. N°38 de la Dirección del Trabajo (arts. 25, 27 y 28),
          calculados por el servidor.{' '}
          {readOnly
            ? 'Acceso de solo lectura mientras tu suscripción está bloqueada.'
            : 'Solo lectura: las correcciones se proponen en Marcas y correcciones.'}
        </p>
      </div>

      <div className="flex flex-col lg:flex-row gap-5 items-start">
        {/* Menu: the six reports in R38 order (RF-ADMIN-16-2) */}
        <nav aria-label="Reportes" className="no-print w-full lg:w-64 shrink-0 bg-surface border border-default rounded-card p-2">
          <ol className="space-y-0.5">
            {REPORT_MENU.map((entry, i) => (
              <li key={entry.id}>
                <button
                  type="button"
                  onClick={() => selectReport(entry.id)}
                  aria-current={selected === entry.id ? 'page' : undefined}
                  className={`w-full text-left px-3 py-2 rounded-control text-xs border-l-2 transition-colors ${
                    selected === entry.id
                      ? 'bg-surface-subtle text-fg font-semibold border-accent'
                      : 'text-muted border-transparent hover:bg-surface-subtle hover:text-fg'
                  }`}
                >
                  <span className="tabular-nums mr-1.5">{i + 1}.</span>
                  {entry.name}
                </button>
              </li>
            ))}
          </ol>
        </nav>

        <div className="flex-1 min-w-0 space-y-4 w-full">
          {reportKind === null ? (
            <div className="bg-surface border border-default rounded-card p-5 space-y-2">
              <h3 className="text-sm font-semibold text-fg flex items-center gap-2">
                <Mail className="w-4 h-4 text-muted" aria-hidden="true" />
                {menuEntry.name}
              </h3>
              <p className="text-xs text-fg">{DAILY_REPORT_INFO}.</p>
            </div>
          ) : (
            <>
              <ReportFilters
                report={reportKind}
                value={form}
                onChange={setForm}
                workers={workers}
                workplaces={workplaces}
                jobTitles={jobTitles}
                optionsNote={optionsNote}
                generating={generating}
                error={error}
                onGenerate={generate}
              />

              {data && generated && (
                <div className="report-print-area space-y-3">
                  <div className="no-print flex flex-wrap items-center justify-between gap-3">
                    <div>
                      <h3 className="text-sm font-semibold text-fg">{menuEntry.name}</h3>
                      <p className="text-[11px] text-muted">
                        Generado el {formatR38DateTime(data.generated_at)} · Las descargas usan los filtros de esta vista
                        previa.
                      </p>
                    </div>
                    <div className="flex flex-wrap gap-2">
                      {(['xlsx', 'pdf', 'docx'] as ExportFormat[]).map(format => (
                        <button
                          key={format}
                          type="button"
                          onClick={() => exportAs(format)}
                          disabled={busy !== null}
                          className={secondaryButton}
                        >
                          {busy === format ? <RefreshCw className="w-3.5 h-3.5 animate-spin" /> : EXPORT_ICONS[format]}
                          {EXPORT_LABELS[format]}
                        </button>
                      ))}
                      <button type="button" onClick={print} disabled={busy !== null} className={secondaryButton}>
                        {busy === 'print' ? <RefreshCw className="w-3.5 h-3.5 animate-spin" /> : <Printer className="w-3.5 h-3.5" />}
                        Imprimir
                      </button>
                    </div>
                  </div>

                  {actionError && (
                    <p className="no-print text-xs text-danger" role="alert">
                      {actionError}
                    </p>
                  )}

                  <div className="report-preview space-y-3">
                    <p className="print-only hidden text-sm font-semibold">{menuEntry.name}</p>
                    {data.sections.length === 0 ? (
                      data.empty_message && (
                        <div className="bg-surface border border-default rounded-card p-5 text-xs text-fg">
                          {data.empty_message}
                        </div>
                      )
                    ) : (
                      data.sections.map((section, i) => (
                        <ReportTable key={i} section={section} employer={data.employer} />
                      ))
                    )}

                    {(data.abbreviations.length > 0 || showAlteredLegend) && (
                      <div className="report-legend bg-surface border border-default rounded-card p-4 text-[11px] space-y-2">
                        {showAlteredLegend && (
                          <p className="flex items-center gap-2 text-fg">
                            <span className="inline-block px-1.5 bg-warning-subtle text-warning font-semibold rounded-badge">
                              {ALTERED_MARK}
                            </span>
                            {ALTERED_LEGEND}
                          </p>
                        )}
                        {data.abbreviations.length > 0 && (
                          <div>
                            <p className="font-medium text-muted mb-1 flex items-center gap-1.5">
                              <Info className="w-3.5 h-3.5" aria-hidden="true" /> Abreviaturas
                            </p>
                            <dl className="grid grid-cols-[auto_1fr] gap-x-3 gap-y-0.5">
                              {data.abbreviations.map(a => (
                                <React.Fragment key={a.code}>
                                  <dt className="font-mono text-fg">{a.code}</dt>
                                  <dd className="text-fg">{a.meaning}</dd>
                                </React.Fragment>
                              ))}
                            </dl>
                          </div>
                        )}
                      </div>
                    )}
                  </div>
                </div>
              )}
            </>
          )}
        </div>
      </div>

      {toast && (
        <div
          role="status"
          aria-live="polite"
          className="no-print fixed bottom-4 right-4 z-50 max-w-md bg-surface border border-default rounded-card shadow-md p-3 flex items-start gap-3 text-xs"
        >
          <p className="flex-1 text-fg [overflow-wrap:anywhere]">{toast.text}</p>
          {toast.showFolder && (
            <button type="button" onClick={openFolder} className="shrink-0 text-accent hover:underline flex items-center gap-1">
              <FolderOpen className="w-3.5 h-3.5" /> Abrir carpeta
            </button>
          )}
          <button type="button" onClick={() => setToast(null)} className="shrink-0 text-muted hover:text-fg" aria-label="Cerrar aviso">
            <X className="w-3.5 h-3.5" />
          </button>
        </div>
      )}
    </div>
  );
};
