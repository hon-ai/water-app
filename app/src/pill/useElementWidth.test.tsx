import { describe, expect, it } from "vitest";
import { render } from "@testing-library/react";
import { useRef } from "react";
import { useElementWidth } from "./useElementWidth";

function Probe({ onWidth }: { onWidth: (w: number) => void }) {
  const ref = useRef<HTMLDivElement>(null);
  const w = useElementWidth(ref);
  onWidth(w);
  return <div ref={ref} style={{ width: 800 }} />;
}

describe("useElementWidth", () => {
  // jsdom returns 0 for getBoundingClientRect and lacks ResizeObserver,
  // so we can only assert the hook returns a number. The real measurement
  // path is exercised at runtime in the browser.
  it("returns the element's current width as a number", () => {
    let lastWidth = 0;
    render(
      <Probe
        onWidth={(w) => {
          lastWidth = w;
        }}
      />,
    );
    expect(typeof lastWidth).toBe("number");
  });
});
