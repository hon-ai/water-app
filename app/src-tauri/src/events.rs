//! Typed event-bus helpers for renderer↔core communication. Mirrors
//! `app/src/ipc/events.ts::WaterEventPayloads`. Add variants as features land.

use serde::Serialize;
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
