import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { ThemeProvider } from "../theme/ThemeProvider";

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (cmd: string, args?: Record<string, unknown>) => invokeMock(cmd, args),
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

import { SettingsSheet } from "./SettingsSheet";

const wrap = (ui: React.ReactNode) => <ThemeProvider>{ui}</ThemeProvider>;

describe("SettingsSheet", () => {
  it("renders three sections (Appearance, Providers, Developer info)", async () => {
    invokeMock.mockResolvedValue({
      has_open_project: false,
      project_root: null,
      router_primary_id: null,
      sidecar: null,
      provider_health: [],
    });
    render(wrap(<SettingsSheet open onClose={() => {}} />));
    await waitFor(() => expect(invokeMock).toHaveBeenCalledWith("diagnostics_status", undefined));
    expect(screen.getByText(/appearance/i)).toBeInTheDocument();
    expect(screen.getByText(/providers/i)).toBeInTheDocument();
    expect(screen.getByText(/developer/i)).toBeInTheDocument();
  });

  it("clicking the dark theme segment sets data-theme to dark", async () => {
    invokeMock.mockResolvedValue({
      has_open_project: false,
      project_root: null,
      router_primary_id: null,
      sidecar: null,
      provider_health: [],
    });
    render(wrap(<SettingsSheet open onClose={() => {}} />));
    fireEvent.click(screen.getByRole("button", { name: /^dark$/i }));
    expect(document.documentElement.getAttribute("data-theme")).toBe("dark");
  });
});
