import { listen, type UnlistenFn } from "@tauri-apps/api/event";

/**
 * Bidirectional event bus type catalogue.
 * Add new event names + payload types as features land.
 * Mirrors the payload structs in `app/src-tauri/src/events.rs`. Each entry here should have a corresponding `#[derive(Serialize, Clone)] struct` on the Rust side.
 */
export interface WaterEventPayloads {
  "bus:ping": { tick: number };
  "sidecar:status": {
    status: "loading" | "ready" | "error";
    detail: string | null;
  };
  // typing:telemetry, pill:emerged, etc. added in later tasks
}

export type WaterEventName = keyof WaterEventPayloads;

export async function onWaterEvent<K extends WaterEventName>(
  name: K,
  cb: (payload: WaterEventPayloads[K]) => void,
): Promise<UnlistenFn> {
  return listen<WaterEventPayloads[K]>(name, (e) => cb(e.payload));
}
