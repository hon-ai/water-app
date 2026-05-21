import { useMemo } from "react";
import type { CSSProperties } from "react";

interface Props {
  /** Live width of the editor's <main> container. Drives both the
   *  SVG sizing and the mask geometry. When 0 (unmeasured) the
   *  component renders nothing — avoids a 1-frame mis-sized ribbon. */
  parentWidth: number;
  /** Max width of the writer's markdown column. Defaults to the
   *  --water-canvas-max token (720). The column is centered with
   *  `margin: 0 auto`, so the visible side gutters are
   *  (parentWidth - columnWidth) / 2 each. */
  columnMaxWidth?: number;
  /** Vertical entry / exit. Both shoulders flow downward in the same
   *  direction so the eye reads a single continuous stream hidden
   *  behind the markdown — not two diverging arcs. */
  entryY?: number;
  exitY?: number;
  /** Base thickness (px). Modulates ±50% sinusoidally along the path. */
  baseThickness?: number;
  /** Number of centerline samples for variable-width rendering. */
  samples?: number;
}

/**
 * Ambient water-stream ribbon behind the editor canvas.
 *
 * The SVG is laid out at 3× the parent's width, positioned at
 * left = -parentWidth, so its coordinate system is 1:1 with pixels
 * (no viewBox stretching). This means the ribbon's proportions stay
 * consistent across window sizes — the path looks the same whether
 * the writer's window is narrow or wide.
 *
 * The mask is computed from the actual markdown column position
 * (centered, max-width 720) instead of a percentage of the wrapper.
 * Result: the ribbon is occluded by exactly the writing area at
 * every window width, with consistent soft-edge zones on each side.
 *
 * Both shoulders flow downward (entryY < exitY) so the eye reads
 * one continuous stream hidden behind the prose.
 */
export function WaterRibbon({
  parentWidth,
  columnMaxWidth = 720,
  entryY = 200,
  exitY = 360,
  baseThickness = 56,
  samples = 96,
}: Props) {
  // Geometry derived from the actual parent + column.
  const columnWidth = Math.min(columnMaxWidth, Math.max(0, parentWidth - 48));
  const columnLeft = Math.max(0, (parentWidth - columnWidth) / 2);
  const columnRight = columnLeft + columnWidth;
  // Soft-edge zone on either side of the column where the ribbon
  // fades to transparent. Same on every window size so the transition
  // feels constant.
  const SOFT = 48;

  const ribbonShape = useMemo(() => {
    if (parentWidth <= 0) return null;
    const W = parentWidth;
    const VB_W = W * 3;
    const xs: number[] = [];
    const ys: number[] = [];
    const widths: number[] = [];
    const brightness: number[] = [];

    for (let i = 0; i <= samples; i++) {
      const t = i / samples;
      // X spans -W to 2W in absolute pixel space.
      const x = -W + t * VB_W;
      // Single descending Bezier: entry → midDip → exit. Both visible
      // shoulders are on a descending trajectory.
      const midDip = exitY + 160;
      const y =
        (1 - t) * (1 - t) * (1 - t) * entryY +
        3 * (1 - t) * (1 - t) * t * midDip +
        3 * (1 - t) * t * t * midDip +
        t * t * t * exitY;

      // Width envelope: two overlapping waves + soft taper at the
      // viewport-loop boundaries so the ribbon thins as it leaves
      // the visible area.
      const taper = Math.min(1, Math.sin(t * Math.PI) * 1.6);
      const swell =
        0.65 +
        0.35 * Math.sin(t * Math.PI * 2.5 + 0.4) +
        0.15 * Math.sin(t * Math.PI * 7 + 1.2);
      const w = Math.max(8, baseThickness * swell * Math.max(0.25, taper));

      // Brightness envelope independent of width.
      const b =
        0.5 +
        0.4 * Math.sin(t * Math.PI * 3.2 + 1.8) +
        0.2 * Math.sin(t * Math.PI * 6 + 0.6);

      xs.push(x);
      ys.push(y);
      widths.push(w);
      brightness.push(Math.max(0.2, Math.min(1, b)));
    }

    // Top / bot edges via perpendicular tangent.
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

    let d = `M ${top[0]!.x} ${top[0]!.y}`;
    for (let i = 1; i < top.length; i++) d += ` L ${top[i]!.x} ${top[i]!.y}`;
    for (let i = bot.length - 1; i >= 0; i--) d += ` L ${bot[i]!.x} ${bot[i]!.y}`;
    d += " Z";

    let edge = `M ${top[0]!.x} ${top[0]!.y}`;
    for (let i = 1; i < top.length; i++) edge += ` L ${top[i]!.x} ${top[i]!.y}`;

    // 8-stop brightness array for the length gradient.
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

    return { d, edge, stopValues, W, VB_W };
  }, [parentWidth, samples, entryY, exitY, baseThickness]);

  if (!ribbonShape) return null;
  const VB_H = Math.max(entryY, exitY) + 360;

  // Mask: black at edges → transparent across the column → black on
  // the far side. Coordinates are absolute pixels, sized to the real
  // markdown column at any window width.
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

  // SVG positioned at left=-parentWidth, sized at 3×parentWidth.
  // translateX(-33.33%) per loop moves it by exactly parentWidth,
  // creating a seamless cycle.
  return (
    <div data-testid="water-ribbon" style={wrapperStyle} aria-hidden>
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

        <path d={ribbonShape.d} fill="url(#wr-grad)" opacity={0.55} filter="url(#wr-glow-wide)" />
        <path d={ribbonShape.d} fill="url(#wr-grad)" opacity={0.7} filter="url(#wr-glow-mid)" />
        <path d={ribbonShape.d} fill="url(#wr-grad)" opacity={0.45} />
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
