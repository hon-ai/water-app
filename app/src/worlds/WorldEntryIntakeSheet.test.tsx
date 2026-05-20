import { beforeEach, describe, expect, it, vi } from "vitest";

// jsdom does not implement HTMLDialogElement; stub the minimum surface
// our Sheet primitive uses. Mirrors `CharacterIntakeSheet.test.tsx`.
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
});

// Mock the IPC singleton. Factory self-contained for vitest hoisting.
vi.mock("../ipc/commands", () => ({
  ipc: {
    worldIntakeSchema: vi.fn(),
    worldEntryRead: vi.fn(),
    worldEntryUpdateField: vi.fn(),
    worldEntryDeleteIfEmpty: vi.fn(),
  },
}));

import { render, screen, waitFor, fireEvent } from "@testing-library/react";
import { WorldEntryIntakeSheet } from "./WorldEntryIntakeSheet";
import { ipc } from "../ipc/commands";

describe("WorldEntryIntakeSheet", () => {
  beforeEach(() => {
    vi.clearAllMocks();
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
    (ipc.worldEntryRead as ReturnType<typeof vi.fn>).mockResolvedValue({
      id: "draft-1",
      segment_id: "seg-l",
      schema_version: "locations@1",
      name: "",
      aliases: [],
      main: { name: "" },
    });
    (ipc.worldEntryUpdateField as ReturnType<typeof vi.fn>).mockResolvedValue(
      undefined,
    );
    (ipc.worldEntryDeleteIfEmpty as ReturnType<typeof vi.fn>).mockResolvedValue(
      true,
    );
  });

  it("walks the intake for the segment's template", async () => {
    render(
      <WorldEntryIntakeSheet
        segmentId="seg-l"
        draftEntryId="draft-1"
        onComplete={() => {}}
        onClose={() => {}}
      />,
    );
    // `ConversationalIntake` renders the prompt_question as an <h3>.
    await screen.findByRole("heading", { name: /what's this place called\?/i });
    expect(ipc.worldIntakeSchema).toHaveBeenCalledWith("seg-l");
    expect(ipc.worldEntryRead).toHaveBeenCalledWith("draft-1");
  });

  it("answering a field calls worldEntryUpdateField with dotted-path field_id", async () => {
    render(
      <WorldEntryIntakeSheet
        segmentId="seg-l"
        draftEntryId="draft-1"
        onComplete={() => {}}
        onClose={() => {}}
      />,
    );
    const input = await screen.findByRole("textbox");
    fireEvent.change(input, { target: { value: "The Pell Library" } });
    fireEvent.keyDown(input, { key: "Enter" });
    await waitFor(() =>
      expect(ipc.worldEntryUpdateField).toHaveBeenCalledWith({
        entryId: "draft-1",
        fieldId: "main.name",
        value: "The Pell Library",
      }),
    );
  });

  it("onClose reaps the orphan draft before propagating", async () => {
    const onClose = vi.fn();
    render(
      <WorldEntryIntakeSheet
        segmentId="seg-l"
        draftEntryId="draft-1"
        onComplete={() => {}}
        onClose={onClose}
      />,
    );
    // Wait for the intake to finish loading so the Save & close button is
    // mounted (the loading-state Sheet uses the header X but the post-load
    // Sheet exposes the explicit Save & close affordance from
    // `ConversationalIntake`).
    await screen.findByRole("heading", { name: /what's this place called\?/i });
    fireEvent.click(screen.getByRole("button", { name: /save .* close/i }));
    await waitFor(() => {
      expect(ipc.worldEntryDeleteIfEmpty).toHaveBeenCalledWith("draft-1");
      expect(onClose).toHaveBeenCalled();
    });
  });

  it("completing the intake routes to the entry view via onComplete", async () => {
    const onComplete = vi.fn();
    render(
      <WorldEntryIntakeSheet
        segmentId="seg-l"
        draftEntryId="draft-1"
        onComplete={onComplete}
        onClose={() => {}}
      />,
    );
    const input = await screen.findByRole("textbox");
    fireEvent.change(input, { target: { value: "The Pell Library" } });
    fireEvent.keyDown(input, { key: "Enter" });
    // Single-field schema -> Enter advances past the end -> onComplete fires
    // with the draft entry id (which is now a real, populated entry).
    await waitFor(() => expect(onComplete).toHaveBeenCalledWith("draft-1"));
  });
});
