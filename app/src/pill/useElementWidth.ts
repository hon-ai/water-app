import { useEffect, useState, type RefObject } from "react";

/**
 * Tracks the rendered width (in CSS px) of the element referenced by `ref`.
 *
 * Returns `0` until the first measurement lands (initial render and any
 * environment without `ResizeObserver`, e.g. jsdom). Consumers should treat
 * `0` as "not yet measured" rather than "the element has zero width" so
 * narrow-viewport fallbacks don't flash on first paint.
 *
 * The ResizeObserver is established on mount and disconnected on unmount;
 * the effect re-runs if the ref identity changes (rare in practice).
 */
export function useElementWidth(ref: RefObject<HTMLElement>): number {
  const [width, setWidth] = useState(0);

  useEffect(() => {
    if (!ref.current || typeof ResizeObserver === "undefined") return;
    const el = ref.current;
    const ro = new ResizeObserver((entries) => {
      for (const e of entries) {
        setWidth(e.contentRect.width);
      }
    });
    ro.observe(el);
    // Seed with current size so consumers don't sit at 0 until the next
    // resize event.
    setWidth(el.getBoundingClientRect().width);
    return () => ro.disconnect();
  }, [ref]);

  return width;
}
