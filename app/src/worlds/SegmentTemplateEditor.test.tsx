import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor, fireEvent } from "@testing-library/react";

vi.mock("../ipc/commands", () => ({
  ipc: {
    worldSegmentCreate: vi.fn(),
    worldSegmentUpdateTemplate: vi.fn(),
  },
}));

import { SegmentTemplateEditor } from "./SegmentTemplateEditor";
import { ipc } from "../ipc/commands";

describe("SegmentTemplateEditor", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    (ipc.worldSegmentCreate as ReturnType<typeof vi.fn>).mockResolvedValue(
      "new-seg-id",
    );
    (
      ipc.worldSegmentUpdateTemplate as ReturnType<typeof vi.fn>
    ).mockResolvedValue(undefined);
  });

  it("create mode saves a new segment with derived field ids", async () => {
    const onSave = vi.fn();
    render(
      <SegmentTemplateEditor mode="create" onSave={onSave} onClose={() => {}} />,
    );
    fireEvent.change(screen.getByTestId("segment-name-input"), {
      target: { value: "Magic Systems" },
    });
    fireEvent.click(screen.getByLabelText("Collection"));
    fireEvent.click(screen.getByTestId("add-field-button"));
    fireEvent.change(screen.getByTestId("field-label-0"), {
      target: { value: "System Name" },
    });
    fireEvent.click(screen.getByTestId("save-button"));
    await waitFor(() => expect(ipc.worldSegmentCreate).toHaveBeenCalled());
    const call = (ipc.worldSegmentCreate as ReturnType<typeof vi.fn>).mock
      .calls[0]?.[0] as {
      name: string;
      isCollection: boolean;
      template: { fields: Array<{ id: string }> };
    };
    expect(call.name).toBe("Magic Systems");
    expect(call.isCollection).toBe(true);
    expect(call.template.fields[0]?.id).toBe("main.system_name");
    await waitFor(() => expect(onSave).toHaveBeenCalledWith("new-seg-id"));
  });

  it("string_list fields derive ids under the lists.* prefix", async () => {
    render(
      <SegmentTemplateEditor
        mode="create"
        onSave={() => {}}
        onClose={() => {}}
      />,
    );
    fireEvent.change(screen.getByTestId("segment-name-input"), {
      target: { value: "Trinkets" },
    });
    fireEvent.click(screen.getByTestId("add-field-button"));
    fireEvent.change(screen.getByTestId("field-label-0"), {
      target: { value: "Notable Items" },
    });
    // The field-kind cell now wraps a GlassSelect — open it and pick
    // the "list" option (the GlassSelect label for "string_list").
    const kindCell = screen.getByTestId("field-kind-0");
    const kindTrigger = kindCell.querySelector(
      'button[data-glass-select="true"]',
    ) as HTMLButtonElement | null;
    expect(kindTrigger).not.toBeNull();
    fireEvent.click(kindTrigger!);
    fireEvent.click(await screen.findByRole("option", { name: /^list$/ }));
    fireEvent.click(screen.getByTestId("save-button"));
    await waitFor(() => expect(ipc.worldSegmentCreate).toHaveBeenCalled());
    const call = (ipc.worldSegmentCreate as ReturnType<typeof vi.fn>).mock
      .calls[0]?.[0] as { template: { fields: Array<{ id: string }> } };
    expect(call.template.fields[0]?.id).toBe("lists.notable_items");
  });

  it("edit mode on built-in segment locks existing fields and disables name + type", () => {
    render(
      <SegmentTemplateEditor
        mode="edit"
        initial={{
          name: "Concept",
          isCollection: false,
          fields: [
            {
              id: "main.core_premise",
              label: "Core Premise",
              prompt_question: "?",
              kind: { type: "long_text" },
              optional_skip: false,
            },
          ],
          isBuiltin: true,
          segmentId: "seg-concept",
        }}
        onSave={() => {}}
        onClose={() => {}}
      />,
    );
    expect(screen.getByTestId("segment-name-input")).toBeDisabled();
    expect(screen.getByTestId("field-label-0")).toBeDisabled();
    expect(screen.queryByTestId("field-remove-0")).not.toBeInTheDocument();
    expect(screen.getByLabelText("Single document")).toBeDisabled();
    expect(screen.getByLabelText("Collection")).toBeDisabled();
  });

  it("edit mode allows appending new fields on built-in segments", () => {
    render(
      <SegmentTemplateEditor
        mode="edit"
        initial={{
          name: "Concept",
          isCollection: false,
          fields: [
            {
              id: "main.core_premise",
              label: "Core Premise",
              prompt_question: "?",
              kind: { type: "long_text" },
              optional_skip: false,
            },
          ],
          isBuiltin: true,
          segmentId: "seg-concept",
        }}
        onSave={() => {}}
        onClose={() => {}}
      />,
    );
    fireEvent.click(screen.getByTestId("add-field-button"));
    expect(screen.getByTestId("field-row-1")).toBeInTheDocument();
    expect(screen.getByTestId("field-label-1")).not.toBeDisabled();
    expect(screen.getByTestId("field-remove-1")).toBeInTheDocument();
  });

  it("edit mode on built-in saves via worldSegmentUpdateTemplate, not create", async () => {
    const onSave = vi.fn();
    render(
      <SegmentTemplateEditor
        mode="edit"
        initial={{
          name: "Concept",
          isCollection: false,
          fields: [
            {
              id: "main.core_premise",
              label: "Core Premise",
              prompt_question: "?",
              kind: { type: "long_text" },
              optional_skip: false,
            },
          ],
          isBuiltin: true,
          segmentId: "seg-concept",
        }}
        onSave={onSave}
        onClose={() => {}}
      />,
    );
    fireEvent.click(screen.getByTestId("save-button"));
    await waitFor(() =>
      expect(ipc.worldSegmentUpdateTemplate).toHaveBeenCalled(),
    );
    expect(ipc.worldSegmentCreate).not.toHaveBeenCalled();
    await waitFor(() => expect(onSave).toHaveBeenCalledWith("seg-concept"));
  });

  it("Cancel calls onClose without saving", () => {
    const onClose = vi.fn();
    render(
      <SegmentTemplateEditor
        mode="create"
        onSave={() => {}}
        onClose={onClose}
      />,
    );
    fireEvent.click(screen.getByText("Cancel"));
    expect(onClose).toHaveBeenCalled();
    expect(ipc.worldSegmentCreate).not.toHaveBeenCalled();
  });
});
