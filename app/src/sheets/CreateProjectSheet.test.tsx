import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";

const { invokeMock, pickFolderMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  pickFolderMock: vi.fn(),
}));

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (cmd: string, args?: Record<string, unknown>) => invokeMock(cmd, args),
}));
vi.mock("../ipc/dialog", () => ({
  dialog: { pickFolder: () => pickFolderMock() },
}));

// jsdom does not implement HTMLDialogElement; stub the minimum surface.
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
  pickFolderMock.mockReset();
});

import { CreateProjectSheet } from "./CreateProjectSheet";

describe("CreateProjectSheet", () => {
  it("Browse… fills the parent dir from the native folder picker", async () => {
    pickFolderMock.mockResolvedValue("C:\\Users\\me\\Desktop");
    render(<CreateProjectSheet open onClose={() => {}} onCreated={() => {}} />);
    fireEvent.click(screen.getByRole("button", { name: /browse/i }));
    await waitFor(() => {
      expect(screen.getByLabelText(/parent directory/i)).toHaveValue("C:\\Users\\me\\Desktop");
    });
  });

  it("Submit calls ipc.createProject with the form values + fires onCreated", async () => {
    invokeMock.mockResolvedValue({
      root: "C:\\Users\\me\\Desktop\\Demo.water",
      name: "Demo",
      project_id: "01H8X4",
      default_manuscript_id: "01H8X5",
    });
    const onCreated = vi.fn();
    render(<CreateProjectSheet open onClose={() => {}} onCreated={onCreated} />);
    fireEvent.change(screen.getByLabelText(/project name/i), { target: { value: "Demo" } });
    fireEvent.change(screen.getByLabelText(/parent directory/i), {
      target: { value: "C:\\Users\\me\\Desktop" },
    });
    fireEvent.click(screen.getByRole("button", { name: /create/i }));
    await waitFor(() => {
      expect(invokeMock).toHaveBeenCalledWith("create_project", {
        parentDir: "C:\\Users\\me\\Desktop",
        name: "Demo",
      });
      expect(onCreated).toHaveBeenCalled();
    });
  });
});
