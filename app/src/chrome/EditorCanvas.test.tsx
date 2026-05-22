import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor, act } from "@testing-library/react";

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (cmd: string, args?: Record<string, unknown>) => invokeMock(cmd, args),
}));

// Phase 5 wired `editor_pills:updated` into the canvas hydrate
// effect; the canvas subscribes via @tauri-apps/api/event. Stub the
// listen call so it resolves to a no-op unlisten instead of trying
// to reach the Tauri runtime (which doesn't exist in jsdom).
vi.mock("@tauri-apps/api/event", () => ({
  listen: () => Promise.resolve(() => {}),
}));

// Mock the ProseMirror editor with a plain textarea so we can drive
// content changes with fireEvent.change. EditorCanvas tests own only
// the autosave + rename wiring; the real editor is covered by
// app/src/editor/Editor.test.tsx.
vi.mock("../editor/Editor", () => ({
  Editor: (props: {
    value: string;
    onChange: (md: string) => void;
    placeholder?: string;
  }) => {
    return (
      <textarea
        data-testid="editor-body"
        aria-label="Scene body"
        placeholder={props.placeholder}
        value={props.value}
        onChange={(e) => props.onChange(e.currentTarget.value)}
      />
    );
  },
}));

// The pill overlay subscribes to Tauri events on mount; this test file
// does not mock `@tauri-apps/api/event`, so stub the layer out. Pill
// behavior is covered by app/src/pill/PillLayer.test.tsx.
vi.mock("../pill/PillLayer", () => ({
  PillLayer: () => null,
}));

// PinnedColumn also fetches state + subscribes on mount; stub it out for
// the same reason. Coverage lives in app/src/pill/PinnedColumn.test.tsx.
vi.mock("../pill/PinnedColumn", () => ({
  PinnedColumn: () => null,
}));

// HeatmapStrip subscribes to heat:updated via @tauri-apps/api/event
// (not mocked here). Stub it out — coverage lives in
// app/src/heat/HeatmapStrip.test.tsx (when it lands in Phase E follow-up).
vi.mock("../heat/HeatmapStrip", () => ({
  HeatmapStrip: () => null,
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
    const body = screen.getByTestId("editor-body");
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

  it("flushes a pending body write to disk when the component unmounts mid-debounce", async () => {
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "scene_read") return "";
      if (cmd === "scene_list") return [{ id: "01H8X4", name: "Scene A", ordering: 0, word_count: 0 }];
      if (cmd === "scene_write_body") {
        return { id: "01H8X4", name: "Scene A", ordering: 0, word_count: 2 };
      }
      return null;
    });

    const { unmount } = render(<EditorCanvas sceneId="01H8X4" onRenamed={() => {}} />);
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("scene_read", { id: "01H8X4" }));
    const body = screen.getByTestId("editor-body");
    fireEvent.change(body, { target: { value: "Mid-debounce text" } });

    // Unmount BEFORE the 2s debounce has fired.
    unmount();

    // Cleanup should have fired the write synchronously (fire-and-forget).
    // It happens during unmount, so we don't need to advance timers.
    await waitFor(() => {
      const writeCalls = invokeMock.mock.calls.filter((c) => c[0] === "scene_write_body");
      expect(writeCalls.length).toBeGreaterThanOrEqual(1);
      const last = writeCalls[writeCalls.length - 1];
      expect(last?.[1]).toMatchObject({ id: "01H8X4", body: "Mid-debounce text" });
    });
  });
});
