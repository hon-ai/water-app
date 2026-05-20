import { useCallback, useEffect, useRef, useState } from "react";
import {
  ipc,
  type HeatMetricKind,
  type HeatReadResponse,
} from "../ipc/commands";
import { onWaterEvent } from "../ipc/events";
import { HeatmapMetricPicker } from "./HeatmapMetricPicker";
import { phraseFor } from "./phrasebank";

const INTRO_SEEN_KEY = "water:heatmap-intro-seen";

interface HoverState {
  columnIx: number;
  clientX: number;
}

interface Props {
  sceneId: string;
}

/**
 * The M5 Heatmap strip. A 24 px-tall ambient barometer above the
 * editor title. Per-paragraph cells stretch to fill the canvas width;
 * cell opacity is proportional to the metric value.
 *
 * v1 renders Pacing only; the metric picker (Task 14) opts other
 * metrics in. The strip subscribes to `heat:updated` and refetches
 * for the matching scene id so the writer sees the strip evolve as
 * they write.
 *
 * Hue selection: each metric has its own --water-hue-* token (see
 * `metricHue` below). When only one metric is on, the strip is a
 * single hue band; when multiple are toggled on, each draws as a
 * semi-transparent layer above the canvas.
 */
export function HeatmapStrip({ sceneId }: Props) {
  const [metrics, setMetrics] = useState<HeatReadResponse["metrics"] | null>(
    null,
  );
  const [enabled, setEnabled] = useState<
    Partial<Record<HeatMetricKind, boolean>>
  >({});
  const [pickerOpen, setPickerOpen] = useState(false);
  const [pickerAnchor, setPickerAnchor] = useState<DOMRect | null>(null);
  const chipRef = useRef<HTMLButtonElement | null>(null);
  const [hover, setHover] = useState<HoverState | null>(null);
  const [introSeen, setIntroSeen] = useState<boolean>(() => {
    try {
      return localStorage.getItem(INTRO_SEEN_KEY) === "true";
    } catch {
      return true;
    }
  });
  const stripRef = useRef<HTMLDivElement | null>(null);
  const scrubbingRef = useRef(false);

  function dismissIntro() {
    try {
      localStorage.setItem(INTRO_SEEN_KEY, "true");
    } catch {
      /* swallow — intro will re-appear next session, no harm */
    }
    setIntroSeen(true);
  }

  /**
   * Map a clientX to a paragraph index using the strip's grid-track
   * geometry. Returns null when the strip hasn't rendered or the
   * cursor is outside the column area (i.e., over the right-edge
   * chip).
   */
  const columnAtX = useCallback(
    (clientX: number, columnCount: number): number | null => {
      const el = stripRef.current;
      if (!el) return null;
      const rect = el.getBoundingClientRect();
      // The chip is on the right; columns occupy the left portion.
      // Use the inner grid container's bounding rect for precision.
      const grid = el.querySelector<HTMLDivElement>("[data-testid='heatmap-grid']");
      if (!grid) return null;
      const gr = grid.getBoundingClientRect();
      if (clientX < gr.left || clientX >= gr.right) return null;
      const colWidth = gr.width / columnCount;
      const ix = Math.floor((clientX - gr.left) / colWidth);
      if (ix < 0) return 0;
      if (ix >= columnCount) return columnCount - 1;
      // Suppress unused-rect warning in case the linter ever complains.
      void rect;
      return ix;
    },
    [],
  );

  /**
   * Scroll the editor body so the targeted paragraph is in view.
   * Strategy: find the editor canvas via the data-pill-id-less
   * container and scrollToParagraph by offset proportion. The
   * editor renders blocks with `[data-bid]` attributes (one per
   * block); if the body has at least `columnCount` blocks we scroll
   * the `ix`-th one into view. Falls back to proportional scroll.
   */
  function scrollToParagraph(ix: number, columnCount: number) {
    // First try: nth block with [data-bid]
    const blocks = document.querySelectorAll<HTMLElement>(
      ".water-editor-canvas [data-bid], [data-bid]",
    );
    if (blocks.length > 0 && ix < blocks.length) {
      blocks[ix]?.scrollIntoView({
        behavior: "smooth",
        block: "center",
      });
      return;
    }
    // Fallback: proportional scroll of the editor's main container.
    const main = document.querySelector<HTMLElement>("main");
    if (!main) return;
    const frac = ix / Math.max(columnCount - 1, 1);
    main.scrollTo({
      top: frac * (main.scrollHeight - main.clientHeight),
      behavior: "smooth",
    });
  }

  const refetch = useCallback(async () => {
    try {
      const resp = await ipc.heatRead(sceneId);
      setMetrics(resp.metrics);
    } catch {
      /* swallow — strip stays at last-known state */
    }
  }, [sceneId]);

  // Initial paint + restore picker state.
  useEffect(() => {
    void refetch();
    void (async () => {
      try {
        const settings = await ipc.heatReadSettings();
        // Default: Pacing on, everything else off. Persisted toggles override.
        setEnabled({ pacing: true, ...settings.enabled });
      } catch {
        setEnabled({ pacing: true });
      }
    })();
  }, [refetch]);

  // Subscribe to heat:updated for THIS scene.
  useEffect(() => {
    let unsub: (() => void) | undefined;
    let cancelled = false;
    void (async () => {
      const u = await onWaterEvent("heat:updated", (payload) => {
        if (payload.scene_id === sceneId) void refetch();
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
  }, [sceneId, refetch]);

  if (!metrics) {
    return (
      <div
        aria-label="Heatmap"
        style={{
          height: 24,
          margin: "0 0 12px 0",
          opacity: 0,
          transition:
            "opacity var(--water-dur-medium) var(--water-ease-out-soft)",
        }}
      />
    );
  }

  // Which metrics actually have rows to render? Pacing always shows;
  // other metrics only render their layer when both enabled AND have
  // data.
  const activeKinds = (Object.keys(enabled) as HeatMetricKind[]).filter(
    (k) => enabled[k] === true && (metrics[k]?.length ?? 0) > 0,
  );

  // For the single-metric case (just Pacing), pick its row count for
  // the strip's column count. For multiple metrics, take the max so
  // every layer can render its full track.
  const columnCount = Math.max(
    ...activeKinds.map((k) => metrics[k]?.length ?? 0),
    1,
  );

  const primaryKind: HeatMetricKind = activeKinds[0] ?? "pacing";
  const primaryLabel = METRIC_LABEL[primaryKind];

  const onPointerMove = (e: React.PointerEvent<HTMLDivElement>) => {
    const ix = columnAtX(e.clientX, columnCount);
    if (ix === null) {
      setHover(null);
      return;
    }
    setHover({ columnIx: ix, clientX: e.clientX });
    if (scrubbingRef.current) {
      scrollToParagraph(ix, columnCount);
    }
  };

  const onPointerDown = (e: React.PointerEvent<HTMLDivElement>) => {
    const ix = columnAtX(e.clientX, columnCount);
    if (ix === null) return;
    scrubbingRef.current = true;
    e.currentTarget.setPointerCapture(e.pointerId);
    scrollToParagraph(ix, columnCount);
  };

  const onPointerUp = (e: React.PointerEvent<HTMLDivElement>) => {
    scrubbingRef.current = false;
    try {
      e.currentTarget.releasePointerCapture(e.pointerId);
    } catch {
      /* swallow — already released */
    }
  };

  const onPointerLeave = () => {
    setHover(null);
  };

  return (
    <div
      ref={stripRef}
      aria-label="Heatmap"
      role="img"
      data-testid="heatmap-strip"
      onPointerMove={onPointerMove}
      onPointerDown={onPointerDown}
      onPointerUp={onPointerUp}
      onPointerLeave={onPointerLeave}
      style={{
        position: "relative",
        height: 24,
        margin: "0 0 12px 0",
        display: "flex",
        alignItems: "stretch",
        gap: 0,
        background: "var(--water-bg-canvas)",
        borderRadius: "var(--water-r-8)",
        overflow: "visible",
        cursor: scrubbingRef.current ? "grabbing" : hover ? "grab" : "default",
        animation:
          "water-pill-fade-in var(--water-dur-medium) var(--water-ease-out-soft) both",
        touchAction: "none",
      }}
    >
      {/* Per-column wrapper — each column stacks one cell per active metric. */}
      <div
        data-testid="heatmap-grid"
        style={{
          flex: 1,
          display: "grid",
          gridTemplateColumns: `repeat(${columnCount}, 1fr)`,
          overflow: "hidden",
          borderRadius: "var(--water-r-8) 0 0 var(--water-r-8)",
        }}
      >
        {Array.from({ length: columnCount }).map((_, ix) => (
          <div
            key={ix}
            data-testid={`heatmap-col-${ix}`}
            data-paragraph-ix={ix}
            style={{ position: "relative", overflow: "hidden" }}
          >
            {activeKinds.map((kind) => {
              const row = metrics[kind]?.[ix];
              if (!row) return null;
              return (
                <div
                  key={kind}
                  data-testid={`heatmap-cell-${kind}-${ix}`}
                  style={{
                    position: "absolute",
                    inset: 0,
                    background: `var(${metricHue(kind)})`,
                    opacity: cellOpacity(kind, row.value),
                  }}
                />
              );
            })}
          </div>
        ))}
      </div>
      {/* Right-edge chip + picker. Picker is a SIBLING of the button
          (not a child) so it doesn't inherit text-align:center from
          the button + so its z-stacking is independent. */}
      <div
        style={{
          position: "relative",
          flexShrink: 0,
        }}
      >
        <button
          ref={chipRef}
          type="button"
          data-testid="heatmap-chip"
          onClick={() => {
            setPickerOpen((v) => {
              const next = !v;
              if (next && chipRef.current) {
                setPickerAnchor(chipRef.current.getBoundingClientRect());
              }
              return next;
            });
          }}
          aria-label="Heatmap metrics"
          aria-haspopup="menu"
          aria-expanded={pickerOpen ? "true" : "false"}
          style={{
            height: "100%",
            padding: "0 10px",
            display: "flex",
            alignItems: "center",
            gap: 4,
            border: "none",
            background: "var(--water-bg-raised)",
            color: "var(--water-fg-muted)",
            fontFamily: "var(--water-font-sans)",
            fontSize: 11,
            fontWeight: 500,
            textTransform: "lowercase",
            letterSpacing: 0.3,
            boxShadow: "var(--water-elev-1)",
            cursor: "pointer",
            transition:
              "color var(--water-dur-tiny) var(--water-ease-out-soft)",
          }}
          onMouseEnter={(e) => {
            e.currentTarget.style.color = "var(--water-fg-default)";
          }}
          onMouseLeave={(e) => {
            e.currentTarget.style.color = "var(--water-fg-muted)";
          }}
        >
          {primaryLabel}
          {activeKinds.length > 1 ? ` +${activeKinds.length - 1}` : ""}
          <span aria-hidden style={{ fontSize: 9, marginLeft: 2 }}>
            ▾
          </span>
        </button>
        <HeatmapMetricPicker
          open={pickerOpen}
          enabled={enabled}
          anchor={pickerAnchor}
          onToggle={(kind, next) =>
            setEnabled((prev) => ({ ...prev, [kind]: next }))
          }
          onClose={() => setPickerOpen(false)}
        />
      </div>
      {hover && activeKinds.length > 0 && (
        <HoverTooltip
          metrics={metrics}
          activeKinds={activeKinds}
          columnIx={hover.columnIx}
          clientX={hover.clientX}
          stripEl={stripRef.current}
        />
      )}
      {!introSeen && <IntroOverlay onDismiss={dismissIntro} />}
    </div>
  );
}

/**
 * Floating tooltip beneath the strip showing the metric name + phrase
 * for the hovered paragraph. Phrase comes from the deterministic
 * phrasebank.
 */
function HoverTooltip({
  metrics,
  activeKinds,
  columnIx,
  clientX,
  stripEl,
}: {
  metrics: HeatReadResponse["metrics"];
  activeKinds: HeatMetricKind[];
  columnIx: number;
  clientX: number;
  stripEl: HTMLDivElement | null;
}) {
  if (!stripEl) return null;
  const rect = stripEl.getBoundingClientRect();
  const left = Math.max(rect.left, Math.min(rect.right - 200, clientX - 80));
  const top = rect.bottom + 6;
  const lines = activeKinds
    .map((kind) => {
      const row = metrics[kind]?.[columnIx];
      if (!row) return null;
      const phrase = phraseFor(kind, row.value, columnIx);
      const label = METRIC_LABEL[kind];
      return { kind, phrase, label };
    })
    .filter(
      (x): x is { kind: HeatMetricKind; phrase: string; label: string } =>
        x !== null,
    );
  if (lines.length === 0) return null;
  return (
    <div
      data-testid="heatmap-tooltip"
      style={{
        position: "fixed",
        left,
        top,
        zIndex: 41,
        padding: "6px 10px",
        background: "var(--water-bg-raised)",
        color: "var(--water-fg-default)",
        fontFamily: "var(--water-font-sans)",
        fontSize: 11,
        lineHeight: 1.45,
        borderRadius: "var(--water-r-8)",
        boxShadow: "var(--water-elev-2)",
        pointerEvents: "none",
        animation:
          "water-pill-fade-in var(--water-dur-tiny) var(--water-ease-out-soft) both",
      }}
    >
      {lines.map(({ kind, phrase, label }) => (
        <div
          key={kind}
          style={{
            display: "flex",
            alignItems: "center",
            gap: 6,
          }}
        >
          <span
            aria-hidden
            style={{
              width: 6,
              height: 6,
              borderRadius: "50%",
              background: `var(${metricHue(kind)})`,
            }}
          />
          <span
            style={{
              color: "var(--water-fg-muted)",
              textTransform: "lowercase",
              letterSpacing: 0.3,
            }}
          >
            {label}
          </span>
          <span style={{ color: "var(--water-fg-default)" }}>{phrase}</span>
        </div>
      ))}
    </div>
  );
}

/**
 * First-launch tooltip that points at the strip and tells the writer
 * what it does. Persists dismissal via localStorage.
 */
function IntroOverlay({ onDismiss }: { onDismiss: () => void }) {
  // Stop pointer/click propagation so the parent strip's drag-scrub
  // handlers don't swallow the dismiss click. Without this,
  // pointerdown on the × bubbles to the strip, the strip calls
  // setPointerCapture on itself, and the click never lands.
  const stopProp = (e: React.SyntheticEvent) => {
    e.stopPropagation();
  };
  return (
    <div
      data-testid="heatmap-intro"
      role="dialog"
      aria-label="Heatmap introduction"
      onPointerDown={stopProp}
      onPointerMove={stopProp}
      onPointerUp={stopProp}
      onClick={stopProp}
      style={{
        position: "absolute",
        // Sits ABOVE the strip so it doesn't obscure the scene title
        // below. The editor canvas has ~72 px of breathing room above
        // the strip; the overlay tucks into that space.
        bottom: "calc(100% + 8px)",
        left: 0,
        minWidth: 260,
        maxWidth: 320,
        padding: "10px 12px",
        background: "var(--water-bg-paper)",
        color: "var(--water-fg-default)",
        fontFamily: "var(--water-font-sans)",
        fontSize: 12,
        lineHeight: 1.5,
        borderRadius: "var(--water-r-16)",
        boxShadow: "var(--water-elev-2)",
        zIndex: 1001,
        display: "flex",
        alignItems: "flex-start",
        gap: 8,
        cursor: "default",
        animation:
          "water-pill-fade-in var(--water-dur-medium) var(--water-ease-out-soft) both",
      }}
    >
      <div style={{ flex: 1 }}>
        this is the heat. drag to find a spot.
      </div>
      <button
        type="button"
        aria-label="Dismiss"
        onPointerDown={stopProp}
        onClick={(e) => {
          e.stopPropagation();
          onDismiss();
        }}
        style={{
          border: "none",
          background: "transparent",
          color: "var(--water-fg-muted)",
          cursor: "pointer",
          padding: "2px 8px",
          fontSize: 16,
          lineHeight: 1,
          borderRadius: "var(--water-r-8)",
          transition:
            "background-color var(--water-dur-tiny) var(--water-ease-out-soft)",
        }}
        onMouseEnter={(e) => {
          e.currentTarget.style.background =
            "color-mix(in srgb, var(--water-fg-faint) 14%, transparent)";
        }}
        onMouseLeave={(e) => {
          e.currentTarget.style.background = "transparent";
        }}
      >
        ×
      </button>
    </div>
  );
}

const METRIC_LABEL: Record<HeatMetricKind, string> = {
  pacing: "pacing",
  valence: "valence",
  coherence: "coherence",
  presence: "presence",
  world_refs: "world refs",
};

function metricHue(kind: HeatMetricKind): string {
  switch (kind) {
    case "pacing":
      return "--water-hue-pace";
    case "valence":
      return "--water-hue-valence-pos";
    case "coherence":
      return "--water-hue-coherence";
    case "presence":
      return "--water-hue-character-default";
    case "world_refs":
      return "--water-hue-cartographer";
  }
}

/**
 * Map a metric's raw value into a 0..1 opacity for the cell. Most
 * metrics are already 0..1; Valence is -1..1 and uses |v| so cold
 * AND warm paragraphs both render visibly (the hue token shifts
 * when we ship the cold variant in a future iteration).
 */
function cellOpacity(kind: HeatMetricKind, value: number): number {
  if (kind === "valence") {
    return Math.min(1, Math.abs(value));
  }
  return Math.max(0, Math.min(1, value));
}
