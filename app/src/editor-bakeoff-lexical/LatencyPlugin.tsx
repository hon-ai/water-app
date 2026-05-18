// LatencyPlugin — wraps the editor's update lifecycle to measure
// keypress→paint latency.
//
// Strategy:
//   - Listen for native `beforeinput` events on the editor's contenteditable
//     surface (most accurate "user pressed a key" timestamp; fires before
//     Lexical processes the event).
//   - Push the timestamp onto a queue.
//   - In `registerUpdateListener`, schedule a double-rAF, then drain the
//     queue, computing `paintTime - pressTime`. Two rAFs is the standard
//     "after layout + paint" approximation in browsers.
//
// Stored: every sample, plus rolling stats (count, median, p95) so the
// harness toolbar can display them after the 60 s window.

import { useEffect, useRef } from "react";
import { useLexicalComposerContext } from "@lexical/react/LexicalComposerContext";

export type LatencyStats = {
  count: number;
  medianMs: number;
  p95Ms: number;
  samples: number[];
};

export function emptyStats(): LatencyStats {
  return { count: 0, medianMs: 0, p95Ms: 0, samples: [] };
}

export function summarize(samples: number[]): LatencyStats {
  if (samples.length === 0) return emptyStats();
  const sorted = [...samples].sort((a, b) => a - b);
  const median = sorted[Math.floor(sorted.length / 2)] ?? 0;
  const p95 =
    sorted[Math.min(sorted.length - 1, Math.floor(sorted.length * 0.95))] ?? 0;
  return { count: samples.length, medianMs: median, p95Ms: p95, samples };
}

export type LatencyController = {
  start: () => void;
  stop: () => LatencyStats;
  isRunning: () => boolean;
};

export function LatencyPlugin({
  controllerRef,
}: {
  controllerRef: React.MutableRefObject<LatencyController | null>;
}) {
  const [editor] = useLexicalComposerContext();
  const runningRef = useRef(false);
  const samplesRef = useRef<number[]>([]);
  const pendingPressRef = useRef<number[]>([]);

  useEffect(() => {
    const root = editor.getRootElement();
    if (!root) return;

    const onBeforeInput = () => {
      if (!runningRef.current) return;
      pendingPressRef.current.push(performance.now());
    };

    root.addEventListener("beforeinput", onBeforeInput);

    const unregisterUpdate = editor.registerUpdateListener(() => {
      if (!runningRef.current) return;
      if (pendingPressRef.current.length === 0) return;
      const pressTimes = pendingPressRef.current.splice(0);
      requestAnimationFrame(() => {
        requestAnimationFrame(() => {
          const painted = performance.now();
          for (const t of pressTimes) {
            const dt = painted - t;
            // Guard against absurd values from background-tab throttling.
            if (dt >= 0 && dt < 5000) samplesRef.current.push(dt);
          }
        });
      });
    });

    controllerRef.current = {
      start: () => {
        runningRef.current = true;
        samplesRef.current = [];
        pendingPressRef.current = [];
      },
      stop: () => {
        runningRef.current = false;
        return summarize(samplesRef.current);
      },
      isRunning: () => runningRef.current,
    };

    return () => {
      root.removeEventListener("beforeinput", onBeforeInput);
      unregisterUpdate();
      controllerRef.current = null;
    };
  }, [editor, controllerRef]);

  return null;
}
