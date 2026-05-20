//! Pill verbs invoked from the renderer. M2 Phase F wires:
//! - `pill_expand` / `pill_regenerate`: dispatch into the per-project
//!   `OrchestratorService`, which assembles the bouquet prompt, calls the
//!   primary LLM provider, anti-loop-filters the result, and emits
//!   `bouquet:ready`.
//! - `pill_pin`: writes to `pinned_pill` + emits `pill:pinned`.
//! - `pill_dismiss`: deletes from `pinned_pill` (if present) + emits both
//!   `pill:dismissed` and `pill:unpinned`.
//! - `pinned_list`: read-side query used by the renderer's PinnedColumn on
//!   mount to rehydrate the existing pin set.

use crate::events::emit;
use crate::orchestrator_service::{parse_id, OrchestratorRequest};
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, State};
use tokio::sync::Mutex;
use water_core::Db;

/// Payload mirrored on the renderer side as `Pill` (see
/// `app/src/pill/types.ts`). Used as both the `pill_pin` input and the
/// `pinned_list` row shape, plus the `pill:pinned` event body.
#[derive(Serialize, Deserialize, Clone)]
pub struct PinnedPill {
    pub pill_id: String,
    pub speaker_id: String,
    pub hue_token: String,
    pub text: String,
    pub block_target_id: Option<String>,
    pub trigger_id: String,
}

#[tauri::command]
pub async fn pill_expand(state: State<'_, AppState>, parent_pill_id: String) -> Result<(), String> {
    let handle = {
        let proj = state.project.lock().await;
        proj.as_ref().and_then(|p| p.orchestrator.as_ref().cloned())
    };
    if let Some(h) = handle {
        let pid = parse_id(&parent_pill_id)?;
        h.send(OrchestratorRequest::Expand {
            parent_pill_id: pid,
        })
        .await;
    }
    // No-op when no project is open / orchestrator is missing. The
    // renderer treats expand as fire-and-forget; events arrive
    // asynchronously when the bouquet lands.
    Ok(())
}

#[tauri::command]
pub async fn pill_regenerate(
    state: State<'_, AppState>,
    parent_pill_id: String,
) -> Result<(), String> {
    let handle = {
        let proj = state.project.lock().await;
        proj.as_ref().and_then(|p| p.orchestrator.as_ref().cloned())
    };
    if let Some(h) = handle {
        let pid = parse_id(&parent_pill_id)?;
        h.send(OrchestratorRequest::Regenerate {
            parent_pill_id: pid,
        })
        .await;
    }
    Ok(())
}

/// Response payload returned by `pill_pin`. `pin_id` is the pinned row's
/// id (currently the pill's own id, since `pinned_pill.id` is the pill
/// id; the field exists so future versions can decouple if the schema
/// shifts to a synthetic surrogate). `stub_entry_id` is `Some(id)` only
/// when the pin path created a new `world_entry` stub (M4 T29: Chorus +
/// `no_universe_yet` only); the renderer routes to the new entry sheet
/// when the field is non-null.
#[derive(Debug, Serialize, Clone)]
pub struct PinPillResponse {
    pub pin_id: String,
    pub stub_entry_id: Option<String>,
    /// `Some(segment_id)` whenever `stub_entry_id` is `Some` — the
    /// renderer needs both to route to the new entry sheet (which is
    /// addressed by (segment, entry) in the worlds surface). Always
    /// `None` when `stub_entry_id` is `None`.
    pub stub_segment_id: Option<String>,
}

/// Core implementation of `pill_pin` extracted so unit tests can drive
/// it without a Tauri runtime. The Tauri command wrapper handles
/// `AppState` unpacking + `pill:pinned` event emission.
///
/// Persists the row, then — if the pinned pill came from the Chorus
/// `no_universe_yet` path — creates a stub `world_entry` in the
/// `locations` segment seeded with the pill's snippet so the writer can
/// flesh out the implied location without leaving the surface.
pub async fn pill_pin_core(
    db: Arc<Mutex<Db>>,
    project_root: PathBuf,
    project_id: String,
    pill: PinnedPill,
    scene_id: String,
    block_id: String,
    snippet: String,
) -> Result<PinPillResponse, String> {
    let now = chrono::Utc::now().to_rfc3339();
    let g = db.lock().await;
    // INSERT OR IGNORE: re-pinning the same pill_id is a no-op (the row
    // already records the original pin time + bouquet context).
    //
    // M4 T29 adds `origin_trigger` (the new v4 column) alongside the
    // existing `trigger_class` column. Both are set to `pill.trigger_id`
    // for now — they carry the same semantic content (the trigger that
    // produced the pill); the column split exists so the M5+ Heatmap
    // can attach richer classification metadata without disturbing the
    // origin-trigger lookup used by the Chorus-stub branch below.
    g.conn()
        .execute(
            "INSERT OR IGNORE INTO pinned_pill \
             (id, scene_id, block_id, snippet, speaker_kind, speaker_id, message, hue, \
              rabbit_hole_path, created_at, parent_pill_id, pinned_at, trigger_class, \
              bouquet_position, origin_trigger) \
             VALUES (?1, ?2, ?3, ?4, 'persona', ?5, ?6, ?7, NULL, ?8, NULL, ?8, ?9, NULL, ?9)",
            rusqlite::params![
                pill.pill_id,
                scene_id,
                block_id,
                snippet,
                pill.speaker_id,
                pill.text,
                pill.hue_token,
                now,
                pill.trigger_id,
            ],
        )
        .map_err(|e| e.to_string())?;
    drop(g);

    // Chorus + no_universe_yet → create a `locations` stub seeded with
    // the snippet so the writer can elaborate immediately. Other
    // (speaker, trigger) combinations return `stub_entry_id: None` and
    // `stub_segment_id: None`.
    let (stub_entry_id, stub_segment_id) =
        if pill.speaker_id == "chorus" && pill.trigger_id == "no_universe_yet" {
            let project_id = parse_id(&project_id)?;
            let g = db.lock().await;
            let store = water_core::world::WorldStore::new(&g, project_root.clone());
            let seg = store
                .find_segment_by_slug(&project_id, "locations")
                .map_err(|e| e.to_string())?
                .ok_or_else(|| "locations segment missing".to_string())?;
            let entry_id = store
                .create_entry_seeded(&seg.id, "", "main.sensory_detail", &snippet)
                .map_err(|e| e.to_string())?;
            (Some(entry_id.to_string()), Some(seg.id.to_string()))
        } else {
            (None, None)
        };

    Ok(PinPillResponse {
        pin_id: pill.pill_id.clone(),
        stub_entry_id,
        stub_segment_id,
    })
}

#[tauri::command]
pub async fn pill_pin(
    app: AppHandle,
    state: State<'_, AppState>,
    pill: PinnedPill,
    scene_id: String,
    block_id: String,
    snippet: String,
) -> Result<PinPillResponse, String> {
    let (db, root, project_id) = {
        let proj = state.project.lock().await;
        let p = proj.as_ref().ok_or("no project open")?;
        (p.db.clone(), p.root.clone(), p.project_id.clone())
    };
    let resp = pill_pin_core(db, root, project_id, pill.clone(), scene_id, block_id, snippet).await?;
    emit(&app, "pill:pinned", pill).map_err(|e| e.to_string())?;
    Ok(resp)
}

#[tauri::command]
pub async fn pill_dismiss(
    app: AppHandle,
    state: State<'_, AppState>,
    pill_id: String,
) -> Result<(), String> {
    // Delete from pinned_pill if present. "no project open" is not an
    // error here — un-pinning during shutdown should still emit events so
    // the renderer can clean up its state.
    let db_opt = {
        let proj = state.project.lock().await;
        proj.as_ref().map(|p| p.db.clone())
    };
    if let Some(db) = db_opt {
        let g = db.lock().await;
        let _ = g.conn().execute(
            "DELETE FROM pinned_pill WHERE id = ?1",
            rusqlite::params![pill_id],
        );
    }
    #[derive(Serialize, Clone)]
    struct Dismiss {
        pill_id: String,
    }
    let _ = emit(
        &app,
        "pill:dismissed",
        Dismiss {
            pill_id: pill_id.clone(),
        },
    );
    #[derive(Serialize, Clone)]
    struct Unpinned {
        pill_id: String,
    }
    emit(&app, "pill:unpinned", Unpinned { pill_id }).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn pinned_list(state: State<'_, AppState>) -> Result<Vec<PinnedPill>, String> {
    let db = {
        let proj = state.project.lock().await;
        proj.as_ref().ok_or("no project open")?.db.clone()
    };
    let g = db.lock().await;
    let mut stmt = g
        .conn()
        .prepare(
            "SELECT id, speaker_id, hue, message, COALESCE(rabbit_hole_path, ''), \
                    COALESCE(trigger_class, '') \
             FROM pinned_pill \
             ORDER BY pinned_at DESC, created_at DESC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok(PinnedPill {
                pill_id: r.get(0)?,
                speaker_id: r.get(1)?,
                hue_token: r.get(2)?,
                text: r.get(3)?,
                block_target_id: {
                    let s: String = r.get(4)?;
                    if s.is_empty() {
                        None
                    } else {
                        Some(s)
                    }
                },
                trigger_id: r.get(5)?,
            })
        })
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use water_core::{Db, ProjectStore};

    fn mk_pill(speaker_id: &str, trigger_id: &str) -> PinnedPill {
        PinnedPill {
            pill_id: water_core::Id::new().to_string(),
            speaker_id: speaker_id.to_string(),
            hue_token: "--water-hue-persona-chorus".to_string(),
            text: "—".to_string(),
            block_target_id: None,
            trigger_id: trigger_id.to_string(),
        }
    }

    /// Returns (db, dir, project_id, scene_id).
    async fn mk_world() -> (
        Arc<Mutex<Db>>,
        tempfile::TempDir,
        String,
        String,
    ) {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open(dir.path().join("project.db")).unwrap();
        let project = ProjectStore::new(&db).insert("P").unwrap();
        let manuscript = water_core::ManuscriptStore::new(&db)
            .insert(&project.id, "M", 0)
            .unwrap();
        water_core::world::WorldStore::new(&db, dir.path().to_path_buf())
            .seed_builtins(&project.id)
            .unwrap();
        let scene = water_core::SceneStore::new(&db, dir.path().to_path_buf())
            .create(water_core::NewScene {
                manuscript_id: manuscript.id,
                chapter_id: None,
                name: "S".into(),
                ordering: 0,
            })
            .unwrap();
        (
            Arc::new(Mutex::new(db)),
            dir,
            project.id.to_string(),
            scene.id.to_string(),
        )
    }

    #[tokio::test]
    async fn pill_pin_with_chorus_no_universe_yet_creates_locations_stub() {
        let (db, dir, project_id, scene_id) = mk_world().await;
        let pill = mk_pill("chorus", "no_universe_yet");
        let snippet = "A library that remembers the dust on your fingertips".to_string();

        let resp = pill_pin_core(
            db.clone(),
            dir.path().to_path_buf(),
            project_id.clone(),
            pill,
            scene_id,
            "blk-1".to_string(),
            snippet.clone(),
        )
        .await
        .unwrap();

        let stub_id_str = resp
            .stub_entry_id
            .expect("Chorus + no_universe_yet must create a stub");
        let stub_id: water_core::Id = stub_id_str.parse().unwrap();

        let g = db.lock().await;
        let store = water_core::world::WorldStore::new(&g, dir.path().to_path_buf());
        let entry = store.read_entry(&stub_id).unwrap();
        let main = entry
            .data
            .get("main")
            .expect("entry has main section")
            .as_object()
            .unwrap();
        let sensory = main
            .get("sensory_detail")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        assert!(
            sensory.contains("library that remembers"),
            "snippet must be seeded into main.sensory_detail; got {sensory:?}"
        );

        // M4 T30: `stub_segment_id` must also be set so the renderer
        // can route to the new entry sheet (addressed by (seg, entry)).
        let stub_seg = resp
            .stub_segment_id
            .expect("stub_segment_id must accompany stub_entry_id");
        let loc_seg_id = store
            .find_segment_by_slug(&p_id(&g, &project_id), "locations")
            .unwrap()
            .unwrap()
            .id
            .to_string();
        assert_eq!(stub_seg, loc_seg_id);
    }

    /// Helper to look up the project id row from a borrowed Db ref. The
    /// stub test reaches into the DB after the core fn has returned, so
    /// we re-parse the project_id rather than threading it as a typed
    /// param.
    fn p_id(_g: &Db, project_id_str: &str) -> water_core::Id {
        project_id_str.parse().unwrap()
    }

    #[tokio::test]
    async fn pill_pin_with_non_chorus_speaker_does_not_create_stub() {
        let (db, dir, project_id, scene_id) = mk_world().await;
        let pill = mk_pill("architect", "no_universe_yet");

        let resp = pill_pin_core(
            db,
            dir.path().to_path_buf(),
            project_id,
            pill,
            scene_id,
            "blk-1".to_string(),
            "—".to_string(),
        )
        .await
        .unwrap();

        assert!(
            resp.stub_entry_id.is_none(),
            "non-Chorus speaker must not create a stub"
        );
    }

    #[tokio::test]
    async fn pill_pin_with_chorus_but_other_trigger_does_not_create_stub() {
        let (db, dir, project_id, scene_id) = mk_world().await;
        let pill = mk_pill("chorus", "character_dissonance");

        let resp = pill_pin_core(
            db,
            dir.path().to_path_buf(),
            project_id,
            pill,
            scene_id,
            "blk-1".to_string(),
            "—".to_string(),
        )
        .await
        .unwrap();

        assert!(
            resp.stub_entry_id.is_none(),
            "Chorus with non-no_universe_yet trigger must not create a stub"
        );
    }

    #[tokio::test]
    async fn pill_pin_persists_origin_trigger_column() {
        let (db, dir, project_id, scene_id) = mk_world().await;
        let pill = mk_pill("echo", "pace_floor");
        let pill_id = pill.pill_id.clone();

        let _ = pill_pin_core(
            db.clone(),
            dir.path().to_path_buf(),
            project_id,
            pill,
            scene_id,
            "blk-1".to_string(),
            "—".to_string(),
        )
        .await
        .unwrap();

        let g = db.lock().await;
        let stored: String = g
            .conn()
            .query_row(
                "SELECT origin_trigger FROM pinned_pill WHERE id = ?1",
                rusqlite::params![pill_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(stored, "pace_floor");
    }
}
