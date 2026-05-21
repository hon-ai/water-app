import { useMemo } from "react";
import type { CSSProperties } from "react";

interface Props {
  parentWidth: number;
  columnMaxWidth?: number;
  /** Y of the wave's resting line. */
  baseY?: number;
  /** Base thickness; modulated by periodic harmonics. */
  baseThickness?: number;
  /** Samples along one viewport-wide period. The SVG renders three
   *  periods to fill 3×parentWidth. */
  samplesPerPeriod?: number;
}

/**
 * Pseudo-random in [0, 1) for a given (i, salt) pair. Deterministic
 * so droplet positions don't churn on re-render.
 */
function prand(i: number, salt: number): number {
  const x = Math.sin(i * 9301 + salt * 49297) * 233280;
  return x - Math.floor(x);
}

/**
 * Ambient water-stream ribbon behind the editor canvas.
 *
 * Periodic across the parent's width — y, width, brightness, and
 * alpha are all sums of harmonics of period `parentWidth`. The SVG
 * is laid out 3×parentWidth wide at left=-parentWidth, so
 * translateX(-33.33%) shifts the SVG by exactly one period — and
 * because every quantity is periodic with that period, the loop is
 * visually seamless: no retract, no hard reset.
 *
 * 3D character comes from:
 *  - Variable width along the path (two overlapping harmonics).
 *  - Variable brightness (independent harmonics, different phases).
 *  - Variable alpha (third independent harmonic set) so some
 *    sections feel near-transparent and others more solid — gives
 *    the ribbon the look of a turning, light-catching surface.
 *  - A top-edge highlight stroke that picks up the brightness
 *    envelope — the "lit edge" of a 3D ribbon.
 *  - Tiled water-droplet particles around the path with their own
 *    glow, variable sizes, scattered perpendicular offsets — like
 *    spray catching ambient light.
 *
 * The mask occludes the central markdown column with absolute-pixel
 * geometry so the ribbon's visible shoulders track the actual column
 * at every window size.
 */
export function WaterRibbon({
  parentWidth,
  columnMaxWidth = 720,
  baseY = 280,
  baseThickness = 56,
  samplesPerPeriod = 96,
}: Props) {
  const columnWidth = Math.min(columnMaxWidth, Math.max(0, parentWidth - 48));
  const columnLeft = Math.max(0, (parentWidth - columnWidth) / 2);
  const columnRight = columnLeft + columnWidth;
  const SOFT = 48;

  const shape = useMemo(() => {
    if (parentWidth <= 0) return null;
    const W = parentWidth;
    const SAMPLES = samplesPerPeriod * 3;
    const tau = (2 * Math.PI) / W;

    const xs: number[] = [];
    const ys: number[] = [];
    const widths: number[] = [];
    const brightness: number[] = [];
    const alpha: number[] = [];

    // Closed-form helpers so we can sample y at arbitrary x for
    // droplet placement later.
    const yAt = (x: number) =>
      baseY +
      52 * Math.sin(tau * x + 0.0) +
      24 * Math.sin(2 * tau * x + 1.05) +
      14 * Math.sin(3 * tau * x + 0.4);
    const widthAt = (x: number) => {
      const swell =
        0.55 +
        0.35 * Math.sin(tau * x + 0.6) +
        0.18 * Math.sin(3 * tau * x + 1.4);
      return Math.max(8, baseThickness * swell);
    };
    const brightAt = (x: number) =>
      Math.max(
        0.2,
        Math.min(
          1,
          0.5 +
            0.38 * Math.sin(tau * x + 1.7) +
            0.18 * Math.sin(2 * tau * x + 0.3),
        ),
      );
    const alphaAt = (x: number) =>
      Math.max(
        0.1,
        Math.min(
          1,
          0.45 +
            0.4 * Math.sin(tau * x + 0.9) +
            0.18 * Math.sin(2 * tau * x + 2.1),
        ),
      );

    for (let i = 0; i <= SAMPLES; i++) {
      const x = (i / SAMPLES) * (3 * W); // span [0, 3W] matching SVG coords
      xs.push(x);
      ys.push(yAt(x));
      widths.push(widthAt(x));
      brightness.push(brightAt(x));
      alpha.push(alphaAt(x));
    }

    // Top/bot edges via perpendicular tangent.
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

    // 12-stop gradient. Each stop carries both brightness color and
    // alpha — the alpha modulation is what gives the 3D semi-
    // transparent feel along the length.
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

    // Droplets — N per period, tiled 3 times across the SVG so the
    // pattern is periodic. Each droplet is anchored to the ribbon's
    // y(x) plus a perpendicular offset; size, opacity, and glow
    // strength vary per droplet to suggest spray scale.
    const DROPS_PER_PERIOD = 30;
    const drops: { cx: number; cy: number; r: number; opacity: number }[] = [];
    for (let tile = 0; tile < 3; tile++) {
      for (let i = 0; i < DROPS_PER_PERIOD; i++) {
        const xFrac = prand(i, 1);
        const x = xFrac * W + tile * W;
        const perpFrac = prand(i, 2) - 0.5;
        const perpScale = 60 + prand(i, 5) * 100; // 60..160 px spread
        const yWave = yAt(x);
        const cy = yWave + perpFrac * perpScale;
        // Most droplets tiny (0.4..1.5px), a few larger splashes (up to ~3px).
        const sizeRand = prand(i, 3);
        const r =
          sizeRand < 0.85
            ? 0.4 + sizeRand * 1.1
            : 1.5 + (sizeRand - 0.85) * 10;
        const opacity = 0.25 + prand(i, 4) * 0.5;
        drops.push({ cx: x, cy, r, opacity });
      }
    }

    return { d, edge, stopValues, drops };
  }, [parentWidth, baseY, baseThickness, samplesPerPeriod]);

  if (!shape) return null;

  const VB_H = baseY + 200;
  const wrapperStyle: CSSProperties = {
    position: "absolute",
    inset: 0,
    pointerEvents: "none",
    overflow: "hidden",
    zIndex: 0,
    maskImage: `linear-gradient(
      90deg,
      black 0px,
      black ${columnLeft - SOFT}px,
      transparent ${columnLeft}px,
      transparent ${columnRight}px,
      black ${columnRight + SOFT}px,
      black ${parentWidth}px
    )`,
    WebkitMaskImage: `linear-gradient(
      90deg,
      black 0px,
      black ${columnLeft - SOFT}px,
      transparent ${columnLeft}px,
      transparent ${columnRight}px,
      black ${columnRight + SOFT}px,
      black ${parentWidth}px
    )`,
  };

  return (
    <div data-testid="water-ribbon" style={wrapperStyle} aria-hidden>
      {/* PRIMARY ribbon — full opacity, 48s drift. */}
      <svg
        width={parentWidth * 3}
        height={VB_H}
        style={{
          position: "absolute",
          left: -parentWidth,
          top: 0,
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
          <filter id="wr-drop-glow" x="-300%" y="-300%" width="700%" height="700%">
            <feGaussianBlur in="SourceGraphic" stdDeviation="1.5" />
          </filter>
          <linearGradient id="wr-grad" x1="0" y1="0" x2="1" y2="0">
            {shape.stopValues.map((s, ix) => (
              <stop
                key={ix}
                offset={`${(ix / (shape.stopValues.length - 1)) * 100}%`}
                stopColor={
                  s.b > 0.7 ? "var(--water-sea-glow)" : "var(--water-sea-300)"
                }
                stopOpacity={(0.15 + s.b * 0.4) * s.a}
              />
            ))}
          </linearGradient>
          <linearGradient id="wr-edge-grad" x1="0" y1="0" x2="1" y2="0">
            {shape.stopValues.map((s, ix) => (
              <stop
                key={ix}
                offset={`${(ix / (shape.stopValues.length - 1)) * 100}%`}
                stopColor="var(--water-sea-glow)"
                stopOpacity={(0.0 + s.b * 0.55) * s.a}
              />
            ))}
          </linearGradient>
        </defs>

        {/* Inner group wobbles vertically — independent of the
            outer translateX drift. This is what breaks the conveyor-
            belt feel: the ribbon flows L→R AND drifts up/down at the
            same time. Shimmer fades the whole layer's opacity slowly. */}
        <g
          style={{
            animation:
              "water-ribbon-wobble 17s ease-in-out infinite, water-ribbon-shimmer 23s ease-in-out infinite",
          }}
        >
          <path d={shape.d} fill="url(#wr-grad)" opacity={0.55} filter="url(#wr-glow-wide)" />
          <path d={shape.d} fill="url(#wr-grad)" opacity={0.7} filter="url(#wr-glow-mid)" />
          <path d={shape.d} fill="url(#wr-grad)" opacity={0.4} />
          <path
            d={shape.edge}
            fill="none"
            stroke="url(#wr-edge-grad)"
            strokeWidth={1.2}
            strokeLinecap="round"
          />
          {shape.drops.map((d, i) => (
            <g key={i}>
              <circle
                cx={d.cx}
                cy={d.cy}
                r={d.r * 2.2}
                fill="var(--water-sea-glow)"
                opacity={d.opacity * 0.5}
                filter="url(#wr-drop-glow)"
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
      </svg>

      {/* PARALLAX layer — same shape, slower drift, lower opacity,
          opposite wobble phase. Reads as a second stream behind the
          primary; together they break the perceived periodicity that
          made the single layer feel conveyor-belt-like. */}
      <svg
        width={parentWidth * 3}
        height={VB_H}
        style={{
          position: "absolute",
          left: -parentWidth,
          top: 18,
          animation: "water-ribbon-drift-slow 71s linear infinite",
          opacity: 0.5,
        }}
      >
        <defs>
          <filter id="wr-glow-wide-p" x="-10%" y="-30%" width="120%" height="160%">
            <feGaussianBlur in="SourceGraphic" stdDeviation="28" />
          </filter>
          <linearGradient id="wr-grad-p" x1="0" y1="0" x2="1" y2="0">
            {shape.stopValues.map((s, ix) => (
              <stop
                key={ix}
                offset={`${(ix / (shape.stopValues.length - 1)) * 100}%`}
                stopColor="var(--water-sea-200)"
                stopOpacity={(0.1 + s.b * 0.3) * s.a}
              />
            ))}
          </linearGradient>
        </defs>
        <g
          style={{
            animation:
              "water-ribbon-wobble 22s ease-in-out infinite -7s, water-ribbon-shimmer 31s ease-in-out infinite -11s",
          }}
        >
          <path d={shape.d} fill="url(#wr-grad-p)" opacity={0.6} filter="url(#wr-glow-wide-p)" />
        </g>
      </svg>
    </div>
  );
}
