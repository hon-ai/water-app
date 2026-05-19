import { beforeEach, describe, expect, it, vi } from "vitest";

// Mock the IPC singleton. Factory is self-contained so vitest hoisting is
// safe. Mirrors `intake/CharacterIntakeSheet.test.tsx`.
vi.mock("../ipc/commands", () => ({
  ipc: {
    intakeSchema: vi.fn(),
    characterRead: vi.fn(),
    characterUpdateField: vi.fn(),
  },
}));

import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { CharacterSheet } from "./CharacterSheet";
import { ipc } from "../ipc/commands";
import type {
  CharacterFile,
  CharacterIndexEntry,
  IntakeSchemaSection,
} from "../ipc/commands";

const mockSchema: IntakeSchemaSection[] = [
  {
    section: "main",
    fields: [
      {
        id: "main.full_name",
        section: "main",
        label: "Full name",
        prompt_question: "?",
        helper: null,
        examples: [],
        kind: { type: "short_text" },
        optional_skip: false,
      },
      {
        id: "main.role_in_story",
        section: "main",
        label: "Role",
        prompt_question: "?",
        helper: null,
        examples: [],
        kind: { type: "short_text" },
        optional_skip: false,
      },
    ],
  },
];

const mockFile: CharacterFile = {
  id: "c1",
  name: "Marcus Vale",
  schema_version: "lsm-v2.1",
  // Section at TOP level (Rust uses `#[serde(flatten)]` on `data`).
  main: { full_name: "Marcus Vale", role_in_story: "" },
};

const mockUpdated: CharacterIndexEntry = {
  id: "c1",
  full_name: "Marcus Vale",
  role: null,
  hue_token: "--water-hue-character-1",
  completion: 50,
};

beforeEach(() => {
  (ipc.intakeSchema as ReturnType<typeof vi.fn>).mockReset();
  (ipc.characterRead as ReturnType<typeof vi.fn>).mockReset();
  (ipc.characterUpdateField as ReturnType<typeof vi.fn>).mockReset();
  (ipc.intakeSchema as ReturnType<typeof vi.fn>).mockResolvedValue(mockSchema);
  (ipc.characterRead as ReturnType<typeof vi.fn>).mockResolvedValue(mockFile);
  (ipc.characterUpdateField as ReturnType<typeof vi.fn>).mockResolvedValue(
    mockUpdated,
  );
});

describe("CharacterSheet", () => {
  it("renders heading and 50% completion (1/2 required filled)", async () => {
    render(
      <CharacterSheet
        characterId="c1"
        hueToken="--water-hue-character-1"
        onBackToIndex={vi.fn()}
        onContinueIntake={vi.fn()}
      />,
    );
    await screen.findByRole("heading", { level: 1, name: "Marcus Vale" });
    expect(screen.getByText(/50% complete/)).toBeInTheDocument();
  });

  it("shows Continue intake when completion < 100", async () => {
    render(
      <CharacterSheet
        characterId="c1"
        hueToken="--water-hue-character-1"
        onBackToIndex={vi.fn()}
        onContinueIntake={vi.fn()}
      />,
    );
    await screen.findByRole("button", { name: /continue intake/i });
  });

  it("saves a field on blur using the dotted-path field id", async () => {
    render(
      <CharacterSheet
        characterId="c1"
        hueToken="--water-hue-character-1"
        onBackToIndex={vi.fn()}
        onContinueIntake={vi.fn()}
      />,
    );
    // Wait for fields to render.
    await screen.findByRole("heading", { level: 1, name: "Marcus Vale" });
    const roleCell = screen.getByRole("button", { name: /Edit Role/ });
    fireEvent.click(roleCell);
    const input = await screen.findByDisplayValue("");
    fireEvent.change(input, { target: { value: "Skeptic" } });
    fireEvent.blur(input);
    await waitFor(() => {
      expect(ipc.characterUpdateField).toHaveBeenCalledWith(
        "c1",
        "main.role_in_story",
        "Skeptic",
      );
    });
  });

  it("Escape reverts changes without calling characterUpdateField", async () => {
    render(
      <CharacterSheet
        characterId="c1"
        hueToken="--water-hue-character-1"
        onBackToIndex={vi.fn()}
        onContinueIntake={vi.fn()}
      />,
    );
    await screen.findByRole("heading", { level: 1, name: "Marcus Vale" });
    // Click the name cell (display mode).
    const nameCell = screen.getByRole("button", { name: /Edit Full name/ });
    fireEvent.click(nameCell);
    const input = await screen.findByDisplayValue("Marcus Vale");
    fireEvent.change(input, { target: { value: "Marcus Tenebris" } });
    fireEvent.keyDown(input, { key: "Escape" });
    expect(ipc.characterUpdateField).not.toHaveBeenCalled();
    // The display still shows the original value (h1 + the inline cell).
    expect(
      screen.getByRole("heading", { level: 1, name: "Marcus Vale" }),
    ).toBeInTheDocument();
  });

  it("hides Continue intake when completion === 100", async () => {
    (ipc.characterRead as ReturnType<typeof vi.fn>).mockResolvedValue({
      id: "c1",
      name: "Marcus Vale",
      schema_version: "lsm-v2.1",
      main: { full_name: "Marcus Vale", role_in_story: "Skeptic" },
    } satisfies CharacterFile);
    render(
      <CharacterSheet
        characterId="c1"
        hueToken="--water-hue-character-1"
        onBackToIndex={vi.fn()}
        onContinueIntake={vi.fn()}
      />,
    );
    await screen.findByRole("heading", { level: 1, name: "Marcus Vale" });
    expect(screen.getByText(/100% complete/)).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: /continue intake/i }),
    ).not.toBeInTheDocument();
  });

  it("plumbs hueToken through to data-hue-token attribute", async () => {
    const { container } = render(
      <CharacterSheet
        characterId="c1"
        hueToken="--water-hue-character-3"
        onBackToIndex={vi.fn()}
        onContinueIntake={vi.fn()}
      />,
    );
    await screen.findByRole("heading", { level: 1, name: "Marcus Vale" });
    const sheet = container.querySelector(".water-character-sheet");
    expect(sheet).not.toBeNull();
    expect(sheet?.getAttribute("data-hue-token")).toBe(
      "--water-hue-character-3",
    );
  });

  it("invokes onBackToIndex when the back button is clicked", async () => {
    const onBack = vi.fn();
    render(
      <CharacterSheet
        characterId="c1"
        hueToken="--water-hue-character-1"
        onBackToIndex={onBack}
        onContinueIntake={vi.fn()}
      />,
    );
    const back = await screen.findByRole("button", {
      name: /All characters/,
    });
    fireEvent.click(back);
    expect(onBack).toHaveBeenCalledTimes(1);
  });
});
