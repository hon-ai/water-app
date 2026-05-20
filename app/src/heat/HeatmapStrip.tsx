import { useCallback, useEffect, useState } from "react";
import {
  ipc,
  type HeatMetricKind,
  type HeatReadResponse,
  type HeatRow,
} from "../ipc/commands";
import { onWaterEvent } from "../ipc/events";

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

  return (
    <div
      aria-label="Heatmap"
      role="img"
      data-testid="heatmap-strip"
      style={{
        position: "relative",
        height: 24,
        margin: "0 0 12px 0",
        display: "flex",
        alignItems: "stretch",
        gap: 0,
        background: "var(--water-bg-canvas)",
        borderRadius: "var(--water-r-8)",
        overflow: "hidden",
        animation:
          "water-pill-fade-in var(--water-dur-medium) var(--water-ease-out-soft) both",
      }}
    >
      {/* Per-column wrapper — each column stacks one cell per active metric. */}
      <div
        style={{
          flex: 1,
          display: "grid",
          gridTemplateColumns: `repeat(${columnCount}, 1fr)`,
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
      {/* Right-edge chip — metric name + caret (picker opens in Task 14). */}
      <div
        data-testid="heatmap-chip"
        style={{
          flexShrink: 0,
          padding: "0 10px",
          display: "flex",
          alignItems: "center",
          gap: 4,
          background: "var(--water-bg-raised)",
          color: "var(--water-fg-muted)",
          fontFamily: "var(--water-font-sans)",
          fontSize: 11,
          fontWeight: 500,
          textTransform: "lowercase",
          letterSpacing: 0.3,
          boxShadow: "var(--water-elev-1)",
        }}
      >
        {primaryLabel}
        {activeKinds.length > 1 ? ` +${activeKinds.length - 1}` : ""}
      </div>
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
