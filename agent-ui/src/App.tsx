import React from 'react';
import { useApp } from './context/AppContext';
import { ServiceUnavailableView } from './views/ServiceUnavailableView';
import { EnrollmentView } from './views/EnrollmentView';
import { VerificationView } from './views/VerificationView';
import { TamperingAlertView } from './views/TamperingAlertView';
import { RevokedView } from './views/RevokedView';
import { Shell } from './components/Shell';

import { Modal } from './components/Modal';

export const AppContent: React.FC = () => {
  const {
    serviceRunning,
    serviceLoading,
    snapshot,
    enrollmentStep,
    pendingQuit,
    quitLoading,
    handleConfirmQuit,
    handleCancelQuit,
  } = useApp();

  const renderContent = () => {
    if (serviceLoading && !snapshot) {
      return (
        <div className="h-screen w-screen flex flex-col items-center justify-center bg-bg text-fg">
          <div className="w-6 h-6 border-2 border-accent border-t-transparent rounded-full animate-spin mb-3" />
          <span className="text-xs text-muted">Conectando con el servicio…</span>
        </div>
      );
    }

    if (!serviceRunning) {
      return <ServiceUnavailableView />;
    }

    if (snapshot && !snapshot.is_enrolled) {
      switch (enrollmentStep) {
        case 'link':
          return <EnrollmentView />;
        case 'verify':
          return <VerificationView />;
        case 'tamper':
          return <TamperingAlertView />;
        case 'revoked':
          return <RevokedView />;
        default:
          return <EnrollmentView />;
      }
    }

    return <Shell />;
  };

  return (
    <>
      {renderContent()}
      <Modal
        title="¿Salir de Telemetry?"
        isOpen={pendingQuit !== null}
        onClose={handleCancelQuit}
      >
        <div className="space-y-4">
          <p className="text-xs text-muted leading-relaxed">
            Si sales, la telemetría se detiene y no podrás marcar asistencia desde este equipo hasta volver a abrir la app.
          </p>
          <div className="flex space-x-2 pt-2">
            <button
              type="button"
              onClick={handleConfirmQuit}
              disabled={quitLoading}
              className="flex-1 h-9 rounded-control bg-danger text-white hover:bg-danger-hover font-medium text-xs transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-danger"
            >
              {quitLoading ? 'Saliendo…' : 'Salir del servicio'}
            </button>
            <button
              type="button"
              onClick={handleCancelQuit}
              disabled={quitLoading}
              className="flex-1 h-9 rounded-control border border-default bg-surface text-fg hover:bg-surface-subtle font-medium text-xs transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent"
            >
              Cancelar
            </button>
          </div>
        </div>
      </Modal>
    </>
  );
};

export const App: React.FC = () => {
  return <AppContent />;
};

export default App;
