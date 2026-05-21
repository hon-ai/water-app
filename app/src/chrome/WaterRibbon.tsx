import { useMemo } from "react";
import type { CSSProperties } from "react";

/**
 * Ambient water-stream ribbon behind the editor canvas.
 *
 * Visual goals:
 *  - Implies a SINGLE continuous stream: both visible ends (left
 *    shoulder and right shoulder of the writing column) flow in the
 *    same direction so the eye reads them as one ribbon hidden
 *    behind the markdown, not two separate currents.
 *  - 3D feel: width swells and pinches sinusoidally along the path;
 *    a brighter "lit" edge runs along the top while the bottom edge
 *    softens, suggesting a ribbon caught in light.
 *  - Quiet: the whole thing stays in the 0.15–0.35 opacity range so
 *    the prose is always primary.
 *
 * The path is a single shallow downward sweep (not an S). Both
 * visible ends share the same trajectory; the middle dips a touch
 * lower but is hidden by the central mask. This is what makes the
 * stream read as continuous rather than as two diverging arcs.
 *
 * Animation: slow translateX loop on the SVG. The path content is
 * 3 viewports wide so the loop is seamless.
 */
export function WaterRibbon({
  width = 1600,
  /** Vertical entry point on the left edge. */
  entryY = 220,
  /** Vertical exit point on the right edge. Slightly below entry
   *  so the stream reads as gently descending. */
  exitY = 360,
  /** Central text-column fade. */
  textColumnFade = 0.75,
  /** Base ribbon thickness. Real thickness modulates around this
   *  by ±50% along the length via a sinusoidal envelope. */
  baseThickness = 56,
  /** How many segments to subdivide the path into for variable-width
   *  rendering. Higher = smoother width modulation, costlier. */
  samples = 96,
}: {
  width?: number;
  entryY?: number;
  exitY?: number;
  textColumnFade?: number;
  baseThickness?: number;
  samples?: number;
}) {
  const W = width;
  const VB_W = W * 3;

  // Compute the ribbon's centerline as a smooth descending curve
  // from (-W, entryY) through a hidden midpoint dip to (2W, exitY).
  // The dip below the middle is masked away; what's visible is two
  // shoulders flowing in the same downward direction.
  const ribbonShape = useMemo(() => {
    const xs: number[] = [];
    const ys: number[] = [];
    const widths: number[] = [];
    const brightness: number[] = [];

    for (let i = 0; i <= samples; i++) {
      const t = i / samples;
      const x = -W + t * VB_W;

      // Cubic ease that descends from entryY at t=0 toward a mid
      // dip ~120px below the exit at t=0.5, then settles back to
      // exitY at t=1. Both visible regions (low-t and high-t) are
      // descending; the dip in the middle is hidden.
      // Mid dip target:
      const midDip = exitY + 140;
      const y =
        (1 - t) * (1 - t) * (1 - t) * entryY +
        3 * (1 - t) * (1 - t) * t * midDip +
        3 * (1 - t) * t * t * midDip +
        t * t * t * exitY;

      // Width envelope: sinusoidal swell over the length, plus a
      // second higher-frequency wave for organic variance, plus a
      // soft taper at the ends so the ribbon thins where it exits
      // the viewport rather than ending abruptly.
      const taper = Math.min(1, Math.sin(t * Math.PI) * 1.6);
      const swell =
        0.65 +
        0.35 * Math.sin(t * Math.PI * 2.5 + 0.4) +
        0.15 * Math.sin(t * Math.PI * 7 + 1.2);
      const w = Math.max(8, baseThickness * swell * Math.max(0.25, taper));

      // Brightness envelope: a different sinusoid so glow peaks
      // sometimes coincide with width swells and sometimes don't,
      // giving the ribbon a sense of light catching different
      // points along its length.
      const b =
        0.5 +
        0.4 * Math.sin(t * Math.PI * 3.2 + 1.8) +
        0.2 * Math.sin(t * Math.PI * 6 + 0.6);

      xs.push(x);
      ys.push(y);
      widths.push(w);
      brightness.push(Math.max(0.2, Math.min(1, b)));
    }

    // Build top and bottom edges. Perpendicular from local tangent.
    const top: { x: number; y: number }[] = [];
    const bot: { x: number; y: number }[] = [];
    for (let i = 0; i <= samples; i++) {
      const prev = i > 0 ? i - 1 : i;
      const next = i < samples ? i + 1 : i;
      const tx = xs[next]! - xs[prev]!;
      const ty = ys[next]! - ys[prev]!;
      const len = Math.hypot(tx, ty) || 1;
      const nx = -ty / len;
      const ny = tx / len;
      const half = widths[i]! * 0.5;
      top.push({ x: xs[i]! + nx * half, y: ys[i]! + ny * half });
      bot.push({ x: xs[i]! - nx * half, y: ys[i]! - ny * half });
    }

    // Closed polygon path: top forward, bot backward, close.
    let d = `M ${top[0]!.x} ${top[0]!.y}`;
    for (let i = 1; i < top.length; i++) {
      d += ` L ${top[i]!.x} ${top[i]!.y}`;
    }
    for (let i = bot.length - 1; i >= 0; i--) {
      d += ` L ${bot[i]!.x} ${bot[i]!.y}`;
    }
    d += " Z";

    // Top-edge highlight path: the lit edge alone, not closed.
    // Renders as a thin bright stroke along the top of the ribbon.
    let edge = `M ${top[0]!.x} ${top[0]!.y}`;
    for (let i = 1; i < top.length; i++) {
      edge += ` L ${top[i]!.x} ${top[i]!.y}`;
    }

    // Brightness stop list for the gradient along the length. We
    // approximate by averaging brightness across the samples and
    // emitting 8 stops.
    const STOPS = 8;
    const stopValues: number[] = [];
    for (let s = 0; s < STOPS; s++) {
      const start = Math.floor((s / STOPS) * samples);
      const end = Math.floor(((s + 1) / STOPS) * samples);
      let sum = 0;
      let n = 0;
      for (let i = start; i <= end; i++) {
        sum += brightness[i] ?? 0.5;
        n += 1;
      }
      stopValues.push(sum / Math.max(1, n));
    }

    return { d, edge, stopValues };
  }, [W, VB_W, entryY, exitY, baseThickness, samples]);

  const fadeStart = 50 - textColumnFade * 28;
  const fadeEnd = 50 + textColumnFade * 28;

  const wrapperStyle: CSSProperties = {
    position: "absolute",
    inset: 0,
    pointerEvents: "none",
    overflow: "hidden",
    zIndex: 0,
    maskImage: `linear-gradient(
      90deg,
      black 0%,
      color-mix(in srgb, black 90%, transparent) ${fadeStart - 18}%,
      transparent ${fadeStart}%,
      transparent ${fadeEnd}%,
      color-mix(in srgb, black 90%, transparent) ${fadeEnd + 18}%,
      black 100%
    )`,
    WebkitMaskImage: `linear-gradient(
      90deg,
      black 0%,
      color-mix(in srgb, black 90%, transparent) ${fadeStart - 18}%,
      transparent ${fadeStart}%,
      transparent ${fadeEnd}%,
      color-mix(in srgb, black 90%, transparent) ${fadeEnd + 18}%,
      black 100%
    )`,
  };

  const VB_H = Math.max(entryY, exitY) + 320;

  return (
    <div data-testid="water-ribbon" style={wrapperStyle} aria-hidden>
      <svg
        viewBox={`${-W} 0 ${VB_W} ${VB_H}`}
        preserveAspectRatio="none"
        width="100%"
        height="100%"
        style={{
          position: "absolute",
          inset: 0,
          animation: "water-ribbon-drift 48s linear infinite",
        }}
      >
        <defs>
          <filter id="wr-glow-wide" x="-10%" y="-30%" width="120%" height="160%">
            <feGaussianBlur in="SourceGraphic" stdDeviation="22" />
          </filter>
          <filter id="wr-glow-mid" x="-10%" y="-30%" width="120%" height="160%">
            <feGaussianBlur in="SourceGraphic" stdDeviation="6" />
          </filter>
          {/* Brightness gradient along the ribbon's length. The
              eight stops sample the brightness envelope so light
              catches different points along the stream. */}
          <linearGradient id="wr-grad" x1="0" y1="0" x2="1" y2="0">
            {ribbonShape.stopValues.map((b, ix) => (
              <stop
                key={ix}
                offset={`${(ix / (ribbonShape.stopValues.length - 1)) * 100}%`}
                stopColor={
                  b > 0.7 ? "var(--water-sea-glow)" : "var(--water-sea-300)"
                }
                stopOpacity={0.25 + b * 0.45}
              />
            ))}
          </linearGradient>
          {/* Top-edge highlight: brighter, near-white where the
              "light" catches the curve of a 3D ribbon. */}
          <linearGradient id="wr-edge-grad" x1="0" y1="0" x2="1" y2="0">
            {ribbonShape.stopValues.map((b, ix) => (
              <stop
                key={ix}
                offset={`${(ix / (ribbonShape.stopValues.length - 1)) * 100}%`}
                stopColor="var(--water-sea-glow)"
                stopOpacity={0.0 + b * 0.6}
              />
            ))}
          </linearGradient>
        </defs>

        {/* Layer 1 — wide diffuse halo. Heaviest blur, lowest opacity. */}
        <path
          d={ribbonShape.d}
          fill="url(#wr-grad)"
          opacity={0.55}
          filter="url(#wr-glow-wide)"
        />
        {/* Layer 2 — mid-blur ribbon body. */}
        <path
          d={ribbonShape.d}
          fill="url(#wr-grad)"
          opacity={0.7}
          filter="url(#wr-glow-mid)"
        />
        {/* Layer 3 — sharp ribbon core (no blur). The actual shape. */}
        <path
          d={ribbonShape.d}
          fill="url(#wr-grad)"
          opacity={0.45}
        />
        {/* Layer 4 — top-edge highlight ("lit edge" of a 3D ribbon).
            Thin sharp stroke along the top edge only. */}
        <path
          d={ribbonShape.edge}
          fill="none"
          stroke="url(#wr-edge-grad)"
          strokeWidth={1.2}
          strokeLinecap="round"
        />
      </svg>
    </div>
  );
}
