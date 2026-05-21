import { useEffect, useRef, useState } from "react";
import type { HeatReadResponse } from "../ipc/commands";
import type { CanvasCard } from "./CanvasSurface";
import { CARD_W, CARD_H } from "./SceneCard";

interface Props {
  cards: CanvasCard[];
  heatPerScene: Record<string, HeatReadResponse["metrics"]>;
  /** Left edge of the visible canvas viewport, canvas-space px.
   *  The ribbon extends from this minus padding so it always fills
   *  the visible area regardless of zoom level. */
  viewportMinX: number;
  /** Right edge of the visible canvas viewport, canvas-space px. */
  viewportMaxX: number;
}

interface DisplayedScene {
  id: string;
  x: number;
  y: number;
  heat: number;
}

const INFLUENCE_R = 360;
const BASE_W = 14;
const HEAT_BUMP = 110;
const SAMPLES = 120;
/** Padding beyond viewport so the ribbon visibly enters/exits from
 *  off-screen rather than terminating at the edge. */
const X_PAD = 240;
/** Cap on centerline Y deflection from baseY — keeps the ribbon
 *  flowing L→R rather than chasing scenes vertically. */
const Y_DEFLECT_CAP = 220;
/** rAF easing factor per frame. 0.06 ≈ 1s convergence at 60fps,
 *  which gives the "slowly streaming toward the new layout" feel. */
const EASE = 0.06;
/** Convergence thresholds — once every scene's displayed value is
 *  within these of the target, the rAF loop stops. */
const CONV_POS = 0.5;
const CONV_HEAT = 0.005;
/** Y-cluster split threshold. If the global Y range of scenes
 *  exceeds this AND a clear gap exists, the ribbon splits into two
 *  parallel strands so vertically-distributed scenes get their own
 *  flow line instead of an averaged middle ground. */
const SPLIT_RANGE = 280;
const SPLIT_GAP = 180;

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
 * Decide if scenes split into upper/lower clusters. Returns the
 * split point (Y value) or null if the ribbon should stay as one
 * strand. We split when both:
 *   - the Y range covers SPLIT_RANGE+ px (otherwise scenes are
 *     vertically compact enough to share one strand), AND
 *   - there's a gap of SPLIT_GAP+ px somewhere in the sorted Y list
 *     (otherwise the spread is gradual and averaging looks fine).
 */
function detectSplit(scenes: DisplayedScene[]): number | null {
  if (scenes.length < 2) return null;
  const ys = scenes.map((s) => s.y).sort((a, b) => a - b);
  const range = ys[ys.length - 1]! - ys[0]!;
  if (range < SPLIT_RANGE) return null;
  let bestGap = 0;
  let bestY = 0;
  for (let i = 1; i < ys.length; i++) {
    const gap = ys[i]! - ys[i - 1]!;
    if (gap > bestGap) {
      bestGap = gap;
      bestY = (ys[i]! + ys[i - 1]!) / 2;
    }
  }
  if (bestGap < SPLIT_GAP) return null;
  return bestY;
}

/**
 * Build one ribbon strand (closed polygon + top-edge highlight)
 * from a scene subset spanning [xMin, xMax]. The strand's centerline
 * is base_y + influence-weighted Y deflection from its scenes; the
 * width swells with heat.
 */
function buildStrand(
  scenes: DisplayedScene[],
  xMin: number,
  xMax: number,
  baseY: number,
): { d: string; edge: string; stopValues: number[] } | null {
  if (scenes.length === 0) return null;

  const xs: number[] = [];
  const ys: number[] = [];
  const widths: number[] = [];
  const brightness: number[] = [];

  for (let i = 0; i <= SAMPLES; i++) {
    const t = i / SAMPLES;
    const sx = xMin + t * (xMax - xMin);

    let totalWeight = 0;
    let yOffsetWeighted = 0;
    let widthBump = 0;
    for (const s of scenes) {
      const d = Math.hypot(s.x - sx, s.y - baseY);
      if (d > INFLUENCE_R) continue;
      const f = (1 - d / INFLUENCE_R) ** 2;
      totalWeight += f;
      yOffsetWeighted += (s.y - baseY) * f;
      widthBump += s.heat * HEAT_BUMP * f;
    }

    const rawDeflect = totalWeight > 0 ? yOffsetWeighted / totalWeight : 0;
    const deflect = Math.max(-Y_DEFLECT_CAP, Math.min(Y_DEFLECT_CAP, rawDeflect));
    const cy = baseY + deflect;

    // Soft taper at the viewport ends only.
    const taperT = Math.min(1, t * 14, (1 - t) * 14);
    const w = (BASE_W + widthBump) * Math.max(0.25, taperT);

    const b =
      0.45 +
      0.4 * Math.sin(t * Math.PI * 2.3 + 0.6) +
      0.2 * Math.sin(t * Math.PI * 5.5 + 1.7);

    xs.push(sx);
    ys.push(cy);
    widths.push(Math.max(6, w));
    brightness.push(Math.max(0.2, Math.min(1, b)));
  }

  const top: { x: number; y: number }[] = [];
  const bot: { x: number; y: number }[] = [];
  for (let i = 0; i <= SAMPLES; i++) {
    const prev = i > 0 ? i - 1 : i;
    const next = i < SAMPLES ? i + 1 : i;
    const tx = xs[next]! - xs[prev]!;
    const ty = ys[next]! - ys[prev]!;
    const len = Math.hypot(tx, ty) || 1;
    const nx = -ty / len;
    const ny = tx / len;
    const half = widths[i]! * 0.5;
    top.push({ x: xs[i]! + nx * half, y: ys[i]! + ny * half });
    bot.push({ x: xs[i]! - nx * half, y: ys[i]! - ny * half });
  }

  let d = `M ${top[0]!.x} ${top[0]!.y}`;
  for (let i = 1; i < top.length; i++) d += ` L ${top[i]!.x} ${top[i]!.y}`;
  for (let i = bot.length - 1; i >= 0; i--) d += ` L ${bot[i]!.x} ${bot[i]!.y}`;
  d += " Z";

  let edge = `M ${top[0]!.x} ${top[0]!.y}`;
  for (let i = 1; i < top.length; i++) edge += ` L ${top[i]!.x} ${top[i]!.y}`;

  const STOPS = 8;
  const stopValues: number[] = [];
  for (let s = 0; s < STOPS; s++) {
    const start = Math.floor((s / STOPS) * SAMPLES);
    const end = Math.floor(((s + 1) / STOPS) * SAMPLES);
    let sum = 0;
    let n = 0;
    for (let i = start; i <= end; i++) {
      sum += brightness[i] ?? 0.5;
      n += 1;
    }
    stopValues.push(sum / Math.max(1, n));
  }

  return { d, edge, stopValues };
}

/**
 * Field-based flow ribbon for the spatial canvas.
 *
 * Three properties that make this not-a-connector and not-snappy:
 *
 *  1. Independent of scene order or position. The ribbon's
 *     centerline runs L→R across the full viewport extent (not just
 *     the scene bounding box), so it stays present and flowing even
 *     when zoomed out far past the scenes.
 *
 *  2. Smooth morph via rAF. When cards drag or heat updates, each
 *     scene's "displayed" influence point eases toward its target
 *     over ~1 second. The ribbon recomputes from displayed values,
 *     so its shape evolves gradually rather than snapping.
 *
 *  3. Splits into multiple strands when scenes spread vertically.
 *     If the Y range covers > SPLIT_RANGE and a clear gap exists in
 *     the Y distribution, the ribbon renders TWO parallel strands —
 *     upper and lower — each pulled by its half's scenes. Where the
 *     halves' influences converge the strands overlap; where they
 *     diverge the strands pull apart.
 */
export function CanvasFlowRibbon({
  cards,
  heatPerScene,
  viewportMinX,
  viewportMaxX,
}: Props) {
  const [displayed, setDisplayed] = useState<DisplayedScene[]>([]);
  const displayedRef = useRef<DisplayedScene[]>([]);
  const targetRef = useRef<DisplayedScene[]>([]);
  const rafRef = useRef<number | null>(null);
  const initializedRef = useRef(false);

  // Build target from props each render; kick rAF on change.
  useEffect(() => {
    const target: DisplayedScene[] = cards
      .filter((c) => c.isPrimary)
      .map((c) => ({
        id: c.id,
        x: c.x + CARD_W / 2,
        y: c.y + CARD_H / 2,
        heat: sceneIntensity(heatPerScene[c.id]),
      }));
    targetRef.current = target;

    // First time we see scenes: skip the morph and snap to target
    // so the ribbon doesn't animate in from nothing.
    if (!initializedRef.current) {
      initializedRef.current = true;
      displayedRef.current = target;
      setDisplayed(target);
      return;
    }

    if (rafRef.current !== null) return; // already running

    const tick = () => {
      const targets = targetRef.current;
      const current = displayedRef.current;
      const byId = new Map(current.map((s) => [s.id, s]));

      let converged = true;
      const next: DisplayedScene[] = targets.map((t) => {
        const c = byId.get(t.id);
        if (!c) return { ...t }; // new scene appears at target
        const dx = t.x - c.x;
        const dy = t.y - c.y;
        const dh = t.heat - c.heat;
        if (
          Math.abs(dx) > CONV_POS ||
          Math.abs(dy) > CONV_POS ||
          Math.abs(dh) > CONV_HEAT
        ) {
          converged = false;
        }
        return {
          id: t.id,
          x: c.x + dx * EASE,
          y: c.y + dy * EASE,
          heat: c.heat + dh * EASE,
        };
      });

      displayedRef.current = next;
      setDisplayed(next);

      if (!converged) {
        rafRef.current = requestAnimationFrame(tick);
      } else {
        rafRef.current = null;
      }
    };
    rafRef.current = requestAnimationFrame(tick);
  }, [cards, heatPerScene]);

  // Cleanup rAF on unmount.
  useEffect(
    () => () => {
      if (rafRef.current !== null) {
        cancelAnimationFrame(rafRef.current);
        rafRef.current = null;
      }
    },
    [],
  );

  if (displayed.length === 0) return null;
  if (viewportMaxX <= viewportMinX) return null;

  // Strand assignment: detect vertical split, partition scenes.
  const split = detectSplit(displayed);
  const xMin = viewportMinX - X_PAD;
  const xMax = viewportMaxX + X_PAD;

  let strands: ReturnType<typeof buildStrand>[] = [];
  if (split === null) {
    const baseY =
      displayed.reduce((acc, s) => acc + s.y, 0) / displayed.length;
    strands = [buildStrand(displayed, xMin, xMax, baseY)];
  } else {
    const upper = displayed.filter((s) => s.y < split);
    const lower = displayed.filter((s) => s.y >= split);
    const upperBaseY =
      upper.reduce((acc, s) => acc + s.y, 0) / Math.max(1, upper.length);
    const lowerBaseY =
      lower.reduce((acc, s) => acc + s.y, 0) / Math.max(1, lower.length);
    strands = [
      buildStrand(upper, xMin, xMax, upperBaseY),
      buildStrand(lower, xMin, xMax, lowerBaseY),
    ];
  }

  const validStrands = strands.filter(
    (s): s is { d: string; edge: string; stopValues: number[] } => s !== null,
  );
  if (validStrands.length === 0) return null;

  // Bounds: union of all strand paths.
  const ys = displayed.map((s) => s.y);
  const minStrandY = Math.min(...ys) - 200;
  const maxStrandY = Math.max(...ys) + 200;
  const w = xMax - xMin;
  const h = maxStrandY - minStrandY;

  return (
    <svg
      aria-hidden
      data-testid="canvas-flow-ribbon"
      style={{
        position: "absolute",
        left: xMin,
        top: minStrandY,
        width: w,
        height: h,
        pointerEvents: "none",
        zIndex: 0,
      }}
      viewBox={`${xMin} ${minStrandY} ${w} ${h}`}
    >
      <defs>
        <filter id="cf-glow-wide" x="-10%" y="-30%" width="120%" height="160%">
          <feGaussianBlur in="SourceGraphic" stdDeviation="14" />
        </filter>
        <filter id="cf-glow-mid" x="-10%" y="-30%" width="120%" height="160%">
          <feGaussianBlur in="SourceGraphic" stdDeviation="4" />
        </filter>
      </defs>
      {validStrands.map((strand, ix) => (
        <g key={ix}>
          <defs>
            <linearGradient id={`cf-grad-${ix}`} x1="0" y1="0" x2="1" y2="0">
              {strand.stopValues.map((b, six) => (
                <stop
                  key={six}
                  offset={`${(six / (strand.stopValues.length - 1)) * 100}%`}
                  stopColor={
                    b > 0.7 ? "var(--water-sea-glow)" : "var(--water-sea-300)"
                  }
                  stopOpacity={0.18 + b * 0.32}
                />
              ))}
            </linearGradient>
            <linearGradient id={`cf-edge-grad-${ix}`} x1="0" y1="0" x2="1" y2="0">
              {strand.stopValues.map((b, six) => (
                <stop
                  key={six}
                  offset={`${(six / (strand.stopValues.length - 1)) * 100}%`}
                  stopColor="var(--water-sea-glow)"
                  stopOpacity={0.0 + b * 0.4}
                />
              ))}
            </linearGradient>
          </defs>
          <path d={strand.d} fill={`url(#cf-grad-${ix})`} opacity={0.55} filter="url(#cf-glow-wide)" />
          <path d={strand.d} fill={`url(#cf-grad-${ix})`} opacity={0.65} filter="url(#cf-glow-mid)" />
          <path d={strand.d} fill={`url(#cf-grad-${ix})`} opacity={0.4} />
          <path
            d={strand.edge}
            fill="none"
            stroke={`url(#cf-edge-grad-${ix})`}
            strokeWidth={1.1}
            strokeLinecap="round"
          />
        </g>
      ))}
    </svg>
  );
}
