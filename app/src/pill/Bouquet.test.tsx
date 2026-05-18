import { describe, expect, it, beforeEach, vi } from "vitest";

const { pillRegenerateMock, pillPinMock, pillDismissMock } = vi.hoisted(() => ({
  pillRegenerateMock: vi.fn().mockResolvedValue(undefined),
  pillPinMock: vi.fn().mockResolvedValue(undefined),
  pillDismissMock: vi.fn().mockResolvedValue(undefined),
}));

vi.mock("../ipc/commands", () => ({
  ipc: {
    pillRegenerate: pillRegenerateMock,
    pillPin: pillPinMock,
    pillDismiss: pillDismissMock,
  },
}));

import { fireEvent, render, screen } from "@testing-library/react";
import { Bouquet, type BouquetItem } from "./Bouquet";

const sampleItems: BouquetItem[] = [
  { sub_pill_id: "p1-1", angle: "feel", text: "feel something at the threshold" },
  { sub_pill_id: "p1-2", angle: "notice", text: "the bell rings somewhere unseen" },
  { sub_pill_id: "p1-3", angle: "wonder", text: "what is held in that pause" },
];

beforeEach(() => {
  pillRegenerateMock.mockClear();
  pillPinMock.mockClear();
  pillDismissMock.mockClear();
});

describe("Bouquet", () => {
  it("renders 3 sub-capsules + regenerate + pin + X buttons", () => {
    render(
      <Bouquet
        parentId="p1"
        hueToken="--water-hue-muse"
        items={sampleItems}
        onClose={() => {}}
      />,
    );
    expect(screen.getByText("feel something at the threshold")).toBeInTheDocument();
    expect(screen.getByText("the bell rings somewhere unseen")).toBeInTheDocument();
    expect(screen.getByText("what is held in that pause")).toBeInTheDocument();
    expect(screen.getByLabelText("Regenerate bouquet")).toBeInTheDocument();
    expect(screen.getByLabelText("Pin pill")).toBeInTheDocument();
    expect(screen.getByLabelText("Dismiss pill")).toBeInTheDocument();
  });

  it("clicking regenerate calls ipc.pillRegenerate with the parent id", () => {
    render(
      <Bouquet
        parentId="p1"
        hueToken="--water-hue-muse"
        items={sampleItems}
        onClose={() => {}}
      />,
    );
    fireEvent.click(screen.getByLabelText("Regenerate bouquet"));
    expect(pillRegenerateMock).toHaveBeenCalledTimes(1);
    expect(pillRegenerateMock).toHaveBeenCalledWith("p1");
  });

  it("clicking pin calls ipc.pillPin with a synthesised Pill payload when pillForPinning is omitted", () => {
    render(
      <Bouquet
        parentId="p1"
        hueToken="--water-hue-muse"
        items={sampleItems}
        onClose={() => {}}
      />,
    );
    fireEvent.click(screen.getByLabelText("Pin pill"));
    expect(pillPinMock).toHaveBeenCalledTimes(1);
    expect(pillPinMock).toHaveBeenCalledWith(
      expect.objectContaining({ pill_id: "p1", hue_token: "--water-hue-muse" }),
      "",
      "",
      "",
    );
  });

  it("clicking pin forwards the supplied pillForPinning + threads sceneId/blockId, computing snippet from the anchored block", () => {
    const fullPill = {
      pill_id: "p1",
      speaker_id: "muse",
      hue_token: "--water-hue-muse",
      text: "ripple",
      block_target_id: "b9",
      trigger_id: "trg-1",
    };

    // Stand in for the manuscript block the pill reacted to. Bouquet
    // computes the snippet by querySelector at pin-time so it reflects
    // the latest editor state, not props captured at mount.
    const fakeBlock = document.createElement("div");
    fakeBlock.setAttribute("data-bid", "b9");
    fakeBlock.textContent = "the bell rings somewhere unseen";
    document.body.appendChild(fakeBlock);

    try {
      render(
        <Bouquet
          parentId="p1"
          hueToken="--water-hue-muse"
          items={sampleItems}
          onClose={() => {}}
          pillForPinning={fullPill}
          sceneId="s1"
          blockId="b9"
        />,
      );
      fireEvent.click(screen.getByLabelText("Pin pill"));
      expect(pillPinMock).toHaveBeenCalledWith(
        fullPill,
        "s1",
        "b9",
        "the bell rings somewhere unseen",
      );
    } finally {
      document.body.removeChild(fakeBlock);
    }
  });

  it("clicking pin with a blockId that's not in the DOM passes an empty snippet (still threads sceneId/blockId)", () => {
    // Guards against the C1 regression: prior to threading, Bouquet
    // hard-coded "" / "" / "" for sceneId/blockId/snippet and the orchestrator's
    // INSERT into `pinned_pill` blew up on the scene_id NOT NULL FK.
    const fullPill = {
      pill_id: "p1",
      speaker_id: "muse",
      hue_token: "--water-hue-muse",
      text: "ripple",
      block_target_id: "b-missing",
      trigger_id: "trg-1",
    };
    render(
      <Bouquet
        parentId="p1"
        hueToken="--water-hue-muse"
        items={sampleItems}
        onClose={() => {}}
        pillForPinning={fullPill}
        sceneId="s-real"
        blockId="b-missing"
      />,
    );
    fireEvent.click(screen.getByLabelText("Pin pill"));
    expect(pillPinMock).toHaveBeenCalledWith(fullPill, "s-real", "b-missing", "");
  });

  it("clicking X calls ipc.pillDismiss and onClose", () => {
    const onClose = vi.fn();
    render(
      <Bouquet
        parentId="p1"
        hueToken="--water-hue-muse"
        items={sampleItems}
        onClose={onClose}
      />,
    );
    fireEvent.click(screen.getByLabelText("Dismiss pill"));
    expect(pillDismissMock).toHaveBeenCalledTimes(1);
    expect(pillDismissMock).toHaveBeenCalledWith("p1");
    expect(onClose).toHaveBeenCalledTimes(1);
  });
});
