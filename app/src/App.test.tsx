import { render, screen } from "@testing-library/react";
import { describe, it, expect, vi } from "vitest";
import App from "./App";

// SceneList mounts on App render and calls ipc.sceneList(). In jsdom there
// is no Tauri IPC bridge, so we mock @tauri-apps/api/core to keep the test
// hermetic.
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async () => []),
}));

describe("App", () => {
  it("renders the Water title", () => {
    render(<App />);
    expect(screen.getByText("Water")).toBeInTheDocument();
  });
});
