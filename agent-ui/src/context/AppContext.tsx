import React, { createContext, useContext, useEffect, useState, useCallback, useRef } from 'react';
import type { AppSnapshot } from '../api/types';
import * as bridge from '../api/tauri-bridge';

export type NavView = 'attendance' | 'records' | 'telemetry' | 'my-data' | 'settings';
export type EnrollmentStep = 'link' | 'verify' | 'tamper' | 'revoked';
export type TelemetryMode = 'view' | 'consent' | 'withdraw';

interface AppContextType {
  serviceRunning: boolean;
  serviceLoading: boolean;
  snapshot: AppSnapshot | null;
  currentView: NavView;
  enrollmentStep: EnrollmentStep;
  safetyCode: string | null;
  telemetryMode: TelemetryMode;
  error: string | null;
  pendingQuit: { nonce: string; expiresInSecs: number } | null;
  quitLoading: boolean;
  setCurrentView: (view: NavView) => void;
  setTelemetryMode: (mode: TelemetryMode) => void;
  setEnrollmentStep: (step: EnrollmentStep, code?: string) => void;
  setError: (err: string | null) => void;
  refreshSnapshot: () => Promise<void>;
  retryService: () => Promise<void>;
  handleConfirmQuit: () => Promise<void>;
  handleCancelQuit: () => Promise<void>;
}

const AppContext = createContext<AppContextType | undefined>(undefined);

export const AppProvider: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const [serviceRunning, setServiceRunning] = useState<boolean>(true);
  const [serviceLoading, setServiceLoading] = useState<boolean>(true);
  const [snapshot, setSnapshot] = useState<AppSnapshot | null>(null);
  const [currentView, setCurrentView] = useState<NavView>('attendance');
  const [enrollmentStep, setEnrollmentStepState] = useState<EnrollmentStep>('link');
  const [safetyCode, setSafetyCode] = useState<string | null>(null);
  const [telemetryMode, setTelemetryMode] = useState<TelemetryMode>('view');
  const [error, setError] = useState<string | null>(null);
  const [pendingQuit, setPendingQuit] = useState<{ nonce: string; expiresInSecs: number } | null>(null);
  const [quitLoading, setQuitLoading] = useState<boolean>(false);

  const initialHintHandled = useRef<boolean>(false);

  const setEnrollmentStep = useCallback((step: EnrollmentStep, code?: string) => {
    setEnrollmentStepState(step);
    if (code) setSafetyCode(code);
  }, []);

  const handleSnapshot = useCallback((snap: AppSnapshot) => {
    setSnapshot(snap);
    setServiceRunning(true);
    setServiceLoading(false);

    // RF-AGENT-11-6: tray -> window navigation hint
    if (!initialHintHandled.current && snap.suggested_screen) {
      initialHintHandled.current = true;
      if (snap.suggested_screen === 'TELEMETRY') {
        setCurrentView('telemetry');
        setTelemetryMode('view');
      } else if (snap.suggested_screen === 'WITHDRAW') {
        setCurrentView('telemetry');
        setTelemetryMode('withdraw');
      } else if (snap.suggested_screen === 'HOME') {
        setCurrentView('attendance');
      }
    }
  }, []);

  const refreshSnapshot = useCallback(async () => {
    try {
      const snap = await bridge.fetchSnapshot();
      handleSnapshot(snap);
      setError(null);
    } catch (err: unknown) {
      const msg = err && typeof err === 'object' && 'message' in err ? String(err.message) : String(err);
      setError(msg);
      setServiceRunning(false);
    }
  }, [handleSnapshot]);

  const retryService = useCallback(async () => {
    setServiceLoading(true);
    setError(null);
    try {
      await bridge.ensureService();
      const snap = await bridge.fetchSnapshot();
      handleSnapshot(snap);
      setServiceRunning(true);
    } catch (err: unknown) {
      const msg = err && typeof err === 'object' && 'message' in err ? String(err.message) : String(err);
      setError(msg);
      setServiceRunning(false);
    } finally {
      setServiceLoading(false);
    }
  }, [handleSnapshot]);

  const handleConfirmQuit = useCallback(async () => {
    if (!pendingQuit) return;
    setQuitLoading(true);
    try {
      await bridge.confirmQuit(pendingQuit.nonce);
      setPendingQuit(null);
    } catch (_e) {
      // ignore
    } finally {
      setQuitLoading(false);
    }
  }, [pendingQuit]);

  const handleCancelQuit = useCallback(async () => {
    if (!pendingQuit) return;
    try {
      await bridge.cancelQuit(pendingQuit.nonce);
    } catch (_e) {
      // ignore
    } finally {
      setPendingQuit(null);
    }
  }, [pendingQuit]);

  // Mount effect: connect to service and subscribe to events
  useEffect(() => {
    const unlisteners: Array<() => void> = [];

    async function init() {
      setServiceLoading(true);
      try {
        await bridge.ensureService();
        const snap = await bridge.fetchSnapshot();
        handleSnapshot(snap);
        setServiceRunning(true);
      } catch (_err) {
        setServiceRunning(false);
      } finally {
        setServiceLoading(false);
      }

      try {
        const uSnap = await bridge.listenSnapshot((snap) => {
          handleSnapshot(snap);
        });
        unlisteners.push(uSnap);

        const uQuitReq = await bridge.listenQuitRequested((payload) => {
          setPendingQuit({ nonce: payload.nonce, expiresInSecs: payload.expires_in_secs });
        });
        unlisteners.push(uQuitReq);

        const uQuitCancel = await bridge.listenQuitCancelled(() => {
          setPendingQuit(null);
          setQuitLoading(false);
        });
        unlisteners.push(uQuitCancel);

        const uDisconnect = await bridge.listenServiceDisconnected(() => {
          setServiceRunning(false);
        });
        unlisteners.push(uDisconnect);
      } catch (_e) {
        // Listening failed or running in preview
      }
    }

    init();

    return () => {
      unlisteners.forEach((u) => u());
    };
  }, [handleSnapshot]);

  return (
    <AppContext.Provider
      value={{
        serviceRunning,
        serviceLoading,
        snapshot,
        currentView,
        enrollmentStep,
        safetyCode,
        telemetryMode,
        error,
        pendingQuit,
        quitLoading,
        setCurrentView,
        setTelemetryMode,
        setEnrollmentStep,
        setError,
        refreshSnapshot,
        retryService,
        handleConfirmQuit,
        handleCancelQuit,
      }}
    >
      {children}
    </AppContext.Provider>
  );
};

export function useApp() {
  const context = useContext(AppContext);
  if (!context) {
    throw new Error('useApp must be used within an AppProvider');
  }
  return context;
}
