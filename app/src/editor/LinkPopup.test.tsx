import { describe, expect, it, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, act } from "@testing-library/react";
import { LinkPopup } from "./LinkPopup";

const onApply = vi.fn();
const onRemove = vi.fn();
const onClose = vi.fn();

beforeEach(() => {
  onApply.mockReset();
  onRemove.mockReset();
  onClose.mockReset();
});

const anchor = { left: 100, top: 100 };

describe("LinkPopup", () => {
  it("renders a URL input + Apply button", () => {
    render(
      <LinkPopup
        anchor={anchor}
        initialUrl=""
        editing={false}
        onApply={onApply}
        onRemove={onRemove}
        onClose={onClose}
      />,
    );
    expect(screen.getByLabelText("URL")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Apply" })).toBeInTheDocument();
  });

  it("does not show Remove button in add mode", () => {
    render(
      <LinkPopup
        anchor={anchor}
        initialUrl=""
        editing={false}
        onApply={onApply}
        onRemove={onRemove}
        onClose={onClose}
      />,
    );
    expect(screen.queryByRole("button", { name: "Remove" })).toBeNull();
  });

  it("shows Remove button in edit mode", () => {
    render(
      <LinkPopup
        anchor={anchor}
        initialUrl="https://example.com"
        editing={true}
        onApply={onApply}
        onRemove={onRemove}
        onClose={onClose}
      />,
    );
    expect(screen.getByRole("button", { name: "Remove" })).toBeInTheDocument();
  });

  it("Apply with valid URL calls onApply + onClose", () => {
    render(
      <LinkPopup
        anchor={anchor}
        initialUrl=""
        editing={false}
        onApply={onApply}
        onRemove={onRemove}
        onClose={onClose}
      />,
    );
    const input = screen.getByLabelText("URL") as HTMLInputElement;
    act(() => {
      fireEvent.change(input, { target: { value: "https://water.app" } });
    });
    act(() => {
      fireEvent.click(screen.getByRole("button", { name: "Apply" }));
    });
    expect(onApply).toHaveBeenCalledWith("https://water.app");
    expect(onClose).toHaveBeenCalled();
  });

  it("Apply with empty URL in edit mode calls onRemove + onClose", () => {
    render(
      <LinkPopup
        anchor={anchor}
        initialUrl="https://example.com"
        editing={true}
        onApply={onApply}
        onRemove={onRemove}
        onClose={onClose}
      />,
    );
    const input = screen.getByLabelText("URL") as HTMLInputElement;
    act(() => {
      fireEvent.change(input, { target: { value: "" } });
    });
    act(() => {
      fireEvent.click(screen.getByRole("button", { name: "Apply" }));
    });
    expect(onRemove).toHaveBeenCalled();
    expect(onApply).not.toHaveBeenCalled();
    expect(onClose).toHaveBeenCalled();
  });

  it("Remove button calls onRemove + onClose", () => {
    render(
      <LinkPopup
        anchor={anchor}
        initialUrl="https://example.com"
        editing={true}
        onApply={onApply}
        onRemove={onRemove}
        onClose={onClose}
      />,
    );
    act(() => {
      fireEvent.click(screen.getByRole("button", { name: "Remove" }));
    });
    expect(onRemove).toHaveBeenCalled();
    expect(onClose).toHaveBeenCalled();
  });

  it("Escape closes without changes", () => {
    render(
      <LinkPopup
        anchor={anchor}
        initialUrl=""
        editing={false}
        onApply={onApply}
        onRemove={onRemove}
        onClose={onClose}
      />,
    );
    const input = screen.getByLabelText("URL");
    act(() => {
      fireEvent.keyDown(input, { key: "Escape" });
    });
    expect(onClose).toHaveBeenCalled();
    expect(onApply).not.toHaveBeenCalled();
    expect(onRemove).not.toHaveBeenCalled();
  });

  it("rejects javascript: URL", () => {
    render(
      <LinkPopup
        anchor={anchor}
        initialUrl=""
        editing={false}
        onApply={onApply}
        onRemove={onRemove}
        onClose={onClose}
      />,
    );
    const input = screen.getByLabelText("URL") as HTMLInputElement;
    act(() => {
      fireEvent.change(input, { target: { value: "javascript:alert(1)" } });
    });
    act(() => {
      fireEvent.click(screen.getByRole("button", { name: "Apply" }));
    });
    expect(onApply).not.toHaveBeenCalled();
    expect(onClose).not.toHaveBeenCalled();
  });
});
