import { useEffect, useState } from "react";
import { onWaterEvent } from "../ipc/events";
import { PillCapsule } from "./PillCapsule";
import type { Pill } from "./types";

const MAX_ON_SCREEN = 2;

/**
 * Absolute-positioned overlay anchored to the top-right of the editor canvas.
 *
 * Subscribes to `pill:emerged` / `pill:dismissed` / `pill:evicted` and renders
 * up to `MAX_ON_SCREEN` (2) capsules. When a third pill emerges, FIFO
 * eviction drops the oldest. The layer itself is `pointer-events: none`;
 * the capsules re-enable pointer events so clicks land on them.
 *
 * The async-subscribe-with-`cancelled`-flag pattern (T4) keeps cleanup
 * correct even when the component unmounts before any `onWaterEvent`
 * promise resolves.
 */
export function PillLayer() {
  const [pills, setPills] = useState<Pill[]>([]);

  useEffect(() => {
    let cancelled = false;
    const unsubs: Array<() => void> = [];

    (async () => {
      const u1 = await onWaterEvent("pill:emerged", (p) => {
        setPills((prev) => {
          const next = [...prev, p];
          // FIFO: when over capacity, drop the oldest entries.
          return next.length > MAX_ON_SCREEN
            ? next.slice(next.length - MAX_ON_SCREEN)
            : next;
        });
      });
      if (cancelled) {
        u1();
        return;
      }
      unsubs.push(u1);

      const u2 = await onWaterEvent("pill:dismissed", (e) => {
        setPills((prev) => prev.filter((x) => x.pill_id !== e.pill_id));
      });
      if (cancelled) {
        u2();
        return;
      }
      unsubs.push(u2);

      const u3 = await onWaterEvent("pill:evicted", (e) => {
        setPills((prev) => prev.filter((x) => x.pill_id !== e.pill_id));
      });
      if (cancelled) {
        u3();
        return;
      }
      unsubs.push(u3);
    })();

    return () => {
      cancelled = true;
      for (const u of unsubs) u();
    };
  }, []);

  return (
    <div
      aria-label="pill margin"
      style={{
        position: "absolute",
        top: 72,
        right: 16,
        width: 240,
        display: "flex",
        flexDirection: "column",
        gap: 12,
        pointerEvents: "none",
      }}
    >
      {pills.map((p) => (
        <PillCapsule key={p.pill_id} pill={p} />
      ))}
    </div>
  );
}
