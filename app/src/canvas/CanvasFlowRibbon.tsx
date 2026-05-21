import type { HeatReadResponse } from "../ipc/commands";
import type { CanvasCard } from "./CanvasSurface";
import { CARD_W, CARD_H } from "./SceneCard";

interface Props {
  /** Canonical (isPrimary) cards in manuscript order. The flow
   *  ribbon visits scenes in narrative sequence regardless of where
   *  they sit in space. */
  cards: CanvasCard[];
  /** Per-scene heat metrics. Used to scale the ribbon's thickness
   *  per scene — heavier scenes thicker, quieter scenes thinner. */
  heatPerScene: Record<string, HeatReadResponse["metrics"]>;
}

/** Min/max thickness in canvas-space px. Quiet scenes pinch to MIN,
 *  busy scenes swell to MAX. The range is wide enough to read at a
 *  glance but never overwhelms the cards above. */
const MIN_W = 18;
const MAX_W = 88;

/**
 * Mean of every available heat metric for a single scene, normalized
 * to 0..1. Drives the ribbon thickness at this scene's anchor point.
 * Returns 0.4 (middle thickness) when no heat data is available so
 * the ribbon never visually collapses on a fresh project.
 */
function sceneIntensity(metrics: HeatReadResponse["metrics"] | undefined): number {
  if (!metrics) return 0.4;
  let total = 0;
  let count = 0;
  for (const rows of Object.values(metrics)) {
    if (!rows || rows.length === 0) continue;
    const mean = rows.reduce((acc, r) => acc + r.value, 0) / rows.length;
    total += Math.max(0, Math.min(1, Math.abs(mean)));
    count += 1;
  }
  if (count === 0) return 0.4;
  return total / count;
}

/**
 * Ambient flow ribbon that snakes through the scenes in manuscript
 * order. Thickness modulates by overall heat intensity per scene so
 * the writer sees, at a glance, the shape of their story's energy
 * across the canvas — wider where the heat is up, narrower where the
 * pacing breathes.
 *
 * Rendered as a single closed SVG polygon: a centerline through
 * each scene's anchor, expanded perpendicular by the scene's
 * intensity. Two layers — a blurred halo + a sharp inner edge —
 * give the same soft-glow character as the editor ribbon.
 *
 * Underneath the cards (z-index 0) but above the canvas substrate.
 * Passes behind every card; the ribbon's perpendicular swell is
 * always small enough that it stays within the inter-card gutter.
 */
export function CanvasFlowRibbon({ cards, heatPerScene }: Props) {
  if (cards.length < 2) return null;

  const sorted = [...cards]
    .filter((c) => c.isPrimary)
    .sort((a, b) => a.manuscript_ordering - b.manuscript_ordering);
  if (sorted.length < 2) return null;

  // Centerline points + perpendicular widths per scene.
  const points = sorted.map((c) => ({
    x: c.x + CARD_W / 2,
    y: c.y + CARD_H / 2,
    w: MIN_W + (MAX_W - MIN_W) * sceneIntensity(heatPerScene[c.id]),
  }));

  // For each point, the perpendicular normal direction. Use the
  // neighbor-difference so endpoints get a sensible tangent.
  const offsets = points.map((p, i) => {
    const prev = points[i - 1] ?? p;
    const next = points[i + 1] ?? p;
    let tx = next.x - prev.x;
    let ty = next.y - prev.y;
    const len = Math.hypot(tx, ty) || 1;
    tx /= len;
    ty /= len;
    // Perpendicular (rotate 90° CCW).
    return { nx: -ty, ny: tx, w: p.w };
  });

  // Top edge: each point + half-width along the normal.
  // Bot edge: each point - half-width.
  const top = points.map((p, i) => ({
    x: p.x + offsets[i]!.nx * p.w * 0.5,
    y: p.y + offsets[i]!.ny * p.w * 0.5,
  }));
  const bot = points.map((p, i) => ({
    x: p.x - offsets[i]!.nx * p.w * 0.5,
    y: p.y - offsets[i]!.ny * p.w * 0.5,
  }));

  // Path: smooth Bezier through top forward, then bot reversed,
  // closed. Smoothing via quadratic with midpoints as control.
  const smoothLine = (pts: { x: number; y: number }[]) => {
    if (pts.length === 0) return "";
    let d = `M ${pts[0]!.x} ${pts[0]!.y}`;
    for (let i = 1; i < pts.length; i++) {
      const a = pts[i - 1]!;
      const b = pts[i]!;
      const mx = (a.x + b.x) / 2;
      const my = (a.y + b.y) / 2;
      // Quadratic curve from a → mid via control = a, then a→b is
      // approximated by Q (mid, b). For polyline-smoothed feel use S.
      d += ` Q ${a.x} ${a.y} ${mx} ${my}`;
      d += ` T ${b.x} ${b.y}`;
    }
    return d;
  };

  // Build the closed ribbon polygon.
  const ribbonD = `${smoothLine(top)} L ${bot[bot.length - 1]!.x} ${bot[bot.length - 1]!.y} ${smoothLine([...bot].reverse()).replace(/^M/, "L")} Z`;

  // SVG bounds.
  const allX = [
    ...top.map((p) => p.x),
    ...bot.map((p) => p.x),
  ];
  const allY = [
    ...top.map((p) => p.y),
    ...bot.map((p) => p.y),
  ];
  const minX = Math.min(...allX) - 60;
  const minY = Math.min(...allY) - 60;
  const maxX = Math.max(...allX) + 60;
  const maxY = Math.max(...allY) + 60;
  const w = maxX - minX;
  const h = maxY - minY;

  return (
    <svg
      aria-hidden
      data-testid="canvas-flow-ribbon"
      style={{
        position: "absolute",
        left: minX,
        top: minY,
        width: w,
        height: h,
        pointerEvents: "none",
        zIndex: 0,
      }}
      viewBox={`${minX} ${minY} ${w} ${h}`}
    >
      <defs>
        <filter id="canvas-flow-glow" x="-10%" y="-10%" width="120%" height="120%">
          <feGaussianBlur in="SourceGraphic" stdDeviation="6" />
        </filter>
        <linearGradient id="canvas-flow-grad" x1="0" y1="0" x2="1" y2="0">
          <stop offset="0%"   stopColor="var(--water-sea-200)" stopOpacity="0.15" />
          <stop offset="50%"  stopColor="var(--water-sea-glow)" stopOpacity="0.32" />
          <stop offset="100%" stopColor="var(--water-sea-300)" stopOpacity="0.18" />
        </linearGradient>
      </defs>
      {/* Halo */}
      <path
        d={ribbonD}
        fill="url(#canvas-flow-grad)"
        filter="url(#canvas-flow-glow)"
        opacity={0.55}
      />
      {/* Sharper inner — same shape, smaller blur, brighter. */}
      <path
        d={ribbonD}
        fill="url(#canvas-flow-grad)"
        opacity={0.4}
      />
    </svg>
  );
}
