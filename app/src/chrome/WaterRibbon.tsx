import type { CSSProperties } from "react";

/**
 * Ambient water-stream ribbon that flows left-to-right behind the
 * editor canvas. Inspired by Honkai: Star Rail's light-cone reveal:
 * a single, slow, wide sweep of light that curls around the central
 * card rather than weaving through it. Not a multi-wavelength sine
 * wave — one long curve, brightest where it arcs farthest from the
 * edges, fading where it leaves the surface.
 *
 * Two layered SVG shapes:
 *  - A wide, gaussian-blurred backing in `--water-sea-200` (the
 *    halo / glow).
 *  - A thin highlight along the centerline in `--water-sea-glow`.
 *
 * Slow translateX loop drives the apparent flow without disturbing
 * layout. The middle of the surface (where the writer's text column
 * sits) is masked to transparent so the ribbon shows only at the
 * left and right shoulders; pills sit above on z-index and naturally
 * occlude.
 *
 * Picks up the active palette via `--water-sea-*`. Reduced motion
 * stops the drift; the ribbon stays at its resting position.
 */
export function WaterRibbon({
  /** Width of the parent in CSS px. The internal path is laid out
   *  three viewport-widths wide so the translateX loop never exposes
   *  an edge. */
  width = 1600,
  /** Distance (px) the ribbon dips between its left and right
   *  entry points. */
  amplitude = 360,
  /** Vertical center of the ribbon's resting position. */
  centerY = 320,
  /** Strength of the central text-column fade. 0 = no fade, 1 = hard
   *  cutout. Default leaves a wide soft band around the prose. */
  textColumnFade = 0.75,
}: {
  width?: number;
  amplitude?: number;
  centerY?: number;
  textColumnFade?: number;
}) {
  const W = width;
  // The ribbon is a closed shape: a single shallow S between two
  // parallel curves, ~52px tall at its midpoint. Path runs from -W
  // through 2W so the loop can translate one full viewport width
  // without visible seam.
  const drawRibbon = (thickness: number) => {
    const h = thickness / 2;
    const topY = centerY - h;
    const botY = centerY + h;
    // Single long S: enters top-left, arcs down past center, climbs
    // up past 2W. Each segment is one full viewport wide for an
    // unhurried feel.
    return `
      M ${-W} ${topY}
      C ${-W * 0.4} ${topY},
        ${0}        ${topY + amplitude},
        ${W * 0.5}  ${topY + amplitude}
      C ${W * 1.0}  ${topY + amplitude},
        ${W * 1.4}  ${topY},
        ${W * 2}    ${topY}
      L ${W * 2}    ${botY}
      C ${W * 1.4}  ${botY},
        ${W * 1.0}  ${botY + amplitude},
        ${W * 0.5}  ${botY + amplitude}
      C ${0}        ${botY + amplitude},
        ${-W * 0.4} ${botY},
        ${-W}       ${botY}
      Z
    `;
  };

  // Center-fade mask. Wider safe-zone than the multi-wave version
  // since the single curve dips low — we want to avoid clipping
  // the dip too aggressively.
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

  // viewBox: -W to 2W horizontally, generous vertical so the dip
  // never clips. The SVG fills its parent via width/height 100%.
  const VB_H = centerY * 2 + amplitude + 80;

  return (
    <div data-testid="water-ribbon" style={wrapperStyle} aria-hidden>
      <svg
        viewBox={`${-W} 0 ${W * 3} ${VB_H}`}
        preserveAspectRatio="none"
        width="100%"
        height="100%"
        style={{
          position: "absolute",
          inset: 0,
          /* One slow drift — full cycle covers a viewport width over
             40s. The SVG content is 3 viewports wide; translateX
             moves 1 viewport (33.33%) per cycle for seamless loop. */
          animation: "water-ribbon-drift 40s linear infinite",
        }}
      >
        <defs>
          <filter
            id="water-ribbon-glow"
            x="-10%"
            y="-10%"
            width="120%"
            height="120%"
          >
            <feGaussianBlur in="SourceGraphic" stdDeviation="18" />
          </filter>
          {/* Gradient along the ribbon's length — brightest in the
              middle of each viewport, fading at the edges so the
              loop seam stays invisible even if the mask softens. */}
          <linearGradient id="water-ribbon-grad" x1="0" y1="0" x2="1" y2="0">
            <stop offset="0%"   stopColor="var(--water-sea-300)" stopOpacity="0" />
            <stop offset="20%"  stopColor="var(--water-sea-300)" stopOpacity="0.55" />
            <stop offset="50%"  stopColor="var(--water-sea-glow)" stopOpacity="0.7" />
            <stop offset="80%"  stopColor="var(--water-sea-300)" stopOpacity="0.55" />
            <stop offset="100%" stopColor="var(--water-sea-300)" stopOpacity="0" />
          </linearGradient>
        </defs>
        {/* Soft glow halo — wide blurred shape underneath. */}
        <path
          d={drawRibbon(72)}
          fill="url(#water-ribbon-grad)"
          opacity={0.55}
          filter="url(#water-ribbon-glow)"
        />
        {/* Sharper inner ribbon — the readable highlight. */}
        <path
          d={drawRibbon(18)}
          fill="url(#water-ribbon-grad)"
          opacity={0.75}
        />
      </svg>
    </div>
  );
}
