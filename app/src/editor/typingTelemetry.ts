import { invoke } from "@tauri-apps/api/core";
import type { WaterEventPayloads } from "../ipc/events";

export type TypingTelemetry = WaterEventPayloads["typing:telemetry"];

/** Invoke a Rust command that emits the typing:telemetry event back to the
 *  renderer's bus. Going through Rust lets the orchestrator subscribe in
 *  Phase C without renderer-to-core direct invocation. */
export async function emitTypingTelemetry(p: TypingTelemetry): Promise<void> {
  try {
    await invoke("typing_telemetry", { payload: p });
  } catch {
    // Telemetry is fire-and-forget; swallow errors.
  }
}
