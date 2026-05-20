import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor, fireEvent } from "@testing-library/react";

vi.mock("../ipc/commands", () => ({
  ipc: {
    worldSegmentList: vi.fn(),
    worldIntakeSchema: vi.fn(),
    worldSingleDocRead: vi.fn(),
    worldSingleDocUpdateField: vi.fn(),
    worldEntryList: vi.fn(),
    worldEntryCreate: vi.fn(),
  },
}));

import { WorldSegmentView } from "./WorldSegmentView";
import { ipc } from "../ipc/commands";

describe("WorldSegmentView (single-doc)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    (ipc.worldSegmentList as ReturnType<typeof vi.fn>).mockResolvedValue([
      {
        id: "seg-c",
        slug: "concept",
        name: "Concept",
        ordering: 0,
        is_collection: false,
        hue_token: "--water-hue-world-1",
        hidden: false,
        has_template_override: false,
      },
    ]);
    (ipc.worldIntakeSchema as ReturnType<typeof vi.fn>).mockResolvedValue({
      id: "concept",
      label: "Concept",
      fields: [
        {
          id: "main.core_premise",
          label: "Core Premise",
          prompt_question: "?",
          kind: { type: "long_text" },
          optional_skip: false,
        },
      ],
    });
    (ipc.worldSingleDocRead as ReturnType<typeof vi.fn>).mockResolvedValue({
      id: "x",
      segment_id: "seg-c",
      schema_version: "concept@1",
      name: "Concept",
      aliases: [],
      main: { core_premise: "An echo through stone." },
    });
  });

  it("renders single-doc sheet with template fields + current values", async () => {
    render(
      <WorldSegmentView
        segmentId="seg-c"
        onOpenEntry={() => {}}
        onOpenIntake={() => {}}
      />,
    );
    await waitFor(() => screen.getByText("Core Premise"));
    expect(screen.getByText("An echo through stone.")).toBeInTheDocument();
  });
});

describe("WorldSegmentView (collection)", () => {
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
  });

  it("renders entry cards from worldEntryList", async () => {
    (ipc.worldEntryList as ReturnType<typeof vi.fn>).mockResolvedValue([
      {
        id: "e1",
        segment_id: "seg-l",
        name: "The Pell Library",
        preview: "Dust thick…",
      },
      {
        id: "e2",
        segment_id: "seg-l",
        name: "Aren's Atelier",
        preview: "Light from the…",
      },
    ]);
    render(
      <WorldSegmentView
        segmentId="seg-l"
        onOpenEntry={() => {}}
        onOpenIntake={() => {}}
      />,
    );
    await waitFor(() => screen.getByTestId("entry-card-e1"));
    expect(screen.getByText("The Pell Library")).toBeInTheDocument();
    expect(screen.getByText("Aren's Atelier")).toBeInTheDocument();
  });

  it("clicking + New entry creates a draft and routes to intake", async () => {
    (ipc.worldEntryList as ReturnType<typeof vi.fn>).mockResolvedValue([]);
    (ipc.worldEntryCreate as ReturnType<typeof vi.fn>).mockResolvedValue(
      "draft-1",
    );
    const openIntake = vi.fn();
    render(
      <WorldSegmentView
        segmentId="seg-l"
        onOpenEntry={() => {}}
        onOpenIntake={openIntake}
      />,
    );
    await waitFor(() => screen.getByTestId("new-entry-button"));
    fireEvent.click(screen.getByTestId("new-entry-button"));
    await waitFor(() =>
      expect(openIntake).toHaveBeenCalledWith("seg-l", "draft-1"),
    );
  });

  it("unnamed entry shows '(unnamed)'", async () => {
    (ipc.worldEntryList as ReturnType<typeof vi.fn>).mockResolvedValue([
      {
        id: "e-stub",
        segment_id: "seg-l",
        name: "",
        preview: "Some snippet…",
      },
    ]);
    render(
      <WorldSegmentView
        segmentId="seg-l"
        onOpenEntry={() => {}}
        onOpenIntake={() => {}}
      />,
    );
    await waitFor(() => screen.getByTestId("entry-card-e-stub"));
    expect(screen.getByText("(unnamed)")).toBeInTheDocument();
  });
});
