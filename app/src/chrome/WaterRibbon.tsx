import { useEffect, useReducer, useRef } from "react";
import type { CSSProperties } from "react";

export interface Anchor {
  /** Stable identity. Used for easing the displayed position when
   *  this anchor's target x/y changes. */
  id: string;
  /** Parent-x (CSS px from the left edge of the WaterRibbon's host). */
  x: number;
  /** Parent-y. The ribbon's centerline interpolates between adjacent
   *  anchors so it actually passes through this point. */
  y: number;
  /** Optional weight, currently used only as a flag (>0 = active). */
  weight?: number;
}

interface Props {
  parentWidth: number;
  baseY?: number;
  baseThickness?: number;
  samplesPerPeriod?: number;
  columnMaxWidth?: number;
  zIndex?: number;
  /**
   * Optional list of anchor points (e.g., scene positions on the
   * canvas). When ≥2 anchors are provided the ribbon's centerline
   * becomes a Catmull-Rom spline THROUGH every anchor — scenes are
   * the bends of the river, noise is small modulation on top.
   * Vertically-clustered anchors split the ribbon into multiple
   * parallel strands.
   *
   * With 0 or 1 anchor the ribbon falls back to the periodic wave
   * (editor / characters / worlds surfaces).
   */
  anchors?: Anchor[];
}

/** Threshold for splitting anchors into separate lanes (strands).
 *  Anchors closer than this in Y go in one strand; further apart
 *  cluster into separate strands. ~half a card-row gap. */
const LANE_GAP = 80;
/** Noise overlay amplitude in spline mode — kept small so the
 *  scene-bound skeleton dominates and the noise reads as a living
 *  surface, not the primary shape. */
const SPLINE_NOISE_Y_AMP = 9;
/** Anchor-easing factor per rAF frame. ~0.05 ≈ 1s convergence at
 *  60fps. Gives the writer time to perceive the river adjusting. */
const EASE = 0.05;

function prand(i: number, salt: number): number {
  const x = Math.sin(i * 9301 + salt * 49297) * 233280;
  return x - Math.floor(x);
}

/** Centripetal Catmull-Rom Y interpolation between four control
 *  points. p0/p3 frame the tangents; the curve passes through p1
 *  and p2 exactly at t=0 and t=1. */
function catmullRomY(
  p0: number,
  p1: number,
  p2: number,
  p3: number,
  t: number,
): number {
  const t2 = t * t;
  const t3 = t2 * t;
  return (
    0.5 *
    (2 * p1 +
      (-p0 + p2) * t +
      (2 * p0 - 5 * p1 + 4 * p2 - p3) * t2 +
      (-p0 + 3 * p1 - 3 * p2 + p3) * t3)
  );
}

function sampleSplineY(sortedByX: Anchor[], x: number): number {
  const n = sortedByX.length;
  if (n === 0) return 0;
  if (n === 1) return sortedByX[0]!.y;
  if (x <= sortedByX[0]!.x) return sortedByX[0]!.y;
  if (x >= sortedByX[n - 1]!.x) return sortedByX[n - 1]!.y;
  for (let i = 0; i < n - 1; i++) {
    const a = sortedByX[i]!;
    const b = sortedByX[i + 1]!;
    if (x >= a.x && x <= b.x) {
      const prev = i > 0 ? sortedByX[i - 1]! : a;
      const next = i + 2 < n ? sortedByX[i + 2]! : b;
      const span = b.x - a.x || 1;
      const tParam = (x - a.x) / span;
      return catmullRomY(prev.y, a.y, b.y, next.y, tParam);
    }
  }
  return sortedByX[n - 1]!.y;
}

/** Group anchors into lanes by Y gap. Anchors within LANE_GAP of
 *  each other (after sorting by Y) end up in the same lane. */
function clusterByY(anchors: Anchor[]): Anchor[][] {
  if (anchors.length === 0) return [];
  const sorted = [...anchors].sort((a, b) => a.y - b.y);
  const lanes: Anchor[][] = [[sorted[0]!]];
  for (let i = 1; i < sorted.length; i++) {
    const prev = sorted[i - 1]!;
    const curr = sorted[i]!;
    if (curr.y - prev.y < LANE_GAP) {
      lanes[lanes.length - 1]!.push(curr);
    } else {
      lanes.push([curr]);
    }
  }
  return lanes;
}

interface StrandShape {
  d: string;
  edge: string;
  stopValues: { b: number; a: number }[];
  drops: { cx: number; cy: number; r: number; opacity: number }[];
}

/**
 * Build one strand's geometry for spline mode. The centerline is the
 * Catmull-Rom spline through this lane's anchors (sorted by X). On
 * top of that goes a small time-modulated noise, plus width swells
 * and brightness/alpha envelopes for the 3D look.
 */
function buildSplineStrand(
  laneAnchors: Anchor[],
  parentWidth: number,
  baseThickness: number,
  samples: number,
  t: number,
  laneIx: number,
): StrandShape {
  const W = parentWidth;
  const sortedX = [...laneAnchors].sort((a, b) => a.x - b.x);

  // Frequencies (rad/s) for the small noise overlay. Match the
  // existing wave-mode pace so a writer who watches both modes feels
  // the same breathing tempo.
  const omegaY1 = 0.16 + laneIx * 0.04;
  const omegaY2 = 0.27 + laneIx * 0.03;
  const omegaW1 = 0.22 + laneIx * 0.05;
  const omegaB1 = 0.31;
  const omegaA1 = 0.21;
  const tau = (2 * Math.PI) / W;

  const xs: number[] = [];
  const ys: number[] = [];
  const widths: number[] = [];
  const brightness: number[] = [];
  const alpha: number[] = [];

  for (let i = 0; i <= samples; i++) {
    const x = (i / samples) * W;
    const splineY = sampleSplineY(sortedX, x);
    const noiseY =
      SPLINE_NOISE_Y_AMP * Math.sin(tau * x * 2 + omegaY1 * t + laneIx * 0.7) +
      0.5 *
        SPLINE_NOISE_Y_AMP *
        Math.sin(tau * x * 4 + omegaY2 * t + laneIx * 1.3);
    const y = splineY + noiseY;
    const swell =
      0.78 + 0.18 * Math.sin(tau * x * 1.5 + omegaW1 * t + laneIx * 0.4);
    const w = Math.max(10, baseThickness * swell);
    const b =
      0.55 +
      0.35 * Math.sin(tau * x * 1.7 + omegaB1 * t + laneIx * 0.9) +
      0.12 * Math.sin(tau * x * 3.3 + omegaB1 * 1.7 * t);
    const a =
      0.6 +
      0.3 * Math.sin(tau * x * 1.3 + omegaA1 * t + laneIx * 0.5);
    xs.push(x);
    ys.push(y);
    widths.push(w);
    brightness.push(Math.max(0.3, Math.min(1, b)));
    alpha.push(Math.max(0.45, Math.min(1, a)));
  }

  // Top/bot edges via perpendicular tangent.
  const top: { x: number; y: number }[] = [];
  const bot: { x: number; y: number }[] = [];
  for (let i = 0; i <= samples; i++) {
    const prev = i > 0 ? i - 1 : i;
    const next = i < samples ? i + 1 : i;
    const dx = xs[next]! - xs[prev]!;
    const dy = ys[next]! - ys[prev]!;
    const len = Math.hypot(dx, dy) || 1;
    const nx = -dy / len;
    const ny = dx / len;
    const half = widths[i]! * 0.5;
    top.push({ x: xs[i]! + nx * half, y: ys[i]! + ny * half });
    bot.push({ x: xs[i]! - nx * half, y: ys[i]! - ny * half });
  }

  let d = `M ${top[0]!.x} ${top[0]!.y}`;
  for (let i = 1; i < top.length; i++) d += ` L ${top[i]!.x} ${top[i]!.y}`;
  for (let i = bot.length - 1; i >= 0; i--)
    d += ` L ${bot[i]!.x} ${bot[i]!.y}`;
  d += " Z";

  let edge = `M ${top[0]!.x} ${top[0]!.y}`;
  for (let i = 1; i < top.length; i++) edge += ` L ${top[i]!.x} ${top[i]!.y}`;

  // 12-stop gradient samples.
  const STOPS = 12;
  const stopValues: { b: number; a: number }[] = [];
  for (let s = 0; s < STOPS; s++) {
    const start = Math.floor((s / STOPS) * samples);
    const end = Math.floor(((s + 1) / STOPS) * samples);
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

  // Droplets along this strand. Anchored to a few random samples
  // of the strand's own centerline.
  const NUM_DROPS = 22;
  const drops: { cx: number; cy: number; r: number; opacity: number }[] = [];
  for (let i = 0; i < NUM_DROPS; i++) {
    const seedOffset = laneIx * 100;
    const xFrac = prand(i + seedOffset, 1);
    const dx = xFrac * W;
    // Look up the strand's y at this x via the samples we already
    // generated (linear interpolation in xs/ys).
    const sampleIx = Math.min(samples, Math.floor(xFrac * samples));
    const yCenter = ys[sampleIx]!;
    const perpFrac = prand(i + seedOffset, 2) - 0.5;
    const perpScale = 30 + prand(i + seedOffset, 5) * 90;
    const driftPerp =
      10 * Math.sin(0.6 * t + prand(i + seedOffset, 6) * 6.28);
    const cy = yCenter + perpFrac * perpScale + driftPerp;
    const sizeRand = prand(i + seedOffset, 3);
    const r =
      sizeRand < 0.85 ? 0.5 + sizeRand * 1.1 : 1.5 + (sizeRand - 0.85) * 10;
    const opacity =
      (0.25 + prand(i + seedOffset, 4) * 0.5) *
      (0.7 + 0.3 * Math.sin(0.9 * t + prand(i + seedOffset, 7) * 6.28));
    drops.push({ cx: dx, cy, r, opacity });
  }

  return { d, edge, stopValues, drops };
}

/**
 * Wave-mode strand (no anchors). Periodic harmonics — the original
 * "free flow" used by editor, characters, worlds, splash.
 */
function buildWaveStrand(
  parentWidth: number,
  baseY: number,
  baseThickness: number,
  samples: number,
  t: number,
): StrandShape {
  const W = parentWidth;
  const tau = (2 * Math.PI) / W;
  const omegaY1 = 0.13;
  const omegaY2 = 0.21;
  const omegaY3 = 0.34;
  const omegaW1 = 0.18;
  const omegaW2 = 0.28;
  const omegaB1 = 0.24;
  const omegaB2 = 0.41;
  const omegaA1 = 0.16;
  const omegaA2 = 0.31;

  const xs: number[] = [];
  const ys: number[] = [];
  const widths: number[] = [];
  const brightness: number[] = [];
  const alpha: number[] = [];

  const yAt = (x: number) =>
    baseY +
    52 * Math.sin(tau * x + 0.0 + omegaY1 * t) +
    24 * Math.sin(2 * tau * x + 1.05 + omegaY2 * t) +
    14 * Math.sin(3 * tau * x + 0.4 + omegaY3 * t);

  for (let i = 0; i <= samples; i++) {
    const x = (i / samples) * (3 * W);
    const y = yAt(x);
    const swell =
      0.55 +
      0.35 * Math.sin(tau * x + 0.6 + omegaW1 * t) +
      0.18 * Math.sin(3 * tau * x + 1.4 + omegaW2 * t);
    const w = Math.max(8, baseThickness * swell);
    const b = Math.max(
      0.2,
      Math.min(
        1,
        0.5 +
          0.38 * Math.sin(tau * x + 1.7 + omegaB1 * t) +
          0.18 * Math.sin(2 * tau * x + 0.3 + omegaB2 * t),
      ),
    );
    const a = Math.max(
      0.3,
      Math.min(
        1,
        0.55 +
          0.35 * Math.sin(tau * x + 0.9 + omegaA1 * t) +
          0.15 * Math.sin(2 * tau * x + 2.1 + omegaA2 * t),
      ),
    );
    xs.push(x);
    ys.push(y);
    widths.push(w);
    brightness.push(b);
    alpha.push(a);
  }

  const top: { x: number; y: number }[] = [];
  const bot: { x: number; y: number }[] = [];
  for (let i = 0; i <= samples; i++) {
    const prev = i > 0 ? i - 1 : i;
    const next = i < samples ? i + 1 : i;
    const dx = xs[next]! - xs[prev]!;
    const dy = ys[next]! - ys[prev]!;
    const len = Math.hypot(dx, dy) || 1;
    const nx = -dy / len;
    const ny = dx / len;
    const half = widths[i]! * 0.5;
    top.push({ x: xs[i]! + nx * half, y: ys[i]! + ny * half });
    bot.push({ x: xs[i]! - nx * half, y: ys[i]! - ny * half });
  }

  let d = `M ${top[0]!.x} ${top[0]!.y}`;
  for (let i = 1; i < top.length; i++) d += ` L ${top[i]!.x} ${top[i]!.y}`;
  for (let i = bot.length - 1; i >= 0; i--)
    d += ` L ${bot[i]!.x} ${bot[i]!.y}`;
  d += " Z";

  let edge = `M ${top[0]!.x} ${top[0]!.y}`;
  for (let i = 1; i < top.length; i++) edge += ` L ${top[i]!.x} ${top[i]!.y}`;

  const STOPS = 12;
  const stopValues: { b: number; a: number }[] = [];
  for (let s = 0; s < STOPS; s++) {
    const start = Math.floor((s / STOPS) * samples);
    const end = Math.floor(((s + 1) / STOPS) * samples);
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

  const DROPS_PER_PERIOD = 28;
  const drops: { cx: number; cy: number; r: number; opacity: number }[] = [];
  for (let tile = 0; tile < 3; tile++) {
    for (let i = 0; i < DROPS_PER_PERIOD; i++) {
      const xFrac = prand(i, 1);
      const x = xFrac * W + tile * W;
      const perpFrac = prand(i, 2) - 0.5;
      const perpScale = 60 + prand(i, 5) * 100;
      const driftPerp = 12 * Math.sin(0.6 * t + prand(i, 6) * 6.28);
      const cy = yAt(x) + perpFrac * perpScale + driftPerp;
      const sizeRand = prand(i, 3);
      const r =
        sizeRand < 0.85
          ? 0.5 + sizeRand * 1.1
          : 1.5 + (sizeRand - 0.85) * 10;
      const opacity =
        (0.25 + prand(i, 4) * 0.5) *
        (0.7 + 0.3 * Math.sin(0.9 * t + prand(i, 7) * 6.28));
      drops.push({ cx: x, cy, r, opacity });
    }
  }

  return { d, edge, stopValues, drops };
}

export function WaterRibbon({
  parentWidth,
  baseY = 280,
  baseThickness = 56,
  samplesPerPeriod = 96,
  columnMaxWidth = 0,
  zIndex = 0,
  anchors = [],
}: Props) {
  // rAF: forces re-render each frame AND eases displayed anchors
  // toward the prop targets. Targets live in a ref so the loop
  // sees the latest without restarting.
  const [, force] = useReducer((x: number) => x + 1, 0);
  const targetAnchorsRef = useRef<Anchor[]>(anchors);
  const displayedAnchorsRef = useRef<Anchor[]>(anchors);
  // Keep the target ref synced — render-time write is fine because
  // it's reading from props which React already memoized.
  targetAnchorsRef.current = anchors;

  useEffect(() => {
    let raf = 0;
    const loop = () => {
      const targets = targetAnchorsRef.current;
      const current = displayedAnchorsRef.current;
      const byId = new Map(current.map((a) => [a.id, a]));
      const next: Anchor[] = targets.map((t) => {
        const c = byId.get(t.id);
        if (!c) return { ...t }; // new anchor: snap
        return {
          id: t.id,
          x: c.x + (t.x - c.x) * EASE,
          y: c.y + (t.y - c.y) * EASE,
          weight: t.weight,
        };
      });
      displayedAnchorsRef.current = next;
      force();
      raf = requestAnimationFrame(loop);
    };
    raf = requestAnimationFrame(loop);
    return () => cancelAnimationFrame(raf);
  }, []);

  if (parentWidth <= 0) return null;

  const t = performance.now() / 1000;
  const W = parentWidth;
  const displayedAnchors = displayedAnchorsRef.current;

  // Spline mode triggers at 2+ anchors. With 1 or 0, the ribbon
  // doesn't have enough info to build a meaningful skeleton — fall
  // back to the periodic wave so the surface stays alive.
  const splineMode = displayedAnchors.length >= 2;

  const strands: StrandShape[] = [];
  if (splineMode) {
    const lanes = clusterByY(displayedAnchors);
    lanes.forEach((lane, ix) => {
      strands.push(
        buildSplineStrand(lane, W, baseThickness, samplesPerPeriod, t, ix),
      );
    });
  } else {
    strands.push(buildWaveStrand(W, baseY, baseThickness, samplesPerPeriod * 3, t));
  }

  // SVG sizing. Spline mode uses parent width directly (no triple
  // tiling) because the ribbon's domain matches anchor extents.
  // Wave mode uses 3× parentWidth for the loop-tile trick (legacy).
  const svgW = splineMode ? W : W * 3;
  const svgLeft = splineMode ? 0 : -W;
  const ys = splineMode
    ? displayedAnchors.map((a) => a.y)
    : [baseY];
  const yMin = Math.min(...ys);
  const yMax = Math.max(...ys);
  const svgH = Math.max(yMax + 200, baseY + 200);
  // For spline mode the SVG can start at min(0, yMin - 100) so
  // anchors near the top aren't clipped.
  const svgTop = splineMode ? Math.min(0, yMin - 100) : 0;

  // Mask.
  const columnWidth =
    columnMaxWidth > 0
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

  return (
    <div data-testid="water-ribbon" style={wrapperStyle} aria-hidden>
      <svg
        width={svgW}
        height={svgH - svgTop}
        style={{
          position: "absolute",
          left: svgLeft,
          top: svgTop,
          display: "block",
          overflow: "visible",
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
        </defs>
        {strands.map((strand, ix) => (
          <g key={ix}>
            <defs>
              <linearGradient
                id={`wr-grad-${ix}`}
                x1="0"
                y1="0"
                x2="1"
                y2="0"
              >
                {strand.stopValues.map((s, six) => (
                  <stop
                    key={six}
                    offset={`${
                      (six / (strand.stopValues.length - 1)) * 100
                    }%`}
                    stopColor={`color-mix(in oklch, var(--water-sea-300), var(--water-sea-glow) ${Math.round(
                      s.b * 60,
                    )}%)`}
                    stopOpacity={(0.22 + s.b * 0.35) * s.a}
                  />
                ))}
              </linearGradient>
              <linearGradient
                id={`wr-edge-grad-${ix}`}
                x1="0"
                y1="0"
                x2="1"
                y2="0"
              >
                {strand.stopValues.map((s, six) => (
                  <stop
                    key={six}
                    offset={`${
                      (six / (strand.stopValues.length - 1)) * 100
                    }%`}
                    stopColor="var(--water-sea-glow)"
                    stopOpacity={(0.05 + s.b * 0.45) * s.a}
                  />
                ))}
              </linearGradient>
            </defs>
            <path
              d={strand.d}
              fill={`url(#wr-grad-${ix})`}
              opacity={0.6}
              filter="url(#wr-glow-wide)"
            />
            <path
              d={strand.d}
              fill={`url(#wr-grad-${ix})`}
              opacity={0.75}
              filter="url(#wr-glow-mid)"
            />
            <path d={strand.d} fill={`url(#wr-grad-${ix})`} opacity={0.5} />
            <path
              d={strand.edge}
              fill="none"
              stroke={`url(#wr-edge-grad-${ix})`}
              strokeWidth={1.2}
              strokeLinecap="round"
            />
            {strand.drops.map((dr, di) => (
              <g key={di}>
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
          </g>
        ))}
      </svg>
    </div>
  );
}
