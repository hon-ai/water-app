import type { CanvasCard } from "./CanvasSurface";

interface Props {
  cards: CanvasCard[];
  cardW: number;
  cardH: number;
}

/**
 * SVG layer drawn underneath the scene cards. Renders smoothed
 * cubic-Bezier curves between scenes in manuscript order so the
 * writer can compare spatial layout to narrative order at a glance.
 *
 * The curve control points pull each endpoint slightly inward in
 * the direction of the path so curves don't criss-cross too
 * harshly when spatial layout diverges from manuscript order.
 *
 * Cards is expected to be pre-sorted by manuscript_ordering.
 */
export function ReadingOrderOverlay({ cards, cardW, cardH }: Props) {
  if (cards.length < 2) return null;
  const segments: string[] = [];
  for (let i = 0; i < cards.length - 1; i++) {
    const a = cards[i];
    const b = cards[i + 1];
    if (!a || !b) continue;
    const ax = a.x + cardW / 2;
    const ay = a.y + cardH / 2;
    const bx = b.x + cardW / 2;
    const by = b.y + cardH / 2;
    const dx = (bx - ax) * 0.4;
    // Curve symmetrically: pull both control points inward by 40% of
    // the horizontal distance. This bends arcs without flipping
    // direction on every segment.
    const c1x = ax + dx;
    const c1y = ay;
    const c2x = bx - dx;
    const c2y = by;
    segments.push(`M ${ax} ${ay} C ${c1x} ${c1y}, ${c2x} ${c2y}, ${bx} ${by}`);
  }
  // SVG bounds: pad generously so curves passing outside card
  // centers still render. Bounds are computed from card positions.
  const maxX = Math.max(...cards.map((c) => c.x + cardW)) + 200;
  const maxY = Math.max(...cards.map((c) => c.y + cardH)) + 200;
  return (
    <svg
      aria-hidden
      data-testid="reading-order-overlay"
      style={{
        position: "absolute",
        left: -100,
        top: -100,
        width: maxX,
        height: maxY,
        pointerEvents: "none",
        opacity: 0.3,
      }}
    >
      <g transform="translate(100, 100)">
        {segments.map((d, ix) => (
          <path
            key={ix}
            d={d}
            stroke="var(--water-hue-flow)"
            strokeWidth={1.5}
            fill="none"
          />
        ))}
        {/* Numbered endpoints at each card center so the writer can
            count along the order without squinting. */}
        {cards.map((c, ix) => (
          <text
            key={c.id}
            x={c.x + cardW / 2}
            y={c.y + cardH / 2 + 4}
            fontSize={10}
            textAnchor="middle"
            fill="var(--water-fg-default)"
            style={{ fontFamily: "var(--water-font-sans)", opacity: 0.7 }}
          >
            {ix + 1}
          </text>
        ))}
      </g>
    </svg>
  );
}
