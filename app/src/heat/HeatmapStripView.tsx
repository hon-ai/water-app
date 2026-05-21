import type { HeatMetricKind, HeatReadResponse } from "../ipc/commands";

interface Props {
  metrics: HeatReadResponse["metrics"];
  activeKinds: HeatMetricKind[];
  height?: number;
  borderRadius?: string;
  ariaLabel?: string;
}

/**
 * Pure presentational heatmap renderer. No fetching, no event
 * subscriptions, no chip, no hover tooltip — just the grid of
 * per-paragraph per-metric cells. Used by both:
 *
 * - `HeatmapStrip` (the editor-canvas barometer; full-size at 24 px).
 * - `SceneCard` on the M6 macro canvas (small sparkline at 8 px).
 *
 * The container's parent gives it the width; columns fill that
 * width via grid-template-columns. Empty metric vecs render as
 * blank cells (the renderer treats that as "no data").
 */
export function HeatmapStripView({
  metrics,
  activeKinds,
  height = 24,
  borderRadius = "var(--water-r-8)",
  ariaLabel = "Heatmap",
}: Props) {
  const visibleKinds = activeKinds.filter(
    (k) => (metrics[k]?.length ?? 0) > 0,
  );
  const columnCount = Math.max(
    ...visibleKinds.map((k) => metrics[k]?.length ?? 0),
    1,
  );

  return (
    <div
      aria-label={ariaLabel}
      role="img"
      data-testid="heatmap-strip-view"
      style={{
        width: "100%",
        height,
        background: "var(--water-bg-canvas)",
        borderRadius,
        overflow: "hidden",
        display: "grid",
        gridTemplateColumns: `repeat(${columnCount}, 1fr)`,
      }}
    >
      {Array.from({ length: columnCount }).map((_, ix) => (
        <div
          key={ix}
          data-paragraph-ix={ix}
          style={{ position: "relative", overflow: "hidden" }}
        >
          {visibleKinds.map((kind) => {
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
  );
}

export function metricHue(kind: HeatMetricKind): string {
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
 * AND warm paragraphs both render visibly.
 */
export function cellOpacity(kind: HeatMetricKind, value: number): number {
  if (kind === "valence") {
    return Math.min(1, Math.abs(value));
  }
  return Math.max(0, Math.min(1, value));
}
