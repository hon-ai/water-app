import { useEffect, useReducer } from "react";
import type { CSSProperties } from "react";

interface Influence {
  /** Parent-x coordinate (CSS px from the left edge of the parent
   *  container) where the influence is anchored. The ribbon's local
   *  centerline bulges toward this point and the local width
   *  thickens, with a gaussian falloff over INFLUENCE_SIGMA. */
  x: number;
  /** Relative weight, typically 0..1. Scales the local warp. */
  weight: number;
}

interface Props {
  parentWidth: number;
  /** Vertical center of the ribbon's resting line. */
  baseY?: number;
  /** Base thickness; modulated by periodic harmonics. */
  baseThickness?: number;
  /** Samples per period — three periods fill the SVG. */
  samplesPerPeriod?: number;
  /**
   * Mask the central markdown column. When `columnMaxWidth > 0` the
   * ribbon fades to transparent across the column. Pass `0` (or
   * omit) for open surfaces (canvas, characters, world) where no
   * column masking is desired and the ribbon spans full width.
   */
  columnMaxWidth?: number;
  /** Render order. The component sets a small inline z-index so a
   *  caller doesn't need wrapper styling to put it behind content. */
  zIndex?: number;
  /** Optional list of influence anchors (e.g., scene positions on
   *  the canvas). The ribbon warps gently toward each anchor with
   *  the same time modulation rate as its base harmonics — so the
   *  ribbon visibly "touches" each scene as it flows past, and the
   *  warps shift gradually when scenes move. Pass an empty array
   *  (or omit) for surfaces without scene state. */
  influences?: Influence[];
}

const INFLUENCE_SIGMA = 140;

function prand(i: number, salt: number): number {
  const x = Math.sin(i * 9301 + salt * 49297) * 233280;
  return x - Math.floor(x);
}

/**
 * Ambient water-stream ribbon. Real-time path morphing — y, width,
 * brightness, and alpha along the path are all sinusoidal functions
 * of (x, t) where t = performance.now()/1000. The ribbon is a single
 * shape that continuously deforms: width swells and pinches in place,
 * brightness shimmers, alpha drifts. The L→R apparent motion comes
 * from phase advancement in the y(x, t) terms, not from a CSS
 * translateX — so the ribbon never reads as a conveyor belt.
 *
 * Multiple instances stay synchronized because they all read the
 * same performance.now() clock. Switching between surfaces shows a
 * continuous flow rather than a phase-resetting jump.
 */
export function WaterRibbon({
  parentWidth,
  baseY = 280,
  baseThickness = 56,
  samplesPerPeriod = 96,
  columnMaxWidth = 0,
  zIndex = 0,
  influences = [],
}: Props) {
  // rAF forces a re-render each frame; the actual time value comes
  // from performance.now() inside render so the shape is computed
  // from a single canonical clock.
  const [, tick] = useReducer((x: number) => x + 1, 0);
  useEffect(() => {
    let raf = 0;
    const loop = () => {
      tick();
      raf = requestAnimationFrame(loop);
    };
    raf = requestAnimationFrame(loop);
    return () => cancelAnimationFrame(raf);
  }, []);

  if (parentWidth <= 0) return null;

  const t = performance.now() / 1000;
  const W = parentWidth;
  const tau = (2 * Math.PI) / W;
  const SAMPLES = samplesPerPeriod * 3;

  // Frequency constants in rad/s — each modulation has its own pace
  // so the surface stays organic.
  const omegaY1 = 0.13; // primary horizontal drift (same as old 48s loop)
  const omegaY2 = 0.21;
  const omegaY3 = 0.34;
  const omegaW1 = 0.18;
  const omegaW2 = 0.28;
  const omegaB1 = 0.24;
  const omegaB2 = 0.41;
  const omegaA1 = 0.16;
  const omegaA2 = 0.31;

  // Influences from the caller (e.g., scene positions on the canvas)
  // get tiled three times so each influence affects all three
  // periods of the path, keeping the loop seam invisible.
  const tiled: { ix: number; weight: number; phaseOffset: number }[] = [];
  for (const inf of influences) {
    // Per-anchor phase offset so each scene's local warp breathes at
    // a slightly different beat — derived deterministically from x
    // so the same scene always has the same phase across renders.
    const phaseOffset = (inf.x * 0.0123) % (2 * Math.PI);
    for (let tile = 0; tile < 3; tile++) {
      tiled.push({
        ix: inf.x + tile * W,
        weight: inf.weight,
        phaseOffset,
      });
    }
  }
  const sigma2 = 2 * INFLUENCE_SIGMA * INFLUENCE_SIGMA;
  const influenceY = (x: number) => {
    if (tiled.length === 0) return 0;
    let sum = 0;
    // Same modulation rate as the base harmonics (omegaY1) so the
    // warps shift at the same pace as the ribbon already breathes.
    for (const { ix, weight, phaseOffset } of tiled) {
      const dx = x - ix;
      const bump = Math.exp(-(dx * dx) / sigma2);
      sum += weight * 26 * Math.sin(omegaY1 * t + phaseOffset) * bump;
    }
    return sum;
  };
  const influenceW = (x: number) => {
    if (tiled.length === 0) return 0;
    let sum = 0;
    for (const { ix, weight, phaseOffset } of tiled) {
      const dx = x - ix;
      const bump = Math.exp(-(dx * dx) / sigma2);
      // Width is bumped *up* by a positive offset modulated by an
      // independent sin so the thickness pulse desyncs from the y
      // pulse — feels like local water turbulence near each scene.
      sum +=
        weight *
        18 *
        (0.6 + 0.4 * Math.sin(omegaW1 * t + phaseOffset * 1.3)) *
        bump;
    }
    return sum;
  };

  const yAt = (x: number) =>
    baseY +
    52 * Math.sin(tau * x + 0.0 + omegaY1 * t) +
    24 * Math.sin(2 * tau * x + 1.05 + omegaY2 * t) +
    14 * Math.sin(3 * tau * x + 0.4 + omegaY3 * t) +
    influenceY(x);
  const widthAt = (x: number) => {
    const swell =
      0.55 +
      0.35 * Math.sin(tau * x + 0.6 + omegaW1 * t) +
      0.18 * Math.sin(3 * tau * x + 1.4 + omegaW2 * t);
    return Math.max(8, baseThickness * swell + influenceW(x));
  };
  const brightAt = (x: number) =>
    Math.max(
      0.2,
      Math.min(
        1,
        0.5 +
          0.38 * Math.sin(tau * x + 1.7 + omegaB1 * t) +
          0.18 * Math.sin(2 * tau * x + 0.3 + omegaB2 * t),
      ),
    );
  const alphaAt = (x: number) =>
    Math.max(
      0.3,
      Math.min(
        1,
        0.55 +
          0.35 * Math.sin(tau * x + 0.9 + omegaA1 * t) +
          0.15 * Math.sin(2 * tau * x + 2.1 + omegaA2 * t),
      ),
    );

  const xs: number[] = [];
  const ys: number[] = [];
  const widths: number[] = [];
  const brightness: number[] = [];
  const alpha: number[] = [];
  for (let i = 0; i <= SAMPLES; i++) {
    const x = (i / SAMPLES) * (3 * W);
    xs.push(x);
    ys.push(yAt(x));
    widths.push(widthAt(x));
    brightness.push(brightAt(x));
    alpha.push(alphaAt(x));
  }

  // Top + bot edges via perpendicular tangent.
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

  // 12-stop gradient sampling brightness + alpha.
  const STOPS = 12;
  const stopValues: { b: number; a: number }[] = [];
  for (let s = 0; s < STOPS; s++) {
    const start = Math.floor((s / STOPS) * SAMPLES);
    const end = Math.floor(((s + 1) / STOPS) * SAMPLES);
    let bSum = 0;
    let aSum = 0;
    let n = 0;
    for (let i = start; i <= end; i++) {
      bSum += brightness[i] ?? 0.5;
      aSum += alpha[i] ?? 0.5;
      n += 1;
    }
    stopValues.push({
      b: bSum / Math.max(1, n),
      a: aSum / Math.max(1, n),
    });
  }

  // Droplets: tile a deterministic pattern three times so the seam
  // is invisible. Each droplet anchored to y(x) at its own x.
  const DROPS_PER_PERIOD = 28;
  const drops: { cx: number; cy: number; r: number; opacity: number }[] = [];
  for (let tile = 0; tile < 3; tile++) {
    for (let i = 0; i < DROPS_PER_PERIOD; i++) {
      const xFrac = prand(i, 1);
      const x = xFrac * W + tile * W;
      const perpFrac = prand(i, 2) - 0.5;
      const perpScale = 60 + prand(i, 5) * 100;
      // Droplet y also slightly time-modulated for living spray feel.
      const driftPerp =
        12 * Math.sin(0.6 * t + prand(i, 6) * 6.28);
      const cy = yAt(x) + perpFrac * perpScale + driftPerp;
      const sizeRand = prand(i, 3);
      const r =
        sizeRand < 0.85 ? 0.5 + sizeRand * 1.1 : 1.5 + (sizeRand - 0.85) * 10;
      const opacity =
        (0.25 + prand(i, 4) * 0.5) *
        (0.7 + 0.3 * Math.sin(0.9 * t + prand(i, 7) * 6.28));
      drops.push({ cx: x, cy, r, opacity });
    }
  }

  // Mask: only when columnMaxWidth > 0. Otherwise full width.
  const columnWidth = columnMaxWidth > 0
    ? Math.min(columnMaxWidth, Math.max(0, parentWidth - 48))
    : 0;
  const columnLeft = columnWidth > 0 ? (parentWidth - columnWidth) / 2 : 0;
  const columnRight = columnLeft + columnWidth;
  const SOFT = 48;
  const maskImage =
    columnWidth > 0
      ? `linear-gradient(
          90deg,
          black 0px,
          black ${columnLeft - SOFT}px,
          transparent ${columnLeft}px,
          transparent ${columnRight}px,
          black ${columnRight + SOFT}px,
          black ${parentWidth}px
        )`
      : undefined;

  const wrapperStyle: CSSProperties = {
    position: "absolute",
    inset: 0,
    pointerEvents: "none",
    overflow: "hidden",
    zIndex,
    ...(maskImage && {
      maskImage,
      WebkitMaskImage: maskImage,
    }),
  };

  const VB_H = baseY + 200;

  return (
    <div data-testid="water-ribbon" style={wrapperStyle} aria-hidden>
      <svg
        width={parentWidth * 3}
        height={VB_H}
        style={{
          position: "absolute",
          left: -parentWidth,
          top: 0,
          display: "block",
        }}
      >
        <defs>
          <filter id="wr-glow-wide" x="-10%" y="-30%" width="120%" height="160%">
            <feGaussianBlur in="SourceGraphic" stdDeviation="22" />
          </filter>
          <filter id="wr-glow-mid" x="-10%" y="-30%" width="120%" height="160%">
            <feGaussianBlur in="SourceGraphic" stdDeviation="6" />
          </filter>
          <filter id="wr-drop-glow" x="-300%" y="-300%" width="700%" height="700%">
            <feGaussianBlur in="SourceGraphic" stdDeviation="1.5" />
          </filter>
          {/* Smooth gradient: stop color is a continuous color-mix
              between sea-300 and sea-glow driven by brightness. The
              previous binary threshold (b > 0.7) flipped stops
              between two colors as the brightness modulator crossed
              the line, which the writer perceived as light/dark
              glitching. With color-mix the transition is continuous. */}
          <linearGradient id="wr-grad" x1="0" y1="0" x2="1" y2="0">
            {stopValues.map((s, ix) => (
              <stop
                key={ix}
                offset={`${(ix / (stopValues.length - 1)) * 100}%`}
                stopColor={`color-mix(in oklch, var(--water-sea-300), var(--water-sea-glow) ${Math.round(
                  s.b * 60,
                )}%)`}
                stopOpacity={(0.22 + s.b * 0.35) * s.a}
              />
            ))}
          </linearGradient>
          <linearGradient id="wr-edge-grad" x1="0" y1="0" x2="1" y2="0">
            {stopValues.map((s, ix) => (
              <stop
                key={ix}
                offset={`${(ix / (stopValues.length - 1)) * 100}%`}
                stopColor="var(--water-sea-glow)"
                stopOpacity={(0.05 + s.b * 0.45) * s.a}
              />
            ))}
          </linearGradient>
        </defs>
        <path d={d} fill="url(#wr-grad)" opacity={0.6} filter="url(#wr-glow-wide)" />
        <path d={d} fill="url(#wr-grad)" opacity={0.75} filter="url(#wr-glow-mid)" />
        <path d={d} fill="url(#wr-grad)" opacity={0.5} />
        <path
          d={edge}
          fill="none"
          stroke="url(#wr-edge-grad)"
          strokeWidth={1.2}
          strokeLinecap="round"
        />
        {drops.map((dr, i) => (
          <g key={i}>
            <circle
              cx={dr.cx}
              cy={dr.cy}
              r={dr.r * 2.2}
              fill="var(--water-sea-glow)"
              opacity={dr.opacity * 0.5}
              filter="url(#wr-drop-glow)"
            />
            <circle
              cx={dr.cx}
              cy={dr.cy}
              r={dr.r}
              fill="var(--water-sea-glow)"
              opacity={dr.opacity}
            />
          </g>
        ))}
      </svg>
    </div>
  );
}
