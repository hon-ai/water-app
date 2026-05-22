import { useEffect, useReducer } from "react";
import type { CSSProperties } from "react";
import {
  getRibbonDisplayed,
  setRibbonTarget,
  subscribeRibbon,
} from "./ribbonState";

export interface Anchor {
  /** Stable id used to ease this anchor's displayed position as the
   *  prop target changes. */
  id: string;
  /** Parent-x (CSS px from the left edge of the WaterRibbon's host). */
  x: number;
  /** Parent-y. The ribbon's centerline gets pulled toward this point
   *  with a gaussian kernel; the strand "touches" the scene without
   *  being forced through its center, so sharp corners are avoided. */
  y: number;
  /** Optional weight. Defaults to 1. */
  weight?: number;
}

interface Props {
  parentWidth: number;
  baseY?: number;
  baseThickness?: number;
  samplesPerPeriod?: number;
  columnMaxWidth?: number;
  zIndex?: number;
  anchors?: Anchor[];
  /**
   * Visual extent of the stream. "open" (default): the ribbon spans
   * the full parent width. "narrow": clipped to roughly the left
   * ~55%, which matches the writing-text column margins on the
   * "Begin a scene" page. CSS transitions on mask-size animate
   * between modes when the prop changes — the stream "gradually
   * expands" as the writer moves between pages.
   */
  streamMode?: "open" | "narrow";
}

/** Kernel bandwidth (px) for centerline smoothing. Larger ⇒ smoother
 *  curve, anchors influence further out. */
const KERNEL_SIGMA = 200;
/** Touch-radius (px) for the strand to "touch" a scene — the ribbon's
 *  half-width gets local bumps so it visibly reaches each scene. */
const TOUCH_RADIUS = 60;

function prand(i: number, salt: number): number {
  const x = Math.sin(i * 9301 + salt * 49297) * 233280;
  return x - Math.floor(x);
}

/** smoothstep(0, 1) — used for fade envelopes on droplet lifetimes. */
function smoothstep(edge0: number, edge1: number, x: number): number {
  const t = Math.max(0, Math.min(1, (x - edge0) / (edge1 - edge0)));
  return t * t * (3 - 2 * t);
}

/** Kernel-smoothed Y at a given X. Each anchor contributes a gaussian
 *  weight. Anchors that the curve has to bend "least" toward (close
 *  to existing flow) end up dominating; the rest pull gently. */
function kernelY(
  anchors: Anchor[],
  x: number,
  fallbackY: number,
): number {
  if (anchors.length === 0) return fallbackY;
  let wsum = 0.04; // baseline weight so far-from-any-anchor regions don't drift
  let ysum = 0.04 * fallbackY;
  const sigma2 = 2 * KERNEL_SIGMA * KERNEL_SIGMA;
  for (const a of anchors) {
    const dx = x - a.x;
    const w = Math.exp(-(dx * dx) / sigma2) * (a.weight ?? 1);
    wsum += w;
    ysum += w * a.y;
  }
  return ysum / wsum;
}

/** Local width bump near a scene so the strand visibly touches it.
 *  At the anchor's X the bump pushes the half-thickness to within
 *  TOUCH_RADIUS of the anchor's Y; falls off gaussian. */
/** Cap on per-anchor width inflation. Without this an anchor far
 *  from the centerline asked for ribbon-thickness several hundred
 *  px wide, which read as a "blob" — the funky physics flagged by
 *  the writer. Now any individual anchor contributes at most
 *  ~MAX_BUMP px to the local width. */
const MAX_BUMP = 160;

function widthBump(
  anchors: Anchor[],
  x: number,
  centerY: number,
): number {
  if (anchors.length === 0) return 0;
  let bump = 0;
  // Wider sigma than the centerline kernel: smoother width
  // transitions between anchors, less peaky bubbling. KERNEL_SIGMA
  // * 0.85 keeps the bump localized but soft.
  const sigma2 = 2 * (KERNEL_SIGMA * 0.85) * (KERNEL_SIGMA * 0.85);
  for (const a of anchors) {
    const weight = a.weight ?? 1;
    if (weight <= 0) continue;
    const dx = x - a.x;
    const dy = Math.abs(a.y - centerY);
    // need: how much thickness is required to reach this anchor.
    // Capped + softened with a sqrt curve so a very-far anchor
    // doesn't drive an absurd local swell.
    const rawNeed = Math.max(0, dy - TOUCH_RADIUS);
    const need = Math.min(MAX_BUMP * 0.5, Math.sqrt(rawNeed * 14));
    if (need <= 0) continue;
    const f = Math.exp(-(dx * dx) / sigma2) * weight;
    bump = Math.max(bump, Math.min(MAX_BUMP, 2 * need * f));
  }
  return bump;
}

/** Detect X-clusters of anchors that are stacked vertically. Returns
 *  a list of forks: each entry describes one local split — the X
 *  position and the anchors involved. Non-stacked anchors don't
 *  trigger any forks. */

interface StrandShape {
  d: string;
  edge: string;
  stopValues: { b: number; a: number }[];
  drops: {
    cx: number;
    cy: number;
    r: number;
    /** Final opacity is `opacity * lifecycleEnvelope(t)`. */
    opacityBase: number;
    /** Birth time offset (seconds) within the dot's own lifetime. */
    birthOffset: number;
    /** Total lifetime (seconds). Each dot has its own randomly. */
    lifetime: number;
  }[];
}

/** Unified single-ribbon strand. Always uses kernel smoothing for the
 *  centerline; noise amplitude blends with `ambientFactor` — the
 *  inverse of total anchor weight. When scenes are present, the
 *  scene skeleton dominates and noise is subordinate; when anchors
 *  fade out (surface change, scene deleted), ambient grows to
 *  full wave-mode amplitude. One shape, no mode switch. */
function buildStrand(
  anchors: Anchor[],
  parentWidth: number,
  baseY: number,
  baseThickness: number,
  samples: number,
  t: number,
): StrandShape {
  const W = parentWidth;
  const tau = (2 * Math.PI) / W;
  // Negative omega → wave appears to drift RIGHTWARD (forward) over
  // time. Positive would advance the phase, dragging the visible
  // wave pattern leftward — which the writer flagged as feeling
  // like backflow on the splash page. A stream "moves forward."
  const omegaY1 = -0.16;
  const omegaY2 = -0.27;
  const omegaY3 = -0.34;
  const omegaW1 = -0.22;
  const omegaW2 = -0.39;
  const omegaB1 = -0.31;
  const omegaA1 = -0.21;

  // Total active anchor weight drives the scene-vs-ambient blend.
  // 0 weight → full ambient (looks like a free-flowing wave). High
  // weight → scene-driven (small subordinate noise on top of the
  // kernel-smoothed skeleton).
  const totalWeight = anchors.reduce(
    (s, a) => s + (a.weight ?? 1),
    0,
  );
  const ambientFactor = Math.exp(-totalWeight * 0.35);
  // Noise amplitudes blend between scene-mode (14, 8, 0) and ambient
  // (52, 24, 14) — the latter matches the old wave-mode shape.
  const noiseAmpY1 = 14 + 38 * ambientFactor;
  const noiseAmpY2 = 8 + 16 * ambientFactor;
  const noiseAmpY3 = 14 * ambientFactor;

  const xs: number[] = [];
  const ys: number[] = [];
  const widths: number[] = [];
  const brightness: number[] = [];
  const alpha: number[] = [];

  for (let i = 0; i <= samples; i++) {
    const x = (i / samples) * W;
    const center = kernelY(anchors, x, baseY);
    const noiseY =
      noiseAmpY1 * Math.sin(tau * x * 1.6 + omegaY1 * t) +
      noiseAmpY2 * Math.sin(tau * x * 3.7 + omegaY2 * t + 1.2) +
      noiseAmpY3 * Math.sin(tau * x + 0.4 + omegaY3 * t);
    const y = center + noiseY;
    const swell =
      0.55 +
      0.35 * Math.sin(tau * x * 1.4 + omegaW1 * t) +
      0.22 * Math.sin(tau * x * 3.8 + omegaW2 * t + 0.6);
    // Width stays constant regardless of anchor count / vertical
    // stacking. Earlier we used `widthBump(anchors, x, center)` to
    // swell the strand near anchors, but with multiple scenes
    // stacked in a column the swells overlapped + read as a
    // clunky bubble. Bending (via `kernelY` above) still tracks
    // scene positions; only the *thickness* now ignores them.
    const w = Math.max(8, baseThickness * swell);
    const b =
      0.5 +
      0.34 * Math.sin(tau * x * 1.7 + omegaB1 * t) +
      0.18 * Math.sin(tau * x * 3.3 + omegaB1 * 1.6 * t);
    const a = 0.6 + 0.25 * Math.sin(tau * x * 1.3 + omegaA1 * t);
    xs.push(x);
    ys.push(y);
    widths.push(w);
    brightness.push(Math.max(0.3, Math.min(1, b)));
    alpha.push(Math.max(0.45, Math.min(1, a)));
  }

  // Low-pass smoothing on the widths array. A 5-tap symmetric filter
  // applied twice flattens any sharp bump-discontinuities (the cause
  // of the "funky bubbling" the writer flagged). The y-centerline
  // already comes out of a wide gaussian kernel and doesn't need
  // extra smoothing.
  for (let pass = 0; pass < 2; pass++) {
    const src = [...widths];
    for (let i = 0; i <= samples; i++) {
      const i0 = src[Math.max(0, i - 2)] ?? src[i]!;
      const i1 = src[Math.max(0, i - 1)] ?? src[i]!;
      const i2 = src[i]!;
      const i3 = src[Math.min(samples, i + 1)] ?? src[i]!;
      const i4 = src[Math.min(samples, i + 2)] ?? src[i]!;
      widths[i] = (i0 + 2 * i1 + 4 * i2 + 2 * i3 + i4) / 10;
    }
  }

  // Edges + path.
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

  // Droplets along the strand with the lifecycle envelope (fade in /
  // hold / fade out) for organic spray.
  const NUM_DROPS = 26;
  const drops: StrandShape["drops"] = [];
  for (let i = 0; i < NUM_DROPS; i++) {
    const xFrac = prand(i, 1);
    const dx = xFrac * W;
    const sampleIx = Math.min(samples, Math.floor(xFrac * samples));
    const yCenter = ys[sampleIx]!;
    const perpFrac = prand(i, 2) - 0.5;
    const perpScale = 30 + prand(i, 5) * 100;
    const cy = yCenter + perpFrac * perpScale;
    const sizeRand = prand(i, 3);
    const r =
      sizeRand < 0.88 ? 0.4 + sizeRand * 1.0 : 1.3 + (sizeRand - 0.88) * 8;
    const opacityBase = 0.16 + prand(i, 4) * 0.24;
    const lifetime = 6 + prand(i, 8) * 10;
    const birthOffset = prand(i, 9) * lifetime;
    drops.push({ cx: dx, cy, r, opacityBase, birthOffset, lifetime });
  }

  return { d, edge, stopValues, drops };
}

/** Opacity envelope for a droplet given its lifetime + birth offset.
 *  Fades in over the first 20% of life, holds, fades out over the
 *  last 20%. After death loops to a new cycle. */
function lifecycleEnv(t: number, birthOffset: number, lifetime: number): number {
  const phase = ((t + birthOffset) % lifetime) / lifetime;
  // Fade in 0..0.2, hold 0.2..0.8, fade out 0.8..1
  return smoothstep(0, 0.2, phase) * (1 - smoothstep(0.8, 1, phase));
}

export function WaterRibbon({
  parentWidth,
  baseY = 280,
  baseThickness = 56,
  samplesPerPeriod = 96,
  columnMaxWidth = 0,
  zIndex = 0,
  anchors = [],
  streamMode = "open",
}: Props) {
  // Sync the module store's target with this instance's anchors prop.
  // Each WaterRibbon instance writes the same target; the module's
  // single rAF loop drives easing for whichever instance is currently
  // mounted. Across surface unmount/remount cycles the *displayed*
  // anchors persist — no blink, no phase reset.
  useEffect(() => {
    setRibbonTarget(anchors);
  }, [anchors]);
  // On final unmount, clear the target so subsequent surfaces without
  // anchors don't inherit stale scene anchors.
  useEffect(() => () => setRibbonTarget([]), []);

  // Per-frame re-render driven by the module clock.
  const [, force] = useReducer((x: number) => x + 1, 0);
  useEffect(() => subscribeRibbon(force), []);

  if (parentWidth <= 0) return null;

  const t = performance.now() / 1000;
  // Extend the drawing surface past the viewport on both sides so
  // the strand's start + end live outside the visible bounds. The
  // wrapper below is shifted -RIBBON_BLEED on left + extends
  // +RIBBON_BLEED past right so the geometry lines up; anchors
  // (which are in window-space) get a +RIBBON_BLEED offset before
  // being fed to `buildStrand`.
  const RIBBON_BLEED_DRAW = 240;
  const W = parentWidth + 2 * RIBBON_BLEED_DRAW;
  const displayedAnchors = getRibbonDisplayed().map((a) => ({
    ...a,
    x: a.x + RIBBON_BLEED_DRAW,
  }));

  // One unified strand always — kernel smoothing for scene-driven
  // shape, blended with ambient noise based on total weight. When
  // weights are 0 (no scenes / scenes faded out), strand looks like
  // the free-flowing wave; when weights are 1, scenes drive the curve.
  const strand = buildStrand(
    displayedAnchors,
    W,
    baseY,
    baseThickness,
    samplesPerPeriod,
    t,
  );

  const svgW = W;
  const svgLeft = 0;
  const ys = displayedAnchors.length > 0
    ? displayedAnchors.map((a) => a.y)
    : [baseY];
  const yMin = Math.min(...ys);
  const yMax = Math.max(...ys);
  const svgTop = Math.min(0, yMin - 200);
  const svgH = Math.max(yMax + 200, baseY + 200) - svgTop;

  const columnWidth =
    columnMaxWidth > 0
      ? Math.min(columnMaxWidth, Math.max(0, parentWidth - 48))
      : 0;
  // Mask coords live in the *expanded* surface (which is shifted
  // -RIBBON_BLEED_DRAW relative to the viewport). The column is
  // centered against the viewport, so its left/right in surface
  // coords gets the bleed offset added back.
  const columnLeft =
    columnWidth > 0
      ? RIBBON_BLEED_DRAW + (parentWidth - columnWidth) / 2
      : 0;
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
          black ${W}px
        )`
      : undefined;

  // Mask geometry. Three cases compose:
  //   - columnMaxWidth > 0: cut the central text column (editor surface)
  //   - streamMode === "narrow": clip the visible region to the left
  //     ~55% so the stream tucks against the centered text on the
  //     Begin-a-scene page.
  //   - "open": no horizontal clipping; mask-size 100% × 100%.
  // Transitioning between modes works because mask-size is animatable —
  // a CSS transition on it makes the visible band grow / shrink.
  const useNarrow = streamMode === "narrow";
  const narrowOrColumn = useNarrow ? undefined : maskImage;
  // The narrow mask uses a single full-extent gradient; the visible
  // extent is then driven by mask-size, which CSS can transition.
  const narrowGradient =
    "linear-gradient(90deg, black 0%, black 88%, transparent 100%)";

  // Extend the wrapper past the visible viewport on both sides so
  // the strand's geometric edges live outside the visible bounds.
  // The drawn SVG width below is bumped by the same amount; together
  // they keep the cutoff invisible to the writer at any window size.
  const RIBBON_BLEED = 240;

  const wrapperStyle: CSSProperties = {
    position: "absolute",
    top: 0,
    bottom: 0,
    left: -RIBBON_BLEED,
    right: -RIBBON_BLEED,
    pointerEvents: "none",
    overflow: "hidden",
    zIndex,
    transition:
      "mask-size var(--water-dur-medium) var(--water-ease-in-out-water), -webkit-mask-size var(--water-dur-medium) var(--water-ease-in-out-water)",
    ...(useNarrow
      ? {
          maskImage: narrowGradient,
          WebkitMaskImage: narrowGradient,
          maskRepeat: "no-repeat",
          WebkitMaskRepeat: "no-repeat",
          maskPosition: "0 0",
          WebkitMaskPosition: "0 0",
          maskSize: "60% 100%",
          WebkitMaskSize: "60% 100%",
        }
      : narrowOrColumn
        ? {
            maskImage: narrowOrColumn,
            WebkitMaskImage: narrowOrColumn,
          }
        : {
            // Open: fully visible. Set explicit mask-size 100% so the
            // CSS transition has a stable target if the writer
            // navigates back to a narrow page later.
            maskSize: "100% 100%",
            WebkitMaskSize: "100% 100%",
          }),
  };

  const renderStrand = (strand: StrandShape, ix: number, dropsEnabled: boolean) => (
    <g key={ix}>
      <defs>
        <linearGradient id={`wr-grad-${ix}`} x1="0" y1="0" x2="1" y2="0">
          {strand.stopValues.map((s, six) => (
            <stop
              key={six}
              offset={`${(six / (strand.stopValues.length - 1)) * 100}%`}
              stopColor={`color-mix(in oklch, var(--water-sea-300), var(--water-sea-glow) ${Math.round(
                s.b * 60,
              )}%)`}
              stopOpacity={(0.22 + s.b * 0.35) * s.a}
            />
          ))}
        </linearGradient>
        <linearGradient id={`wr-edge-grad-${ix}`} x1="0" y1="0" x2="1" y2="0">
          {strand.stopValues.map((s, six) => (
            <stop
              key={six}
              offset={`${(six / (strand.stopValues.length - 1)) * 100}%`}
              stopColor="var(--water-sea-glow)"
              stopOpacity={(0.05 + s.b * 0.45) * s.a}
            />
          ))}
        </linearGradient>
      </defs>
      <path d={strand.d} fill={`url(#wr-grad-${ix})`} opacity={0.6} filter="url(#wr-glow-wide)" />
      <path d={strand.d} fill={`url(#wr-grad-${ix})`} opacity={0.75} filter="url(#wr-glow-mid)" />
      <path d={strand.d} fill={`url(#wr-grad-${ix})`} opacity={0.5} />
      <path
        d={strand.edge}
        fill="none"
        stroke={`url(#wr-edge-grad-${ix})`}
        strokeWidth={1.2}
        strokeLinecap="round"
      />
      {dropsEnabled &&
        strand.drops.map((dr, di) => {
          const env = lifecycleEnv(t, dr.birthOffset, dr.lifetime);
          const finalOpacity = dr.opacityBase * env;
          if (finalOpacity < 0.005) return null;
          return (
            <g key={di}>
              <circle
                cx={dr.cx}
                cy={dr.cy}
                r={dr.r * 2.2}
                fill="var(--water-sea-glow)"
                opacity={finalOpacity * 0.5}
                filter="url(#wr-drop-glow)"
              />
              <circle
                cx={dr.cx}
                cy={dr.cy}
                r={dr.r}
                fill="var(--water-sea-glow)"
                opacity={finalOpacity}
              />
            </g>
          );
        })}
    </g>
  );

  return (
    <div data-testid="water-ribbon" style={wrapperStyle} aria-hidden>
      <svg
        width={svgW}
        height={svgH}
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
        {/* One unified strand. */}
        {renderStrand(strand, 0, true)}
      </svg>
    </div>
  );
}
