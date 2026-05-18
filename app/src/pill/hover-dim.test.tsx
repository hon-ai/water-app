import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { HoverDim } from "./hover-dim";

const fakeRect = (overrides: Partial<DOMRect> = {}): DOMRect =>
  ({
    top: 100,
    left: 100,
    right: 200,
    bottom: 120,
    x: 100,
    y: 100,
    width: 100,
    height: 20,
    toJSON: () => "",
    ...overrides,
  }) as DOMRect;

describe("HoverDim", () => {
  it("renders a backdrop with opacity 0 by default", () => {
    render(
      <HoverDim
        active={false}
        anchorRect={null}
        sourceRect={null}
        hueToken="--water-hue-muse"
      />,
    );
    const backdrop = screen.getByTestId("water-hover-dim");
    expect(backdrop.style.opacity).toBe("0");
  });

  it("sets opacity > 0 when active", () => {
    const r = fakeRect();
    render(
      <HoverDim
        active={true}
        anchorRect={r}
        sourceRect={r}
        hueToken="--water-hue-muse"
      />,
    );
    const backdrop = screen.getByTestId("water-hover-dim");
    expect(parseFloat(backdrop.style.opacity)).toBeGreaterThan(0);
  });

  it("renders an SVG line when active with both rects", () => {
    const a = fakeRect();
    const s = fakeRect({ top: 50, left: 800, right: 900, x: 800, y: 50, height: 30, bottom: 80 });
    render(
      <HoverDim
        active={true}
        anchorRect={a}
        sourceRect={s}
        hueToken="--water-hue-muse"
      />,
    );
    expect(screen.getByTestId("water-hover-line")).toBeInTheDocument();
  });
});
