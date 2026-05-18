import { describe, expect, it, vi } from "vitest";

const { openMock } = vi.hoisted(() => ({ openMock: vi.fn() }));
vi.mock("@tauri-apps/plugin-shell", () => ({ open: openMock }));

import { ipc } from "./commands";

describe("ipc.openExternalLink", () => {
  it("invokes plugin-shell open with the URL", async () => {
    openMock.mockResolvedValue(undefined);
    await ipc.openExternalLink("https://example.com");
    expect(openMock).toHaveBeenCalledWith("https://example.com");
  });

  it("propagates plugin errors", async () => {
    openMock.mockRejectedValue(new Error("scope denied"));
    await expect(ipc.openExternalLink("ftp://nope")).rejects.toThrow(
      "scope denied",
    );
  });
});
