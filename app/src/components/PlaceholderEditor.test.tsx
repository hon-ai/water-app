import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor, act } from "@testing-library/react";

const invokeMock = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (cmd: string, args?: Record<string, unknown>) => invokeMock(cmd, args),
}));

import { PlaceholderEditor } from "./PlaceholderEditor";

describe("PlaceholderEditor", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    vi.useFakeTimers({ shouldAdvanceTime: true });
  });

  it("does NOT call scene_write_body when only the initial read has happened", async () => {
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "scene_read") return "Existing body.";
      if (cmd === "scene_write_body") {
        return { id: "x", name: "S", ordering: 0, word_count: 2 };
      }
      return null;
    });

    render(<PlaceholderEditor sceneId="01H8X4" />);

    // Flush the initial scene_read.
    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("scene_read", { id: "01H8X4" });
    });

    // Advance well past the 2s autosave debounce.
    await act(async () => {
      vi.advanceTimersByTime(5000);
    });

    // The write must NOT have fired — the user didn't touch the textarea.
    const writeCalls = invokeMock.mock.calls.filter((c) => c[0] === "scene_write_body");
    expect(writeCalls).toHaveLength(0);
  });

  it("calls scene_write_body 2s after the user types", async () => {
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "scene_read") return "";
      if (cmd === "scene_write_body") {
        return { id: "x", name: "S", ordering: 0, word_count: 2 };
      }
      return null;
    });

    render(<PlaceholderEditor sceneId="01H8X4" />);
    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("scene_read", { id: "01H8X4" });
    });

    const textarea = screen.getByPlaceholderText(/begin where/i);
    fireEvent.change(textarea, { target: { value: "Hello world" } });

    await act(async () => {
      vi.advanceTimersByTime(2100);
    });

    const writeCalls = invokeMock.mock.calls.filter((c) => c[0] === "scene_write_body");
    expect(writeCalls).toHaveLength(1);
    expect(writeCalls[0]?.[1]).toMatchObject({ id: "01H8X4", body: "Hello world" });
  });
});
