import { describe, expect, it, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { InlineField } from "./InlineField";
import type { IntakeField } from "../ipc/commands";

const shortTextField: IntakeField = {
  id: "main.full_name",
  section: "main",
  label: "Full name",
  prompt_question: "Full name?",
  helper: null,
  examples: [],
  kind: { type: "short_text" },
  optional_skip: false,
};

const choiceField: IntakeField = {
  id: "perspectives.gender",
  section: "perspectives",
  label: "Gender",
  prompt_question: "Gender?",
  helper: null,
  examples: [],
  kind: { type: "choice", options: ["female", "male", "nonbinary"] },
  optional_skip: true,
};

const listField: IntakeField = {
  id: "bonus_traits.fears",
  section: "bonus_traits",
  label: "Fears",
  prompt_question: "Fears?",
  helper: null,
  examples: [],
  kind: { type: "string_list" },
  optional_skip: false,
};

describe("InlineField (display mode)", () => {
  it("renders value in display mode when non-empty", () => {
    const onSave = vi.fn();
    render(
      <InlineField field={shortTextField} value="Marcus Vale" onSave={onSave} />,
    );
    expect(screen.getByText("Marcus Vale")).toBeInTheDocument();
    expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
  });

  it("renders an em-dash placeholder when empty", () => {
    const onSave = vi.fn();
    render(<InlineField field={shortTextField} value="" onSave={onSave} />);
    expect(screen.getByText(/— empty —/)).toBeInTheDocument();
  });
});

describe("InlineField (edit mode — short_text)", () => {
  it("enters edit mode on click and saves on blur with new value", async () => {
    const onSave = vi.fn().mockResolvedValue(undefined);
    render(
      <InlineField field={shortTextField} value="Marcus" onSave={onSave} />,
    );
    const cell = screen.getByRole("button", { name: /Edit Full name/ });
    fireEvent.click(cell);
    const input = await screen.findByDisplayValue("Marcus");
    fireEvent.change(input, { target: { value: "Marcus Vale" } });
    fireEvent.blur(input);
    await waitFor(() => {
      expect(onSave).toHaveBeenCalledTimes(1);
      expect(onSave).toHaveBeenCalledWith("Marcus Vale");
    });
  });

  it("Escape reverts changes without calling onSave", async () => {
    const onSave = vi.fn().mockResolvedValue(undefined);
    render(
      <InlineField field={shortTextField} value="Marcus" onSave={onSave} />,
    );
    fireEvent.click(screen.getByRole("button", { name: /Edit Full name/ }));
    const input = await screen.findByDisplayValue("Marcus");
    fireEvent.change(input, { target: { value: "Marcus Tenebris" } });
    fireEvent.keyDown(input, { key: "Escape" });
    expect(onSave).not.toHaveBeenCalled();
    // Display value restored.
    expect(screen.getByText("Marcus")).toBeInTheDocument();
  });

  it("does not call onSave when committed value is unchanged", async () => {
    const onSave = vi.fn().mockResolvedValue(undefined);
    render(
      <InlineField field={shortTextField} value="Marcus" onSave={onSave} />,
    );
    fireEvent.click(screen.getByRole("button", { name: /Edit Full name/ }));
    const input = await screen.findByDisplayValue("Marcus");
    fireEvent.blur(input);
    // Allow any microtasks to flush.
    await Promise.resolve();
    expect(onSave).not.toHaveBeenCalled();
  });

  it("commits on Enter", async () => {
    const onSave = vi.fn().mockResolvedValue(undefined);
    render(<InlineField field={shortTextField} value="" onSave={onSave} />);
    fireEvent.click(screen.getByRole("button", { name: /Edit Full name/ }));
    const input = await screen.findByDisplayValue("");
    fireEvent.change(input, { target: { value: "Skeptic" } });
    fireEvent.keyDown(input, { key: "Enter" });
    await waitFor(() => {
      expect(onSave).toHaveBeenCalledWith("Skeptic");
    });
  });

  it("shows an error chip when onSave rejects", async () => {
    const onSave = vi.fn().mockRejectedValue(new Error("boom"));
    render(<InlineField field={shortTextField} value="" onSave={onSave} />);
    fireEvent.click(screen.getByRole("button", { name: /Edit Full name/ }));
    const input = await screen.findByDisplayValue("");
    fireEvent.change(input, { target: { value: "Skeptic" } });
    fireEvent.blur(input);
    await screen.findByText("Save failed");
  });
});

describe("InlineField (string_list)", () => {
  it("displays a comma-separated value", () => {
    const onSave = vi.fn();
    render(
      <InlineField
        field={listField}
        value={["heights", "abandonment"]}
        onSave={onSave}
      />,
    );
    expect(screen.getByText("heights, abandonment")).toBeInTheDocument();
  });

  it("parses comma input into a trimmed string array on save", async () => {
    const onSave = vi.fn().mockResolvedValue(undefined);
    render(<InlineField field={listField} value={[]} onSave={onSave} />);
    fireEvent.click(screen.getByRole("button", { name: /Edit Fears/ }));
    const input = await screen.findByDisplayValue("");
    fireEvent.change(input, { target: { value: " heights ,  abandonment " } });
    fireEvent.blur(input);
    await waitFor(() => {
      expect(onSave).toHaveBeenCalledWith(["heights", "abandonment"]);
    });
  });
});

describe("InlineField (choice)", () => {
  it("renders a glass dropdown with the kind's options when editing", async () => {
    const onSave = vi.fn().mockResolvedValue(undefined);
    render(
      <InlineField field={choiceField} value="" onSave={onSave} />,
    );
    fireEvent.click(screen.getByRole("button", { name: /Edit Gender/ }));
    // GlassSelect renders a button trigger with aria-label === field
    // label, plus a portal-mounted listbox.
    const trigger = await screen.findByLabelText("Gender");
    fireEvent.click(trigger);
    fireEvent.click(await screen.findByRole("option", { name: /nonbinary/ }));
    await waitFor(() => {
      expect(onSave).toHaveBeenCalledWith("nonbinary");
    });
  });
});
