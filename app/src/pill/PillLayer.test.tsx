import { describe, expect, it, beforeEach, vi } from "vitest";

const { onWaterEventMock } = vi.hoisted(() => ({ onWaterEventMock: vi.fn() }));
vi.mock("../ipc/events", () => ({ onWaterEvent: onWaterEventMock }));

import { render, screen, waitFor, act } from "@testing-library/react";
import { PillLayer } from "./PillLayer";
import type { Pill } from "./types";

type EmergedHandler = (p: Pill) => void;
type DismissedHandler = (p: { pill_id: string }) => void;

interface Handlers {
  emerged?: EmergedHandler;
  dismissed?: DismissedHandler;
  evicted?: DismissedHandler;
}

function mockHandlers(): Handlers {
  const handlers: Handlers = {};
  onWaterEventMock.mockImplementation(async (name: string, cb: unknown) => {
    if (name === "pill:emerged") handlers.emerged = cb as EmergedHandler;
    else if (name === "pill:dismissed") handlers.dismissed = cb as DismissedHandler;
    else if (name === "pill:evicted") handlers.evicted = cb as DismissedHandler;
    return vi.fn();
  });
  return handlers;
}

const samplePill = (overrides: Partial<Pill> = {}): Pill => ({
  pill_id: "p1",
  speaker_id: "echo",
  hue_token: "--water-hue-muse",
  text: "Something held at the threshold.",
  block_target_id: "^bk-0001",
  trigger_id: "block_anchored_drift",
  ...overrides,
});

beforeEach(() => {
  onWaterEventMock.mockReset();
});

describe("PillLayer", () => {
  it("renders a capsule when pill:emerged fires", async () => {
    const handlers = mockHandlers();
    render(<PillLayer />);
    await waitFor(() => expect(handlers.emerged).toBeDefined());
    act(() => {
      handlers.emerged!(samplePill());
    });
    expect(screen.getByText("Something held at the threshold.")).toBeInTheDocument();
  });

  it("removes the capsule when pill:dismissed fires", async () => {
    const handlers = mockHandlers();
    render(<PillLayer />);
    await waitFor(() => {
      expect(handlers.emerged).toBeDefined();
      expect(handlers.dismissed).toBeDefined();
    });
    act(() => handlers.emerged!(samplePill({ pill_id: "p1", text: "first pill", block_target_id: null })));
    expect(screen.getByText("first pill")).toBeInTheDocument();
    act(() => handlers.dismissed!({ pill_id: "p1" }));
    expect(screen.queryByText("first pill")).toBeNull();
  });

  it("displays at most 4 pills simultaneously (FIFO evicts oldest)", async () => {
    const handlers = mockHandlers();
    render(<PillLayer />);
    await waitFor(() => expect(handlers.emerged).toBeDefined());
    // Use multi-word texts so assertions can't collide with the
    // single-letter speaker-chip glyphs (E/A/D/C/H) added in Phase 3.
    act(() =>
      handlers.emerged!(
        samplePill({ pill_id: "p1", text: "first-pill", block_target_id: null }),
      ),
    );
    act(() =>
      handlers.emerged!(
        samplePill({
          pill_id: "p2",
          speaker_id: "editor",
          hue_token: "--water-hue-editor",
          text: "second-pill",
          block_target_id: null,
        }),
      ),
    );
    act(() =>
      handlers.emerged!(
        samplePill({
          pill_id: "p3",
          speaker_id: "architect",
          hue_token: "--water-hue-architect",
          text: "third-pill",
          block_target_id: null,
        }),
      ),
    );
    act(() =>
      handlers.emerged!(
        samplePill({
          pill_id: "p4",
          speaker_id: "echo",
          hue_token: "--water-hue-echo",
          text: "fourth-pill",
          block_target_id: null,
        }),
      ),
    );
    act(() =>
      handlers.emerged!(
        samplePill({
          pill_id: "p5",
          speaker_id: "chorus",
          hue_token: "--water-hue-chorus",
          text: "fifth-pill",
          block_target_id: null,
        }),
      ),
    );
    // FIFO @ MAX_ON_SCREEN=4: first-pill evicts, the rest remain.
    expect(screen.queryByText("first-pill")).toBeNull();
    expect(screen.getByText("second-pill")).toBeInTheDocument();
    expect(screen.getByText("third-pill")).toBeInTheDocument();
    expect(screen.getByText("fourth-pill")).toBeInTheDocument();
    expect(screen.getByText("fifth-pill")).toBeInTheDocument();
  });
});
