import { describe, expect, it, beforeEach, vi } from "vitest";
import { act, render, screen, waitFor } from "@testing-library/react";

const { pinnedListMock, pillDismissMock, onWaterEventMock } = vi.hoisted(() => ({
  pinnedListMock: vi.fn(),
  pillDismissMock: vi.fn().mockResolvedValue(undefined),
  onWaterEventMock: vi.fn(),
}));

vi.mock("../ipc/commands", () => ({
  ipc: {
    pinnedList: pinnedListMock,
    pillDismiss: pillDismissMock,
  },
}));
vi.mock("../ipc/events", () => ({ onWaterEvent: onWaterEventMock }));

beforeEach(() => {
  if (!HTMLDialogElement.prototype.showModal) {
    HTMLDialogElement.prototype.showModal = function () {
      (this as unknown as { open: boolean }).open = true;
    };
  }
  if (!HTMLDialogElement.prototype.close) {
    HTMLDialogElement.prototype.close = function () {
      (this as unknown as { open: boolean }).open = false;
    };
  }
  pinnedListMock.mockReset();
  pillDismissMock.mockClear();
  onWaterEventMock.mockReset();
  onWaterEventMock.mockResolvedValue(vi.fn());
});

import { PinnedColumn } from "./PinnedColumn";

describe("PinnedColumn", () => {
  it("renders an empty 56px-wide column when nothing pinned", async () => {
    pinnedListMock.mockResolvedValue([]);
    render(<PinnedColumn />);
    await waitFor(() => expect(pinnedListMock).toHaveBeenCalledTimes(1));
    const col = screen.getByLabelText("pinned column");
    expect(col).toBeInTheDocument();
    // Inline style is "56px"; computed style fallback also accepted.
    expect(col.style.width).toBe("56px");
    expect(screen.queryAllByTestId("water-pinned-dot")).toHaveLength(0);
  });

  it("renders a dot per pinned pill returned by pinned_list", async () => {
    pinnedListMock.mockResolvedValue([
      {
        pill_id: "a",
        speaker_id: "muse",
        hue_token: "--water-hue-muse",
        text: "first",
        block_target_id: null,
        trigger_id: "t-1",
      },
      {
        pill_id: "b",
        speaker_id: "muse",
        hue_token: "--water-hue-muse",
        text: "second",
        block_target_id: null,
        trigger_id: "t-2",
      },
    ]);
    render(<PinnedColumn />);
    await waitFor(() =>
      expect(screen.getAllByTestId("water-pinned-dot")).toHaveLength(2),
    );
  });

  it("reacts to pill:pinned event by prepending a new dot", async () => {
    pinnedListMock.mockResolvedValue([]);
    let pinnedHandler: ((p: unknown) => void) | null = null;
    onWaterEventMock.mockImplementation(async (name: string, cb: unknown) => {
      if (name === "pill:pinned") {
        pinnedHandler = cb as (p: unknown) => void;
      }
      return vi.fn();
    });
    render(<PinnedColumn />);
    await waitFor(() => expect(pinnedHandler).not.toBeNull());
    await act(async () => {
      pinnedHandler!({
        pill_id: "fresh",
        speaker_id: "muse",
        hue_token: "--water-hue-muse",
        text: "fresh pill",
        block_target_id: null,
        trigger_id: "t-9",
      });
    });
    await waitFor(() =>
      expect(screen.getAllByTestId("water-pinned-dot")).toHaveLength(1),
    );
  });
});
