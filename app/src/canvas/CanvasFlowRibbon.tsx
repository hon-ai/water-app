import { useMemo } from "react";
import type { HeatReadResponse } from "../ipc/commands";
import type { CanvasCard } from "./CanvasSurface";
import { CARD_W, CARD_H } from "./SceneCard";

interface Props {
  /** Canonical (isPrimary) cards. Order doesn't matter for the field
   *  computation — every scene is a point of influence regardless of
   *  manuscript position. */
  cards: CanvasCard[];
  /** Per-scene heat metrics. Drives local thickness via field
   *  contribution. */
  heatPerScene: Record<string, HeatReadResponse["metrics"]>;
}

/** Radius (canvas-space px) over which a scene influences the
 *  ribbon's width + Y position. Beyond this, falloff is 0. */
const INFLUENCE_R = 360;
/** Floor thickness so the ribbon is always visible even in regions
 *  without nearby scenes. */
const BASE_W = 14;
/** Per-scene heat contribution to local width (added to BASE_W,
 *  multiplied by falloff and scene intensity). */
const HEAT_BUMP = 110;
/** Centerline samples across the canvas extent. Higher = smoother. */
const SAMPLES = 120;
/** Padding on either side of the canvas extent so the ribbon enters
 *  from off-screen rather than abruptly. */
const X_PAD = 360;
/** Cap on how far the centerline Y can deflect from its base — keeps
 *  the ribbon "flowing L→R" rather than chasing scenes vertically. */
const Y_DEFLECT_CAP = 220;

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
 * Ambient flow ribbon on the spatial canvas.
 *
 * Not a connector. The ribbon's centerline flows L→R across the
 * canvas extent on its own path; scenes exert *influence* on it —
 * thicker where heat is high and scenes are close, attracted
 * vertically toward nearby scenes' Y positions. As cards drag, the
 * influence field shifts and the ribbon morphs to follow.
 *
 * Each centerline sample point:
 *   y(x)     = base_y + Σ scene_y_offset × falloff(distance)
 *   width(x) = BASE_W + Σ scene_heat × HEAT_BUMP × falloff(distance)
 *
 * Multi-layer rendering matches the editor ribbon: wide halo,
 * mid-blur body, sharp core, top-edge highlight. Brightness varies
 * along the length so the ribbon catches "light" unevenly.
 */
export function CanvasFlowRibbon({ cards, heatPerScene }: Props) {
  const shape = useMemo(() => {
    const primaries = cards.filter((c) => c.isPrimary);
    if (primaries.length === 0) return null;

    // Scene influence points: anchor at the card center, weighted
    // by heat intensity.
    const scenes = primaries.map((c) => ({
      x: c.x + CARD_W / 2,
      y: c.y + CARD_H / 2,
      heat: sceneIntensity(heatPerScene[c.id]),
    }));

    const minX = Math.min(...scenes.map((s) => s.x)) - X_PAD;
    const maxX = Math.max(...scenes.map((s) => s.x)) + X_PAD;
    const baseY =
      scenes.reduce((acc, s) => acc + s.y, 0) / Math.max(1, scenes.length);

    const xs: number[] = [];
    const ys: number[] = [];
    const widths: number[] = [];
    const brightness: number[] = [];

    for (let i = 0; i <= SAMPLES; i++) {
      const t = i / SAMPLES;
      const sx = minX + t * (maxX - minX);

      // Aggregate influence from every scene.
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

      // Centerline Y: base + weighted scene-Y attraction, capped.
      const rawDeflect =
        totalWeight > 0 ? yOffsetWeighted / totalWeight : 0;
      const deflect = Math.max(-Y_DEFLECT_CAP, Math.min(Y_DEFLECT_CAP, rawDeflect));
      const cy = baseY + deflect;

      // Width: floor + summed heat contribution. Plus a soft taper
      // at the entry/exit so the ribbon thins as it leaves the
      // canvas, not a hard cut.
      const taper = Math.min(1, Math.sin(t * Math.PI) * 1.6);
      const w = (BASE_W + widthBump) * Math.max(0.25, taper);

      // Brightness envelope independent of width — varies along the
      // path so the ribbon catches light at unrelated points.
      const b =
        0.45 +
        0.4 * Math.sin(t * Math.PI * 2.3 + 0.6) +
        0.2 * Math.sin(t * Math.PI * 5.5 + 1.7);

      xs.push(sx);
      ys.push(cy);
      widths.push(Math.max(6, w));
      brightness.push(Math.max(0.2, Math.min(1, b)));
    }

    // Top + bottom edges via perpendicular tangent.
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
    for (let i = 1; i < top.length; i++) {
      d += ` L ${top[i]!.x} ${top[i]!.y}`;
    }
    for (let i = bot.length - 1; i >= 0; i--) {
      d += ` L ${bot[i]!.x} ${bot[i]!.y}`;
    }
    d += " Z";

    let edge = `M ${top[0]!.x} ${top[0]!.y}`;
    for (let i = 1; i < top.length; i++) {
      edge += ` L ${top[i]!.x} ${top[i]!.y}`;
    }

    // Brightness stop list (8 stops, average within each bucket).
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

    // Bounding box for SVG layout.
    const allX = [...top.map((p) => p.x), ...bot.map((p) => p.x)];
    const allY = [...top.map((p) => p.y), ...bot.map((p) => p.y)];
    return {
      d,
      edge,
      stopValues,
      bounds: {
        minX: Math.min(...allX) - 80,
        minY: Math.min(...allY) - 80,
        maxX: Math.max(...allX) + 80,
        maxY: Math.max(...allY) + 80,
      },
    };
  }, [cards, heatPerScene]);

  if (!shape) return null;
  const { d, edge, stopValues, bounds } = shape;
  const w = bounds.maxX - bounds.minX;
  const h = bounds.maxY - bounds.minY;

  return (
    <svg
      aria-hidden
      data-testid="canvas-flow-ribbon"
      style={{
        position: "absolute",
        left: bounds.minX,
        top: bounds.minY,
        width: w,
        height: h,
        pointerEvents: "none",
        zIndex: 0,
        // Smooth transitions when the ribbon shape changes (card
        // drag). React re-renders give us a new path; CSS filter
        // doesn't tween paths, but opacity does — fading the ribbon
        // during morph hides the discrete jump.
        transition: "filter var(--water-dur-medium) var(--water-ease-out-soft)",
      }}
      viewBox={`${bounds.minX} ${bounds.minY} ${w} ${h}`}
    >
      <defs>
        <filter id="cf-glow-wide" x="-10%" y="-30%" width="120%" height="160%">
          <feGaussianBlur in="SourceGraphic" stdDeviation="14" />
        </filter>
        <filter id="cf-glow-mid" x="-10%" y="-30%" width="120%" height="160%">
          <feGaussianBlur in="SourceGraphic" stdDeviation="4" />
        </filter>
        <linearGradient id="cf-grad" x1="0" y1="0" x2="1" y2="0">
          {stopValues.map((b, ix) => (
            <stop
              key={ix}
              offset={`${(ix / (stopValues.length - 1)) * 100}%`}
              stopColor={
                b > 0.7 ? "var(--water-sea-glow)" : "var(--water-sea-300)"
              }
              stopOpacity={0.18 + b * 0.32}
            />
          ))}
        </linearGradient>
        <linearGradient id="cf-edge-grad" x1="0" y1="0" x2="1" y2="0">
          {stopValues.map((b, ix) => (
            <stop
              key={ix}
              offset={`${(ix / (stopValues.length - 1)) * 100}%`}
              stopColor="var(--water-sea-glow)"
              stopOpacity={0.0 + b * 0.4}
            />
          ))}
        </linearGradient>
      </defs>
      <path d={d} fill="url(#cf-grad)" opacity={0.55} filter="url(#cf-glow-wide)" />
      <path d={d} fill="url(#cf-grad)" opacity={0.65} filter="url(#cf-glow-mid)" />
      <path d={d} fill="url(#cf-grad)" opacity={0.4} />
      <path d={edge} fill="none" stroke="url(#cf-edge-grad)" strokeWidth={1.1} strokeLinecap="round" />
    </svg>
  );
}
