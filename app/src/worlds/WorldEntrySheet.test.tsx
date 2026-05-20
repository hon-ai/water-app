import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor, fireEvent } from "@testing-library/react";
import { WorldEntrySheet } from "./WorldEntrySheet";

// Mock the IPC singleton. Factory is self-contained so vitest hoisting is
// safe. Mirrors `WorldSingleDocSheet.test.tsx`'s pattern but adds the
// alias-CRUD + entry-read mocks T23 needs.
vi.mock("../ipc/commands", () => ({
  ipc: {
    worldSegmentList: vi.fn(),
    worldIntakeSchema: vi.fn(),
    worldEntryRead: vi.fn(),
    worldEntryUpdateAliases: vi.fn(),
    worldEntryUpdateField: vi.fn(),
  },
}));

import { ipc } from "../ipc/commands";

describe("WorldEntrySheet", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    (ipc.worldSegmentList as ReturnType<typeof vi.fn>).mockResolvedValue([
      {
        id: "seg-l",
        slug: "locations",
        name: "Locations",
        ordering: 1,
        is_collection: true,
        hue_token: "--water-hue-world-2",
        hidden: false,
        has_template_override: false,
      },
    ]);
    (ipc.worldIntakeSchema as ReturnType<typeof vi.fn>).mockResolvedValue({
      id: "locations",
      label: "Location",
      fields: [
        {
          id: "main.name",
          label: "Name",
          prompt_question: "?",
          kind: { type: "short_text" },
          optional_skip: false,
        },
        {
          id: "main.type",
          label: "Type",
          prompt_question: "?",
          kind: { type: "short_text" },
          optional_skip: false,
        },
      ],
    });
    (ipc.worldEntryRead as ReturnType<typeof vi.fn>).mockResolvedValue({
      id: "e1",
      segment_id: "seg-l",
      schema_version: "locations@1",
      name: "The Pell Library",
      aliases: ["Pell"],
      main: { name: "The Pell Library", type: "library" },
    });
    (ipc.worldEntryUpdateAliases as ReturnType<typeof vi.fn>).mockResolvedValue(
      undefined,
    );
    (ipc.worldEntryUpdateField as ReturnType<typeof vi.fn>).mockResolvedValue(
      undefined,
    );
  });

  it("renders entry name + aliases + template field values", async () => {
    render(<WorldEntrySheet segmentId="seg-l" entryId="e1" />);
    await waitFor(() => screen.getByTestId("entry-name"));
    // Header reflects the canonical entry name.
    expect(screen.getByTestId("entry-name").textContent).toBe(
      "The Pell Library",
    );
    // Alias chip is rendered.
    expect(screen.getByText("Pell")).toBeInTheDocument();
    // InlineField shows its current value as text in non-edit mode —
    // "library" is the second template field's value.
    expect(screen.getByText("library")).toBeInTheDocument();
  });

  it("adding an alias commits via worldEntryUpdateAliases", async () => {
    render(<WorldEntrySheet segmentId="seg-l" entryId="e1" />);
    await waitFor(() => screen.getByTestId("alias-input"));
    fireEvent.change(screen.getByTestId("alias-input"), {
      target: { value: "the library" },
    });
    fireEvent.click(screen.getByTestId("alias-add-button"));
    await waitFor(() =>
      expect(ipc.worldEntryUpdateAliases).toHaveBeenCalledWith({
        entryId: "e1",
        aliases: ["Pell", "the library"],
      }),
    );
  });

  it("removing an alias commits the new list via ipc", async () => {
    render(<WorldEntrySheet segmentId="seg-l" entryId="e1" />);
    await waitFor(() => screen.getByTestId("remove-alias-0"));
    fireEvent.click(screen.getByTestId("remove-alias-0"));
    await waitFor(() =>
      expect(ipc.worldEntryUpdateAliases).toHaveBeenCalledWith({
        entryId: "e1",
        aliases: [],
      }),
    );
  });

  it("unnamed entry shows '(unnamed)'", async () => {
    (ipc.worldEntryRead as ReturnType<typeof vi.fn>).mockResolvedValue({
      id: "e-stub",
      segment_id: "seg-l",
      schema_version: "locations@1",
      name: "",
      aliases: [],
      main: { sensory_detail: "Dust thick enough\u2026" },
    });
    render(<WorldEntrySheet segmentId="seg-l" entryId="e-stub" />);
    await waitFor(() => screen.getByTestId("entry-name"));
    expect(screen.getByTestId("entry-name").textContent).toBe("(unnamed)");
  });
});
