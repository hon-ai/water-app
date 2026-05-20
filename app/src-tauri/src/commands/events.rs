use crate::events::{emit, BusPing, TypingTelemetryPayload};
use crate::orchestrator_service::{parse_id, OrchestratorRequest};
use crate::state::AppState;
use serde::Deserialize;
use std::str::FromStr;
use tauri::{AppHandle, State};
use water_core::orchestrator::{
    CursorClassification, ProjectSnapshot, SceneSnapshot, StructuralInflection, TypingTelemetry,
};
use water_core::Id;

/// Smoke command used by tests + manual ping. Removed in M3+ once the bus
/// has many real events.
#[tauri::command]
pub fn bus_ping(app: AppHandle, tick: u64) -> Result<(), String> {
    emit(&app, "bus:ping", BusPing { tick }).map_err(|e| e.to_string())
}

/// Renderer fires this for every typing tick. The handler re-emits as a
/// Tauri event for any renderer-side observers (debug panel, eval harness),
/// then forwards the parsed payload into the per-project orchestrator
/// channel if one is running.
#[tauri::command]
pub async fn typing_telemetry(
    app: AppHandle,
    state: State<'_, AppState>,
    payload: TypingTelemetryPayload,
) -> Result<(), String> {
    // Best-effort renderer re-emit. Cloning the payload is cheap (a handful
    // of small fields) and we want the event to fire even if no project is
    // open yet — useful for dev-time wiring.
    let _ = emit(&app, "typing:telemetry", payload.clone());

    let handle = {
        let proj = state.project.lock().await;
        proj.as_ref().and_then(|p| p.orchestrator.as_ref().cloned())
    };
    if let Some(h) = handle {
        let core_payload = TypingTelemetry {
            idle_for_ms: payload.idle_for_ms,
            cursor_classification: parse_cursor(&payload.cursor_classification),
            block_id: payload.block_id,
            recent_word_delta: payload.recent_word_delta,
            structural_inflection: parse_inflection(&payload.structural_inflection),
        };
        // `last_block_text` is carried alongside the core `TypingTelemetry`
        // (rather than embedded in it) because it's an `AnalysisSnapshot`
        // concern that the trigger types don't need on every tick — only
        // the orchestrator service consumes it to update its analysis
        // snapshot. Renderer sends `Some` only on idle pulses (>=3 s).
        h.send(OrchestratorRequest::Telemetry {
            telemetry: core_payload,
            last_block_text: payload.last_block_text,
        })
        .await;
    }
    Ok(())
}

fn parse_cursor(s: &str) -> CursorClassification {
    match s {
        "at_sentence_end" => CursorClassification::AtSentenceEnd,
        "at_paragraph_end" => CursorClassification::AtParagraphEnd,
        // "mid_sentence" plus any unknown value falls through to MidSentence,
        // which is the safest default (mid-sentence gates suppress pills).
        _ => CursorClassification::MidSentence,
    }
}

fn parse_inflection(s: &str) -> StructuralInflection {
    match s {
        "new_scene" => StructuralInflection::NewScene,
        "new_chapter" => StructuralInflection::NewChapter,
        "pov_change" => StructuralInflection::PovChange,
        "location_change" => StructuralInflection::LocationChange,
        _ => StructuralInflection::None,
    }
}

/// Renderer-side payload for `scene_state`. Mirrored on the TS side as
/// the `sceneState` ipc argument.
///
/// `character_count` and `world_entry_count` are project-level. M2 ships
/// with both pinned to 0 (the `no_universe_yet` trigger therefore stays
/// the eager-fire path documented in spec § 6.1); M3/M4 will populate
/// them from the CharacterStore / WorldStore once those land.
#[derive(Deserialize)]
pub struct ScenePayload {
    pub scene_id: String,
    pub pov_character_id: Option<String>,
    pub location_id: Option<String>,
    pub characters_present: Vec<String>,
    pub word_count: u32,
    pub body_text: String,
    pub character_count: u32,
    pub world_entry_count: u32,
}

/// Push the current scene + project snapshot into the orchestrator. Called
/// by the renderer whenever a scene loads and after each successful body
/// save. The orchestrator caches the body text so subsequent telemetry
/// ticks can build prompt excerpts without re-reading from disk.
#[tauri::command]
pub async fn scene_state(state: State<'_, AppState>, payload: ScenePayload) -> Result<(), String> {
    let handle = {
        let proj = state.project.lock().await;
        proj.as_ref().and_then(|p| p.orchestrator.as_ref().cloned())
    };
    let Some(h) = handle else { return Ok(()) };

    let scene = SceneSnapshot {
        id: parse_id(&payload.scene_id)?,
        pov_character_id: payload.pov_character_id.and_then(|s| Id::from_str(&s).ok()),
        location_id: payload.location_id.and_then(|s| Id::from_str(&s).ok()),
        characters_present: payload
            .characters_present
            .into_iter()
            .filter_map(|s| Id::from_str(&s).ok())
            .collect(),
        word_count: payload.word_count,
        // Initial value: assume a full cooldown's worth of time has passed
        // since the (non-existent) previous pill. Real per-scene tracking
        // lands when the orchestrator persists per-scene last_pill_at.
        seconds_since_last_pill: 60,
    };
    let project = ProjectSnapshot {
        character_count: payload.character_count,
        world_entry_count: payload.world_entry_count,
    };
    h.send(OrchestratorRequest::SceneState(
        scene,
        project,
        payload.body_text,
    ))
    .await;
    Ok(())
}
