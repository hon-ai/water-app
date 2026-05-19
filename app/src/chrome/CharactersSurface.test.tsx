import { describe, expect, it, vi } from "vitest";

/**
 * `CharactersSurface` is the router for the Characters area (T20). These
 * tests pin the *routing* contract — index ↔ sheet navigation, intake
 * overlay, scroll-preservation mount strategy — not the inner
 * components' behavior (which is covered by CharacterIndex/Sheet/Intake
 * suites). All three children are module-mocked so this file makes
 * zero IPC calls.
 */
vi.mock("../characters/CharacterIndex", () => ({
  CharacterIndex: ({
    onOpenCharacter,
    onOpenIntake,
    reloadKey,
  }: {
    onOpenCharacter: (id: string, hueToken: string) => void;
    onOpenIntake: (id: string) => void;
    reloadKey?: number;
  }) => (
    <div data-testid="character-index" data-reload-key={reloadKey ?? 0}>
      <button onClick={() => onOpenCharacter("c1", "--water-hue-character-1")}>
        open c1
      </button>
      <button onClick={() => onOpenIntake("c-new")}>new intake</button>
    </div>
  ),
}));

vi.mock("../characters/CharacterSheet", () => ({
  CharacterSheet: ({
    characterId,
    hueToken,
    onBackToIndex,
    onContinueIntake,
  }: {
    characterId: string;
    hueToken: string;
    onBackToIndex: () => void;
    onContinueIntake: () => void;
  }) => (
    <div
      data-testid="character-sheet"
      data-character-id={characterId}
      data-hue-token={hueToken}
    >
      <span>sheet for {characterId}</span>
      <button onClick={onBackToIndex}>back</button>
      <button onClick={onContinueIntake}>continue</button>
    </div>
  ),
}));

vi.mock("../intake/CharacterIntakeSheet", () => ({
  CharacterIntakeSheet: ({
    characterId,
    open,
    onClose,
    onCompleted,
  }: {
    characterId: string;
    open: boolean;
    onClose: () => void;
    onCompleted: () => void;
  }) =>
    open ? (
      <div data-testid="intake-sheet" data-character-id={characterId}>
        <span>intake for {characterId}</span>
        <button onClick={onClose}>close intake</button>
        <button onClick={onCompleted}>complete intake</button>
      </div>
    ) : null,
}));

import { fireEvent, render, screen } from "@testing-library/react";
import { CharactersSurface } from "./CharactersSurface";

describe("CharactersSurface (router)", () => {
  it("renders the index view by default", () => {
    render(<CharactersSurface />);
    const index = screen.getByTestId("character-index");
    expect(index).toBeInTheDocument();
    // Visible (no display:none).
    expect(index.parentElement?.style.display).toBe("block");
    expect(screen.queryByTestId("character-sheet")).toBeNull();
    expect(screen.queryByTestId("intake-sheet")).toBeNull();
  });

  it("navigates to sheet view when a card is opened (carries hueToken)", () => {
    render(<CharactersSurface />);
    fireEvent.click(screen.getByText("open c1"));
    const sheet = screen.getByTestId("character-sheet");
    expect(sheet).toHaveAttribute("data-character-id", "c1");
    expect(sheet).toHaveAttribute("data-hue-token", "--water-hue-character-1");
    // Index stays mounted (scroll preservation) but hidden.
    const index = screen.getByTestId("character-index");
    expect(index.parentElement?.style.display).toBe("none");
  });

  it("back button returns to the index view", () => {
    render(<CharactersSurface />);
    fireEvent.click(screen.getByText("open c1"));
    fireEvent.click(screen.getByText("back"));
    expect(screen.queryByTestId("character-sheet")).toBeNull();
    const index = screen.getByTestId("character-index");
    expect(index.parentElement?.style.display).toBe("block");
  });

  it("opens the intake sheet when triggered from the index", () => {
    render(<CharactersSurface />);
    fireEvent.click(screen.getByText("new intake"));
    const intake = screen.getByTestId("intake-sheet");
    expect(intake).toHaveAttribute("data-character-id", "c-new");
    // Index remains mounted and visible underneath.
    const index = screen.getByTestId("character-index");
    expect(index.parentElement?.style.display).toBe("block");
  });

  it("closing intake returns control to the underlying view", () => {
    render(<CharactersSurface />);
    fireEvent.click(screen.getByText("new intake"));
    fireEvent.click(screen.getByText("close intake"));
    expect(screen.queryByTestId("intake-sheet")).toBeNull();
    expect(screen.getByTestId("character-index")).toBeInTheDocument();
  });

  it("Continue intake from the sheet view opens intake for that character", () => {
    render(<CharactersSurface />);
    fireEvent.click(screen.getByText("open c1"));
    fireEvent.click(screen.getByText("continue"));
    const intake = screen.getByTestId("intake-sheet");
    expect(intake).toHaveAttribute("data-character-id", "c1");
    // Sheet stays underneath while intake is overlaid.
    expect(screen.getByTestId("character-sheet")).toBeInTheDocument();
  });

  it("intake onCompleted bumps reloadKey so the index refetches", () => {
    render(<CharactersSurface />);
    expect(screen.getByTestId("character-index")).toHaveAttribute(
      "data-reload-key",
      "0",
    );
    fireEvent.click(screen.getByText("new intake"));
    fireEvent.click(screen.getByText("complete intake"));
    expect(screen.getByTestId("character-index")).toHaveAttribute(
      "data-reload-key",
      "1",
    );
  });
});
