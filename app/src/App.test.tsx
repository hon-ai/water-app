import { render, screen, waitFor } from "@testing-library/react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import App from "./App";

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (cmd: string, args?: Record<string, unknown>) => invokeMock(cmd, args),
}));
vi.mock("./ipc/dialog", () => ({
  dialog: { pickFolder: async () => null },
}));

beforeEach(() => {
  if (!HTMLDialogElement.prototype.showModal) {
    HTMLDialogElement.prototype.showModal = function () {
      (this as unknown as { open: boolean }).open = true;
    };
  }
  if (!HTMLDialogElement.prototype.close) {
    HTMLDialogElement.prototype.close = function () {
      (this as unknown as { open: boolean }).open = false;
    };
  }
  invokeMock.mockReset();
  localStorage.clear();
});

describe("App", () => {
  it("renders the icon rail and the empty state when no project is open", async () => {
    invokeMock.mockResolvedValue({
      has_open_project: false,
      project_root: null,
      router_primary_id: null,
      sidecar: null,
      provider_health: [],
    });
    render(<App />);
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("diagnostics_status", undefined));
    expect(screen.getByRole("button", { name: /settings/i })).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: /just flow/i })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /create new project/i })).toBeInTheDocument();
  });
});
