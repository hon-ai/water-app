import { useEffect, useRef, useState } from "react";
import type { HeatReadResponse } from "../ipc/commands";
import type { CanvasCard } from "./CanvasSurface";
import { CARD_W, CARD_H } from "./SceneCard";

interface Props {
  cards: CanvasCard[];
  heatPerScene: Record<string, HeatReadResponse["metrics"]>;
  viewportMinX: number;
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
const X_PAD = 280;
const Y_DEFLECT_CAP = 220;
/** Slower morph per the writer's note. 0.018 ≈ 3–4s to converge at
 *  60fps, so the ribbon settles into its new shape gradually rather
 *  than snapping. */
const EASE = 0.018;
const CONV_POS = 0.3;
const CONV_HEAT = 0.003;
/** Split thresholds: any Y range over SPLIT_RANGE with a gap of at
 *  least SPLIT_GAP triggers the strand split. Tuned so scenes in two
 *  adjacent lane rows (140px apart) trigger the split correctly. */
const SPLIT_RANGE = 120;
const SPLIT_GAP = 100;
/** Vertical bounds padding so wide strands and droplets don't get
 *  clipped by the SVG bounding box. */
const Y_PAD = 360;

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

function prand(i: number, salt: number): number {
  const x = Math.sin(i * 9301 + salt * 49297) * 233280;
  return x - Math.floor(x);
}

interface StrandShape {
  d: string;
  edge: string;
  stopValues: { b: number; a: number }[];
  drops: { cx: number; cy: number; r: number; opacity: number }[];
  yAt: (x: number) => number;
}

function buildStrand(
  scenes: DisplayedScene[],
  xMin: number,
  xMax: number,
  baseY: number,
  strandSeed: number,
): StrandShape | null {
  if (scenes.length === 0) return null;

  const xs: number[] = [];
  const ys: number[] = [];
  const widths: number[] = [];
  const brightness: number[] = [];
  const alpha: number[] = [];

  // Closure: y at any x via influence field. Used both for ribbon
  // and droplet placement.
  const yAt = (sx: number) => {
    let totalWeight = 0;
    let yOffsetWeighted = 0;
    for (const s of scenes) {
      const d = Math.hypot(s.x - sx, s.y - baseY);
      if (d > INFLUENCE_R) continue;
      const f = (1 - d / INFLUENCE_R) ** 2;
      totalWeight += f;
      yOffsetWeighted += (s.y - baseY) * f;
    }
    const rawDeflect = totalWeight > 0 ? yOffsetWeighted / totalWeight : 0;
    const deflect = Math.max(-Y_DEFLECT_CAP, Math.min(Y_DEFLECT_CAP, rawDeflect));
    return baseY + deflect;
  };

  for (let i = 0; i <= SAMPLES; i++) {
    const t = i / SAMPLES;
    const sx = xMin + t * (xMax - xMin);

    let widthBump = 0;
    for (const s of scenes) {
      const d = Math.hypot(s.x - sx, s.y - baseY);
      if (d > INFLUENCE_R) continue;
      const f = (1 - d / INFLUENCE_R) ** 2;
      widthBump += s.heat * HEAT_BUMP * f;
    }

    const cy = yAt(sx);
    const taperT = Math.min(1, t * 14, (1 - t) * 14);
    const w = (BASE_W + widthBump) * Math.max(0.25, taperT);

    // Brightness + alpha envelopes — independent harmonics for 3D
    // ribbon feel. Phases tied to strandSeed so the upper and lower
    // strands don't share the same brightness pattern.
    const b =
      0.45 +
      0.4 * Math.sin(t * Math.PI * 2.3 + 0.6 + strandSeed) +
      0.2 * Math.sin(t * Math.PI * 5.5 + 1.7 + strandSeed * 0.5);
    const a =
      0.5 +
      0.4 * Math.sin(t * Math.PI * 1.9 + 0.3 + strandSeed * 0.7) +
      0.18 * Math.sin(t * Math.PI * 4.1 + 1.2 + strandSeed);

    xs.push(sx);
    ys.push(cy);
    widths.push(Math.max(6, w));
    brightness.push(Math.max(0.3, Math.min(1, b)));
    // Alpha floor raised from 0.1 — the old floor let sections fade
    // to nearly-invisible, which combined with the wide-halo
    // gaussian blur made the whole ribbon read as absent on lighter
    // surfaces. Floor of 0.4 keeps the ribbon legible at every point.
    alpha.push(Math.max(0.4, Math.min(1, a)));
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

  // Droplets along this strand. Random x within xMin..xMax, y from
  // the strand's centerline + perpendicular offset. Seeds tied to
  // strandSeed so upper/lower strands have different spray.
  const NUM_DROPS = 28;
  const drops: { cx: number; cy: number; r: number; opacity: number }[] = [];
  for (let i = 0; i < NUM_DROPS; i++) {
    const seedOffset = strandSeed * 100;
    const xFrac = prand(i + seedOffset, 1);
    const dx = xMin + xFrac * (xMax - xMin);
    const yCenter = yAt(dx);
    const perpFrac = prand(i + seedOffset, 2) - 0.5;
    const perpScale = 40 + prand(i + seedOffset, 5) * 100;
    const cy = yCenter + perpFrac * perpScale;
    const sizeRand = prand(i + seedOffset, 3);
    const r =
      sizeRand < 0.85
        ? 0.5 + sizeRand * 1.1
        : 1.5 + (sizeRand - 0.85) * 10;
    const opacity = 0.25 + prand(i + seedOffset, 4) * 0.5;
    drops.push({ cx: dx, cy, r, opacity });
  }

  return { d, edge, stopValues, drops, yAt };
}

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

    if (!initializedRef.current) {
      initializedRef.current = true;
      displayedRef.current = target;
      setDisplayed(target);
      return;
    }

    if (rafRef.current !== null) return;

    const tick = () => {
      const targets = targetRef.current;
      const current = displayedRef.current;
      const byId = new Map(current.map((s) => [s.id, s]));

      let converged = true;
      const next: DisplayedScene[] = targets.map((t) => {
        const c = byId.get(t.id);
        if (!c) return { ...t };
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

  const split = detectSplit(displayed);
  const xMin = viewportMinX - X_PAD;
  const xMax = viewportMaxX + X_PAD;

  let strands: StrandShape[] = [];
  if (split === null) {
    const baseY =
      displayed.reduce((acc, s) => acc + s.y, 0) / displayed.length;
    const s = buildStrand(displayed, xMin, xMax, baseY, 0);
    if (s) strands = [s];
  } else {
    const upper = displayed.filter((s) => s.y < split);
    const lower = displayed.filter((s) => s.y >= split);
    const upperBaseY =
      upper.reduce((acc, s) => acc + s.y, 0) / Math.max(1, upper.length);
    const lowerBaseY =
      lower.reduce((acc, s) => acc + s.y, 0) / Math.max(1, lower.length);
    const u = buildStrand(upper, xMin, xMax, upperBaseY, 0);
    const l = buildStrand(lower, xMin, xMax, lowerBaseY, 1);
    if (u) strands.push(u);
    if (l) strands.push(l);
  }

  if (strands.length === 0) return null;

  const ys = displayed.map((s) => s.y);
  const minStrandY = Math.min(...ys) - Y_PAD;
  const maxStrandY = Math.max(...ys) + Y_PAD;
  const w = xMax - xMin;
  const h = maxStrandY - minStrandY;

  return (
    <svg
      aria-hidden
      data-testid="canvas-flow-ribbon"
      width={w}
      height={h}
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
        <filter id="cf-drop-glow" x="-300%" y="-300%" width="700%" height="700%">
          <feGaussianBlur in="SourceGraphic" stdDeviation="1.4" />
        </filter>
      </defs>
      {strands.map((strand, ix) => (
        // Inner-group animations: each strand wobbles + shimmers with
        // its own period and offset so they don't move in lockstep.
        // The wobble breaks the perceived stillness when scenes
        // aren't moving — the ribbon feels alive on its own.
        <g
          key={ix}
          style={{
            animation: `water-ribbon-wobble ${
              19 + ix * 6
            }s ease-in-out infinite ${ix * -5}s, water-ribbon-shimmer ${
              27 + ix * 4
            }s ease-in-out infinite ${ix * -9}s`,
          }}
        >
          <defs>
            <linearGradient id={`cf-grad-${ix}`} x1="0" y1="0" x2="1" y2="0">
              {strand.stopValues.map((s, six) => (
                <stop
                  key={six}
                  offset={`${(six / (strand.stopValues.length - 1)) * 100}%`}
                  stopColor={
                    s.b > 0.7 ? "var(--water-sea-glow)" : "var(--water-sea-300)"
                  }
                  stopOpacity={(0.3 + s.b * 0.55) * s.a}
                />
              ))}
            </linearGradient>
            <linearGradient id={`cf-edge-grad-${ix}`} x1="0" y1="0" x2="1" y2="0">
              {strand.stopValues.map((s, six) => (
                <stop
                  key={six}
                  offset={`${(six / (strand.stopValues.length - 1)) * 100}%`}
                  stopColor="var(--water-sea-glow)"
                  stopOpacity={(0.0 + s.b * 0.45) * s.a}
                />
              ))}
            </linearGradient>
          </defs>
          <path d={strand.d} fill={`url(#cf-grad-${ix})`} opacity={0.8} filter="url(#cf-glow-wide)" />
          <path d={strand.d} fill={`url(#cf-grad-${ix})`} opacity={0.85} filter="url(#cf-glow-mid)" />
          <path d={strand.d} fill={`url(#cf-grad-${ix})`} opacity={0.65} />
          <path
            d={strand.edge}
            fill="none"
            stroke={`url(#cf-edge-grad-${ix})`}
            strokeWidth={1.1}
            strokeLinecap="round"
          />
          {/* Droplet spray along this strand. */}
          {strand.drops.map((d, di) => (
            <g key={di}>
              <circle
                cx={d.cx}
                cy={d.cy}
                r={d.r * 2.2}
                fill="var(--water-sea-glow)"
                opacity={d.opacity * 0.5}
                filter="url(#cf-drop-glow)"
              />
              <circle
                cx={d.cx}
                cy={d.cy}
                r={d.r}
                fill="var(--water-sea-glow)"
                opacity={d.opacity}
              />
            </g>
          ))}
        </g>
      ))}
    </svg>
  );
}
