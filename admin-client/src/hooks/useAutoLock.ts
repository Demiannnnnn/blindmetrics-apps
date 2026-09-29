import { useEffect, useRef, useState, useCallback } from 'react';
import { lockAndFlushMemory } from '../api/tauri-bridge';

export const useAutoLock = (initialTimeoutMinutes: number = 5) => {
  const [isLocked, setIsLocked] = useState<boolean>(false);
  const [timeoutMinutes, setTimeoutMinutes] = useState<number>(() => {
    const saved = localStorage.getItem('telemetry_autolock_minutes');
    return saved ? parseInt(saved, 10) : initialTimeoutMinutes;
  });

  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const lockApp = useCallback(async () => {
    setIsLocked(true);
    try {
      await lockAndFlushMemory();
    } catch (e) {
      console.error('Failed to flush memory on lock', e);
    }
  }, []);

  const resetTimer = useCallback(() => {
    if (isLocked) return;
    if (timerRef.current) clearTimeout(timerRef.current);
    timerRef.current = setTimeout(lockApp, timeoutMinutes * 60 * 1000);
  }, [isLocked, timeoutMinutes, lockApp]);

  const updateTimeoutMinutes = (minutes: number) => {
    setTimeoutMinutes(minutes);
    localStorage.setItem('telemetry_autolock_minutes', minutes.toString());
  };

  useEffect(() => {
    const events = ['mousemove', 'keydown', 'mousedown', 'scroll'];
    const handleActivity = () => resetTimer();

    events.forEach(event => window.addEventListener(event, handleActivity));
    resetTimer();

    return () => {
      events.forEach(event => window.removeEventListener(event, handleActivity));
      if (timerRef.current) clearTimeout(timerRef.current);
    };
  }, [resetTimer]);

  return {
    isLocked,
    setIsLocked,
    lockApp,
    resetTimer,
    timeoutMinutes,
    setTimeoutMinutes: updateTimeoutMinutes,
  };
};

export default useAutoLock;
