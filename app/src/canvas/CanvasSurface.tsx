import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  ipc,
  type HeatMetricKind,
  type HeatReadResponse,
  type HeatRow,
  type SceneCanvasRow,
} from "../ipc/commands";
import { onWaterEvent } from "../ipc/events";
import { HeatmapMetricPicker } from "../heat/HeatmapMetricPicker";
import { SceneCard, CARD_W, CARD_H } from "./SceneCard";
import { CanvasIntro } from "./CanvasIntro";
import { ReadingOrderOverlay } from "./ReadingOrderOverlay";

interface Props {
  onOpenScene: (sceneId: string) => void;
}

/**
 * Card spacing must match the layout helper in
 * `crates/water-core/src/canvas/layout.rs`. The auto-flow rust fn
 * lays out cards 240×140 apart; the renderer uses the same when no
 * canvas position is persisted yet.
 */
const CARDS_PER_ROW = 8;
const CARD_SPACING_X = 240;
const CARD_SPACING_Y = 140;
const POSITION_SAVE_DEBOUNCE_MS = 400;

/**
 * The macro spatial canvas (M6 Phase D). Renders every scene in the
 * open project as a draggable card on a 2D pan/zoom surface. Each
 * card carries a tiny sparkline of the active heat metric.
 *
 * Position state lives in two layers:
 * - Persisted: `scene.canvas_x` / `canvas_y` in SQLite (+ frontmatter).
 * - Local: `cards` state, the renderer's working copy that drag
 *   updates live so the writer sees the motion. Debounced writes
 *   flush to disk via `ipc.sceneCanvasSetPosition`.
 */
export function CanvasSurface({ onOpenScene }: Props) {
  const [cards, setCards] = useState<CanvasCard[] | null>(null);
  const [heatPerScene, setHeatPerScene] = useState<
    Record<string, HeatReadResponse["metrics"]>
  >({});
  const [activeMetric, setActiveMetric] = useState<HeatMetricKind>("pacing");
  const [enabledMap, setEnabledMap] = useState<
    Partial<Record<HeatMetricKind, boolean>>
  >({ pacing: true });
  const [pickerOpen, setPickerOpen] = useState(false);
  const [pickerAnchor, setPickerAnchor] = useState<DOMRect | null>(null);
  const chipRef = useRef<HTMLButtonElement | null>(null);
  const [overlayOn, setOverlayOn] = useState(false);
  const [pan, setPan] = useState({ x: 24, y: 24 });
  const [zoom, setZoom] = useState(1);
  const containerRef = useRef<HTMLDivElement | null>(null);
  const panStartRef = useRef<{ x: number; y: number; pan: { x: number; y: number } } | null>(
    null,
  );
  const dragStartRef = useRef<{
    sceneId: string;
    pointerX: number;
    pointerY: number;
    startX: number;
    startY: number;
  } | null>(null);
  const debounceTimers = useRef<Record<string, number>>({});

  // Initial fetch.
  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const [scenes, settings] = await Promise.all([
          ipc.sceneCanvasList(),
          ipc.heatReadSettings(),
        ]);
        if (cancelled) return;
        setEnabledMap({ pacing: true, ...settings.enabled });
        setCards(autoFlow(scenes));
        // Fetch heat per scene in parallel.
        const heatResults = await Promise.all(
          scenes.map(async (s) => {
            try {
              const r = await ipc.heatRead(s.id);
              return [s.id, r.metrics] as const;
            } catch {
              return [s.id, {} as HeatReadResponse["metrics"]] as const;
            }
          }),
        );
        if (cancelled) return;
        setHeatPerScene(Object.fromEntries(heatResults));
      } catch {
        if (!cancelled) setCards([]);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  // Heat refetch on heat:updated.
  useEffect(() => {
    let unsub: (() => void) | undefined;
    let cancelled = false;
    void (async () => {
      const u = await onWaterEvent("heat:updated", (payload) => {
        if (cancelled) return;
        void ipc
          .heatRead(payload.scene_id)
          .then((r) => {
            if (cancelled) return;
            setHeatPerScene((prev) => ({
              ...prev,
              [payload.scene_id]: r.metrics,
            }));
          })
          .catch(() => {});
      });
      if (cancelled) {
        u();
        return;
      }
      unsub = u;
    })();
    return () => {
      cancelled = true;
      unsub?.();
    };
  }, []);

  // Keyboard: O toggles reading-order overlay. Only when no input is
  // focused (so typing in a search field doesn't trip it).
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key !== "o" && e.key !== "O") return;
      const t = e.target as HTMLElement | null;
      if (
        t &&
        (t.tagName === "INPUT" ||
          t.tagName === "TEXTAREA" ||
          t.isContentEditable)
      ) {
        return;
      }
      setOverlayOn((v) => !v);
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, []);

  // Fit-all-on-mount when there are > 3 scenes.
  useEffect(() => {
    if (!cards || cards.length <= 3) return;
    const el = containerRef.current;
    if (!el) return;
    const rect = el.getBoundingClientRect();
    const maxX = Math.max(...cards.map((c) => c.x + CARD_W));
    const maxY = Math.max(...cards.map((c) => c.y + CARD_H));
    const z = Math.min(
      1,
      Math.min((rect.width - 80) / maxX, (rect.height - 80) / maxY),
    );
    setZoom(z);
    setPan({ x: 40, y: 40 });
  }, [cards]);

  const debouncedPersist = useCallback(
    (sceneId: string, x: number, y: number) => {
      const existing = debounceTimers.current[sceneId];
      if (existing !== undefined) window.clearTimeout(existing);
      debounceTimers.current[sceneId] = window.setTimeout(() => {
        void ipc.sceneCanvasSetPosition(sceneId, x, y).catch(() => {
          /* swallow */
        });
        delete debounceTimers.current[sceneId];
      }, POSITION_SAVE_DEBOUNCE_MS);
    },
    [],
  );

  const onContainerPointerDown = (e: React.PointerEvent<HTMLDivElement>) => {
    if (e.target !== e.currentTarget) return;
    panStartRef.current = {
      x: e.clientX,
      y: e.clientY,
      pan: { ...pan },
    };
    e.currentTarget.setPointerCapture(e.pointerId);
  };
  const onContainerPointerMove = (e: React.PointerEvent<HTMLDivElement>) => {
    if (dragStartRef.current) {
      const drag = dragStartRef.current;
      const dx = (e.clientX - drag.pointerX) / zoom;
      const dy = (e.clientY - drag.pointerY) / zoom;
      setCards((prev) =>
        prev
          ? prev.map((c) =>
              c.id === drag.sceneId
                ? { ...c, x: drag.startX + dx, y: drag.startY + dy }
                : c,
            )
          : prev,
      );
      return;
    }
    if (!panStartRef.current) return;
    setPan({
      x: panStartRef.current.pan.x + (e.clientX - panStartRef.current.x),
      y: panStartRef.current.pan.y + (e.clientY - panStartRef.current.y),
    });
  };
  const onContainerPointerUp = (e: React.PointerEvent<HTMLDivElement>) => {
    if (dragStartRef.current) {
      const drag = dragStartRef.current;
      const card = cards?.find((c) => c.id === drag.sceneId);
      if (card) debouncedPersist(card.id, card.x, card.y);
      dragStartRef.current = null;
    }
    panStartRef.current = null;
    try {
      e.currentTarget.releasePointerCapture(e.pointerId);
    } catch {
      /* swallow */
    }
  };
  const onContainerWheel = (e: React.WheelEvent<HTMLDivElement>) => {
    if (!(e.ctrlKey || e.metaKey)) return;
    e.preventDefault();
    const el = containerRef.current;
    if (!el) return;
    const rect = el.getBoundingClientRect();
    const localX = e.clientX - rect.left - pan.x;
    const localY = e.clientY - rect.top - pan.y;
    const direction = -Math.sign(e.deltaY);
    const next = Math.max(0.2, Math.min(2, zoom + direction * 0.1));
    const ratio = next / zoom;
    setPan({
      x: pan.x - localX * (ratio - 1),
      y: pan.y - localY * (ratio - 1),
    });
    setZoom(next);
  };

  const onCardPointerDown = (id: string, e: React.PointerEvent) => {
    const card = cards?.find((c) => c.id === id);
    if (!card) return;
    dragStartRef.current = {
      sceneId: id,
      pointerX: e.clientX,
      pointerY: e.clientY,
      startX: card.x,
      startY: card.y,
    };
    // Capture on the container so move events keep firing even if the
    // pointer leaves the card.
    containerRef.current?.setPointerCapture(e.pointerId);
    e.stopPropagation();
  };

  const sortedForOverlay = useMemo(() => {
    if (!cards) return [];
    return [...cards].sort(
      (a, b) => a.manuscript_ordering - b.manuscript_ordering,
    );
  }, [cards]);

  if (cards === null) {
    return (
      <div
        className="canvas-surface"
        style={{
          flex: 1,
          background: "var(--water-bg-paper)",
          display: "grid",
          placeItems: "center",
        }}
      >
        <div className="water-loading">Loading</div>
      </div>
    );
  }

  return (
    <div
      ref={containerRef}
      className="canvas-surface"
      data-testid="canvas-surface"
      onPointerDown={onContainerPointerDown}
      onPointerMove={onContainerPointerMove}
      onPointerUp={onContainerPointerUp}
      onPointerCancel={onContainerPointerUp}
      onWheel={onContainerWheel}
      style={{
        flex: 1,
        position: "relative",
        background: "var(--water-bg-paper)",
        overflow: "hidden",
        touchAction: "none",
        cursor: panStartRef.current ? "grabbing" : "default",
      }}
    >
      {/* Pan/zoom wrapper. Cards are absolutely positioned in this
          space using the persisted canvas_x / canvas_y. */}
      <div
        style={{
          position: "absolute",
          left: 0,
          top: 0,
          transformOrigin: "0 0",
          transform: `translate(${pan.x}px, ${pan.y}px) scale(${zoom})`,
        }}
      >
        {overlayOn && (
          <ReadingOrderOverlay
            cards={sortedForOverlay}
            cardW={CARD_W}
            cardH={CARD_H}
          />
        )}
        {cards.map((card) => (
          <SceneCard
            key={card.id}
            card={card}
            metrics={heatPerScene[card.id] ?? null}
            activeMetric={activeMetric}
            onPointerDown={(e) => onCardPointerDown(card.id, e)}
            onOpen={() => onOpenScene(card.id)}
          />
        ))}
      </div>
      {/* Top-right metric chip + picker (reused from M5). */}
      <div
        style={{
          position: "absolute",
          top: 16,
          right: 24,
          display: "flex",
          alignItems: "center",
          gap: 8,
        }}
      >
        <button
          type="button"
          aria-label="Toggle reading order"
          aria-pressed={overlayOn}
          onClick={() => setOverlayOn((v) => !v)}
          title="Toggle reading-order overlay (O)"
          style={{
            padding: "6px 12px",
            border: "none",
            background: overlayOn
              ? "color-mix(in srgb, var(--water-hue-flow) 22%, transparent)"
              : "var(--water-bg-raised)",
            color: "var(--water-fg-default)",
            fontFamily: "var(--water-font-sans)",
            fontSize: "var(--water-fs-meta)",
            fontWeight: 500,
            borderRadius: "var(--water-r-8)",
            boxShadow: "var(--water-elev-1)",
            cursor: "pointer",
          }}
        >
          reading order
        </button>
        <button
          ref={chipRef}
          type="button"
          aria-label="Heatmap metrics"
          aria-haspopup="menu"
          aria-expanded={pickerOpen ? "true" : "false"}
          onClick={() => {
            setPickerOpen((v) => {
              const next = !v;
              if (next && chipRef.current) {
                setPickerAnchor(chipRef.current.getBoundingClientRect());
              }
              return next;
            });
          }}
          style={{
            padding: "6px 12px",
            border: "none",
            background: "var(--water-bg-raised)",
            color: "var(--water-fg-muted)",
            fontFamily: "var(--water-font-sans)",
            fontSize: "var(--water-fs-meta)",
            fontWeight: 500,
            borderRadius: "var(--water-r-8)",
            boxShadow: "var(--water-elev-1)",
            cursor: "pointer",
            textTransform: "lowercase",
            letterSpacing: 0.3,
          }}
        >
          {activeMetric.replace("_", " ")} ▾
        </button>
      </div>
      <HeatmapMetricPicker
        open={pickerOpen}
        enabled={enabledMap}
        anchor={pickerAnchor}
        onToggle={(kind, next) => {
          setEnabledMap((prev) => ({ ...prev, [kind]: next }));
          if (next) setActiveMetric(kind);
        }}
        onClose={() => setPickerOpen(false)}
      />
      <CanvasIntro />
    </div>
  );
}

interface CanvasCard extends SceneCanvasRow {
  x: number;
  y: number;
}

/**
 * Mirror of `water_core::canvas::auto_flow`: 8 cards per row at
 * 240 × 140 spacing. Applied to scenes with NULL canvas_x/canvas_y.
 * Scenes WITH persisted positions use those positions verbatim.
 */
function autoFlow(scenes: SceneCanvasRow[]): CanvasCard[] {
  const sorted = [...scenes].sort(
    (a, b) => a.manuscript_ordering - b.manuscript_ordering,
  );
  return sorted.map((s, ix) => {
    const col = ix % CARDS_PER_ROW;
    const row = Math.floor(ix / CARDS_PER_ROW);
    return {
      ...s,
      x: s.canvas_x ?? col * CARD_SPACING_X,
      y: s.canvas_y ?? row * CARD_SPACING_Y,
    };
  });
}

export type { CanvasCard, HeatRow };
