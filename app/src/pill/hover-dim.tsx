import type { CSSProperties, ReactElement } from "react";

interface Props {
  active: boolean;
  anchorRect: DOMRect | null;
  sourceRect: DOMRect | null;
  hueToken: string;
}

/**
 * Global hover affordance for pill capsules.
 *
 * Renders two things stacked on top of the editor:
 * 1. A fixed-position dim backdrop that fades up to 8% opacity when `active`,
 *    softening the main canvas so the hovered pill + its anchored block
 *    visually pop.
 * 2. An SVG glow line connecting the pill capsule's midpoint to the anchored
 *    block's midpoint, drawn only when both `anchorRect` and `sourceRect`
 *    are provided.
 *
 * Both elements are `pointer-events: none` so they never steal hover/click
 * from the underlying capsule or editor.
 */
export function HoverDim({ active, anchorRect, sourceRect, hueToken }: Props) {
  const backdropStyle: CSSProperties = {
    position: "fixed",
    inset: 0,
    background: "var(--water-bg-paper)",
    opacity: active ? 0.08 : 0,
    transition: "opacity var(--water-dur-tiny) var(--water-ease-out-soft)",
    pointerEvents: "none",
    zIndex: 30,
  };

  let line: ReactElement | null = null;
  if (active && anchorRect && sourceRect) {
    const x1 = sourceRect.left + sourceRect.width / 2;
    const y1 = sourceRect.top + sourceRect.height / 2;
    const x2 = anchorRect.left + anchorRect.width / 2;
    const y2 = anchorRect.top + anchorRect.height / 2;
    line = (
      <svg
        data-testid="water-hover-line"
        style={{
          position: "fixed",
          inset: 0,
          width: "100vw",
          height: "100vh",
          pointerEvents: "none",
          zIndex: 31,
        }}
      >
        <line
          x1={x1}
          y1={y1}
          x2={x2}
          y2={y2}
          stroke={`var(${hueToken})`}
          strokeWidth="1"
          strokeOpacity="0.6"
          style={{ filter: `drop-shadow(0 0 6px var(${hueToken}))` }}
        />
      </svg>
    );
  }

  return (
    <>
      <div data-testid="water-hover-dim" style={backdropStyle} />
      {line}
    </>
  );
}
