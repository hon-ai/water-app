use crate::events::{emit, BusPing};
use tauri::AppHandle;

/// Smoke command used by tests + manual ping. Removed in M3+ once the bus
/// has many real events.
#[tauri::command]
pub fn bus_ping(app: AppHandle, tick: u64) -> Result<(), String> {
    emit(&app, "bus:ping", BusPing { tick }).map_err(|e| e.to_string())
}
