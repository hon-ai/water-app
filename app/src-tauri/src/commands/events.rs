use crate::events::{emit, BusPing, TypingTelemetryPayload};
use tauri::AppHandle;

/// Smoke command used by tests + manual ping. Removed in M3+ once the bus
/// has many real events.
#[tauri::command]
pub fn bus_ping(app: AppHandle, tick: u64) -> Result<(), String> {
    emit(&app, "bus:ping", BusPing { tick }).map_err(|e| e.to_string())
}

/// Renderer fires this for every typing tick. The handler re-emits as a
/// Tauri event so the orchestrator (subscribed in Phase C) can react.
#[tauri::command]
pub fn typing_telemetry(app: AppHandle, payload: TypingTelemetryPayload) -> Result<(), String> {
    emit(&app, "typing:telemetry", payload).map_err(|e| e.to_string())
}
