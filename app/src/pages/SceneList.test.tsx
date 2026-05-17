import { describe, it, expect, vi } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (cmd: string) => {
    if (cmd === "scene_list") return [];
    if (cmd === "scene_create") return { id: "x", name: "Scene 1", ordering: 0, word_count: 0 };
    return null;
  }),
}));

import { SceneList } from "./SceneList";

describe("SceneList", () => {
  it("renders the new-scene button when list is empty", async () => {
    render(<SceneList />);
    await waitFor(() => {
      expect(screen.getByText(/new scene/i)).toBeInTheDocument();
    });
  });
});
