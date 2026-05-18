import { useEffect, useRef, useState } from "react";
import { ipc } from "../ipc/commands";
import { onWaterEvent } from "../ipc/events";
import { Bouquet, type BouquetItem } from "./Bouquet";
import { HoverDim } from "./hover-dim";
import { PillCapsule } from "./PillCapsule";
import type { Pill } from "./types";

const MAX_ON_SCREEN = 2;

/**
 * Absolute-positioned overlay anchored to the top-right of the editor canvas.
 *
 * Subscribes to `pill:emerged` / `pill:dismissed` / `pill:evicted` and renders
 * up to `MAX_ON_SCREEN` (2) capsules. When a third pill emerges, FIFO
 * eviction drops the oldest. The layer itself is `pointer-events: none`;
 * each capsule's wrapper re-enables pointer events so hover + clicks land.
 *
 * On hover, tracks `hoveredId` and computes:
 * - `sourceRect`: the bounding box of the hovered `[data-pill-id]` capsule.
 * - `anchorRect`: the bounding box of the corresponding `[data-bid]` block
 *   in the editor (or `null` if the pill is unanchored).
 *
 * These rects are passed to `<HoverDim>`, which fades a global backdrop and
 * draws an SVG glow line connecting capsule -> anchored block.
 *
 * Clicking a capsule invokes `ipc.pillExpand`; the orchestrator (M2 stub /
 * Phase F real wiring) responds with a `bouquet:ready` event, after which
 * the capsule is swapped for a `<Bouquet>` with 3 sub-capsules + controls.
 *
 * The async-subscribe-with-`cancelled`-flag pattern (T4) keeps cleanup
 * correct even when the component unmounts before any `onWaterEvent`
 * promise resolves.
 */
export function PillLayer() {
  const [pills, setPills] = useState<Pill[]>([]);
  const [hoveredId, setHoveredId] = useState<string | null>(null);
  const [bouquets, setBouquets] = useState<Record<string, BouquetItem[]>>({});
  const layerRef = useRef<HTMLDivElement>(null);

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
        setBouquets((prev) => {
          if (!(e.pill_id in prev)) return prev;
          const next = { ...prev };
          delete next[e.pill_id];
          return next;
        });
      });
      if (cancelled) {
        u2();
        return;
      }
      unsubs.push(u2);

      const u3 = await onWaterEvent("pill:evicted", (e) => {
        setPills((prev) => prev.filter((x) => x.pill_id !== e.pill_id));
        setBouquets((prev) => {
          if (!(e.pill_id in prev)) return prev;
          const next = { ...prev };
          delete next[e.pill_id];
          return next;
        });
      });
      if (cancelled) {
        u3();
        return;
      }
      unsubs.push(u3);

      const u4 = await onWaterEvent("bouquet:ready", (e) => {
        setBouquets((prev) => ({ ...prev, [e.parent_pill_id]: e.items }));
      });
      if (cancelled) {
        u4();
        return;
      }
      unsubs.push(u4);
    })();

    return () => {
      cancelled = true;
      for (const u of unsubs) u();
    };
  }, []);

  const hoveredPill = pills.find((p) => p.pill_id === hoveredId) ?? null;
  let anchorRect: DOMRect | null = null;
  let sourceRect: DOMRect | null = null;
  if (hoveredPill) {
    const sourceEl = layerRef.current?.querySelector(
      `[data-pill-id="${hoveredPill.pill_id}"]`,
    );
    sourceRect = sourceEl
      ? (sourceEl as HTMLElement).getBoundingClientRect()
      : null;
    if (hoveredPill.block_target_id) {
      const blockEl = document.querySelector(
        `[data-bid="${hoveredPill.block_target_id}"]`,
      );
      anchorRect = blockEl
        ? (blockEl as HTMLElement).getBoundingClientRect()
        : null;
    }
  }

  const closeBouquet = (parentId: string) => {
    setBouquets((prev) => {
      if (!(parentId in prev)) return prev;
      const next = { ...prev };
      delete next[parentId];
      return next;
    });
  };

  return (
    <>
      <HoverDim
        active={hoveredPill !== null}
        anchorRect={anchorRect}
        sourceRect={sourceRect}
        hueToken={hoveredPill?.hue_token ?? "--water-hue-muse"}
      />
      <div
        ref={layerRef}
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
        {pills.map((p) => {
          const bouquetItems = bouquets[p.pill_id];
          if (bouquetItems) {
            return (
              <div key={p.pill_id} style={{ pointerEvents: "auto" }}>
                <Bouquet
                  parentId={p.pill_id}
                  hueToken={p.hue_token}
                  items={bouquetItems}
                  onClose={() => closeBouquet(p.pill_id)}
                />
              </div>
            );
          }
          return (
            <div
              key={p.pill_id}
              onMouseEnter={() => setHoveredId(p.pill_id)}
              onMouseLeave={() =>
                setHoveredId((prev) => (prev === p.pill_id ? null : prev))
              }
              style={{ pointerEvents: "auto" }}
            >
              <PillCapsule
                pill={p}
                onClick={() => {
                  void ipc.pillExpand(p.pill_id);
                }}
              />
            </div>
          );
        })}
      </div>
    </>
  );
}
