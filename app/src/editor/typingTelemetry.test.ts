import { describe, expect, it, vi } from "vitest";

const { invokeMock } = vi.hoisted(() => ({ invokeMock: vi.fn() }));
vi.mock("@tauri-apps/api/core", () => ({ invoke: invokeMock }));

import { emitTypingTelemetry } from "./typingTelemetry";

describe("emitTypingTelemetry", () => {
  it("invokes the typing_telemetry command with payload", async () => {
    invokeMock.mockResolvedValue(undefined);
    await emitTypingTelemetry({
      idle_for_ms: 3000,
      cursor_classification: "at_paragraph_end",
      block_id: "^bk-0001",
      recent_word_delta: 12,
      structural_inflection: "none",
      last_block_text: "The room felt cold.",
    });
    expect(invokeMock).toHaveBeenCalledWith("typing_telemetry", {
      payload: expect.objectContaining({ cursor_classification: "at_paragraph_end" }),
    });
  });

  it("swallows invoke errors silently", async () => {
    invokeMock.mockRejectedValue(new Error("no window"));
    await expect(
      emitTypingTelemetry({
        idle_for_ms: 0, cursor_classification: "mid_sentence", block_id: "^bk-0001",
        recent_word_delta: 0, structural_inflection: "none", last_block_text: null,
      }),
    ).resolves.toBeUndefined();
  });
});
