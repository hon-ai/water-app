import type { CSSProperties } from "react";

interface Props {
  active: boolean;
  anchorRect: DOMRect | null;
  hueToken: string;
}

/**
 * Global hover affordance for pill capsules.
 *
 * Renders two things stacked on top of the editor:
 * 1. A fixed-position dim backdrop that fades up to 8% opacity when `active`,
 *    softening the main canvas so the hovered pill + its anchored block
 *    visually pop.
 * 2. A subtle hue-tinted highlight box drawn over the anchored block (when
 *    `anchorRect` is provided), so the writer can see which paragraph the
 *    pill is reacting to without a connecting line drawn across the canvas.
 *    The old SVG line was visually noisy and could occlude the pill itself
 *    when the geometry overlapped — a positional highlight on the source
 *    block reads as ambient rather than infrastructural.
 *
 * Both elements are `pointer-events: none` so they never steal hover/click
 * from the underlying capsule or editor.
 */
export function HoverDim({ active, anchorRect, hueToken }: Props) {
  const backdropStyle: CSSProperties = {
    position: "fixed",
    inset: 0,
    background: "var(--water-bg-paper)",
    opacity: active ? 0.08 : 0,
    transition: "opacity var(--water-dur-tiny) var(--water-ease-out-soft)",
    pointerEvents: "none",
    zIndex: 30,
  };

  // Tight alignment: pad horizontally by 2px each side, no vertical
  // pad. The earlier ±2 vertical pad was pushing the highlight
  // above/below the text glyphs, which read as misaligned.
  const highlightStyle: CSSProperties | null =
    active && anchorRect
      ? {
          position: "fixed",
          left: anchorRect.left - 2,
          top: anchorRect.top,
          width: anchorRect.width + 4,
          height: anchorRect.height,
          background: `color-mix(in oklch, var(${hueToken}) 14%, transparent)`,
          borderRadius: "var(--water-r-8)",
          pointerEvents: "none",
          zIndex: 31,
          transition:
            "opacity var(--water-dur-tiny) var(--water-ease-out-soft)",
        }
      : null;

  return (
    <>
      <div data-testid="water-hover-dim" style={backdropStyle} />
      {highlightStyle && (
        <div data-testid="water-hover-highlight" style={highlightStyle} />
      )}
    </>
  );
}
