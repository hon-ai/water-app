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
import { SharedAttrConnectors } from "./SharedAttrConnectors";
import { WaterRibbon } from "../chrome/WaterRibbon";

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
const LANE_LABEL_WIDTH = 160;

/**
 * Which axis groups scenes into rows. "free" = use canvas_x/canvas_y;
 * "location" + "character" = layout is automatic (x by manuscript
 * order, y by lane index).
 */
export type LaneMode = "free" | "location" | "character";

/**
 * Snap a (x, y) drag-end position to the nearest grid cell. Keeps
 * the layout visually aligned without forcing it during the drag
 * itself — the writer drags freely and the card lands cleanly on
 * release.
 */
function snapToGrid(x: number, y: number): [number, number] {
  return [
    Math.round(x / CARD_SPACING_X) * CARD_SPACING_X,
    Math.round(y / CARD_SPACING_Y) * CARD_SPACING_Y,
  ];
}

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
  const rawScenesRef = useRef<SceneCanvasRow[]>([]);
  const [overlayOn, setOverlayOn] = useState(false);
  // Canvas view settings persist across navigation. The writer
  // picks lane mode / presence mode / shares-on once and we don't
  // forget when they nav away and back — the cards keep their
  // organization, the ribbon smoothly morphs into whatever the
  // remembered settings imply.
  const [laneMode, setLaneMode] = useState<LaneMode>(() => {
    try {
      const v = localStorage.getItem("water:canvas-lane-mode");
      return v === "free" || v === "location" || v === "character" ? v : "free";
    } catch {
      return "free";
    }
  });
  const [lanes, setLanes] = useState<Lane[]>([]);
  const [presenceMode, setPresenceMode] = useState<PresenceMode>(() => {
    try {
      const v = localStorage.getItem("water:canvas-presence-mode");
      return v === "ghost" || v === "spanning" ? v : "ghost";
    } catch {
      return "ghost";
    }
  });
  const [sharesOn, setSharesOn] = useState<boolean>(() => {
    try {
      return localStorage.getItem("water:canvas-shares-on") === "true";
    } catch {
      return false;
    }
  });
  useEffect(() => {
    try {
      localStorage.setItem("water:canvas-lane-mode", laneMode);
    } catch {
      /* swallow */
    }
  }, [laneMode]);
  useEffect(() => {
    try {
      localStorage.setItem("water:canvas-presence-mode", presenceMode);
    } catch {
      /* swallow */
    }
  }, [presenceMode]);
  useEffect(() => {
    try {
      localStorage.setItem(
        "water:canvas-shares-on",
        sharesOn ? "true" : "false",
      );
    } catch {
      /* swallow */
    }
  }, [sharesOn]);
  const [pan, setPan] = useState({ x: 24, y: 24 });
  const [zoom, setZoom] = useState(1);
  const [containerSize, setContainerSize] = useState({ width: 0, height: 0 });
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
        // Snapshot raw scene rows; recompute placement when layout
        // mode flips between free + lane modes.
        rawScenesRef.current = scenes;
        const { cards: laid, lanes: laid_lanes } = layoutCards(
          scenes,
          laneMode,
          presenceMode,
        );
        setCards(laid);
        setLanes(laid_lanes);
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

  // Recompute layout when lane mode or presence mode flips.
  useEffect(() => {
    if (rawScenesRef.current.length === 0) return;
    const { cards: laid, lanes: laid_lanes } = layoutCards(
      rawScenesRef.current,
      laneMode,
      presenceMode,
    );
    setCards(laid);
    setLanes(laid_lanes);
    centeredOnce.current = false; // re-center when layout changes shape
  }, [laneMode, presenceMode]);

  // Track container size for the ambient WaterRibbon. The effect
  // depends on `cards` because the containerRef is attached only
  // after the loading-state early-return clears (cards !== null).
  // Mounting-once with [] deps left containerRef.current null and
  // the observer never fired — which manifested as the ribbon being
  // invisible on the canvas surface.
  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;
    // Seed immediately so the first frame already has a size.
    const rect = el.getBoundingClientRect();
    setContainerSize({ width: rect.width, height: rect.height });
    const ro = new ResizeObserver((entries) => {
      const entry = entries[0];
      if (!entry) return;
      setContainerSize({
        width: entry.contentRect.width,
        height: entry.contentRect.height,
      });
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, [cards]);

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

  // On first paint: center the cards' bounding box in the viewport,
  // zooming out only as far as needed to fit. > 3 scenes use fit-all;
  // otherwise zoom stays at 1.0 and pan centers regardless.
  const centeredOnce = useRef(false);
  useEffect(() => {
    if (!cards || cards.length === 0) return;
    if (centeredOnce.current) return;
    const el = containerRef.current;
    if (!el) return;
    const rect = el.getBoundingClientRect();
    if (rect.width === 0 || rect.height === 0) return; // not laid out yet
    // In lane modes, lane labels live to the left of the cards at
    // negative x; include them in the fit-bbox so they don't get
    // clipped on first paint.
    const laneOffset = laneMode === "free" ? 0 : LANE_LABEL_WIDTH + 16;
    const minX = Math.min(...cards.map((c) => c.x)) - laneOffset;
    const minY = Math.min(...cards.map((c) => c.y));
    const maxX = Math.max(...cards.map((c) => c.x + CARD_W));
    const maxY = Math.max(...cards.map((c) => c.y + c.height));
    const bbW = maxX - minX;
    const bbH = maxY - minY;
    let z = 1;
    if (cards.length > 3) {
      z = Math.min(
        1,
        Math.min((rect.width - 80) / bbW, (rect.height - 120) / bbH),
      );
    }
    // Center the bounding box midpoint at the viewport midpoint.
    const cx = minX + bbW / 2;
    const cy = minY + bbH / 2;
    setZoom(z);
    setPan({
      x: rect.width / 2 - cx * z,
      y: rect.height / 2 - cy * z,
    });
    centeredOnce.current = true;
  }, [cards, laneMode]);

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
      if (card) {
        // Snap on release so cards visibly land on a grid cell.
        const [sx, sy] = snapToGrid(card.x, card.y);
        setCards((prev) => {
          if (!prev) return prev;
          const updated = prev.map((c) =>
            c.id === card.id
              ? { ...c, x: sx, y: sy, stackIndex: 0, stackSize: 1 }
              : c,
          );
          // Recompute collision stacks since the moved card may now
          // share a cell with another (or have vacated an old one).
          return applyStackOffsets(updated);
        });
        debouncedPersist(card.id, sx, sy);
      }
      dragStartRef.current = null;
    }
    panStartRef.current = null;
    try {
      e.currentTarget.releasePointerCapture(e.pointerId);
    } catch {
      /* swallow */
    }
  };
  /**
   * Plain scroll = zoom (Figma/Miro convention). Cmd/Ctrl modifier
   * accelerates the zoom step. The view zooms around the cursor so
   * the writer can dive into a corner without losing the spatial
   * anchor.
   */
  const onContainerWheel = (e: React.WheelEvent<HTMLDivElement>) => {
    e.preventDefault();
    const el = containerRef.current;
    if (!el) return;
    const rect = el.getBoundingClientRect();
    const localX = e.clientX - rect.left - pan.x;
    const localY = e.clientY - rect.top - pan.y;
    const step = e.ctrlKey || e.metaKey ? 0.15 : 0.06;
    const direction = -Math.sign(e.deltaY);
    const next = Math.max(0.15, Math.min(3, zoom + direction * step));
    const ratio = next / zoom;
    setPan({
      x: pan.x - localX * (ratio - 1),
      y: pan.y - localY * (ratio - 1),
    });
    setZoom(next);
  };

  const onCardPointerDown = (id: string, e: React.PointerEvent) => {
    // Drag-to-move is meaningful only in free mode. In lane modes the
    // x/y are derived from manuscript order × lane index, so a manual
    // drag would just snap back on the next layout pass.
    if (laneMode !== "free") return;
    // In free mode each scene has exactly one card (isPrimary=true),
    // so the first match is fine. find() picks the canonical one.
    const card = cards?.find((c) => c.id === id && c.isPrimary);
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
      {/* Ambient water-ribbon. Same component as the editor surface;
          all instances share a performance.now()-driven clock so the
          flow appears continuous when navigating between surfaces.
          Rendered OUTSIDE the pan/zoom-transformed wrapper so it
          isn't scaled or panned with the canvas content. Scene
          positions are passed as 'influences' so the ribbon warps
          to touch each scene as it flows past — and shifts
          gradually when scenes move. */}
      <WaterRibbon
        parentWidth={containerSize.width}
        anchors={cards
          .filter((c) => c.isPrimary)
          .map((c) => ({
            id: c.id,
            x: (c.x + CARD_W / 2) * zoom + pan.x,
            y: (c.y + CARD_H / 2) * zoom + pan.y,
            weight: 1,
          }))}
      />
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
        {sharesOn && cards.length > 1 && (
          <SharedAttrConnectors
            cards={cards}
            offsetX={Math.min(...cards.map((c) => c.x)) - 80}
            offsetY={Math.min(...cards.map((c) => c.y)) - 80}
            width={
              Math.max(...cards.map((c) => c.x + CARD_W)) -
              Math.min(...cards.map((c) => c.x)) +
              160
            }
            height={
              Math.max(...cards.map((c) => c.y + c.height)) -
              Math.min(...cards.map((c) => c.y)) +
              160
            }
          />
        )}
        {overlayOn && (
          <ReadingOrderOverlay
            cards={sortedForOverlay}
            cardW={CARD_W}
            cardH={CARD_H}
          />
        )}
        {/* Lane labels — when grouped by location or character,
            render the lane name to the left of each row. */}
        {laneMode !== "free" &&
          lanes.map((lane) => (
            <div
              key={lane.id}
              data-testid={`canvas-lane-${lane.id}`}
              style={{
                position: "absolute",
                left: -LANE_LABEL_WIDTH - 16,
                top: lane.row * CARD_SPACING_Y,
                width: LANE_LABEL_WIDTH,
                height: CARD_H,
                display: "flex",
                alignItems: "center",
                justifyContent: "flex-end",
                paddingRight: 12,
                fontFamily: "var(--water-font-sans)",
                fontSize: 11,
                fontWeight: 600,
                textTransform: "uppercase",
                letterSpacing: 0.6,
                color:
                  lane.id === "__unassigned"
                    ? "var(--water-fg-faint)"
                    : "var(--water-fg-muted)",
                pointerEvents: "none",
              }}
            >
              {lane.label}
            </div>
          ))}
        {cards.map((card) => (
          <SceneCard
            key={card.placementKey}
            card={card}
            metrics={heatPerScene[card.id] ?? null}
            activeMetric={activeMetric}
            onPointerDown={(e) => onCardPointerDown(card.id, e)}
            onOpen={() => {
              // Ghost click → focus the canonical card. Pan the view
              // so the primary y is centered; leave x alone since
              // ghost and primary share x. For primary or free-mode
              // cards, opening goes straight to the scene.
              if (!card.isPrimary) {
                const el = containerRef.current;
                if (el) {
                  const rect = el.getBoundingClientRect();
                  setPan((p) => ({
                    x: p.x,
                    y: rect.height / 2 - (card.primaryY + CARD_H / 2) * zoom,
                  }));
                }
                return;
              }
              onOpenScene(card.id);
            }}
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
        {/* Lane-mode chip: cycle free → location → character → free.
            Lets the writer reshape the canvas into plot-line rows for
            quick coordination across POVs or settings, then back to
            the freely-arranged map. */}
        <button
          type="button"
          className="water-floating-chip"
          aria-label="Layout mode"
          data-testid="canvas-lane-mode"
          onClick={() => {
            setLaneMode((m) =>
              m === "free"
                ? "location"
                : m === "location"
                  ? "character"
                  : "free",
            );
          }}
          title="Group rows by location or POV (cycle)"
          style={{
            padding: "6px 12px",
            border: "none",
            background:
              laneMode === "free"
                ? "var(--water-bg-raised)"
                : "color-mix(in srgb, var(--water-hue-flow) 22%, transparent)",
            color: "var(--water-fg-default)",
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
          rows: {laneMode}
        </button>
        {/* Presence-mode chip: ghost vs spanning. Only meaningful in
            lane modes, but kept visible so the writer can preview
            the toggle before switching rows on. */}
        <button
          type="button"
          className="water-floating-chip"
          aria-label="Multi-lane presentation"
          data-testid="canvas-presence-mode"
          onClick={() =>
            setPresenceMode((m) => (m === "ghost" ? "spanning" : "ghost"))
          }
          disabled={laneMode === "free"}
          title={
            laneMode === "free"
              ? "Switch rows: to location or character to use presence modes"
              : "Toggle ghost vs spanning multi-lane cards"
          }
          style={{
            padding: "6px 12px",
            border: "none",
            background:
              laneMode === "free"
                ? "color-mix(in srgb, var(--water-bg-raised) 60%, transparent)"
                : "var(--water-bg-raised)",
            color:
              laneMode === "free"
                ? "var(--water-fg-faint)"
                : "var(--water-fg-default)",
            fontFamily: "var(--water-font-sans)",
            fontSize: "var(--water-fs-meta)",
            fontWeight: 500,
            borderRadius: "var(--water-r-8)",
            boxShadow: laneMode === "free" ? "none" : "var(--water-elev-1)",
            cursor: laneMode === "free" ? "not-allowed" : "pointer",
            textTransform: "lowercase",
            letterSpacing: 0.3,
          }}
        >
          presence: {presenceMode}
        </button>
        {/* Shared-attribute connectors. Faint arcs between scenes
            that share a location or a character — surfaces "these
            two scenes are connected" without forcing a lane mode. */}
        <button
          type="button"
          className="water-floating-chip"
          aria-label="Toggle shared-attribute connectors"
          aria-pressed={sharesOn}
          data-testid="canvas-shares-toggle"
          onClick={() => setSharesOn((v) => !v)}
          title="Show arcs between scenes sharing a location or character"
          style={{
            padding: "6px 12px",
            border: "none",
            background: sharesOn
              ? "color-mix(in srgb, var(--water-hue-flow) 22%, transparent)"
              : "var(--water-bg-raised)",
            color: "var(--water-fg-default)",
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
          shares: {sharesOn ? "on" : "off"}
        </button>
        <button
          type="button"
          className="water-floating-chip"
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
          className="water-floating-chip"
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
  /** Height in canvas-space px. Always CARD_H except in spanning
   *  presentation when a scene's card vertically covers several
   *  rows it participates in. */
  height: number;
  /** Lane this placement belongs to. Empty string in free mode. */
  laneId: string;
  /** True for the canonical placement of a scene (the POV / primary-
   *  location lane). False for ghost placements in secondary lanes. */
  isPrimary: boolean;
  /** Stable React key across re-layouts (sceneId::laneId). */
  placementKey: string;
  /** y of this scene's primary placement — used so clicking a ghost
   *  can pan/focus the canonical card. Equal to y when isPrimary. */
  primaryY: number;
  /** Index within a collision stack at this (x, y). 0 for the
   *  bottom card; only set when stackSize > 1. */
  stackIndex: number;
  /** Total cards sharing this (x, y). 1 means no collision. */
  stackSize: number;
}

export interface Lane {
  /** Unique id (location_id / character_id / "__unassigned"). */
  id: string;
  /** Display label. */
  label: string;
  /** Row index in the lane stack — used to compute y position. */
  row: number;
}

/**
 * How scenes that touch multiple lanes are rendered.
 *
 * - ghost: one canonical card in the primary lane; faded ghost
 *   copies in each secondary lane (same x). Click ghost → focus
 *   primary. The "one scene = one source of truth" model.
 * - spanning: one card per scene, anchored at its top-most lane,
 *   tall enough to vertically span every lane it participates in.
 *   Visually shows ensemble scenes as a single object that "covers"
 *   its plotlines.
 */
export type PresenceMode = "ghost" | "spanning";

/**
 * Compute card placements + lanes based on the current mode.
 *
 * - free: respect canvas_x/canvas_y when set; auto-flow the rest
 *   into 8-per-row chunks. No lanes returned.
 * - location: y = lane row × CARD_SPACING_Y; x = manuscript-order
 *   index × CARD_SPACING_X (compacted, gap-free). One lane per
 *   distinct location; "(no location)" pinned at the bottom.
 * - character: same shape, grouped by character presence.
 *
 * In lane modes a single scene may appear in *multiple* lanes via
 * `character_presences` / `location_presences`. The `presentation`
 * arg decides how those are drawn.
 */
function layoutCards(
  scenes: SceneCanvasRow[],
  mode: LaneMode,
  presentation: PresenceMode,
): { cards: CanvasCard[]; lanes: Lane[] } {
  const sorted = [...scenes].sort(
    (a, b) => a.manuscript_ordering - b.manuscript_ordering,
  );

  if (mode === "free") {
    const cards: CanvasCard[] = sorted.map((s, ix) => {
      const col = ix % CARDS_PER_ROW;
      const row = Math.floor(ix / CARDS_PER_ROW);
      const x = s.canvas_x ?? col * CARD_SPACING_X;
      const y = s.canvas_y ?? row * CARD_SPACING_Y;
      return {
        ...s,
        x,
        y,
        height: CARD_H,
        laneId: "",
        isPrimary: true,
        placementKey: s.id,
        primaryY: y,
        stackIndex: 0,
        stackSize: 1,
      };
    });
    return { cards: applyStackOffsets(cards), lanes: [] };
  }

  // Which lanes does each scene touch? In lane mode the first
  // entry is the primary; rest are secondary (ghost/spanned).
  const presencesOf = (s: SceneCanvasRow): { id: string; label: string }[] => {
    const out: { id: string; label: string }[] = [];
    if (mode === "location") {
      for (const p of s.location_presences) {
        out.push({ id: p.id, label: p.name });
      }
      if (out.length === 0) {
        out.push({ id: "__unassigned", label: "(no location)" });
      }
      return out;
    }
    // character
    for (const p of s.character_presences) {
      out.push({ id: p.id, label: p.name });
    }
    if (out.length === 0) {
      out.push({ id: "__unassigned", label: "(no POV)" });
    }
    return out;
  };

  // Build lane order: first-appearance in manuscript order; the
  // "__unassigned" bucket is pinned to the bottom.
  const laneIndex = new Map<string, Lane>();
  for (const s of sorted) {
    for (const p of presencesOf(s)) {
      if (!laneIndex.has(p.id)) {
        laneIndex.set(p.id, { id: p.id, label: p.label, row: -1 });
      }
    }
  }
  const named = [...laneIndex.values()].filter((l) => l.id !== "__unassigned");
  const unassigned = laneIndex.get("__unassigned");
  const ordered: Lane[] = named.map((l, ix) => ({ ...l, row: ix }));
  if (unassigned) {
    ordered.push({ ...unassigned, row: ordered.length });
  }
  const rowFor = (id: string) =>
    ordered.find((l) => l.id === id)?.row ?? 0;

  // Column = compacted manuscript-order index. Using the array index
  // (not ordering) keeps columns gap-free even when manuscript
  // ordering has holes.
  const colByScene = new Map<string, number>();
  sorted.forEach((s, ix) => colByScene.set(s.id, ix));

  const cards: CanvasCard[] = [];
  for (const s of sorted) {
    const presences = presencesOf(s);
    const x = (colByScene.get(s.id) ?? 0) * CARD_SPACING_X;
    const rows = presences.map((p) => rowFor(p.id));
    const minRow = Math.min(...rows);
    const maxRow = Math.max(...rows);
    const primaryY = minRow * CARD_SPACING_Y;

    if (presentation === "spanning") {
      // ONE card per scene, anchored at its top-most lane, vertically
      // covering everywhere it touches.
      const height =
        (maxRow - minRow) * CARD_SPACING_Y + CARD_H;
      cards.push({
        ...s,
        x,
        y: primaryY,
        height,
        laneId: presences[0]?.id ?? "",
        isPrimary: true,
        placementKey: `${s.id}::span`,
        primaryY,
        stackIndex: 0,
        stackSize: 1,
      });
    } else {
      // Ghost mode: one canonical card + one ghost per secondary
      // presence. They sit at the same x; only y differs.
      presences.forEach((p, ix) => {
        cards.push({
          ...s,
          x,
          y: rowFor(p.id) * CARD_SPACING_Y,
          height: CARD_H,
          laneId: p.id,
          isPrimary: ix === 0,
          placementKey: `${s.id}::${p.id}`,
          primaryY,
          stackIndex: 0,
          stackSize: 1,
        });
      });
    }
  }

  return { cards: applyStackOffsets(cards), lanes: ordered };
}

/**
 * Find cards sharing a snapped grid cell and assign stackIndex /
 * stackSize. Only canonical (isPrimary) cards participate — ghosts
 * stay flush so the writer can still see "this scene is also in
 * lane Y" without the ghost contributing to the fan.
 *
 * Order within a stack is the card's original index, which keeps the
 * fan stable across re-renders.
 */
function applyStackOffsets(cards: CanvasCard[]): CanvasCard[] {
  // Group canonical cards by snapped (x, y) cell.
  const cellKey = (x: number, y: number) =>
    `${Math.round(x / CARD_SPACING_X)},${Math.round(y / CARD_SPACING_Y)}`;
  const cellMembers: Map<string, number[]> = new Map();
  cards.forEach((c, ix) => {
    if (!c.isPrimary) return;
    const k = cellKey(c.x, c.y);
    if (!cellMembers.has(k)) cellMembers.set(k, []);
    cellMembers.get(k)!.push(ix);
  });
  const out = cards.slice();
  for (const members of cellMembers.values()) {
    if (members.length <= 1) continue;
    members.forEach((cardIx, stackIx) => {
      out[cardIx] = {
        ...out[cardIx]!,
        stackIndex: stackIx,
        stackSize: members.length,
      };
    });
  }
  return out;
}

export type { CanvasCard, HeatRow };
