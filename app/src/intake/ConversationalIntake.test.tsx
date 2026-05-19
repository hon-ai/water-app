import { describe, expect, it, vi } from "vitest";
import { render, screen, fireEvent, waitFor, act } from "@testing-library/react";
import type { IntakeSchemaSection } from "../ipc/commands";
import { ConversationalIntake } from "./ConversationalIntake";

const schema: IntakeSchemaSection[] = [
  {
    section: "main",
    fields: [
      {
        id: "full_name",
        section: "main",
        label: "Full name",
        prompt_question: "What's the character's full name?",
        helper: null,
        examples: [],
        kind: { type: "short_text" },
        optional_skip: false,
      },
      {
        id: "role",
        section: "main",
        label: "Role",
        prompt_question: "What role do they play?",
        helper: null,
        examples: [],
        kind: { type: "short_text" },
        optional_skip: true,
      },
    ],
  },
];

// Schema used by the spec § 7 resume-priority test: required, optional,
// required (in order). Resume should skip the optional middle field when
// a later required field is unanswered.
const requiredFirstSchema: IntakeSchemaSection[] = [
  {
    section: "main",
    fields: [
      {
        id: "full_name",
        section: "main",
        label: "Full name",
        prompt_question: "What's the full name?",
        helper: null,
        examples: [],
        kind: { type: "short_text" },
        optional_skip: false,
      },
      {
        id: "aliases",
        section: "main",
        label: "Aliases",
        prompt_question: "Any aliases?",
        helper: null,
        examples: [],
        kind: { type: "string_list" },
        optional_skip: true,
      },
      {
        id: "role",
        section: "main",
        label: "Role",
        prompt_question: "What role?",
        helper: null,
        examples: [],
        kind: { type: "short_text" },
        optional_skip: false,
      },
    ],
  },
];

describe("ConversationalIntake", () => {
  it("renders the first question on mount", () => {
    render(
      <ConversationalIntake
        schema={schema}
        initialValues={{}}
        onAnswer={vi.fn().mockResolvedValue(undefined)}
        onComplete={vi.fn()}
        onClose={vi.fn()}
      />,
    );
    expect(
      screen.getByRole("heading", { name: /full name/i }),
    ).toBeInTheDocument();
    expect(screen.getByTestId("progress")).toHaveTextContent("1 / 2");
  });

  it("advances on Enter after typing", async () => {
    const onAnswer = vi.fn().mockResolvedValue(undefined);
    render(
      <ConversationalIntake
        schema={schema}
        initialValues={{}}
        onAnswer={onAnswer}
        onComplete={vi.fn()}
        onClose={vi.fn()}
      />,
    );
    const input = screen.getByRole("textbox");
    act(() => {
      fireEvent.change(input, { target: { value: "Marcus Vale" } });
    });
    act(() => {
      fireEvent.keyDown(input, { key: "Enter" });
    });
    await waitFor(() => {
      expect(onAnswer).toHaveBeenCalledWith("full_name", "Marcus Vale");
    });
    expect(
      screen.getByRole("heading", { name: /role/i }),
    ).toBeInTheDocument();
  });

  it("does not advance when required field is empty", async () => {
    const onAnswer = vi.fn();
    render(
      <ConversationalIntake
        schema={schema}
        initialValues={{}}
        onAnswer={onAnswer}
        onComplete={vi.fn()}
        onClose={vi.fn()}
      />,
    );
    act(() => {
      fireEvent.click(screen.getByRole("button", { name: /^Next$/ }));
    });
    await waitFor(() => {
      expect(onAnswer).not.toHaveBeenCalled();
    });
    expect(
      screen.getByRole("heading", { name: /full name/i }),
    ).toBeInTheDocument();
  });

  it("skip button is disabled on required fields", () => {
    render(
      <ConversationalIntake
        schema={schema}
        initialValues={{}}
        onAnswer={vi.fn()}
        onComplete={vi.fn()}
        onClose={vi.fn()}
      />,
    );
    const required = screen.getByRole("button", { name: /Required/ });
    expect(required).toBeDisabled();
  });

  it("resumes at first unanswered field", () => {
    render(
      <ConversationalIntake
        schema={schema}
        initialValues={{ full_name: "Marcus Vale" }}
        onAnswer={vi.fn()}
        onComplete={vi.fn()}
        onClose={vi.fn()}
      />,
    );
    expect(
      screen.getByRole("heading", { name: /role/i }),
    ).toBeInTheDocument();
    expect(screen.getByTestId("progress")).toHaveTextContent("2 / 2");
  });

  it("calls onComplete after last field", async () => {
    const onAnswer = vi.fn().mockResolvedValue(undefined);
    const onComplete = vi.fn();
    render(
      <ConversationalIntake
        schema={schema}
        initialValues={{ full_name: "Marcus Vale" }}
        onAnswer={onAnswer}
        onComplete={onComplete}
        onClose={vi.fn()}
      />,
    );
    const input = screen.getByRole("textbox");
    act(() => {
      fireEvent.change(input, { target: { value: "Skeptic" } });
    });
    act(() => {
      fireEvent.keyDown(input, { key: "Enter" });
    });
    await waitFor(() => {
      expect(onComplete).toHaveBeenCalled();
    });
  });

  it("renders helper text and examples when set", () => {
    const schemaWithHelper: IntakeSchemaSection[] = [
      {
        section: "main",
        fields: [
          {
            id: "full_name",
            section: "main",
            label: "Full name",
            prompt_question: "What's the character's full name?",
            helper: "Use the protagonist's full legal name.",
            examples: ["Marcus Vale", "Talia Rho"],
            kind: { type: "short_text" },
            optional_skip: false,
          },
        ],
      },
    ];
    render(
      <ConversationalIntake
        schema={schemaWithHelper}
        initialValues={{}}
        onAnswer={vi.fn().mockResolvedValue(undefined)}
        onComplete={vi.fn()}
        onClose={vi.fn()}
      />,
    );
    expect(
      screen.getByText(/Use the protagonist's full legal name\./),
    ).toBeInTheDocument();
    // Examples rendered together; just assert both strings are in the DOM.
    expect(screen.getByText(/Marcus Vale/)).toBeInTheDocument();
    expect(screen.getByText(/Talia Rho/)).toBeInTheDocument();
  });

  it("focuses the next field's input on advance", async () => {
    const onAnswer = vi.fn().mockResolvedValue(undefined);
    render(
      <ConversationalIntake
        schema={schema}
        initialValues={{}}
        onAnswer={onAnswer}
        onComplete={vi.fn()}
        onClose={vi.fn()}
      />,
    );
    const firstInput = screen.getByRole("textbox");
    fireEvent.change(firstInput, { target: { value: "Marcus Vale" } });
    fireEvent.keyDown(firstInput, { key: "Enter" });
    await waitFor(() => {
      expect(
        screen.getByRole("heading", { name: /role/i }),
      ).toBeInTheDocument();
    });
    // The second field's input should now have focus thanks to the
    // key={field.id} remount + autoFocus.
    expect(screen.getByRole("textbox")).toHaveFocus();
  });

  it("calls onComplete on mount if all fields are already answered", async () => {
    const onComplete = vi.fn();
    render(
      <ConversationalIntake
        schema={schema}
        initialValues={{ full_name: "Marcus Vale", role: "Skeptic" }}
        onAnswer={vi.fn()}
        onComplete={onComplete}
        onClose={vi.fn()}
      />,
    );
    await waitFor(() => {
      expect(onComplete).toHaveBeenCalledTimes(1);
    });
  });

  it("Back restores the just-confirmed answer from in-session state", async () => {
    const onAnswer = vi.fn().mockResolvedValue(undefined);
    render(
      <ConversationalIntake
        schema={schema}
        initialValues={{}}
        onAnswer={onAnswer}
        onComplete={vi.fn()}
        onClose={vi.fn()}
      />,
    );
    // Answer field 1.
    fireEvent.change(screen.getByRole("textbox"), {
      target: { value: "Marcus Vale" },
    });
    fireEvent.keyDown(screen.getByRole("textbox"), { key: "Enter" });
    await waitFor(() => {
      expect(
        screen.getByRole("heading", { name: /role/i }),
      ).toBeInTheDocument();
    });
    // Now Back to field 1.
    fireEvent.click(screen.getByRole("button", { name: "Back" }));
    // The input should pre-fill with what was just typed, not the
    // original empty `initialValues`.
    expect(screen.getByRole("textbox")).toHaveValue("Marcus Vale");
  });

  it("resumes at first unanswered required, skipping prior unanswered optionals (spec § 7)", () => {
    render(
      <ConversationalIntake
        schema={requiredFirstSchema}
        // `aliases` (optional) unanswered, `role` (required) unanswered.
        initialValues={{ full_name: "Marcus Vale" }}
        onAnswer={vi.fn()}
        onComplete={vi.fn()}
        onClose={vi.fn()}
      />,
    );
    // Should jump past `aliases` (optional) to `role` (required).
    expect(
      screen.getByRole("heading", { name: /what role/i }),
    ).toBeInTheDocument();
  });
});
