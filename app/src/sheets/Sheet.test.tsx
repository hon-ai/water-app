import { describe, it, expect, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { Sheet } from "./Sheet";

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
});

describe("Sheet slide-in", () => {
  it("transitions data-state from opening to open after mount", async () => {
    render(
      <Sheet open={true} onClose={() => {}} title="t">
        x
      </Sheet>,
    );
    const dlg = screen.getByText("x").closest("dialog")!;
    await waitFor(() => expect(dlg.getAttribute("data-state")).toBe("open"));
    expect(dlg.style.transform).toContain("translateX(0)");
  });
});
