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
      <HoverDim active={false} anchorRect={null} hueToken="--water-hue-muse" />,
    );
    const backdrop = screen.getByTestId("water-hover-dim");
    expect(backdrop.style.opacity).toBe("0");
  });

  it("sets opacity > 0 when active", () => {
    const r = fakeRect();
    render(
      <HoverDim active={true} anchorRect={r} hueToken="--water-hue-muse" />,
    );
    const backdrop = screen.getByTestId("water-hover-dim");
    expect(parseFloat(backdrop.style.opacity)).toBeGreaterThan(0);
  });

  it("renders an anchor highlight rect when active with anchorRect", () => {
    const a = fakeRect();
    render(
      <HoverDim active={true} anchorRect={a} hueToken="--water-hue-muse" />,
    );
    const highlight = screen.getByTestId("water-hover-highlight");
    expect(highlight).toBeInTheDocument();
    // Positioned over the anchored block with tight horizontal pad
    // and no vertical offset (the earlier vertical pad was reading
    // as misaligned with the text glyphs).
    expect(highlight.style.position).toBe("fixed");
    expect(parseFloat(highlight.style.left)).toBeCloseTo(a.left - 2);
    expect(parseFloat(highlight.style.top)).toBeCloseTo(a.top);
  });

  it("does not render the highlight when anchorRect is null", () => {
    render(
      <HoverDim active={true} anchorRect={null} hueToken="--water-hue-muse" />,
    );
    expect(
      screen.queryByTestId("water-hover-highlight"),
    ).not.toBeInTheDocument();
  });

  it("does not render the highlight when not active", () => {
    const a = fakeRect();
    render(
      <HoverDim active={false} anchorRect={a} hueToken="--water-hue-muse" />,
    );
    expect(
      screen.queryByTestId("water-hover-highlight"),
    ).not.toBeInTheDocument();
  });
});
