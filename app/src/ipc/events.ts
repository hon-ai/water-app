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
  "typing:telemetry": {
    idle_for_ms: number;
    cursor_classification: "at_sentence_end" | "at_paragraph_end" | "mid_sentence";
    block_id: string;
    recent_word_delta: number;
    structural_inflection: "new_scene" | "new_chapter" | "pov_change" | "location_change" | "none";
  };
  "pill:emerged": {
    pill_id: string;
    speaker_id: string;
    hue_token: string;
    text: string;
    block_target_id: string | null;
    trigger_id: string;
  };
  "pill:dismissed": { pill_id: string };
  "pill:evicted": { pill_id: string };
}

export type WaterEventName = keyof WaterEventPayloads;

export async function onWaterEvent<K extends WaterEventName>(
  name: K,
  cb: (payload: WaterEventPayloads[K]) => void,
): Promise<UnlistenFn> {
  return listen<WaterEventPayloads[K]>(name, (e) => cb(e.payload));
}
