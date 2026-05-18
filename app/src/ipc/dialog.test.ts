import { describe, it, expect, vi, beforeEach } from "vitest";

const { openMock } = vi.hoisted(() => ({ openMock: vi.fn() }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: openMock }));

import { dialog } from "./dialog";

describe("dialog.pickFolder", () => {
  beforeEach(() => openMock.mockReset());

  it("returns the picked path when user selects a directory", async () => {
    openMock.mockResolvedValue("C:\\Users\\test\\Desktop");
    const r = await dialog.pickFolder();
    expect(r).toBe("C:\\Users\\test\\Desktop");
    expect(openMock).toHaveBeenCalledWith({ directory: true, multiple: false });
  });

  it("returns null when user cancels", async () => {
    openMock.mockResolvedValue(null);
    const r = await dialog.pickFolder();
    expect(r).toBeNull();
  });

  it("returns null when the plugin returns an unexpected array", async () => {
    openMock.mockResolvedValue(["a", "b"]);
    const r = await dialog.pickFolder();
    expect(r).toBeNull();
  });
});
