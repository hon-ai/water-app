interface Props {
  /** Pixel size of the mark's bounding box. The mark scales
   *  cleanly at any size; uses currentColor so the parent can tint
   *  it via `color`. */
  size?: number;
  /** Stroke width at size=24. Other sizes scale proportionally. */
  strokeWidth?: number;
}

/**
 * The Water wordmark — three nested arcs of decreasing weight,
 * suggesting a stream's current. Minimal, single-color (currentColor)
 * so the parent decides hue via `color`. Used in EmptyState splash,
 * IconRail brand corner, About dialog, and favicon.
 */
export function StreamMark({ size = 32, strokeWidth = 1.6 }: Props) {
  // viewBox 0 0 32 32, scaled to `size`.
  const sw = strokeWidth;
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 32 32"
      fill="none"
      aria-hidden
      role="img"
    >
      {/* Three flowing horizontal currents. Each is a quadratic-bezier
          ribbon, decreasing weight + slight horizontal offset to imply
          motion. Stroke color = currentColor so the parent tints. */}
      <path
        d="M 2 10 Q 10 4 16 10 T 30 10"
        stroke="currentColor"
        strokeWidth={sw * 1.4}
        strokeLinecap="round"
        opacity={0.9}
      />
      <path
        d="M 1 16 Q 9 22 16 16 T 31 16"
        stroke="currentColor"
        strokeWidth={sw}
        strokeLinecap="round"
        opacity={1}
      />
      <path
        d="M 3 22 Q 10 18 16 22 T 29 22"
        stroke="currentColor"
        strokeWidth={sw * 0.7}
        strokeLinecap="round"
        opacity={0.7}
      />
    </svg>
  );
}
