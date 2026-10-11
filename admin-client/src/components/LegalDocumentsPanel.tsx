import React, { useEffect, useState } from 'react';
import { FileText, AlertTriangle, Eye, X } from 'lucide-react';
import { legalDocuments, legalStatus, LegalDocumentsBundle, LegalStatus } from '../api/tauri-bridge';
import { MarkdownView } from './MarkdownView';

export const LegalDocumentsPanel: React.FC = () => {
  const [bundle, setBundle] = useState<LegalDocumentsBundle | null>(null);
  const [status, setStatus] = useState<LegalStatus | null>(null);
  const [loading, setLoading] = useState(true);
  const [readingDoc, setReadingDoc] = useState<'employer_terms' | 'privacy_policy' | null>(null);

  useEffect(() => {
    let isMounted = true;
    Promise.all([legalDocuments().catch(() => null), legalStatus().catch(() => null)])
      .then(([b, s]) => {
        if (!isMounted) return;
        setBundle(b);
        setStatus(s);
        setLoading(false);
      });

    return () => {
      isMounted = false;
    };
  }, []);

  const activeDoc = readingDoc && bundle ? bundle[readingDoc] : null;

  return (
    <div className="bg-surface border border-default rounded-card p-5 space-y-4">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-base font-semibold text-fg flex items-center gap-2">
            <FileText className="w-5 h-5 text-accent" />
            Documentos Legales y Términos Aceptados
          </h2>
          <p className="text-xs text-muted mt-0.5">
            Registro de versiones vigentes del servicio y política de privacidad aceptadas por el empleador (LEG-04..07).
          </p>
        </div>
      </div>

      {/* Notice LEG-07 */}
      <div className="p-3 bg-warning-subtle border border-default rounded-card flex items-start gap-2.5 text-xs text-warning">
        <AlertTriangle className="w-4 h-4 shrink-0 text-warning mt-0.5" />
        <span className="leading-relaxed">
          <strong>Aviso legal:</strong> Telemetry está diseñado para cumplir los requisitos de la Resolución
          Exenta N°38 de 2024 de la Dirección del Trabajo, pero <strong>aún no cuenta con la autorización</strong>{' '}
          de ese Servicio. Mientras no la obtenga, no lo uses como tu único sistema de registro de asistencia.
        </span>
      </div>

      {loading ? (
        <div className="py-6 flex items-center justify-center gap-2 text-xs text-muted">
          <div className="w-4 h-4 border-2 border-accent border-t-transparent rounded-full animate-spin" />
          <span>Cargando información legal...</span>
        </div>
      ) : (
        <div className="grid grid-cols-1 md:grid-cols-2 gap-4 pt-1">
          {/* Employer Terms Card */}
          <div className="p-4 bg-surface-subtle border border-default rounded-card space-y-3">
            <div className="flex items-start justify-between gap-2">
              <div>
                <h3 className="text-xs font-semibold text-fg">Términos y Condiciones del Empleador</h3>
                <div className="text-xs text-muted mt-1 space-y-0.5">
                  <div>
                    Versión aceptada:{' '}
                    <strong className="text-fg">
                      {status?.terms_accepted_version || 'No registrada'}
                    </strong>
                  </div>
                  {bundle && (
                    <div>
                      Hash SHA-256:{' '}
                      <code className="font-mono text-xs text-fg">
                        {bundle.employer_terms.hash.slice(0, 16)}…
                      </code>
                    </div>
                  )}
                </div>
              </div>
            </div>

            <button
              type="button"
              onClick={() => setReadingDoc('employer_terms')}
              disabled={!bundle}
              className="px-3 py-1.5 bg-surface hover:bg-surface-subtle border border-default text-xs font-medium rounded-control flex items-center gap-1.5 transition-colors cursor-pointer"
            >
              <Eye className="w-3.5 h-3.5 text-accent" />
              <span>Leer términos completos</span>
            </button>
          </div>

          {/* Privacy Policy Card */}
          <div className="p-4 bg-surface-subtle border border-default rounded-card space-y-3">
            <div className="flex items-start justify-between gap-2">
              <div>
                <h3 className="text-xs font-semibold text-fg">Política de Privacidad</h3>
                <div className="text-xs text-muted mt-1 space-y-0.5">
                  <div>
                    Versión vigente:{' '}
                    <strong className="text-fg">
                      {bundle?.privacy_policy.version || '2026-10-11'}
                    </strong>
                  </div>
                  {bundle && (
                    <div>
                      Hash SHA-256:{' '}
                      <code className="font-mono text-xs text-fg">
                        {bundle.privacy_policy.hash.slice(0, 16)}…
                      </code>
                    </div>
                  )}
                </div>
              </div>
            </div>

            <button
              type="button"
              onClick={() => setReadingDoc('privacy_policy')}
              disabled={!bundle}
              className="px-3 py-1.5 bg-surface hover:bg-surface-subtle border border-default text-xs font-medium rounded-control flex items-center gap-1.5 transition-colors cursor-pointer"
            >
              <Eye className="w-3.5 h-3.5 text-accent" />
              <span>Leer política completa</span>
            </button>
          </div>
        </div>
      )}

      {/* Reader Modal */}
      {activeDoc && (
        <div className="fixed inset-0 z-50 bg-black/60 flex items-center justify-center p-4">
          <div className="bg-surface border border-default rounded-card max-w-3xl w-full max-h-[85vh] flex flex-col shadow-xl">
            <div className="p-4 border-b border-default flex items-center justify-between">
              <div>
                <h3 className="text-sm font-semibold text-fg">{activeDoc.title}</h3>
                <span className="text-xs text-muted">
                  Versión {activeDoc.version} · Hash: <code className="font-mono">{activeDoc.hash.slice(0, 16)}…</code>
                </span>
              </div>
              <button
                type="button"
                onClick={() => setReadingDoc(null)}
                className="p-1.5 rounded-control text-muted hover:text-fg hover:bg-surface-subtle transition-colors cursor-pointer"
                aria-label="Cerrar modal"
              >
                <X className="w-4 h-4" />
              </button>
            </div>

            <div className="p-5 overflow-y-auto flex-1 text-xs">
              <MarkdownView content={activeDoc.content} />
            </div>

            <div className="p-3 border-t border-default flex justify-end">
              <button
                type="button"
                onClick={() => setReadingDoc(null)}
                className="px-4 py-2 bg-accent hover:bg-accent-hover text-accent-fg font-semibold rounded-control text-xs transition-colors cursor-pointer"
              >
                Cerrar
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
};
