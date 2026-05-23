import { useEffect, useRef, useState } from "react";
import { ipc } from "../ipc/commands";
import { onWaterEvent } from "../ipc/events";
import type { BouquetItem } from "./Bouquet";
import { DeepenPanel } from "./DeepenPanel";
import { HoverDim } from "./hover-dim";
import { PillCapsule } from "./PillCapsule";
import { RabbitHole, type RabbitHoleLevel } from "./RabbitHole";
import type { Pill } from "./types";
import {
  computeBlockHash,
  resolveAnchor,
  type AnchorPayload,
  type BlockSnapshot,
} from "./anchorResolver";
import { bestSentenceRange } from "./sentenceMatch";

const MAX_ON_SCREEN = 4;
/** Below this `<main>` width the pill margin overlaps the prose; capsules
 *  drop to 0.7 opacity so writing underneath stays readable. Matches the
 *  PinnedColumn collapse breakpoint so both fallbacks engage in lockstep. */
const NARROW_BREAKPOINT_PX = 1100;
/** Hover this long on a pill and it counts as "engaged" — when the FIFO
 *  later pushes it off-screen, it lands in the stash dropdown instead of
 *  being lost. Five seconds is long enough that an accidental hover-over
 *  while reaching for another pill doesn't qualify. */
const HOVER_ENGAGE_MS = 5000;
/** Soft cap on the stash so a long session doesn't accumulate hundreds
 *  of recall slots. Oldest entries fall off the bottom when the cap is
 *  hit; the writer keeps the more recent engagement context. */
const MAX_STASHED = 20;

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

/**
 * Phase 3.5 — snapshot a block's anchor at pill-emerge time. The
 * snippet is a heuristic phrase derived from the block's leading
 * text (first sentence up to ~60 chars on a word boundary) — until
 * the backend emits a real trigger phrase, this gives us a tighter
 * highlight than the whole paragraph while still resolving cleanly
 * through edits.
 *
 * Returns `null` when the block isn't in the DOM (deeper sub-pills
 * without an explicit block, or the block was deleted before the
 * emerge handler ran). Callers treat null as "no precise anchor —
 * skip the highlight entirely."
 */
function captureAnchor(blockId: string | null): AnchorPayload | null {
  if (!blockId) return null;
  const el = document.querySelector(`[data-bid="${blockId}"]`);
  const text = el?.textContent ?? "";
  if (text.length === 0) return null;

  // Derive a 3–10 word snippet. Prefer a clean sentence end; fall
  // back to a word boundary at ~60 chars; last resort, the first 40
  // chars verbatim. Whitespace is preserved so the substring search
  // matches the DOM text exactly.
  const trimmed = text.replace(/^\s+/, "");
  const startOffset = text.length - trimmed.length;
  const sentenceEnd = trimmed.search(/[.!?](\s|$)/);
  let snippet: string;
  if (sentenceEnd !== -1 && sentenceEnd < 70) {
    snippet = trimmed.slice(0, sentenceEnd + 1);
  } else {
    const cap = trimmed.slice(0, 60);
    const lastSpace = cap.lastIndexOf(" ");
    snippet = lastSpace > 20 ? cap.slice(0, lastSpace) : cap.slice(0, 40);
  }
  return {
    blockId,
    snippet,
    blockHash: computeBlockHash(text),
    offsetHint: startOffset,
  };
}

/**
 * Re-read every block in the editor at the moment of hover. Used by
 * the resolver — anchors that drifted since capture get a fresh shot
 * at being located.
 */
function snapshotEditorBlocks(): BlockSnapshot[] {
  const out: BlockSnapshot[] = [];
  document.querySelectorAll<HTMLElement>("[data-bid]").forEach((el) => {
    const id = el.getAttribute("data-bid") ?? "";
    if (!id) return;
    out.push({ blockId: id, text: el.textContent ?? "" });
  });
  return out;
}

export function PillLayer({ mainWidth = 0, sceneId = "" }: PillLayerProps = {}) {
  const [pills, setPills] = useState<Pill[]>([]);
  const [hoveredId, setHoveredId] = useState<string | null>(null);
  // Per-top-level-pill rabbit-hole path. An entry with one level is the
  // initial single-bouquet expansion (T21); deeper entries are drill-downs.
  const [rabbitHoles, setRabbitHoles] = useState<
    Record<string, RabbitHoleLevel[]>
  >({});
  // Pill anchors captured at emerge time. Survives the session; cleared
  // on dismiss/evict so it doesn't leak across pill churn.
  const anchorsRef = useRef<Record<string, AnchorPayload>>({});
  // Per-pill drift flag — true once the hover resolver fell to tier 4
  // (whole-block fallback). Drives the soft pip on PillCapsule.
  const [driftedByPill, setDriftedByPill] = useState<Record<string, boolean>>(
    {},
  );
  // Phase 4: which pills currently have an active deepen panel
  // open. Replaces the M2 bouquet expansion path for clicked pills.
  const [deepenedIds, setDeepenedIds] = useState<Set<string>>(new Set());
  // Engaged-pill stash. A pill becomes "engaged" via long hover
  // (>= HOVER_ENGAGE_MS), being clicked open, or having a sub-pill
  // drilled. When FIFO eviction would otherwise drop an engaged
  // pill, it lands here instead so the writer can pull it back —
  // useful for the case where a thought from earlier in the
  // session re-becomes relevant once the prose moves on.
  const engagedRef = useRef<Set<string>>(new Set());
  // Per-pill hover timer ids. Tracked separately from `engagedRef`
  // because we need to cancel a pending 5s timer when the writer
  // hovers off a pill before it qualifies. Without this the timer
  // would still fire and falsely mark a fleeting hover as engaged.
  const hoverTimersRef = useRef<Record<string, number>>({});
  const [stashedPills, setStashedPills] = useState<Pill[]>([]);
  const [stashOpen, setStashOpen] = useState(false);
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
        // Snapshot the trigger anchor *before* React paints — the
        // block is still in its emerge-time state. Survives later
        // edits via the 4-tier resolver in anchorResolver.ts.
        const a = captureAnchor(p.block_target_id);
        if (a) anchorsRef.current[p.pill_id] = a;
        setPills((prev) => {
          const next = [...prev, p];
          // FIFO: when over capacity, drop the oldest entries.
          if (next.length > MAX_ON_SCREEN) {
            const evictedItems = next.slice(0, next.length - MAX_ON_SCREEN);
            const toStash: Pill[] = [];
            for (const e of evictedItems) {
              if (engagedRef.current.has(e.pill_id)) {
                // Engaged pill — preserve in stash. Keep its anchor
                // payload so a future restore can re-light the
                // source paragraph on hover. Still send pillEvicted
                // for the orchestrator's attribution close-out; the
                // restore later issues a fresh emerge.
                toStash.push(e);
                engagedRef.current.delete(e.pill_id);
              } else {
                delete anchorsRef.current[e.pill_id];
              }
              // Always tell the orchestrator about the FIFO eviction
              // so the learning loop attributes the outcome at the
              // *moment of dropping from active view*. Restoring
              // from stash re-emerges as a separate event.
              void ipc.pillEvicted(e.pill_id).catch(() => {});
            }
            if (toStash.length > 0) {
              setStashedPills((prevStash) => {
                // Newest stashed entries at the top — that's the
                // order writers will look for them in.
                const merged = [...toStash.reverse(), ...prevStash];
                return merged.slice(0, MAX_STASHED);
              });
            }
            return next.slice(next.length - MAX_ON_SCREEN);
          }
          return next;
        });
      });
      if (cancelled) {
        u1();
        return;
      }
      unsubs.push(u1);

      const u2 = await onWaterEvent("pill:dismissed", (e) => {
        delete anchorsRef.current[e.pill_id];
        setPills((prev) => prev.filter((x) => x.pill_id !== e.pill_id));
        setRabbitHoles((prev) => {
          if (!(e.pill_id in prev)) return prev;
          const next = { ...prev };
          delete next[e.pill_id];
          return next;
        });
        setDriftedByPill((prev) => {
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
        delete anchorsRef.current[e.pill_id];
        setPills((prev) => prev.filter((x) => x.pill_id !== e.pill_id));
        setRabbitHoles((prev) => {
          if (!(e.pill_id in prev)) return prev;
          const next = { ...prev };
          delete next[e.pill_id];
          return next;
        });
        setDriftedByPill((prev) => {
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

  /**
   * Mark a pill engaged. Used by both the long-hover timer and the
   * synchronous click paths. Idempotent — adding an already-engaged
   * id is a no-op. We don't bother re-rendering on this change
   * because the engagement flag is only read at eviction time.
   */
  const markEngaged = (pillId: string) => {
    engagedRef.current.add(pillId);
  };

  /**
   * Hover-enter for pills. In addition to setting `hoveredId` (which
   * drives the trigger-highlight effect), arm a 5s timer that
   * promotes the pill to "engaged" if the writer stays on it. The
   * timer cancels on hover-leave so a fly-over doesn't qualify.
   */
  const handlePillHoverEnter = (pillId: string) => {
    setHoveredId(pillId);
    if (engagedRef.current.has(pillId)) return; // already engaged
    if (hoverTimersRef.current[pillId]) return; // timer already running
    const id = window.setTimeout(() => {
      markEngaged(pillId);
      delete hoverTimersRef.current[pillId];
    }, HOVER_ENGAGE_MS);
    hoverTimersRef.current[pillId] = id;
  };

  const handlePillHoverLeave = (pillId: string) => {
    setHoveredId((prev) => (prev === pillId ? null : prev));
    const id = hoverTimersRef.current[pillId];
    if (id !== undefined) {
      window.clearTimeout(id);
      delete hoverTimersRef.current[pillId];
    }
  };

  // Cancel any in-flight hover timers on unmount so they don't
  // fire against a torn-down ref.
  useEffect(() => {
    return () => {
      for (const id of Object.values(hoverTimersRef.current)) {
        window.clearTimeout(id);
      }
      hoverTimersRef.current = {};
    };
  }, []);

  const hoveredPill = pills.find((p) => p.pill_id === hoveredId) ?? null;

  /**
   * Phase 3.5 — resolve the hovered pill's anchor against the
   * editor's current blocks and dispatch the inline trigger
   * highlight via window CustomEvent (Editor.tsx listens and
   * forwards into the PM plugin). When the resolver returns the
   * fallback tier we flag the pill as drifted, so its capsule
   * shows the soft pip.
   */
  const dispatchHighlightFor = (pill: Pill | null) => {
    if (!pill) {
      window.dispatchEvent(new Event("water:clear-trigger-highlight"));
      return;
    }
    const payload = anchorsRef.current[pill.pill_id];
    if (!payload) {
      window.dispatchEvent(new Event("water:clear-trigger-highlight"));
      return;
    }
    const blocks = snapshotEditorBlocks();
    const resolved = resolveAnchor(payload, blocks);
    if (!resolved) {
      window.dispatchEvent(new Event("water:clear-trigger-highlight"));
      setDriftedByPill((prev) =>
        prev[pill.pill_id] ? prev : { ...prev, [pill.pill_id]: true },
      );
      return;
    }
    // Decide the highlight range. Try sentence-level pinpointing
    // first — `bestSentenceRange` matches the pill's text against
    // each sentence in the target block and returns a tight range
    // when one sentence clearly dominates the keyword overlap.
    // Falls back to the whole block (minus leading/trailing
    // whitespace) when the match is ambiguous or the block is a
    // single sentence. Pinpointing makes the link between pill and
    // source feel precise; the whole-block fallback is still
    // useful when the LLM's observation rides on the paragraph's
    // overall mood instead of a single image.
    const block = blocks.find((b) => b.blockId === resolved.blockId);
    const blockText = block?.text ?? "";
    let start: number;
    let end: number;
    const sentenceRange = bestSentenceRange(blockText, pill.text);
    if (sentenceRange) {
      start = sentenceRange.start;
      end = sentenceRange.end;
    } else if (blockText.length > 0) {
      const leadingWs = blockText.length - blockText.trimStart().length;
      const trailingWs = blockText.length - blockText.trimEnd().length;
      start = leadingWs;
      end = blockText.length - trailingWs;
    } else {
      start = resolved.start;
      end = resolved.end;
    }
    window.dispatchEvent(
      new CustomEvent("water:set-trigger-highlight", {
        detail: {
          blockId: resolved.blockId,
          start,
          end,
        },
      }),
    );
    const drifted = resolved.tier === "fallback";
    setDriftedByPill((prev) => {
      const current = prev[pill.pill_id] === true;
      if (current === drifted) return prev;
      return { ...prev, [pill.pill_id]: drifted };
    });
  };

  useEffect(() => {
    dispatchHighlightFor(hoveredPill);
    // dispatchHighlightFor closes over refs/state setters only; the
    // effect should re-run when the hovered pill changes, not on
    // every render.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [hoveredPill?.pill_id]);

  // Tear down any lingering highlight when the pill margin itself
  // unmounts (scene-switch, surface change).
  useEffect(() => {
    return () => {
      window.dispatchEvent(new Event("water:clear-trigger-highlight"));
    };
  }, []);

  const closeRabbitHole = (rootId: string) => {
    setRabbitHoles((prev) => {
      if (!(rootId in prev)) return prev;
      const next = { ...prev };
      delete next[rootId];
      return next;
    });
  };

  const onSubClick = (rootId: string, level: number, item: BouquetItem) => {
    // Drilling into a sub-pill counts as engagement on the root.
    markEngaged(rootId);
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

  /**
   * Pull a stashed pill back into the active list. Treated as a
   * fresh emerge: we re-insert at the top of `pills` and run the
   * same FIFO logic so the oldest active pill makes room (and
   * itself may stash if engaged). The pill's anchor payload is
   * still in `anchorsRef` (we kept it on stashing), so the source-
   * paragraph highlight lights up on hover immediately.
   *
   * The orchestrator already closed attribution at the original
   * eviction; firing `pillExpand` on restore would be wrong (it
   * means "deepen this pill"). We fire-and-forget a fresh emerge
   * signal via `ipc.pillEmerged` so the learning loop sees the
   * recall as a new exposure. If that command doesn't exist yet
   * the catch swallows the rejection — frontend behavior is
   * unaffected.
   */
  const restoreStashed = (pillId: string) => {
    const pill = stashedPills.find((s) => s.pill_id === pillId);
    if (!pill) return;
    setStashedPills((prev) => prev.filter((s) => s.pill_id !== pillId));
    // Re-add to active pills, re-running the FIFO eviction with
    // the same engagement-aware logic. This mirrors the
    // `pill:emerged` reducer path.
    setPills((prev) => {
      const next = [...prev, pill];
      if (next.length > MAX_ON_SCREEN) {
        const evictedItems = next.slice(0, next.length - MAX_ON_SCREEN);
        const toStash: Pill[] = [];
        for (const e of evictedItems) {
          if (engagedRef.current.has(e.pill_id)) {
            toStash.push(e);
            engagedRef.current.delete(e.pill_id);
          } else {
            delete anchorsRef.current[e.pill_id];
          }
          void ipc.pillEvicted(e.pill_id).catch(() => {});
        }
        if (toStash.length > 0) {
          setStashedPills((prevStash) => {
            const merged = [...toStash.reverse(), ...prevStash];
            return merged.slice(0, MAX_STASHED);
          });
        }
        return next.slice(next.length - MAX_ON_SCREEN);
      }
      return next;
    });
    // TODO(attribution): the orchestrator side needs a
    // `pill_reemerged` command so the learning loop counts the
    // restore as a fresh exposure. UI-side this works in isolation;
    // attribution will start flowing once the backend hook lands.
  };

  return (
    <>
      {/* Phase 3.5: the precise inline trigger-highlight has replaced
          the old whole-paragraph rect; we keep HoverDim only for the
          soft dim backdrop. Passing a null anchorRect tells it to
          skip the rectangle. */}
      <HoverDim
        active={hoveredPill !== null}
        anchorRect={null}
        hueToken={hoveredPill?.hue_token ?? "--water-hue-muse"}
      />
      {/* Stash chip — top-right corner of the nudge panel.
          Hidden when no stashed pills exist so the header stays
          uncluttered for new sessions. Click toggles the dropdown
          below it; the dropdown lists engaged pills the FIFO
          would otherwise have dropped from view. */}
      {stashedPills.length > 0 && (
        <StashChip
          count={stashedPills.length}
          open={stashOpen}
          onToggle={() => setStashOpen((v) => !v)}
          pills={stashedPills}
          onRestore={restoreStashed}
        />
      )}
      <div
        ref={layerRef}
        aria-label="nudge panel"
        style={{
          // The aside (this layer's parent) grows with content
          // rather than capping at viewport height + scrolling
          // internally. Pills + open deepen panels flow naturally
          // here; if the total exceeds viewport, the writer
          // scrolls the page to reach the bottom. Removing the
          // internal `overflowY: auto` is what enables the aside
          // to expand instead of clipping its children.
          //
          // `overflowX: clip` keeps any rogue child (a slab with
          // long nowrap text, a pill capsule running over its
          // budget) from pushing the panel — and via flex
          // ripple, the writing column — wider than 280 px. We
          // use `clip` not `hidden` so it doesn't establish a
          // scroll container that would interfere with the
          // stash dropdown's overflow.
          width: "100%",
          overflowX: "clip",
          padding: "56px 16px 24px 16px",
          boxSizing: "border-box",
          display: "flex",
          flexDirection: "column",
          gap: 12,
          // Pills inside re-enable pointer events on their wrappers;
          // gaps between them stay transparent + non-interactive so
          // the writer can drag-select prose underneath if they
          // wanted to (no longer applies because we're not overlay
          // — kept for symmetry with the wrapper pattern).
          pointerEvents: "auto",
        }}
      >
        {pills.map((p) => {
          // Phase 4: deepen panel takes precedence over the M2
          // bouquet path. Clicking a pill no longer goes through
          // `pillExpand` → bouquet; it goes through `pillDeepen` →
          // DeepenPanel. The legacy RabbitHole path stays alive
          // only for pills that were expanded via the older flow
          // (e.g., kept open across a refactor); newly-clicked
          // pills never enter it.
          if (deepenedIds.has(p.pill_id)) {
            return (
              <div
                key={p.pill_id}
                // Same hover→highlight wiring as plain capsules so
                // clicking a pill (which swaps it for the deepen
                // panel) doesn't break the link back to the source
                // paragraph. The dispatchHighlightFor effect keys
                // off `hoveredPill.pill_id`, which we set to the
                // ROOT pill — the deepen panel's children inherit
                // the highlight from the root.
                onMouseEnter={() => handlePillHoverEnter(p.pill_id)}
                onMouseLeave={() => handlePillHoverLeave(p.pill_id)}
                style={{ pointerEvents: "auto" }}
              >
                <DeepenPanel
                  rootPill={p}
                  onClose={() => {
                    setDeepenedIds((prev) => {
                      if (!prev.has(p.pill_id)) return prev;
                      const next = new Set(prev);
                      next.delete(p.pill_id);
                      return next;
                    });
                  }}
                />
              </div>
            );
          }
          const path = rabbitHoles[p.pill_id];
          if (path && path.length > 0) {
            return (
              <div
                key={p.pill_id}
                onMouseEnter={() => handlePillHoverEnter(p.pill_id)}
                onMouseLeave={() => handlePillHoverLeave(p.pill_id)}
                style={{ pointerEvents: "auto" }}
              >
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
              // `min-width: 0` lets this flex-column child shrink
              // below its content's natural width — without it, a
              // PillCapsule with `width: 100%` still won't contract
              // because the parent's intrinsic content size locks it.
              style={{ pointerEvents: "auto", minWidth: 0 }}
            >
              <PillCapsule
                pill={p}
                anchorDrifted={driftedByPill[p.pill_id] === true}
                onClick={() => {
                  // Clicking to expand is a strong engagement
                  // signal; promote immediately so a fast-evict
                  // burst doesn't lose the pill before the timer
                  // would have fired.
                  markEngaged(p.pill_id);
                  setDeepenedIds((prev) => {
                    if (prev.has(p.pill_id)) return prev;
                    const next = new Set(prev);
                    next.add(p.pill_id);
                    return next;
                  });
                }}
              />
            </div>
          );
        })}
      </div>
    </>
  );
}

/**
 * Top-right chip + dropdown that surfaces stashed (engaged-then-
 * evicted) pills. The chip itself shows a glass disc with the stash
 * count; clicking it expands a card listing each stashed pill as a
 * mini row. Clicking a row restores the pill to the active list.
 *
 * Positioned absolute relative to the parent nudge aside so it sits
 * inside the header strip — `top: 12` clears the panel's rounded
 * top corner and the EditorCanvas-rendered "Nudges" label sits to
 * the left of it.
 */
function StashChip({
  count,
  open,
  onToggle,
  pills,
  onRestore,
}: {
  count: number;
  open: boolean;
  onToggle: () => void;
  pills: Pill[];
  onRestore: (pillId: string) => void;
}) {
  return (
    <div
      style={{
        position: "absolute",
        top: 12,
        right: 14,
        zIndex: 4,
        fontFamily: "var(--water-font-sans)",
      }}
    >
      <button
        type="button"
        onClick={onToggle}
        aria-expanded={open}
        aria-label={`${count} stashed nudges`}
        style={{
          display: "inline-flex",
          alignItems: "center",
          gap: 6,
          padding: "4px 10px 4px 8px",
          height: 24,
          borderRadius: 999,
          fontSize: 10,
          fontWeight: 600,
          letterSpacing: 0.4,
          textTransform: "uppercase",
          color: "var(--water-fg-muted)",
          background:
            "color-mix(in srgb, var(--water-bg-paper) 70%, transparent)",
          border:
            "1px solid color-mix(in srgb, var(--water-hairline) 55%, transparent)",
          backdropFilter: "blur(14px) saturate(160%)",
          WebkitBackdropFilter: "blur(14px) saturate(160%)",
          boxShadow: "var(--water-elev-1)",
          cursor: "pointer",
          transition:
            "background var(--water-dur-tiny) var(--water-ease-out-soft)",
        }}
      >
        <span
          aria-hidden
          style={{
            width: 6,
            height: 6,
            borderRadius: "50%",
            background:
              "color-mix(in srgb, var(--water-hue-flow) 75%, transparent)",
          }}
        />
        {count}
      </button>
      {open && (
        <div
          role="region"
          aria-label="stashed nudges"
          style={{
            position: "absolute",
            top: 30,
            right: 0,
            width: 240,
            maxHeight: 320,
            overflowY: "auto",
            padding: 6,
            display: "flex",
            flexDirection: "column",
            gap: 4,
            borderRadius: "var(--water-r-16)",
            background:
              "color-mix(in srgb, var(--water-bg-paper) 88%, transparent)",
            backdropFilter: "blur(22px) saturate(160%)",
            WebkitBackdropFilter: "blur(22px) saturate(160%)",
            border:
              "1px solid color-mix(in srgb, var(--water-hairline) 60%, transparent)",
            boxShadow: "var(--water-elev-2)",
            animation:
              "water-fade-in var(--water-dur-tiny) var(--water-ease-out-soft) both",
          }}
        >
          {pills.map((p) => (
            <button
              type="button"
              key={p.pill_id}
              onClick={() => onRestore(p.pill_id)}
              title="Restore this nudge"
              style={{
                display: "flex",
                alignItems: "flex-start",
                gap: 8,
                padding: "8px 10px",
                width: "100%",
                background: "transparent",
                border: "none",
                borderRadius: "var(--water-r-12)",
                cursor: "pointer",
                textAlign: "left",
                color: "var(--water-fg-default)",
                fontFamily: "var(--water-font-sans)",
                fontSize: 12,
                lineHeight: 1.4,
                transition:
                  "background var(--water-dur-tiny) var(--water-ease-out-soft)",
              }}
              onMouseEnter={(e) => {
                (e.currentTarget as HTMLButtonElement).style.background =
                  "color-mix(in srgb, var(--water-bg-canvas) 55%, transparent)";
              }}
              onMouseLeave={(e) => {
                (e.currentTarget as HTMLButtonElement).style.background =
                  "transparent";
              }}
            >
              <span
                aria-hidden
                style={{
                  flex: "0 0 6px",
                  width: 6,
                  height: 6,
                  marginTop: 6,
                  borderRadius: "50%",
                  background: `color-mix(in oklch, var(${p.hue_token}) 80%, transparent)`,
                }}
              />
              <span
                style={{
                  flex: 1,
                  minWidth: 0,
                  display: "-webkit-box",
                  WebkitLineClamp: 2,
                  WebkitBoxOrient: "vertical",
                  overflow: "hidden",
                  color: "var(--water-fg-muted)",
                }}
              >
                {p.text || "…"}
              </span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
