import React from 'react';
import type { ReportCell, ReportEmployer, ReportSection } from '../api/types';
import {
  ALTERED_MARK,
  cellAccessibleText,
  cellText,
  isAlteredCell,
  workplaceLabel,
} from '../lib/reports';

interface ReportTableProps {
  section: ReportSection;
  employer: ReportEmployer;
}

/**
 * One section of an R38 report (one table per worker, RF-ADMIN-16-4). Values are shown exactly
 * as the server sent them (ATT-09, R38 art. 28 a); altered cells carry color + symbol + legend.
 * Fixed layout with wrapping columns: never a horizontal scroll.
 */
export const ReportTable: React.FC<ReportTableProps> = ({ section, employer }) => {
  const worker = section.worker;
  const header: Array<[string, string]> = [
    ['Empleador', employer.legal_name],
    ['RUT', employer.rut],
  ];
  if (worker) {
    header.push(['Trabajador', worker.name]);
    if (worker.run) header.push(['RUN', worker.run]);
    const place = workplaceLabel(worker.workplace);
    if (place) header.push(['Lugar de trabajo', place]);
    if (worker.job_title) header.push(['Cargo', worker.job_title]);
  }

  return (
    <section className="report-section bg-surface border border-default rounded-card p-4 space-y-3">
      <dl className="grid grid-cols-[auto_1fr] sm:grid-cols-[auto_1fr_auto_1fr] gap-x-3 gap-y-1 text-xs">
        {header.map(([label, value]) => (
          <React.Fragment key={label}>
            <dt className="text-muted font-medium">{label}</dt>
            <dd className="text-fg [overflow-wrap:anywhere]">{value}</dd>
          </React.Fragment>
        ))}
      </dl>

      {section.header_notes.length > 0 && (
        <ul className="text-xs text-muted space-y-0.5">
          {section.header_notes.map((note, i) => (
            <li key={i}>{note}</li>
          ))}
        </ul>
      )}

      <table className="w-full table-fixed border-collapse text-[11px] tabular-nums">
        <thead>
          <tr className="bg-surface-subtle">
            {section.columns.map((col, i) => (
              <th
                key={i}
                scope="col"
                className="border border-default px-1.5 py-1 text-left font-medium text-muted align-bottom [overflow-wrap:anywhere]"
              >
                {col}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {section.rows.map((row, r) => (
            <tr key={r}>
              {row.map((cell, c) => (
                <ReportTableCell key={c} cell={cell} />
              ))}
            </tr>
          ))}
        </tbody>
        {section.totals.length > 0 && (
          <tfoot>
            {section.totals.map((row, r) => (
              <tr key={r} className="bg-surface-subtle font-semibold">
                {row.map((cell, c) => (
                  <ReportTableCell key={c} cell={cell} />
                ))}
              </tr>
            ))}
          </tfoot>
        )}
      </table>
    </section>
  );
};

const ReportTableCell: React.FC<{ cell: ReportCell }> = ({ cell }) => {
  const text = cellText(cell);
  if (isAlteredCell(cell)) {
    return (
      <td
        className="report-altered border border-default px-1.5 py-1 align-top bg-warning-subtle text-warning font-semibold [overflow-wrap:anywhere]"
        aria-label={cellAccessibleText(cell)}
        title="Valor alterado"
      >
        {text}
        <span aria-hidden="true" className="ml-0.5">
          {ALTERED_MARK}
        </span>
      </td>
    );
  }
  return (
    <td className="border border-default px-1.5 py-1 align-top text-fg [overflow-wrap:anywhere]">{text}</td>
  );
};
