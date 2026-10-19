import React, { useCallback, useEffect, useRef, useState } from 'react';
import ReactMarkdown, { type Components } from 'react-markdown';
import remarkGfm from 'remark-gfm';
import { ArrowLeft, FileText } from 'lucide-react';
import * as bridge from '../api/tauri-bridge';
import type { LegalDocument, LegalDocumentKind } from '../api/types';
import { useApp } from '../context/AppContext';
import { attendanceErrorMessage, canAcceptTerms, hashPrefix, reachedEnd } from '../lib/attendance';
import { BUTTON_PRIMARY, BUTTON_SECONDARY, Notice, Skeleton } from '../components/ui';

/** `initial`: first acceptance (blocking). `update`: new version, marking keeps working. `review`: read again. */
export type TermsMode = 'initial' | 'update' | 'review';

const DOC_FILES: Record<string, LegalDocumentKind> = {
  'terminos-trabajador.md': 'WORKER_TERMS',
  'politica-privacidad.md': 'PRIVACY',
};

const DOC_TITLES: Record<LegalDocumentKind, string> = {
  WORKER_TERMS: 'Términos y condiciones',
  PRIVACY: 'Política de privacidad',
};

const REJECT_MESSAGE = 'Sin aceptar los términos no puedes usar esta app. Habla con tu empleador.';

/**
 * Markdown components: no raw HTML (react-markdown escapes it and `skipHtml` drops it), links
 * between the two documents scroll inside the viewer, other links are shown but never navigate the
 * WebView away from the app.
 */
function markdownComponents(onDocLink: (kind: LegalDocumentKind) => void): Components {
  return {
    h1: ({ children }) => <h1 className="text-lg font-semibold text-fg mt-2 mb-3">{children}</h1>,
    h2: ({ children }) => <h2 className="text-base font-semibold text-fg mt-5 mb-2">{children}</h2>,
    h3: ({ children }) => <h3 className="text-sm font-semibold text-fg mt-4 mb-1.5">{children}</h3>,
    h4: ({ children }) => <h4 className="text-sm font-medium text-fg mt-3 mb-1">{children}</h4>,
    p: ({ children }) => <p className="text-[13px] leading-relaxed text-fg mb-2.5">{children}</p>,
    ul: ({ children }) => <ul className="list-disc pl-5 mb-2.5 space-y-1 text-[13px] text-fg">{children}</ul>,
    ol: ({ children }) => <ol className="list-decimal pl-5 mb-2.5 space-y-1 text-[13px] text-fg">{children}</ol>,
    li: ({ children }) => <li className="leading-relaxed">{children}</li>,
    strong: ({ children }) => <strong className="font-semibold">{children}</strong>,
    blockquote: ({ children }) => (
      <blockquote className="border-l-2 border-strong pl-3 text-muted mb-2.5">{children}</blockquote>
    ),
    code: ({ children }) => (
      <code className="font-mono text-[12px] bg-surface-subtle px-1 rounded-badge">{children}</code>
    ),
    hr: () => <hr className="my-4 border-default" />,
    table: ({ children }) => (
      <div className="overflow-x-auto mb-3">
        <table className="w-full text-[12px] border border-default border-collapse">{children}</table>
      </div>
    ),
    th: ({ children }) => (
      <th className="text-left font-medium text-muted bg-surface-subtle border border-default px-2 py-1.5 align-top">
        {children}
      </th>
    ),
    td: ({ children }) => <td className="border border-default px-2 py-1.5 align-top">{children}</td>,
    a: ({ href, children }) => {
      const file = (href ?? '').split('#')[0].split('/').pop() ?? '';
      const kind = DOC_FILES[file];
      if (kind) {
        return (
          <button
            type="button"
            onClick={() => onDocLink(kind)}
            className="text-accent underline underline-offset-2 hover:text-accent-hover focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent rounded-badge"
          >
            {children}
          </button>
        );
      }
      // External link: shown with its address, never opened inside the WebView.
      return (
        <span className="text-fg">
          {children}
          {href && !href.startsWith('#') ? <span className="text-muted"> ({href})</span> : null}
        </span>
      );
    },
  };
}

export const TermsView: React.FC<{ mode: TermsMode; onClose?: () => void }> = ({ mode, onClose }) => {
  const { refreshSnapshot } = useApp();
  const [docs, setDocs] = useState<LegalDocument[] | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [progress, setProgress] = useState({ terms: false, privacy: false });
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<{ tone: 'danger' | 'warning'; text: string } | null>(null);

  const scrollRef = useRef<HTMLDivElement>(null);
  const endRefs = useRef<Record<LegalDocumentKind, HTMLDivElement | null>>({ WORKER_TERMS: null, PRIVACY: null });
  const startRefs = useRef<Record<LegalDocumentKind, HTMLElement | null>>({ WORKER_TERMS: null, PRIVACY: null });

  const load = useCallback(async () => {
    setLoadError(null);
    try {
      setDocs(await bridge.legalDocuments());
    } catch (err) {
      setLoadError(attendanceErrorMessage(bridge.toCommandError(err)));
    }
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  const checkProgress = useCallback(() => {
    const el = scrollRef.current;
    if (!el) return;
    const ends = endRefs.current;
    const seen = (kind: LegalDocumentKind) => {
      const end = ends[kind];
      return !!end && reachedEnd(end.offsetTop, el.scrollTop, el.clientHeight);
    };
    setProgress((prev) => ({
      terms: prev.terms || seen('WORKER_TERMS'),
      privacy: prev.privacy || seen('PRIVACY'),
    }));
  }, []);

  useEffect(() => {
    if (!docs) return;
    checkProgress();
    window.addEventListener('resize', checkProgress);
    return () => window.removeEventListener('resize', checkProgress);
  }, [docs, checkProgress]);

  const scrollToDoc = useCallback((kind: LegalDocumentKind) => {
    const el = scrollRef.current;
    const start = startRefs.current[kind];
    if (el && start) el.scrollTo({ top: start.offsetTop, behavior: 'smooth' });
  }, []);

  const components = markdownComponents(scrollToDoc);
  const terms = docs?.find((d) => d.document === 'WORKER_TERMS');
  const privacy = docs?.find((d) => d.document === 'PRIVACY');
  const ready = canAcceptTerms(progress, busy) && !!terms && !!privacy;

  const accept = async () => {
    if (!terms || !privacy) return;
    setBusy(true);
    setMessage(null);
    try {
      await bridge.acceptTerms(terms.version, terms.hash, privacy.version, privacy.hash);
      await refreshSnapshot();
      onClose?.();
    } catch (err) {
      setMessage({ tone: 'danger', text: attendanceErrorMessage(bridge.toCommandError(err)) });
    } finally {
      setBusy(false);
    }
  };

  const ordered = [terms, privacy].filter((d): d is LegalDocument => !!d);

  return (
    <div className="h-screen w-screen flex flex-col bg-bg text-fg overflow-hidden">
      <header className="shrink-0 bg-ink text-ink-fg px-4 h-12 flex items-center gap-2">
        {mode !== 'initial' && onClose && (
          <button
            type="button"
            onClick={onClose}
            aria-label="Volver"
            className="p-1 -ml-1 rounded-control text-ink-muted hover:text-ink-fg focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent"
          >
            <ArrowLeft className="w-4 h-4" aria-hidden="true" />
          </button>
        )}
        <FileText className="w-4 h-4 text-brand" aria-hidden="true" />
        <h1 className="text-sm font-semibold">Términos y privacidad</h1>
      </header>

      {mode === 'update' && (
        <div role="status" className="shrink-0 px-4 py-2 text-xs border-b bg-info-subtle border-info/30 text-fg">
          Hay una nueva versión de los términos. Puedes seguir marcando tu asistencia mientras la lees.
        </div>
      )}

      <div className="shrink-0 px-4 py-2 border-b border-default bg-surface text-[11px] text-muted space-y-0.5">
        {ordered.length > 0 ? (
          ordered.map((d) => (
            <div key={d.document} className="flex flex-wrap gap-x-2">
              <span className="font-medium text-fg">{DOC_TITLES[d.document]}</span>
              <span>Versión {d.version || 'sin versión'}</span>
              <span className="font-mono tabular-nums">Hash {hashPrefix(d.hash)}</span>
            </div>
          ))
        ) : (
          <span>Cargando documentos…</span>
        )}
      </div>

      <div
        ref={scrollRef}
        onScroll={checkProgress}
        className="relative flex-1 overflow-y-auto px-5 py-4 bg-surface select-text"
        tabIndex={0}
        aria-label="Texto completo de los términos y la política de privacidad"
      >
        {loadError ? (
          <div className="space-y-3">
            <Notice tone="danger" role="alert">
              No se pudieron cargar los textos legales. {loadError}
            </Notice>
            <button type="button" className={BUTTON_SECONDARY} onClick={load}>
              Reintentar
            </button>
          </div>
        ) : !docs ? (
          <div className="space-y-2">
            <Skeleton className="h-5 w-2/3" />
            <Skeleton className="h-3 w-full" />
            <Skeleton className="h-3 w-full" />
            <Skeleton className="h-3 w-5/6" />
          </div>
        ) : (
          ordered.map((d, i) => (
            <section
              key={d.document}
              ref={(el) => {
                startRefs.current[d.document] = el;
              }}
              aria-label={DOC_TITLES[d.document]}
              className={i > 0 ? 'mt-8 pt-6 border-t-2 border-strong' : ''}
            >
              <ReactMarkdown remarkPlugins={[remarkGfm]} skipHtml components={components}>
                {d.text}
              </ReactMarkdown>
              <div
                ref={(el) => {
                  endRefs.current[d.document] = el;
                }}
                className="h-px"
                aria-hidden="true"
              />
            </section>
          ))
        )}
      </div>

      <footer className="shrink-0 border-t border-default bg-surface px-4 py-3 space-y-2">
        {message && (
          <Notice tone={message.tone} role="alert">
            {message.text}
          </Notice>
        )}
        {mode !== 'review' && !ready && docs && (
          <p className="text-[11px] text-muted">
            {!progress.terms
              ? 'Lee los términos hasta el final para continuar.'
              : 'Lee la política de privacidad hasta el final para continuar.'}
          </p>
        )}
        <div className="flex gap-2">
          {mode === 'review' ? (
            <button type="button" className={BUTTON_SECONDARY} onClick={onClose}>
              Volver
            </button>
          ) : (
            <>
              <button type="button" className={BUTTON_PRIMARY} disabled={!ready} onClick={accept}>
                {busy ? 'Enviando…' : 'He leído y acepto'}
              </button>
              {mode === 'initial' ? (
                <button
                  type="button"
                  className={BUTTON_SECONDARY}
                  disabled={busy}
                  onClick={() => setMessage({ tone: 'warning', text: REJECT_MESSAGE })}
                >
                  No acepto
                </button>
              ) : (
                <button type="button" className={BUTTON_SECONDARY} disabled={busy} onClick={onClose}>
                  Leer después
                </button>
              )}
            </>
          )}
        </div>
      </footer>
    </div>
  );
};
