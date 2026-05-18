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
    const aside = screen.getByRole("complementary");
    expect(aside).toHaveAttribute("data-collapsed", "true");
  });
});
