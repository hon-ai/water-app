import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor, act } from "@testing-library/react";
import { ThemeProvider } from "../theme/ThemeProvider";

const { onWaterEventMock, ipcMock } = vi.hoisted(() => ({
  onWaterEventMock: vi.fn(),
  ipcMock: {
    diagnosticsStatus: vi.fn(),
    providerTest: vi.fn(),
  },
}));

vi.mock("../ipc/events", () => ({ onWaterEvent: onWaterEventMock }));
vi.mock("../ipc/commands", () => ({ ipc: ipcMock }));

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
  onWaterEventMock.mockReset();
  ipcMock.diagnosticsStatus.mockReset();
  ipcMock.providerTest.mockReset();
  // Default: subscriptions resolve to a no-op unsub.
  onWaterEventMock.mockResolvedValue(vi.fn());
  localStorage.clear();
});

import { SettingsSheet } from "./SettingsSheet";

const wrap = (ui: React.ReactNode) => <ThemeProvider>{ui}</ThemeProvider>;

const emptyStatus = {
  has_open_project: false,
  project_root: null,
  router_primary_id: null,
  sidecar: null,
  provider_health: [],
};

describe("SettingsSheet", () => {
  it("renders three sections (Appearance, Providers, Developer info)", async () => {
    ipcMock.diagnosticsStatus.mockResolvedValue(emptyStatus);
    render(wrap(<SettingsSheet open onClose={() => {}} />));
    await waitFor(() => expect(ipcMock.diagnosticsStatus).toHaveBeenCalledTimes(1));
    expect(screen.getByText(/appearance/i)).toBeInTheDocument();
    expect(screen.getByText(/providers/i)).toBeInTheDocument();
    expect(screen.getByText(/developer/i)).toBeInTheDocument();
  });

  it("clicking the dark theme segment sets data-theme to dark", async () => {
    ipcMock.diagnosticsStatus.mockResolvedValue(emptyStatus);
    render(wrap(<SettingsSheet open onClose={() => {}} />));
    fireEvent.click(screen.getByRole("button", { name: /^dark$/i }));
    expect(document.documentElement.getAttribute("data-theme")).toBe("dark");
  });
});

describe("SettingsSheet event subscription", () => {
  it("subscribes to sidecar:status on open and unsubscribes on close", async () => {
    const unsub = vi.fn();
    onWaterEventMock.mockResolvedValue(unsub);
    ipcMock.diagnosticsStatus.mockResolvedValue({
      has_open_project: true,
      project_root: "/p",
      router_primary_id: null,
      sidecar: { base_url: "http://localhost:1", status: "loading", last_status_detail: null },
      provider_health: [],
    });
    const { rerender } = render(wrap(<SettingsSheet open={true} onClose={() => {}} />));
    await waitFor(() =>
      expect(onWaterEventMock).toHaveBeenCalledWith("sidecar:status", expect.any(Function)),
    );
    rerender(wrap(<SettingsSheet open={false} onClose={() => {}} />));
    await waitFor(() => expect(unsub).toHaveBeenCalled());
  });

  it("does NOT poll on a setInterval", async () => {
    vi.useFakeTimers({ shouldAdvanceTime: true });
    onWaterEventMock.mockResolvedValue(vi.fn());
    ipcMock.diagnosticsStatus.mockResolvedValue({
      has_open_project: true,
      project_root: "/p",
      router_primary_id: null,
      sidecar: null,
      provider_health: [],
    });
    render(wrap(<SettingsSheet open={true} onClose={() => {}} />));
    await waitFor(() => expect(ipcMock.diagnosticsStatus).toHaveBeenCalledTimes(1));
    vi.advanceTimersByTime(30_000);
    expect(ipcMock.diagnosticsStatus).toHaveBeenCalledTimes(1);
    vi.useRealTimers();
  });

  it("updates sidecar status when an event fires", async () => {
    let handler: ((p: { status: string; detail: string | null }) => void) | null = null;
    onWaterEventMock.mockImplementation(async (_name: string, cb: unknown) => {
      handler = cb as (p: { status: string; detail: string | null }) => void;
      return vi.fn();
    });
    ipcMock.diagnosticsStatus.mockResolvedValue({
      has_open_project: true,
      project_root: "/p",
      router_primary_id: null,
      sidecar: { base_url: "http://localhost:1", status: "loading", last_status_detail: null },
      provider_health: [],
    });
    render(wrap(<SettingsSheet open={true} onClose={() => {}} />));
    await waitFor(() => expect(handler).not.toBeNull());
    await act(async () => {
      handler!({ status: "ready", detail: null });
    });
    await waitFor(() => expect(screen.getByText(/"status": "ready"/)).toBeInTheDocument());
  });

  it("invokes unsub even when the sheet closes before onWaterEvent resolves", async () => {
    let resolveOnWaterEvent: ((u: () => void) => void) | null = null;
    const unsub = vi.fn();
    onWaterEventMock.mockImplementation(
      () =>
        new Promise<() => void>((resolve) => {
          resolveOnWaterEvent = resolve;
        }),
    );
    ipcMock.diagnosticsStatus.mockResolvedValue({
      has_open_project: true,
      project_root: "/p",
      router_primary_id: null,
      sidecar: null,
      provider_health: [],
    });
    const { rerender } = render(wrap(<SettingsSheet open={true} onClose={() => {}} />));
    // Close before the subscription resolves.
    rerender(wrap(<SettingsSheet open={false} onClose={() => {}} />));
    // Now resolve the subscription. The cleanup already ran with cancelled=true,
    // so the resolved unsub should be invoked immediately.
    await act(async () => {
      resolveOnWaterEvent!(unsub);
    });
    await waitFor(() => expect(unsub).toHaveBeenCalled());
  });
});
