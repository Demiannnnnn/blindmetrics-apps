import React, { useEffect, useRef, useState } from 'react';
import { ShieldCheck, AlertTriangle, FileText, CheckCircle2, LogOut } from 'lucide-react';
import { legalAcceptTerms, legalDocuments, LegalDocumentsBundle } from '../api/tauri-bridge';
import { describeError, errorIs } from '../lib/errors';
import { MarkdownView } from './MarkdownView';

interface TermsGateProps {
  onAccepted: () => void;
  onLogout: () => void;
}

export const TermsGate: React.FC<TermsGateProps> = ({ onAccepted, onLogout }) => {
  const [bundle, setBundle] = useState<LegalDocumentsBundle | null>(null);
  const [loading, setLoading] = useState(true);
  const [submitting, setSubmitting] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);

  const [activeTab, setActiveTab] = useState<'terms' | 'privacy'>('terms');
  const [termsScrolled, setTermsScrolled] = useState(false);
  const [privacyScrolled, setPrivacyScrolled] = useState(false);
  const [acceptedChecked, setAcceptedChecked] = useState(false);

  const termsContainerRef = useRef<HTMLDivElement | null>(null);
  const privacyContainerRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    let isMounted = true;
    setLoading(true);
    setErrorMessage(null);

    legalDocuments()
      .then(b => {
        if (!isMounted) return;
        setBundle(b);
        setLoading(false);
      })
      .catch(err => {
        if (!isMounted) return;
        setErrorMessage(describeError(err, 'No se pudieron cargar los documentos legales.'));
        setLoading(false);
      });

    return () => {
      isMounted = false;
    };
  }, []);

  const checkScrollCompletion = (el: HTMLDivElement | null, type: 'terms' | 'privacy') => {
    if (!el) return;
    const isAtBottom = el.scrollHeight - el.scrollTop - el.clientHeight <= 25;
    if (isAtBottom) {
      if (type === 'terms') {
        setTermsScrolled(true);
      } else {
        setPrivacyScrolled(true);
      }
    }
  };

  // Check scrollability after render or tab switch
  useEffect(() => {
    if (activeTab === 'terms' && termsContainerRef.current) {
      const el = termsContainerRef.current;
      if (el.scrollHeight <= el.clientHeight + 10) {
        setTermsScrolled(true);
      }
    } else if (activeTab === 'privacy' && privacyContainerRef.current) {
      const el = privacyContainerRef.current;
      if (el.scrollHeight <= el.clientHeight + 10) {
        setPrivacyScrolled(true);
      }
    }
  }, [activeTab, bundle]);

  const bothScrolled = termsScrolled && privacyScrolled;

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!bothScrolled || !acceptedChecked || submitting) return;

    setSubmitting(true);
    setErrorMessage(null);

    try {
      await legalAcceptTerms();
      onAccepted();
    } catch (err: unknown) {
      const errStr = String(err);
      if (
        errorIs(err, 'TERMS_VERSION_MISMATCH') ||
        errStr.includes('TERMS_VERSION_MISMATCH') ||
        errStr.includes('409')
      ) {
        setErrorMessage('Actualiza la app para aceptar la versión vigente.');
      } else {
        setErrorMessage(describeError(err, 'Error al registrar la aceptación de términos.'));
      }
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <div className="min-h-screen bg-bg flex flex-col text-fg">
      {/* Ink top bar with the logo (ecosystem auth layout) */}
      <header className="h-14 shrink-0 bg-ink border-b border-ink-line flex items-center px-6 gap-3">
        <span className="w-7 h-7 rounded-control bg-brand flex items-center justify-center" aria-hidden="true">
          <ShieldCheck className="w-4 h-4 text-white" strokeWidth={2} />
        </span>
        <span className="text-sm font-semibold text-ink-fg">Telemetry</span>
        <span className="text-xs text-ink-muted">Consola de administración</span>
      </header>

      <div className="flex-1 flex justify-center items-start px-4 py-8 overflow-y-auto">
        <div className="w-full max-w-2xl bg-surface border border-default rounded-card p-6 sm:p-8 space-y-6">
          <div>
            <h1 className="text-xl sm:text-2xl font-semibold text-fg">Términos del Servicio y Privacidad</h1>
            <p className="text-xs sm:text-sm text-muted mt-1">
              Para continuar utilizando la consola de administración, revisa y acepta los términos vigentes.
            </p>
          </div>

          {/* DT Authorization warning box (marco-legal-cl.md §7 / LEG-07) */}
          <div className="p-3.5 bg-warning-subtle border border-default rounded-card flex items-start gap-3 text-xs text-warning">
            <AlertTriangle className="w-4 h-4 shrink-0 text-warning mt-0.5" />
            <div className="leading-relaxed">
              <strong>Aviso legal:</strong> Telemetry está diseñado para cumplir los requisitos de la Resolución
              Exenta N°38 de 2024 de la Dirección del Trabajo, pero <strong>aún no cuenta con la autorización</strong>{' '}
              de ese Servicio. Mientras no la obtenga, no lo uses como tu único sistema de registro de asistencia.
            </div>
          </div>

          {errorMessage && (
            <div className="p-3 bg-danger-subtle border border-default rounded-control flex items-start gap-2 text-xs text-danger">
              <AlertTriangle className="w-4 h-4 shrink-0 text-danger mt-0.5" />
              <span>{errorMessage}</span>
            </div>
          )}

          {loading ? (
            <div className="py-12 flex flex-col items-center justify-center gap-3 text-muted">
              <div className="w-6 h-6 border-2 border-accent border-t-transparent rounded-full animate-spin" />
              <span className="text-xs">Cargando documentos legales...</span>
            </div>
          ) : bundle ? (
            <form onSubmit={handleSubmit} className="space-y-5">
              {/* Tabs for switching between Employer Terms and Privacy Policy */}
              <div className="flex bg-surface-subtle p-1 rounded-control border border-default text-xs">
                <button
                  type="button"
                  onClick={() => setActiveTab('terms')}
                  className={`flex-1 py-2 px-3 rounded-control font-medium flex items-center justify-center gap-2 transition-colors ${
                    activeTab === 'terms'
                      ? 'bg-surface text-fg border border-default shadow-xs'
                      : 'text-muted hover:text-fg border border-transparent'
                  }`}
                >
                  <FileText className="w-3.5 h-3.5" />
                  <span>Términos del Empleador</span>
                  {termsScrolled && <CheckCircle2 className="w-3.5 h-3.5 text-success" />}
                </button>
                <button
                  type="button"
                  onClick={() => setActiveTab('privacy')}
                  className={`flex-1 py-2 px-3 rounded-control font-medium flex items-center justify-center gap-2 transition-colors ${
                    activeTab === 'privacy'
                      ? 'bg-surface text-fg border border-default shadow-xs'
                      : 'text-muted hover:text-fg border border-transparent'
                  }`}
                >
                  <FileText className="w-3.5 h-3.5" />
                  <span>Política de Privacidad</span>
                  {privacyScrolled && <CheckCircle2 className="w-3.5 h-3.5 text-success" />}
                </button>
              </div>

              {/* Document display */}
              {activeTab === 'terms' && (
                <div className="space-y-2">
                  <div className="flex items-center justify-between text-xs text-muted">
                    <span>
                      Versión <strong className="text-fg">{bundle.employer_terms.version}</strong> · Hash SHA-256:{' '}
                      <code className="font-mono text-xs">{bundle.employer_terms.hash.slice(0, 16)}…</code>
                    </span>
                    <span className={termsScrolled ? 'text-success font-medium' : 'text-muted'}>
                      {termsScrolled ? '✓ Leído hasta el final' : 'Desplaza hasta el final para validar'}
                    </span>
                  </div>
                  <div
                    ref={termsContainerRef}
                    onScroll={e => checkScrollCompletion(e.currentTarget, 'terms')}
                    className="h-72 overflow-y-auto bg-surface-subtle border border-default rounded-card p-4 text-xs"
                    tabIndex={0}
                    role="region"
                    aria-label="Términos del Empleador"
                  >
                    <MarkdownView content={bundle.employer_terms.content} />
                  </div>
                </div>
              )}

              {activeTab === 'privacy' && (
                <div className="space-y-2">
                  <div className="flex items-center justify-between text-xs text-muted">
                    <span>
                      Versión <strong className="text-fg">{bundle.privacy_policy.version}</strong> · Hash SHA-256:{' '}
                      <code className="font-mono text-xs">{bundle.privacy_policy.hash.slice(0, 16)}…</code>
                    </span>
                    <span className={privacyScrolled ? 'text-success font-medium' : 'text-muted'}>
                      {privacyScrolled ? '✓ Leído hasta el final' : 'Desplaza hasta el final para validar'}
                    </span>
                  </div>
                  <div
                    ref={privacyContainerRef}
                    onScroll={e => checkScrollCompletion(e.currentTarget, 'privacy')}
                    className="h-72 overflow-y-auto bg-surface-subtle border border-default rounded-card p-4 text-xs"
                    tabIndex={0}
                    role="region"
                    aria-label="Política de Privacidad"
                  >
                    <MarkdownView content={bundle.privacy_policy.content} />
                  </div>
                </div>
              )}

              {/* Requirement reminder if not both scrolled */}
              {!bothScrolled && (
                <p className="text-xs text-muted italic">
                  Para aceptar, debes revisar ambos documentos desplazándote hasta el final de cada uno.
                  {!termsScrolled && activeTab !== 'terms' && (
                    <button
                      type="button"
                      onClick={() => setActiveTab('terms')}
                      className="ml-1 text-accent underline cursor-pointer"
                    >
                      Ir a Términos del Empleador
                    </button>
                  )}
                  {!privacyScrolled && activeTab !== 'privacy' && (
                    <button
                      type="button"
                      onClick={() => setActiveTab('privacy')}
                      className="ml-1 text-accent underline cursor-pointer"
                    >
                      Ir a Política de Privacidad
                    </button>
                  )}
                </p>
              )}

              {/* Acceptance checkbox */}
              <div className="pt-2 border-t border-default space-y-4">
                <label
                  className={`flex items-start gap-3 text-xs cursor-pointer select-none ${
                    bothScrolled ? 'text-fg' : 'text-muted opacity-50 cursor-not-allowed'
                  }`}
                >
                  <input
                    type="checkbox"
                    checked={acceptedChecked}
                    disabled={!bothScrolled}
                    onChange={e => setAcceptedChecked(e.target.checked)}
                    className="mt-0.5 rounded-control border-default text-accent focus:ring-accent"
                  />
                  <span>
                    He leído y acepto los <strong>Términos del Servicio del Empleador</strong> y la{' '}
                    <strong>Política de Privacidad</strong> en sus versiones vigentes.
                  </span>
                </label>

                <div className="flex items-center justify-between gap-3 pt-2">
                  <button
                    type="button"
                    onClick={onLogout}
                    className="px-4 py-2 border border-default hover:bg-surface-subtle text-muted hover:text-fg text-xs font-medium rounded-control flex items-center gap-1.5 transition-colors cursor-pointer"
                  >
                    <LogOut className="w-3.5 h-3.5" />
                    <span>Cerrar sesión</span>
                  </button>

                  <button
                    type="submit"
                    disabled={!bothScrolled || !acceptedChecked || submitting}
                    className="px-5 py-2 bg-accent hover:bg-accent-hover disabled:opacity-50 text-accent-fg font-semibold rounded-control text-xs transition-colors flex items-center justify-center gap-2 cursor-pointer disabled:cursor-not-allowed"
                  >
                    {submitting ? (
                      <div className="w-3.5 h-3.5 border-2 border-accent-fg border-t-transparent rounded-full animate-spin" />
                    ) : null}
                    <span>Aceptar y continuar</span>
                  </button>
                </div>
              </div>
            </form>
          ) : null}
        </div>
      </div>
    </div>
  );
};
