//! Typed event-bus helpers for renderer↔core communication. Mirrors
//! `app/src/ipc/events.ts::WaterEventPayloads`. Add variants as features land.

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

/// Emit a typed event to the renderer. Returns Ok on success; errors are
/// usually "no window yet" during early boot — log and continue.
pub fn emit<T: Serialize + Clone>(
    app: &AppHandle,
    event_name: &str,
    payload: T,
) -> Result<(), tauri::Error> {
    app.emit(event_name, payload)
}

#[derive(Serialize, Clone)]
pub struct BusPing {
    pub tick: u64,
}

/// Payload for the `sidecar:status` event. Mirrors
/// `WaterEventPayloads["sidecar:status"]` in `app/src/ipc/events.ts`.
/// `status` is one of `"loading" | "ready" | "error"` — the renderer is
/// allowed to assume the closed set.
#[derive(Serialize, Clone)]
pub struct SidecarStatusPayload {
    pub status: String,
    pub detail: Option<String>,
}

/// Payload for the `typing:telemetry` event. Mirrors
/// `WaterEventPayloads["typing:telemetry"]` in `app/src/ipc/events.ts`.
/// Strings are closed sets — see the TS side for the allowed values.
#[derive(Serialize, Deserialize, Clone)]
pub struct TypingTelemetryPayload {
    pub idle_for_ms: u64,
    pub cursor_classification: String, // "at_sentence_end" | "at_paragraph_end" | "mid_sentence"
    pub block_id: String,
    pub recent_word_delta: i32,
    pub structural_inflection: String, // "new_scene" | "new_chapter" | "pov_change" | "location_change" | "none"
}
