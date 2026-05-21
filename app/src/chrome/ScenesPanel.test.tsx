import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (cmd: string, args?: Record<string, unknown>) => invokeMock(cmd, args),
}));

import { ScenesPanel } from "./ScenesPanel";

describe("ScenesPanel", () => {
  beforeEach(() => {
    invokeMock.mockReset();
    localStorage.clear();
  });

  it("renders the project name + new-scene button + list", async () => {
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "scene_list") {
        return [
          { id: "01", name: "Scene A", ordering: 0, word_count: 42 },
          { id: "02", name: "Scene B", ordering: 1, word_count: 7 },
        ];
      }
      return null;
    });
    render(
      <ScenesPanel
        projectName="Test Project"
        activeSceneId="01"
        onSelectScene={() => {}}
        onCreateScene={() => {}}
        onOpenProjectMenu={() => {}}
        collapsed={false}
        onToggleCollapsed={() => {}}
      />,
    );
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("scene_list", undefined));
    expect(screen.getByText("Test Project")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /new scene/i })).toBeInTheDocument();
    expect(screen.getByText("Scene A")).toBeInTheDocument();
    expect(screen.getByText("Scene B")).toBeInTheDocument();
  });

  it("fires onSelectScene with the clicked scene id", async () => {
    invokeMock.mockImplementation(async (cmd: string) => {
      if (cmd === "scene_list") {
        return [{ id: "01", name: "Scene A", ordering: 0, word_count: 0 }];
      }
      return null;
    });
    const onSelect = vi.fn();
    render(
      <ScenesPanel
        projectName="P"
        activeSceneId={null}
        onSelectScene={onSelect}
        onCreateScene={() => {}}
        onOpenProjectMenu={() => {}}
        collapsed={false}
        onToggleCollapsed={() => {}}
      />,
    );
    await waitFor(() => expect(screen.getByText("Scene A")).toBeInTheDocument());
    fireEvent.click(screen.getByRole("button", { name: /Scene A/ }));
    expect(onSelect).toHaveBeenCalledWith("01");
  });

  it("renders collapsed state with width 0", () => {
    invokeMock.mockResolvedValue([]);
    render(
      <ScenesPanel
        projectName="P"
        activeSceneId={null}
        onSelectScene={() => {}}
        onCreateScene={() => {}}
        onOpenProjectMenu={() => {}}
        collapsed
        onToggleCollapsed={() => {}}
      />,
    );
    // Collapsed: the whole panel is now a single glass button.
    const btn = screen.getByRole("button", { name: /expand scenes/i });
    expect(btn).toHaveAttribute("data-collapsed", "true");
  });

  it("reloads scene list when reloadToken changes without remounting", async () => {
    invokeMock.mockImplementationOnce(async (cmd: string) => {
      if (cmd === "scene_list") {
        return [{ id: "01", name: "First", ordering: 0, word_count: 1 }];
      }
      return null;
    });
    const { rerender } = render(
      <ScenesPanel
        projectName="P"
        activeSceneId={null}
        onSelectScene={() => {}}
        onCreateScene={() => {}}
        onOpenProjectMenu={() => {}}
        collapsed={false}
        onToggleCollapsed={() => {}}
        reloadToken={1}
      />,
    );
    await waitFor(() => expect(screen.getByText("First")).toBeInTheDocument());
    // Capture the <aside> node to assert it is the same instance after rerender
    // (a key-based remount would replace this node).
    const asideBefore = screen.getByRole("complementary");
    const scene_list_calls_before = invokeMock.mock.calls.filter((c) => c[0] === "scene_list").length;
    expect(scene_list_calls_before).toBe(1);

    invokeMock.mockImplementationOnce(async (cmd: string) => {
      if (cmd === "scene_list") {
        return [
          { id: "01", name: "First", ordering: 0, word_count: 1 },
          { id: "02", name: "Second", ordering: 1, word_count: 2 },
        ];
      }
      return null;
    });
    rerender(
      <ScenesPanel
        projectName="P"
        activeSceneId={null}
        onSelectScene={() => {}}
        onCreateScene={() => {}}
        onOpenProjectMenu={() => {}}
        collapsed={false}
        onToggleCollapsed={() => {}}
        reloadToken={2}
      />,
    );
    await waitFor(() => expect(screen.getByText("Second")).toBeInTheDocument());
    const asideAfter = screen.getByRole("complementary");
    // Same DOM node instance — proves no remount happened.
    expect(asideAfter).toBe(asideBefore);
    const scene_list_calls_after = invokeMock.mock.calls.filter((c) => c[0] === "scene_list").length;
    expect(scene_list_calls_after).toBe(2);
  });
});
