import { describe, expect, it, vi } from "vitest";

const { ipcMock } = vi.hoisted(() => ({
  ipcMock: {
    pillExpand: vi.fn().mockResolvedValue(undefined),
    pillRegenerate: vi.fn().mockResolvedValue(undefined),
    pillPin: vi.fn().mockResolvedValue(undefined),
    pillDismiss: vi.fn().mockResolvedValue(undefined),
  },
}));
vi.mock("../ipc/commands", () => ({ ipc: ipcMock }));

import { fireEvent, render, screen } from "@testing-library/react";
import { RabbitHole } from "./RabbitHole";
import type { BouquetItem } from "./Bouquet";

const level1: BouquetItem[] = [
  { sub_pill_id: "p1-1", angle: "feel", text: "L1 feel" },
];
const level2: BouquetItem[] = [
  { sub_pill_id: "p2-1", angle: "notice", text: "L2 notice" },
];

describe("RabbitHole", () => {
  it("renders the current bouquet + breadcrumb of prior levels", () => {
    render(
      <RabbitHole
        hueToken="--water-hue-muse"
        path={[
          {
            parentId: "p1",
            parentText: "root observation",
            items: level1,
            chosenSubId: "p1-1",
          },
          {
            parentId: "p1-1",
            parentText: "L1 feel",
            items: level2,
            chosenSubId: null,
          },
        ]}
        onSubClick={() => {}}
        onClose={() => {}}
      />,
    );
    expect(screen.getByText("L2 notice")).toBeInTheDocument();
    expect(screen.getByLabelText("Rabbit hole breadcrumb")).toBeInTheDocument();
  });

  it("renders collapsed glow lines for siblings of chosen path entries", () => {
    render(
      <RabbitHole
        hueToken="--water-hue-muse"
        path={[
          {
            parentId: "p1",
            parentText: "root",
            items: [
              { sub_pill_id: "p1-1", angle: "feel", text: "chosen" },
              { sub_pill_id: "p1-2", angle: "notice", text: "sibling A" },
              { sub_pill_id: "p1-3", angle: "wonder", text: "sibling B" },
            ],
            chosenSubId: "p1-1",
          },
          {
            parentId: "p1-1",
            parentText: "chosen",
            items: level2,
            chosenSubId: null,
          },
        ]}
        onSubClick={() => {}}
        onClose={() => {}}
      />,
    );
    // 2 sibling glow lines from level 1 (siblings of p1-1).
    expect(screen.getAllByTestId("water-glow-line")).toHaveLength(2);
  });

  it("X calls onClose", () => {
    const onClose = vi.fn();
    render(
      <RabbitHole
        hueToken="--water-hue-muse"
        path={[
          {
            parentId: "p1",
            parentText: "x",
            items: level1,
            chosenSubId: null,
          },
        ]}
        onSubClick={() => {}}
        onClose={onClose}
      />,
    );
    fireEvent.click(screen.getByLabelText("Dismiss pill"));
    expect(onClose).toHaveBeenCalled();
  });
});
