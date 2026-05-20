import { useEffect, useRef, useState } from "react";
import { ipc } from "../ipc/commands";
import { onWaterEvent } from "../ipc/events";
import type { BouquetItem } from "./Bouquet";
import { HoverDim } from "./hover-dim";
import { PillCapsule } from "./PillCapsule";
import { RabbitHole, type RabbitHoleLevel } from "./RabbitHole";
import type { Pill } from "./types";

const MAX_ON_SCREEN = 2;
/** Below this `<main>` width the pill margin overlaps the prose; capsules
 *  drop to 0.7 opacity so writing underneath stays readable. Matches the
 *  PinnedColumn collapse breakpoint so both fallbacks engage in lockstep. */
const NARROW_BREAKPOINT_PX = 1100;

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
 * the capsule is swapped for a `<RabbitHole>` (rooted at depth 1).
 *
 * Clicking a sub-capsule inside the rabbit hole drills deeper: the chosen
 * sub's id is recorded on its level, `ipc.pillExpand(sub_pill_id)` fires,
 * and when the next `bouquet:ready` arrives we append a new level.
 *
 * The reducer for `bouquet:ready` distinguishes two cases:
 *   1. `parent_pill_id` matches a known top-level pill -> level-0 expansion
 *      (creates / resets a single-level rabbit hole for that pill).
 *   2. Otherwise we look for a sub-pill in every open rabbit hole whose
 *      `sub_pill_id === parent_pill_id`; the matching rabbit hole grows by
 *      one level.
 *
 * The async-subscribe-with-`cancelled`-flag pattern (T4) keeps cleanup
 * correct even when the component unmounts before any `onWaterEvent`
 * promise resolves.
 */
interface PillLayerProps {
  /** Current width of the editor's `<main>`. `0` = not yet measured
   *  (treated the same as "wide enough"); below `NARROW_BREAKPOINT_PX`
   *  triggers translucent capsules. Defaults to `0` so tests / standalone
   *  renders skip the fallback. */
  mainWidth?: number;
  /** The current scene's id. Threaded down to <Bouquet> so pin payloads
   *  carry the real FK (the `pinned_pill` table requires it). Optional so
   *  standalone renders / tests can omit it; falls back to "" which the
   *  pin path drops as a no-op. */
  sceneId?: string;
}

export function PillLayer({ mainWidth = 0, sceneId = "" }: PillLayerProps = {}) {
  const [pills, setPills] = useState<Pill[]>([]);
  const [hoveredId, setHoveredId] = useState<string | null>(null);
  // Per-top-level-pill rabbit-hole path. An entry with one level is the
  // initial single-bouquet expansion (T21); deeper entries are drill-downs.
  const [rabbitHoles, setRabbitHoles] = useState<
    Record<string, RabbitHoleLevel[]>
  >({});
  const layerRef = useRef<HTMLDivElement>(null);
  // Mirror of `pills` for the `bouquet:ready` reducer to read without stale
  // closures (the effect runs once, but pills mutate over the session).
  const pillsRef = useRef<Pill[]>([]);
  pillsRef.current = pills;

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
        setRabbitHoles((prev) => {
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
        setRabbitHoles((prev) => {
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
        setRabbitHoles((prev) => {
          const topLevelMatch = pillsRef.current.find(
            (p) => p.pill_id === e.parent_pill_id,
          );
          if (topLevelMatch) {
            // Level-0 expansion. (Re)seed a single-level rabbit hole.
            return {
              ...prev,
              [topLevelMatch.pill_id]: [
                {
                  parentId: topLevelMatch.pill_id,
                  parentText: topLevelMatch.text,
                  items: e.items,
                  chosenSubId: null,
                },
              ],
            };
          }
          // Deeper expansion. Find the rabbit hole + level holding the
          // sub-pill whose id matches parent_pill_id.
          for (const [rootId, path] of Object.entries(prev)) {
            for (let i = 0; i < path.length; i++) {
              const lvl = path[i];
              if (!lvl) continue;
              const matchingSub = lvl.items.find(
                (it) => it.sub_pill_id === e.parent_pill_id,
              );
              if (matchingSub) {
                // Trim anything below this level (re-drilling overwrites),
                // mark the chosen sub on the current level, then append.
                const trimmed = path.slice(0, i + 1).map((l, idx) =>
                  idx === i
                    ? { ...l, chosenSubId: matchingSub.sub_pill_id }
                    : l,
                );
                return {
                  ...prev,
                  [rootId]: [
                    ...trimmed,
                    {
                      parentId: matchingSub.sub_pill_id,
                      parentText: matchingSub.text,
                      items: e.items,
                      chosenSubId: null,
                    },
                  ],
                };
              }
            }
          }
          // No match - silently drop. (Could happen if the parent was
          // dismissed before the orchestrator responded.)
          return prev;
        });
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
  if (hoveredPill && hoveredPill.block_target_id) {
    const blockEl = document.querySelector(
      `[data-bid="${hoveredPill.block_target_id}"]`,
    );
    anchorRect = blockEl
      ? (blockEl as HTMLElement).getBoundingClientRect()
      : null;
  }

  const closeRabbitHole = (rootId: string) => {
    setRabbitHoles((prev) => {
      if (!(rootId in prev)) return prev;
      const next = { ...prev };
      delete next[rootId];
      return next;
    });
  };

  const onSubClick = (rootId: string, level: number, item: BouquetItem) => {
    setRabbitHoles((prev) => {
      const path = prev[rootId];
      if (!path) return prev;
      const target = path[level];
      if (!target) return prev;
      const nextPath = path.map((l, idx) =>
        idx === level ? { ...l, chosenSubId: item.sub_pill_id } : l,
      );
      return { ...prev, [rootId]: nextPath };
    });
    void ipc.pillExpand(item.sub_pill_id);
  };

  return (
    <>
      <HoverDim
        active={hoveredPill !== null}
        anchorRect={anchorRect}
        hueToken={hoveredPill?.hue_token ?? "--water-hue-muse"}
      />
      <div
        ref={layerRef}
        aria-label="pill margin"
        data-narrow={
          mainWidth > 0 && mainWidth < NARROW_BREAKPOINT_PX ? "true" : undefined
        }
        style={{
          position: "absolute",
          top: 72,
          right: 16,
          width: 240,
          display: "flex",
          flexDirection: "column",
          gap: 12,
          pointerEvents: "none",
          // Narrow-viewport fallback: capsules overlap the prose, so make
          // them translucent. `mainWidth === 0` (unmeasured) keeps full
          // opacity to avoid a flash on first paint.
          opacity:
            mainWidth > 0 && mainWidth < NARROW_BREAKPOINT_PX ? 0.7 : 1,
          transition: "opacity 160ms ease",
        }}
      >
        {pills.map((p) => {
          const path = rabbitHoles[p.pill_id];
          if (path && path.length > 0) {
            return (
              <div key={p.pill_id} style={{ pointerEvents: "auto" }}>
                <RabbitHole
                  hueToken={p.hue_token}
                  path={path}
                  onSubClick={(level, item) => onSubClick(p.pill_id, level, item)}
                  onClose={() => closeRabbitHole(p.pill_id)}
                  rootPill={p}
                  sceneId={sceneId}
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
