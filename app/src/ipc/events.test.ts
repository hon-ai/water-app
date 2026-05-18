import { describe, expect, it, vi } from "vitest";
import { onWaterEvent } from "./events";

const listeners: Record<string, ((p: unknown) => void)[]> = {};
vi.mock("@tauri-apps/api/event", () => ({
  listen: async (name: string, cb: (e: { payload: unknown }) => void) => {
    const arr = (listeners[name] ??= []);
    const fn = (p: unknown) => cb({ payload: p });
    arr.push(fn);
    return () => {
      const list = listeners[name];
      if (!list) return;
      const i = list.indexOf(fn);
      if (i >= 0) list.splice(i, 1);
    };
  },
}));

describe("onWaterEvent", () => {
  it("subscribes to a named event and forwards payload", async () => {
    const cb = vi.fn();
    const unsub = await onWaterEvent("bus:ping", cb);
    const list = listeners["bus:ping"];
    expect(list).toBeDefined();
    list![0]!({ tick: 1 });
    expect(cb).toHaveBeenCalledWith({ tick: 1 });
    unsub();
  });

  it("unsubscribe removes listener", async () => {
    const cb = vi.fn();
    const unsub = await onWaterEvent("bus:ping", cb);
    unsub();
    const list = listeners["bus:ping"];
    list?.[0]?.({ tick: 2 });
    expect(cb).not.toHaveBeenCalled();
  });
});
