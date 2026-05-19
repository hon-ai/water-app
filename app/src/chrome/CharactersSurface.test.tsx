import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../ipc/commands", () => ({
  ipc: {
    characterCreate: vi.fn().mockResolvedValue({
      id: "c-new",
      full_name: "",
      role: null,
      hue_token: "--water-hue-character-1",
      completion: 0,
    }),
    characterList: vi.fn().mockResolvedValue([]),
  },
}));

vi.mock("../intake/CharacterIntakeSheet", () => ({
  CharacterIntakeSheet: ({
    open,
    onClose,
  }: {
    open: boolean;
    onClose: () => void;
  }) =>
    open ? (
      <div data-testid="intake-sheet">
        <button onClick={onClose}>close</button>
      </div>
    ) : null,
}));

import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { CharactersSurface } from "./CharactersSurface";
import { ipc } from "../ipc/commands";

const listMock = ipc.characterList as ReturnType<typeof vi.fn>;
const createMock = ipc.characterCreate as ReturnType<typeof vi.fn>;

describe("CharactersSurface", () => {
  beforeEach(() => {
    listMock.mockReset().mockResolvedValue([]);
    createMock.mockReset().mockResolvedValue({
      id: "c-new",
      full_name: "",
      role: null,
      hue_token: "--water-hue-character-1",
      completion: 0,
    });
  });

  it("shows empty state initially", async () => {
    render(<CharactersSurface />);
    await screen.findByText(/no characters yet/i);
  });

  it("creates a character and opens intake", async () => {
    listMock
      .mockReset()
      .mockResolvedValueOnce([])
      .mockResolvedValueOnce([
        {
          id: "c-new",
          full_name: "",
          role: null,
          hue_token: "--water-hue-character-1",
          completion: 0,
        },
      ]);
    render(<CharactersSurface />);
    fireEvent.click(
      await screen.findByRole("button", { name: /new character/i }),
    );
    await waitFor(() => {
      expect(screen.getByTestId("intake-sheet")).toBeInTheDocument();
    });
  });

  it("shows 'Continue intake' for incomplete characters and opens intake on click", async () => {
    listMock.mockReset().mockResolvedValue([
      {
        id: "c-partial",
        full_name: "Marcus",
        role: null,
        hue_token: "--water-hue-character-1",
        completion: 50,
      },
    ]);
    render(<CharactersSurface />);
    const continueBtn = await screen.findByRole("button", {
      name: /continue intake/i,
    });
    fireEvent.click(continueBtn);
    expect(screen.getByTestId("intake-sheet")).toBeInTheDocument();
  });

  it("does not show 'Continue intake' for 100%-complete characters", async () => {
    listMock.mockReset().mockResolvedValue([
      {
        id: "c-done",
        full_name: "Talia",
        role: null,
        hue_token: "--water-hue-character-2",
        completion: 100,
      },
    ]);
    render(<CharactersSurface />);
    await screen.findByText(/talia/i);
    expect(
      screen.queryByRole("button", { name: /continue intake/i }),
    ).toBeNull();
  });
});
