import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor, act } from "@testing-library/react";

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (cmd: string, args?: Record<string, unknown>) => invokeMock(cmd, args),
}));

import { EditorCanvas } from "./EditorCanvas";

describe("EditorCanvas", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    vi.useFakeTimers({ shouldAdvanceTime: true });
  });

  it("loads the scene body on mount and does NOT write back when only the initial read happened", async () => {
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "scene_read") return "Existing body.";
      if (cmd === "scene_list") return [{ id: "01H8X4", name: "Scene A", ordering: 0, word_count: 2 }];
      return null;
    });

    render(<EditorCanvas sceneId="01H8X4" onRenamed={() => {}} />);
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("scene_read", { id: "01H8X4" }));
    await act(async () => {
      vi.advanceTimersByTime(5000);
    });
    const writeCalls = invokeMock.mock.calls.filter((c) => c[0] === "scene_write_body");
    expect(writeCalls).toHaveLength(0);
  });

  it("calls scene_write_body 2s after the user types in the body", async () => {
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "scene_read") return "";
      if (cmd === "scene_list") return [{ id: "01H8X4", name: "Scene A", ordering: 0, word_count: 0 }];
      if (cmd === "scene_write_body") {
        return { id: "01H8X4", name: "Scene A", ordering: 0, word_count: 2 };
      }
      return null;
    });

    render(<EditorCanvas sceneId="01H8X4" onRenamed={() => {}} />);
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("scene_read", { id: "01H8X4" }));
    const body = screen.getByPlaceholderText(/begin where/i);
    fireEvent.change(body, { target: { value: "Hello world" } });
    await act(async () => {
      vi.advanceTimersByTime(2100);
    });
    const writeCalls = invokeMock.mock.calls.filter((c) => c[0] === "scene_write_body");
    expect(writeCalls).toHaveLength(1);
    expect(writeCalls[0]?.[1]).toMatchObject({ id: "01H8X4", body: "Hello world" });
  });

  it("calls scene_rename when the title input blurs with a changed value", async () => {
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "scene_read") return "";
      if (cmd === "scene_list") return [{ id: "01H8X4", name: "Original", ordering: 0, word_count: 0 }];
      if (cmd === "scene_rename") {
        return { id: "01H8X4", name: "Renamed", ordering: 0, word_count: 0 };
      }
      return null;
    });

    const onRenamed = vi.fn();
    render(<EditorCanvas sceneId="01H8X4" onRenamed={onRenamed} />);
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("scene_list", undefined));

    const title = screen.getByLabelText(/scene title/i);
    fireEvent.change(title, { target: { value: "Renamed" } });
    fireEvent.blur(title);

    await waitFor(() => {
      const rename = invokeMock.mock.calls.find((c) => c[0] === "scene_rename");
      expect(rename).toBeDefined();
      expect(rename?.[1]).toMatchObject({ id: "01H8X4", name: "Renamed" });
    });
    expect(onRenamed).toHaveBeenCalled();
  });
});
