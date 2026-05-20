import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";

vi.mock("../ipc/commands", () => ({
  ipc: {
    worldSegmentSetHidden: vi.fn(),
    worldSegmentDelete: vi.fn(),
  },
}));

import { WorldSettingsMenu } from "./WorldSettingsMenu";
import { ipc } from "../ipc/commands";
import type { WorldSegment } from "../ipc/commands";

function segment(overrides: Partial<WorldSegment>): WorldSegment {
  return {
    id: "seg-c",
    slug: "concept",
    name: "Concept",
    ordering: 0,
    is_collection: false,
    hue_token: "--water-hue-world-1",
    hidden: false,
    has_template_override: false,
    ...overrides,
  };
}

describe("WorldSettingsMenu", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    (ipc.worldSegmentSetHidden as ReturnType<typeof vi.fn>).mockResolvedValue(
      undefined,
    );
    (ipc.worldSegmentDelete as ReturnType<typeof vi.fn>).mockResolvedValue(
      undefined,
    );
  });

  it("toggling visibility off calls worldSegmentSetHidden with hidden=true", async () => {
    const onChanged = vi.fn();
    render(
      <WorldSettingsMenu
        segments={[segment({})]}
        onChanged={onChanged}
        onClose={() => {}}
      />,
    );
    fireEvent.click(screen.getByTestId("visibility-concept"));
    await waitFor(() => {
      expect(ipc.worldSegmentSetHidden).toHaveBeenCalledWith({
        segmentId: "seg-c",
        hidden: true,
      });
    });
    expect(onChanged).toHaveBeenCalled();
  });

  it("toggling visibility on calls worldSegmentSetHidden with hidden=false", async () => {
    render(
      <WorldSettingsMenu
        segments={[segment({ hidden: true })]}
        onChanged={() => {}}
        onClose={() => {}}
      />,
    );
    fireEvent.click(screen.getByTestId("visibility-concept"));
    await waitFor(() => {
      expect(ipc.worldSegmentSetHidden).toHaveBeenCalledWith({
        segmentId: "seg-c",
        hidden: false,
      });
    });
  });

  it("delete button is hidden for built-in segments", () => {
    render(
      <WorldSettingsMenu
        segments={[
          segment({}),
          segment({ id: "seg-l", slug: "locations", name: "Locations" }),
        ]}
        onChanged={() => {}}
        onClose={() => {}}
      />,
    );
    expect(screen.queryByTestId("delete-seg-c")).not.toBeInTheDocument();
    expect(screen.queryByTestId("delete-seg-l")).not.toBeInTheDocument();
  });

  it("delete button shows for user-added segments and calls worldSegmentDelete on confirm", async () => {
    vi.spyOn(window, "confirm").mockReturnValue(true);
    render(
      <WorldSettingsMenu
        segments={[
          segment({
            id: "seg-user",
            slug: "magic_systems",
            name: "Magic Systems",
          }),
        ]}
        onChanged={() => {}}
        onClose={() => {}}
      />,
    );
    fireEvent.click(screen.getByTestId("delete-seg-user"));
    await waitFor(() => {
      expect(ipc.worldSegmentDelete).toHaveBeenCalledWith("seg-user");
    });
  });

  it("delete cancellation suppresses worldSegmentDelete", () => {
    vi.spyOn(window, "confirm").mockReturnValue(false);
    render(
      <WorldSettingsMenu
        segments={[
          segment({
            id: "seg-user",
            slug: "magic_systems",
            name: "Magic Systems",
          }),
        ]}
        onChanged={() => {}}
        onClose={() => {}}
      />,
    );
    fireEvent.click(screen.getByTestId("delete-seg-user"));
    expect(ipc.worldSegmentDelete).not.toHaveBeenCalled();
  });

  it("close button calls onClose", () => {
    const onClose = vi.fn();
    render(
      <WorldSettingsMenu
        segments={[segment({})]}
        onChanged={() => {}}
        onClose={onClose}
      />,
    );
    fireEvent.click(screen.getByLabelText("Close settings"));
    expect(onClose).toHaveBeenCalled();
  });
});
