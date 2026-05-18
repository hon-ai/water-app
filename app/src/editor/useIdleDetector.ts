import { useCallback, useEffect, useRef } from "react";

export interface IdleDetectorHandle {
  onActivity: () => void;
}

/**
 * Fires `onIdle` after `delayMs` of no activity. Restarts on every call to
 * `onActivity`. Cleans up on unmount.
 */
export function useIdleDetector(delayMs: number, onIdle: () => void): IdleDetectorHandle {
  const timerRef = useRef<number | undefined>(undefined);
  const callbackRef = useRef(onIdle);
  callbackRef.current = onIdle;

  const onActivity = useCallback(() => {
    if (timerRef.current !== undefined) window.clearTimeout(timerRef.current);
    timerRef.current = window.setTimeout(() => callbackRef.current(), delayMs);
  }, [delayMs]);

  useEffect(() => {
    return () => {
      if (timerRef.current !== undefined) window.clearTimeout(timerRef.current);
    };
  }, []);

  return { onActivity };
}
