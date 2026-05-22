import { beforeEach, describe, expect, it, vi } from "vitest";

// Mock the IPC singleton. Factory is self-contained so vitest hoisting is
// safe. Mirrors `chrome/CharactersSurface.test.tsx`.
vi.mock("../ipc/commands", () => ({
  ipc: {
    characterCreate: vi.fn(),
    characterList: vi.fn(),
  },
}));

import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { CharacterIndex } from "./CharacterIndex";
import { ipc } from "../ipc/commands";

const listMock = ipc.characterList as ReturnType<typeof vi.fn>;
const createMock = ipc.characterCreate as ReturnType<typeof vi.fn>;

const defaultList = [
  {
    id: "c1",
    full_name: "Marcus Vale",
    role: "Skeptic",
    hue_token: "--water-hue-character-1",
    completion: 80,
  },
  {
    id: "c2",
    full_name: "Talia Mor",
    role: "Believer",
    hue_token: "--water-hue-character-2",
    completion: 40,
  },
  {
    id: "c3",
    full_name: "Aren",
    role: null,
    hue_token: "--water-hue-character-3",
    completion: 0,
  },
];

describe("CharacterIndex", () => {
  beforeEach(() => {
    listMock.mockReset().mockResolvedValue(defaultList);
    createMock.mockReset().mockResolvedValue({
      id: "c-new",
      full_name: "",
      role: null,
      hue_token: "--water-hue-character-1",
      completion: 0,
    });
  });

  it("renders character cards", async () => {
    render(
      <CharacterIndex onOpenCharacter={vi.fn()} onOpenIntake={vi.fn()} />,
    );
    await screen.findByText("Marcus Vale");
    expect(screen.getByText("Talia Mor")).toBeInTheDocument();
    expect(screen.getByText("Aren")).toBeInTheDocument();
  });

  it("filters by search", async () => {
    render(
      <CharacterIndex onOpenCharacter={vi.fn()} onOpenIntake={vi.fn()} />,
    );
    await screen.findByText("Marcus Vale");
    fireEvent.change(screen.getByRole("searchbox"), {
      target: { value: "talia" },
    });
    expect(screen.queryByText("Marcus Vale")).not.toBeInTheDocument();
    expect(screen.getByText("Talia Mor")).toBeInTheDocument();
  });

  it("sorts by completion desc", async () => {
    render(
      <CharacterIndex onOpenCharacter={vi.fn()} onOpenIntake={vi.fn()} />,
    );
    await screen.findByText("Marcus Vale");
    // Open the GlassSelect chip and pick "Completion" — GlassSelect
    // renders a button trigger + portal-mounted listbox rather than
    // a native <select>, so we drive it via clicks instead of
    // fireEvent.change.
    fireEvent.click(screen.getByLabelText("Sort characters"));
    fireEvent.click(screen.getByRole("option", { name: /Completion/i }));
    // `data-testid="character-card"` is on each CharacterCard's button,
    // and not on the "+ New character" button, so this filters cleanly.
    const cards = screen.getAllByTestId("character-card");
    expect(cards).toHaveLength(3);
    expect(cards[0]).toHaveTextContent("Marcus Vale"); // 80
    expect(cards[1]).toHaveTextContent("Talia Mor"); // 40
    expect(cards[2]).toHaveTextContent("Aren"); // 0
  });

  it("creates a character and opens intake", async () => {
    const onOpenIntake = vi.fn();
    render(
      <CharacterIndex
        onOpenCharacter={vi.fn()}
        onOpenIntake={onOpenIntake}
      />,
    );
    await screen.findByText("Marcus Vale");
    fireEvent.click(
      screen.getByRole("button", { name: /new character/i }),
    );
    await waitFor(() => {
      expect(onOpenIntake).toHaveBeenCalledWith("c-new");
    });
  });

  it("clicking a card calls onOpenCharacter", async () => {
    const onOpenCharacter = vi.fn();
    render(
      <CharacterIndex
        onOpenCharacter={onOpenCharacter}
        onOpenIntake={vi.fn()}
      />,
    );
    await screen.findByText("Marcus Vale");
    fireEvent.click(screen.getByText("Marcus Vale"));
    expect(onOpenCharacter).toHaveBeenCalledWith(
      "c1",
      "--water-hue-character-1",
    );
  });
});
