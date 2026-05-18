import { describe, expect, it, vi, beforeEach, afterEach } from "vitest";
import { render } from "@testing-library/react";
import { useIdleDetector } from "./useIdleDetector";

function Probe({ onIdle }: { onIdle: () => void }) {
  const { onActivity } = useIdleDetector(3000, onIdle);
  (window as any).__triggerActivity = onActivity;
  return null;
}

describe("useIdleDetector", () => {
  beforeEach(() => vi.useFakeTimers({ shouldAdvanceTime: true }));
  afterEach(() => vi.useRealTimers());

  it("fires after 3000ms of inactivity", () => {
    const idle = vi.fn();
    render(<Probe onIdle={idle} />);
    (window as any).__triggerActivity();
    vi.advanceTimersByTime(2999);
    expect(idle).not.toHaveBeenCalled();
    vi.advanceTimersByTime(2);
    expect(idle).toHaveBeenCalledTimes(1);
  });

  it("resets on activity", () => {
    const idle = vi.fn();
    render(<Probe onIdle={idle} />);
    (window as any).__triggerActivity();
    vi.advanceTimersByTime(2000);
    (window as any).__triggerActivity();
    vi.advanceTimersByTime(2000);
    expect(idle).not.toHaveBeenCalled();
    vi.advanceTimersByTime(1001);
    expect(idle).toHaveBeenCalledTimes(1);
  });

  it("does not fire after unmount", () => {
    const idle = vi.fn();
    const { unmount } = render(<Probe onIdle={idle} />);
    (window as any).__triggerActivity();
    unmount();
    vi.advanceTimersByTime(5000);
    expect(idle).not.toHaveBeenCalled();
  });
});
