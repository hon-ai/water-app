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
  "provider:status": {
    provider_id: string;
    ok: boolean;
    error: string | null;
  };
  "typing:telemetry": {
    idle_for_ms: number;
    cursor_classification: "at_sentence_end" | "at_paragraph_end" | "mid_sentence";
    block_id: string;
    recent_word_delta: number;
    structural_inflection: "new_scene" | "new_chapter" | "pov_change" | "location_change" | "none";
    /**
     * Text of the current block; only populated on idle pulses
     * (`idle_for_ms >= 3000`). `null` during typing bursts (5 Hz cap)
     * to keep the renderer→core wire size small. Consumed by
     * `character_dissonance` via the orchestrator's `AnalysisSnapshot`.
     */
    last_block_text: string | null;
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
  "pill:pinned": import("../pill/types").Pill;
  "pill:unpinned": { pill_id: string };
  "bouquet:ready": {
    parent_pill_id: string;
    items: Array<{
      sub_pill_id: string;
      angle: "feel" | "notice" | "wonder";
      text: string;
    }>;
  };
  /**
   * M5: emitted by the orchestrator after a heat-metric recompute
   * lands. The renderer's HeatmapStrip subscribes and re-fetches
   * `heat_read` for the matching scene; small payload so we don't
   * spam every metric track. Fires on every autosave.
   */
  "heat:updated": { scene_id: string };
  /**
   * Phase 4: emitted when the rabbit_fan_4 LLM call returns and the
   * four children have been persisted. `parent_id` is the rabbit
   * thought that was deepened (root or interior).
   */
  "deepen:ready": {
    parent_id: string;
    children: Array<{
      id: string;
      direction: "closer" | "wider" | "opposite" | "deeper" | "root" | "";
      text: string;
    }>;
  };
  /** Phase 4: emitted when LLM dispatch or persistence failed. */
  "deepen:failed": { parent_id: string; reason: string };
  /**
   * Phase 5: emitted after a diagnostic-engine run upserts editor
   * pills for a scene. Renderer's diagnostics tab + inline-underline
   * plugin re-fetch via `editor_pills_list` on receipt.
   */
  "editor_pills:updated": { scene_id: string; count: number };
}

export type WaterEventName = keyof WaterEventPayloads;

export async function onWaterEvent<K extends WaterEventName>(
  name: K,
  cb: (payload: WaterEventPayloads[K]) => void,
): Promise<UnlistenFn> {
  return listen<WaterEventPayloads[K]>(name, (e) => cb(e.payload));
}
