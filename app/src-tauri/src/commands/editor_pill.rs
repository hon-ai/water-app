//! Phase 5 — IPC commands for editor pills.
//!
//! Three verbs:
//!   - `editor_pills_run`: renderer fires after save (or on
//!     debounced edit). Sends per-block text to the diagnostic
//!     engine, persists findings, cleans up rows whose anchor block
//!     has been removed. Emits `editor_pills:updated` so the
//!     renderer refetches.
//!   - `editor_pills_list`: read-side query for the diagnostics tab
//!     and the inline-underline plugin's anchor lookup.
//!   - `editor_pill_dismiss`: writer-flagged-away. Row stays in DB
//!     for telemetry; never resurfaces.
//!
//! The renderer is the source of truth for *what blocks exist* —
//! it extracts the `(blockId, textContent)` list from the
//! ProseMirror doc. The store doesn't try to parse markdown
//! itself.

use crate::events::emit;
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use tauri::{AppHandle, State};
use water_core::editor::{EditorPillRow, EditorPillStore, ScanBlock};
use water_core::prompts::loader::PromptLibrary;
use water_core::Id;

#[derive(Deserialize)]
pub struct EditorPillsRunPayload {
    pub scene_id: String,
    pub blocks: Vec<BlockPayload>,
}

#[derive(Deserialize)]
pub struct BlockPayload {
    pub block_id: String,
    pub text: String,
}

#[derive(Serialize, Clone)]
struct EditorPillsUpdated {
    scene_id: String,
    count: u32,
}

/// Run the diagnostic engine across the renderer-supplied blocks,
/// upsert findings, and emit `editor_pills:updated`. Returns the
/// live set so the caller can synchronously update without waiting
/// for the event round-trip.
#[tauri::command]
pub async fn editor_pills_run(
    app: AppHandle,
    state: State<'_, AppState>,
    payload: EditorPillsRunPayload,
) -> Result<Vec<EditorPillRow>, String> {
    let db = {
        let proj = state.project.lock().await;
        proj.as_ref().ok_or("no project open")?.db.clone()
    };
    let scene_id =
        Id::from_str(&payload.scene_id).map_err(|e| e.to_string())?;
    let live_block_ids: Vec<String> =
        payload.blocks.iter().map(|b| b.block_id.clone()).collect();
    // Load tone clauses once. The library load is cheap (~ms) and
    // the renderer fires this IPC at most every few seconds.
    let tone = PromptLibrary::load_builtin().map_err(|e| e.to_string())?.tone;
    let g = db.lock().await;
    let store = EditorPillStore::new(&g);
    // Build the borrowed scan-block slice. Has to live as long as the
    // upsert call; assemble after acquiring the lock so the borrow
    // checker is happy.
    let scan: Vec<ScanBlock<'_>> = payload
        .blocks
        .iter()
        .map(|b| ScanBlock {
            block_id: b.block_id.as_str(),
            text: b.text.as_str(),
        })
        .collect();
    let live = store
        .run_and_upsert(&scene_id, &scan, &tone)
        .map_err(|e| e.to_string())?;
    // Cleanup orphans whose anchor block is no longer in the scene.
    // Safe-bail when live_block_ids is empty (the store's own guard).
    let _ = store.cleanup_orphaned_blocks(&scene_id, &live_block_ids);
    drop(g);
    let _ = emit(
        &app,
        "editor_pills:updated",
        EditorPillsUpdated {
            scene_id: payload.scene_id,
            count: u32::try_from(live.len()).unwrap_or(0),
        },
    );
    Ok(live)
}

/// Active (non-dismissed) editor pills for a scene.
#[tauri::command]
pub async fn editor_pills_list(
    state: State<'_, AppState>,
    scene_id: String,
) -> Result<Vec<EditorPillRow>, String> {
    let db = {
        let proj = state.project.lock().await;
        proj.as_ref().ok_or("no project open")?.db.clone()
    };
    let sid = Id::from_str(&scene_id).map_err(|e| e.to_string())?;
    let g = db.lock().await;
    let store = EditorPillStore::new(&g);
    store.list_active(&sid).map_err(|e| e.to_string())
}

/// Phase 5.8 — request an LLM polish pass on a single paragraph.
/// Fire-and-forget; the orchestrator may drop the call when the
/// per-scene cap is hit or the per-block cooldown is still active.
/// On success the renderer hears about it via `editor_pills:updated`.
#[tauri::command]
pub async fn editor_polish_request(
    state: State<'_, AppState>,
    scene_id: String,
    block_id: String,
    text: String,
) -> Result<(), String> {
    let handle = {
        let proj = state.project.lock().await;
        proj.as_ref().and_then(|p| p.orchestrator.as_ref().cloned())
    };
    if let Some(h) = handle {
        let sid = Id::from_str(&scene_id).map_err(|e| e.to_string())?;
        h.send(
            crate::orchestrator_service::OrchestratorRequest::EditorPolish {
                scene_id: sid,
                block_id,
                block_text: text,
            },
        )
        .await;
    }
    Ok(())
}

/// Flag an editor pill as dismissed.
#[tauri::command]
pub async fn editor_pill_dismiss(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    let db = {
        let proj = state.project.lock().await;
        proj.as_ref().ok_or("no project open")?.db.clone()
    };
    let pill_id = Id::from_str(&id).map_err(|e| e.to_string())?;
    let g = db.lock().await;
    let store = EditorPillStore::new(&g);
    store.dismiss(&pill_id).map_err(|e| e.to_string())
}
