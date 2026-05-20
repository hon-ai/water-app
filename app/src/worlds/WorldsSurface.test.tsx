import { describe, it, expect, vi, beforeEach } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";

// Mock the IPC singleton. Factory is self-contained so vitest hoisting is
// safe. Mirrors the M3 `chrome/CharactersSurface.test.tsx` pattern.
vi.mock("../ipc/commands", () => ({
  ipc: {
    worldSegmentList: vi.fn(),
    worldEntryList: vi.fn(),
    worldSingleDocRead: vi.fn(),
    worldEntryRead: vi.fn(),
    worldIntakeSchema: vi.fn(),
    worldEntryUpdateField: vi.fn(),
    worldEntryUpdateAliases: vi.fn(),
  },
}));

import { WorldsSurface } from "./WorldsSurface";
import { ipc } from "../ipc/commands";

const listMock = ipc.worldSegmentList as ReturnType<typeof vi.fn>;
const entryListMock = ipc.worldEntryList as ReturnType<typeof vi.fn>;
const singleDocReadMock = ipc.worldSingleDocRead as ReturnType<typeof vi.fn>;

describe("WorldsSurface", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    listMock.mockResolvedValue([
      {
        id: "seg-1",
        slug: "concept",
        name: "Concept",
        ordering: 0,
        is_collection: false,
        hue_token: "--water-hue-world-1",
        hidden: false,
        has_template_override: false,
      },
      {
        id: "seg-2",
        slug: "locations",
        name: "Locations",
        ordering: 1,
        is_collection: true,
        hue_token: "--water-hue-world-2",
        hidden: false,
        has_template_override: false,
      },
    ]);
    entryListMock.mockResolvedValue([]);
    singleDocReadMock.mockResolvedValue({
      id: "x",
      segment_id: "seg-1",
      schema_version: "concept@1",
      name: "Concept",
      aliases: [],
    });
  });

  it("renders the index with all non-hidden segments", async () => {
    render(<WorldsSurface projectId="p1" />);
    await waitFor(() => screen.getByTestId("segment-tile-concept"));
    expect(screen.getByTestId("segment-tile-locations")).toBeInTheDocument();
  });

  it("navigates to segment view on tile click", async () => {
    render(<WorldsSurface projectId="p1" />);
    await waitFor(() => screen.getByTestId("segment-tile-locations"));
    fireEvent.click(screen.getByTestId("segment-tile-locations"));
    // T21–T22: tile click now mounts `WorldSegmentView`, which for a
    // collection segment resolves to `WorldCollectionGrid` with the
    // `+ New entry` button. The single-doc placeholder testid is gone.
    await waitFor(() => screen.getByTestId("new-entry-button"));
    expect(screen.getByRole("heading", { name: "Locations" })).toBeInTheDocument();
  });

  it("back button returns to index from segment view", async () => {
    render(<WorldsSurface projectId="p1" />);
    await waitFor(() => screen.getByTestId("segment-tile-locations"));
    fireEvent.click(screen.getByTestId("segment-tile-locations"));
    fireEvent.click(screen.getByText("← Back"));
    // WorldIndex re-mounts and re-fetches via useEffect on return —
    // wait for the tile to come back rather than checking synchronously.
    await waitFor(() =>
      expect(screen.getByTestId("segment-tile-concept")).toBeInTheDocument(),
    );
  });

  it("routes to the entry view on water:nav-world-entry custom event", async () => {
    (ipc.worldEntryRead as ReturnType<typeof vi.fn>).mockResolvedValue({
      id: "entry-1",
      segment_id: "seg-2",
      schema_version: "locations@1",
      name: "",
      aliases: [],
      main: { sensory_detail: "dust" },
    });
    (ipc.worldIntakeSchema as ReturnType<typeof vi.fn>).mockResolvedValue({
      id: "locations",
      label: "Location",
      fields: [
        {
          id: "main.name",
          label: "Name",
          prompt_question: "What's this place called?",
          kind: { type: "short_text" },
          optional_skip: false,
        },
      ],
    });
    render(<WorldsSurface projectId="p1" />);
    await waitFor(() => screen.getByTestId("segment-tile-locations"));
    window.dispatchEvent(
      new CustomEvent("water:nav-world-entry", {
        detail: { segmentId: "seg-2", entryId: "entry-1" },
      }),
    );
    // After the route change the index tiles are gone and the entry
    // surface fetches via `worldEntryRead`. Assert via the IPC fetch
    // for the targeted entry id, which proves the route reached the
    // entry view without depending on the sheet's specific UI surface.
    await waitFor(() => {
      expect(ipc.worldEntryRead).toHaveBeenCalledWith("entry-1");
    });
    expect(
      screen.queryByTestId("segment-tile-concept"),
    ).not.toBeInTheDocument();
  });

  it("hidden segments are filtered out", async () => {
    listMock.mockResolvedValue([
      {
        id: "seg-1",
        slug: "concept",
        name: "Concept",
        ordering: 0,
        is_collection: false,
        hue_token: "--water-hue-world-1",
        hidden: true,
        has_template_override: false,
      },
    ]);
    render(<WorldsSurface projectId="p1" />);
    await waitFor(() => screen.getByTestId("new-segment-button"));
    expect(screen.queryByTestId("segment-tile-concept")).not.toBeInTheDocument();
  });
});
