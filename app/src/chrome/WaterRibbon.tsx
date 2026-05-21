import { useEffect, useReducer, useRef } from "react";
import type { CSSProperties } from "react";

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
}

/** Kernel bandwidth (px) for centerline smoothing. Larger ⇒ smoother
 *  curve, anchors influence further out. Smaller ⇒ tighter to each
 *  scene but riskier of sharp corners. ~220 reads as a meandering
 *  river that visibly bends through each scene without kinking. */
const KERNEL_SIGMA = 220;
/** Touch-radius (px) for the strand to "touch" a scene. The ribbon's
 *  width near a scene is bumped up so the half-thickness reaches the
 *  scene from whatever Y the smoothed curve passes at. */
const TOUCH_RADIUS = 80;
/** Cluster threshold (px) for detecting X-stacked scenes. Scenes
 *  within this X-distance and outside LANE_Y_TOLERANCE on Y get a
 *  local fork ribbon — only the segment around them splits. */
const X_CLUSTER_DIST = 90;
/** Anchors within this much Y are considered "same lane" — no fork. */
const LANE_Y_TOLERANCE = 60;
/** How wide the fork is around the stacked column. The branch curve
 *  starts diverging this far before the stack X and rejoins this far
 *  after. */
const FORK_FLARE = 140;
/** Anchor-easing factor per rAF frame. ~0.05 ≈ 1s convergence at 60fps. */
const EASE = 0.05;

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
function widthBump(
  anchors: Anchor[],
  x: number,
  centerY: number,
): number {
  if (anchors.length === 0) return 0;
  let bump = 0;
  const sigma2 = 2 * (KERNEL_SIGMA * 0.6) * (KERNEL_SIGMA * 0.6);
  for (const a of anchors) {
    const dx = x - a.x;
    const dy = Math.abs(a.y - centerY);
    // Desired thickness so the half-extent reaches the anchor minus
    // TOUCH_RADIUS slack.
    const need = Math.max(0, dy - TOUCH_RADIUS);
    if (need <= 0) continue;
    const f = Math.exp(-(dx * dx) / sigma2);
    bump = Math.max(bump, 2 * need * f);
  }
  return bump;
}

/** Detect X-clusters of anchors that are stacked vertically. Returns
 *  a list of forks: each entry describes one local split — the X
 *  position and the anchors involved. Non-stacked anchors don't
 *  trigger any forks. */
interface Fork {
  centerX: number;
  members: Anchor[];
}
function detectForks(anchors: Anchor[]): Fork[] {
  if (anchors.length < 2) return [];
  const sortedX = [...anchors].sort((a, b) => a.x - b.x);
  const groups: Anchor[][] = [[sortedX[0]!]];
  for (let i = 1; i < sortedX.length; i++) {
    const prev = sortedX[i - 1]!;
    const curr = sortedX[i]!;
    if (Math.abs(curr.x - prev.x) < X_CLUSTER_DIST) {
      groups[groups.length - 1]!.push(curr);
    } else {
      groups.push([curr]);
    }
  }
  const forks: Fork[] = [];
  for (const g of groups) {
    if (g.length < 2) continue;
    const ys = g.map((a) => a.y);
    const yMin = Math.min(...ys);
    const yMax = Math.max(...ys);
    if (yMax - yMin < LANE_Y_TOLERANCE) continue;
    const meanX = g.reduce((s, a) => s + a.x, 0) / g.length;
    forks.push({ centerX: meanX, members: g });
  }
  return forks;
}

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

/** Filter the anchor list to ones relevant to a specific lane.
 *  Non-fork anchors (i.e., the single-scene flow points) are
 *  relevant to every lane. Fork members are relevant only to the
 *  lane that's assigned to them — otherwise every lane would bump
 *  its width up toward every member of every fork. */
function relevantAnchorsForLane(
  _x: number,
  laneIx: number,
  anchors: Anchor[],
  forks: Fork[],
): Anchor[] {
  const allForkMemberIds = new Set<string>();
  for (const f of forks) {
    for (const m of f.members) allForkMemberIds.add(m.id);
  }
  const myMemberIds = new Set<string>();
  for (const f of forks) {
    const sortedMembers = [...f.members].sort((a, b) => a.y - b.y);
    const member = sortedMembers[laneIx];
    if (member) myMemberIds.add(member.id);
  }
  return anchors.filter((a) => {
    if (!allForkMemberIds.has(a.id)) return true;
    return myMemberIds.has(a.id);
  });
}

/** Compute lane-specific Y at sample x. Outside any fork, every lane
 *  collapses to the kernel-smoothed centerline (so multiple lanes
 *  overlap visually as one ribbon). At each fork, this lane bends
 *  smoothly toward its assigned member's Y across the FORK_FLARE
 *  region — like a river dividing around an island and merging.
 *
 *  Lane assignments are deterministic: members of each fork are
 *  sorted top-to-bottom by Y, and lane `laneIx` takes the member
 *  at that index (or null if the fork has fewer members than lanes).
 *  When this lane has no member in a given fork, it stays at the
 *  smoothed centerline through that fork. */
function laneY(
  x: number,
  laneIx: number,
  forks: Fork[],
  anchors: Anchor[],
  fallbackY: number,
): number {
  const base = kernelY(anchors, x, fallbackY);
  let y = base;
  for (const f of forks) {
    const sortedMembers = [...f.members].sort((a, b) => a.y - b.y);
    const member = sortedMembers[laneIx];
    if (!member) continue;
    const dx = x - f.centerX;
    if (Math.abs(dx) > FORK_FLARE) continue;
    // Cosine envelope: 1 at fork center, 0 at +/- FORK_FLARE. Smooth
    // C1-continuous deflection so the lane fans out and reconverges
    // without any visible kink.
    const env = Math.cos((dx / FORK_FLARE) * (Math.PI / 2));
    y = y + (member.y - base) * env;
  }
  return y;
}

/** Build one lane's strand. Multiple lanes overlap visually outside
 *  fork regions (all at kernelY) and split apart inside forks where
 *  they each pull toward their assigned member. */
function buildLaneStrand(
  laneIx: number,
  anchors: Anchor[],
  forks: Fork[],
  parentWidth: number,
  baseY: number,
  baseThickness: number,
  samples: number,
  t: number,
): StrandShape {
  const W = parentWidth;
  const tau = (2 * Math.PI) / W;
  const omegaY1 = 0.16;
  const omegaY2 = 0.27;
  const omegaW1 = 0.22;
  const omegaW2 = 0.39;
  const omegaB1 = 0.31;
  const omegaA1 = 0.21;

  const xs: number[] = [];
  const ys: number[] = [];
  const widths: number[] = [];
  const brightness: number[] = [];
  const alpha: number[] = [];

  // Per-lane phase offset so multiple lanes (when they overlap
  // outside forks) don't move in perfect lockstep. Tiny offset
  // keeps each lane's noise a touch out of phase from siblings.
  const laneOffset = laneIx * 0.6;

  for (let i = 0; i <= samples; i++) {
    const x = (i / samples) * W;
    // Lane-specific centerline: kernel-smoothed flow + per-fork
    // deflection toward this lane's assigned member.
    const center = laneY(x, laneIx, forks, anchors, baseY);
    // Noise overlay — subordinate to the centerline but not uniform.
    const noiseY =
      14 * Math.sin(tau * x * 1.6 + omegaY1 * t + laneOffset) +
      8 * Math.sin(tau * x * 3.7 + omegaY2 * t + 1.2 + laneOffset);
    const y = center + noiseY;
    const swell =
      0.55 +
      0.35 * Math.sin(tau * x * 1.4 + omegaW1 * t + laneOffset * 0.4) +
      0.22 * Math.sin(tau * x * 3.8 + omegaW2 * t + 0.6 + laneOffset);
    // Touch bump only for anchors that belong to this lane's
    // trajectory. For non-fork anchors, all lanes share them; for
    // fork members, only the assigned lane's member contributes.
    const laneAnchors = relevantAnchorsForLane(
      x,
      laneIx,
      anchors,
      forks,
    );
    const bump = laneAnchors.length > 0
      ? widthBump(laneAnchors, x, center)
      : 0;
    const w = Math.max(8, baseThickness * swell + bump);
    const b =
      0.5 +
      0.34 * Math.sin(tau * x * 1.7 + omegaB1 * t + laneOffset * 0.7) +
      0.18 * Math.sin(tau * x * 3.3 + omegaB1 * 1.6 * t);
    const a =
      0.6 +
      0.25 * Math.sin(tau * x * 1.3 + omegaA1 * t + laneOffset * 0.5);
    xs.push(x);
    ys.push(y);
    widths.push(w);
    brightness.push(Math.max(0.3, Math.min(1, b)));
    alpha.push(Math.max(0.45, Math.min(1, a)));
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

  // Droplets only on the first lane (laneIx === 0) so multiple
  // overlapping lanes don't multiply the spray density. Lifecycle
  // envelope (fade in / hold / fade out) carries the organic feel.
  const drops: StrandShape["drops"] = [];
  if (laneIx === 0) {
    const NUM_DROPS = 26;
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
  }

  return { d, edge, stopValues, drops };
}

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
  for (let i = bot.length - 1; i >= 0; i--) d += ` L ${bot[i]!.x} ${bot[i]!.y}`;
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

  // Wave-mode droplets — same lifecycle as main strand.
  const DROPS_PER_PERIOD = 26;
  const drops: StrandShape["drops"] = [];
  for (let tile = 0; tile < 3; tile++) {
    for (let i = 0; i < DROPS_PER_PERIOD; i++) {
      const xFrac = prand(i, 1);
      const x = xFrac * W + tile * W;
      const perpFrac = prand(i, 2) - 0.5;
      const perpScale = 60 + prand(i, 5) * 100;
      const cy = yAt(x) + perpFrac * perpScale;
      const sizeRand = prand(i, 3);
      const r =
        sizeRand < 0.88 ? 0.4 + sizeRand * 1.0 : 1.3 + (sizeRand - 0.88) * 8;
      const opacityBase = 0.16 + prand(i, 4) * 0.24;
      const lifetime = 6 + prand(i, 8) * 10;
      const birthOffset = prand(i, 9) * lifetime;
      drops.push({ cx: x, cy, r, opacityBase, birthOffset, lifetime });
    }
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
}: Props) {
  const [, force] = useReducer((x: number) => x + 1, 0);
  const targetAnchorsRef = useRef<Anchor[]>(anchors);
  const displayedAnchorsRef = useRef<Anchor[]>(anchors);
  targetAnchorsRef.current = anchors;

  useEffect(() => {
    let raf = 0;
    const loop = () => {
      const targets = targetAnchorsRef.current;
      const current = displayedAnchorsRef.current;
      const byId = new Map(current.map((a) => [a.id, a]));
      const next: Anchor[] = targets.map((t) => {
        const c = byId.get(t.id);
        if (!c) return { ...t };
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

  // ONE river always. anchors >= 1 enters main-strand mode. With 0
  // anchors fall back to the periodic wave.
  const mainMode = displayedAnchors.length >= 1;
  const strands: StrandShape[] = [];

  if (mainMode) {
    // Number of lanes equals the largest fork's member count, or 1
    // if there are no forks. Each lane renders as a full-length
    // strand: outside forks it coincides with the kernel-smoothed
    // centerline (so multiple lanes overlap visually as ONE river);
    // inside a fork it bends toward its assigned member and
    // smoothly rejoins. This is what makes the river "divide and
    // reunite" at stack columns instead of growing tributaries.
    const forks = detectForks(displayedAnchors);
    const laneCount =
      forks.length === 0
        ? 1
        : Math.max(...forks.map((f) => f.members.length));
    for (let lane = 0; lane < laneCount; lane++) {
      strands.push(
        buildLaneStrand(
          lane,
          displayedAnchors,
          forks,
          W,
          baseY,
          baseThickness,
          samplesPerPeriod,
          t,
        ),
      );
    }
  } else {
    strands.push(
      buildWaveStrand(W, baseY, baseThickness, samplesPerPeriod * 3, t),
    );
  }

  const svgW = mainMode ? W : W * 3;
  const svgLeft = mainMode ? 0 : -W;
  const ys = mainMode
    ? displayedAnchors.map((a) => a.y)
    : [baseY];
  const yMin = ys.length > 0 ? Math.min(...ys) : baseY;
  const yMax = ys.length > 0 ? Math.max(...ys) : baseY;
  const svgTop = mainMode ? Math.min(0, yMin - 200) : 0;
  const svgH = Math.max(yMax + 200, baseY + 200) - svgTop;

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
        {/* All lanes render as full-length strands. Outside forks
            their centerlines coincide so the polygons overlap and
            read as one ribbon; inside forks they smoothly diverge. */}
        {strands.map((s, ix) => renderStrand(s, ix, ix === 0))}
      </svg>
    </div>
  );
}
