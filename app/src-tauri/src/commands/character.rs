//! Character CRUD Tauri commands (M3 T12).
//!
//! Each `#[tauri::command]` is a thin shim over a `_core` helper that takes
//! the raw `(db, root, ...)` inputs. This split exists so the integration
//! tests in this file can drive the same code path without having to
//! construct a `tauri::State<'_, AppState>` (which has no public test
//! constructor in current Tauri).
//!
//! Lock ordering, mirroring scene commands (KNOWN_FRAGILE #6):
//!   1. Grab `project` guard, clone out `Arc<Mutex<Db>>` + root + lock
//!      registries, then drop the project guard.
//!   2. For mutating commands, acquire the per-character write lock.
//!   3. Acquire the DB lock.
//!
//! The character write lock is acquired BEFORE the DB lock so two
//! concurrent `character_update_field` calls for the same id serialize at
//! step 2 — they don't pile up holding the DB mutex.

use crate::state::AppState;
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::State;
use tokio::sync::Mutex;
use water_core::{
    character::{
        autosuggest::suggest_for_scene_body,
        completion_pct,
        intake::{IntakeField, LSM_V2_1},
        next_hue_token, CharacterFile, CharacterStore, NewCharacter,
    },
    CharacterWriteLocks, Db, Id, SceneWriteLocks,
};

/// Renderer-facing index view of a character. Carries every field the
/// index panel needs plus the precomputed completion percentage (0..=100).
#[derive(Serialize, Debug, Clone)]
pub struct CharacterIndexEntry {
    pub id: String,
    pub full_name: String,
    pub role: Option<String>,
    pub hue_token: String,
    pub completion: u8,
}

/// One section of an intake schema. Mirrors `(section, fields)` pairs in
/// [`water_core::character::intake::LSM_V2_1`]. Serialized with snake_case
/// field names to match the rest of the character command surface.
#[derive(Serialize, Debug, Clone)]
pub struct IntakeSchemaSection {
    pub section: String,
    pub fields: Vec<IntakeField>,
}

/// Renderer-facing autosuggest hit. `character_id` is stringified for
/// JSON friendliness (the renderer treats it as opaque). `mention_count`
/// is `u32` to match the scanner — the renderer typically displays it
/// as "N mentions" next to the hit.
#[derive(Serialize, Debug, Clone)]
pub struct AutosuggestResultDto {
    pub character_id: String,
    pub full_name: String,
    pub mention_count: u32,
}

// ----------------------------------------------------------------------
// Core helpers (testable without Tauri State)
// ----------------------------------------------------------------------

async fn character_create_core(
    db: Arc<Mutex<Db>>,
    root: PathBuf,
    project_id: String,
) -> Result<CharacterIndexEntry, String> {
    let project_id: Id = project_id.parse().map_err(|e: water_core::Error| e.to_string())?;
    let db_guard = db.lock().await;
    let hue = next_hue_token(&db_guard).map_err(|e| e.to_string())?;
    let store = CharacterStore::new(&db_guard, root);
    let row = store
        .create(NewCharacter {
            project_id,
            hue_token: hue,
        })
        .map_err(|e| e.to_string())?;
    Ok(CharacterIndexEntry {
        id: row.id.to_string(),
        full_name: row.full_name,
        role: row.role,
        hue_token: row.hue_token,
        completion: completion_pct(&row.data_json),
    })
}

async fn character_read_core(
    db: Arc<Mutex<Db>>,
    root: PathBuf,
    id: String,
) -> Result<CharacterFile, String> {
    let id: Id = id.parse().map_err(|e: water_core::Error| e.to_string())?;
    let db_guard = db.lock().await;
    let store = CharacterStore::new(&db_guard, root);
    store.read(&id).map_err(|e| e.to_string())
}

async fn character_list_core(
    db: Arc<Mutex<Db>>,
) -> Result<Vec<CharacterIndexEntry>, String> {
    let db_guard = db.lock().await;
    let rows = CharacterStore::list_index(&db_guard).map_err(|e| e.to_string())?;
    Ok(rows
        .into_iter()
        .map(|r| CharacterIndexEntry {
            id: r.id.to_string(),
            full_name: r.full_name,
            role: r.role,
            hue_token: r.hue_token,
            completion: completion_pct(&r.data_json),
        })
        .collect())
}

async fn character_update_field_core(
    db: Arc<Mutex<Db>>,
    root: PathBuf,
    locks: CharacterWriteLocks,
    id: String,
    field_id: String,
    value: serde_json::Value,
) -> Result<CharacterIndexEntry, String> {
    let id: Id = id.parse().map_err(|e: water_core::Error| e.to_string())?;
    // Per-character write lock BEFORE the DB lock — see module docs.
    let _guard = locks.acquire(&id).await;
    let db_guard = db.lock().await;
    let store = CharacterStore::new(&db_guard, root);
    let row = store
        .update_field(&id, &field_id, &value)
        .map_err(|e| e.to_string())?;
    Ok(CharacterIndexEntry {
        id: row.id.to_string(),
        full_name: row.full_name,
        role: row.role,
        hue_token: row.hue_token,
        completion: completion_pct(&row.data_json),
    })
}

async fn character_delete_core(
    db: Arc<Mutex<Db>>,
    root: PathBuf,
    locks: CharacterWriteLocks,
    id: String,
) -> Result<(), String> {
    let id: Id = id.parse().map_err(|e: water_core::Error| e.to_string())?;
    // Hold the per-character write lock for the cascade so an in-flight
    // `update_field` can't observe a half-deleted state.
    let _guard = locks.acquire(&id).await;
    let db_guard = db.lock().await;
    let store = CharacterStore::new(&db_guard, root);
    store.delete_and_cascade(&id).map_err(|e| e.to_string())
}

// ----------------------------------------------------------------------
// Scene linkage core helpers (M3 T13)
// ----------------------------------------------------------------------
//
// These take the scene write lock (NOT the character write lock) because
// they mutate scene metadata — `scene_character_presence` rows and
// `scene.pov_character_id`. The lock-ordering convention matches scene
// commands (KNOWN_FRAGILE #6): acquire the per-scene write lock BEFORE
// the DB lock so concurrent body/rename writes for the same scene
// serialize at the scene-lock layer rather than the DB layer.

async fn character_link_to_scene_core(
    db: Arc<Mutex<Db>>,
    scene_locks: SceneWriteLocks,
    scene_id: String,
    character_id: String,
) -> Result<(), String> {
    let scene_id: Id = scene_id
        .parse()
        .map_err(|e: water_core::Error| e.to_string())?;
    let character_id: Id = character_id
        .parse()
        .map_err(|e: water_core::Error| e.to_string())?;
    // Scene write lock — link touches scene metadata (the body is
    // unchanged but the metadata table is logically part of "scene state").
    let _g = scene_locks.acquire(&scene_id).await;
    let db_guard = db.lock().await;
    db_guard
        .conn()
        .execute(
            "INSERT OR IGNORE INTO scene_character_presence (scene_id, character_id) VALUES (?1, ?2)",
            rusqlite::params![scene_id.as_str(), character_id.as_str()],
        )
        .map_err(|e| e.to_string())?;
    Ok(())
}

async fn character_unlink_from_scene_core(
    db: Arc<Mutex<Db>>,
    scene_locks: SceneWriteLocks,
    scene_id: String,
    character_id: String,
) -> Result<(), String> {
    let scene_id: Id = scene_id
        .parse()
        .map_err(|e: water_core::Error| e.to_string())?;
    let character_id: Id = character_id
        .parse()
        .map_err(|e: water_core::Error| e.to_string())?;
    let _g = scene_locks.acquire(&scene_id).await;
    let db_guard = db.lock().await;
    // Transactional: remove the presence row and, if this character was
    // POV for the scene, clear POV in the same step. Per spec § 20:
    // "If a writer removes a character from `characters_present` while
    // they are still POV, the POV is auto-cleared."
    //
    // `unchecked_transaction()` is the documented `rusqlite` escape hatch
    // for shared connections — `Connection::transaction()` requires
    // `&mut Connection`, but `Db::conn()` exposes `&Connection`.
    let tx = db_guard
        .conn()
        .unchecked_transaction()
        .map_err(|e| e.to_string())?;
    tx.execute(
        "DELETE FROM scene_character_presence WHERE scene_id = ?1 AND character_id = ?2",
        rusqlite::params![scene_id.as_str(), character_id.as_str()],
    )
    .map_err(|e| e.to_string())?;
    tx.execute(
        "UPDATE scene SET pov_character_id = NULL WHERE id = ?1 AND pov_character_id = ?2",
        rusqlite::params![scene_id.as_str(), character_id.as_str()],
    )
    .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

async fn character_set_pov_core(
    db: Arc<Mutex<Db>>,
    scene_locks: SceneWriteLocks,
    scene_id: String,
    character_id: Option<String>,
) -> Result<(), String> {
    let scene_id: Id = scene_id
        .parse()
        .map_err(|e: water_core::Error| e.to_string())?;
    let _g = scene_locks.acquire(&scene_id).await;
    let db_guard = db.lock().await;

    match character_id {
        Some(c_str) => {
            let c_id: Id = c_str
                .parse()
                .map_err(|e: water_core::Error| e.to_string())?;
            // Constraint enforced application-side (the schema FK only
            // points POV → character.id, not POV → presence): the POV
            // character must already be in `scene_character_presence` for
            // this scene. Per spec § 20.
            let present: i64 = db_guard
                .conn()
                .query_row(
                    "SELECT COUNT(*) FROM scene_character_presence WHERE scene_id = ?1 AND character_id = ?2",
                    rusqlite::params![scene_id.as_str(), c_id.as_str()],
                    |r| r.get(0),
                )
                .map_err(|e| e.to_string())?;
            if present == 0 {
                return Err("POV character must be in characters_present; link them first".into());
            }
            db_guard
                .conn()
                .execute(
                    "UPDATE scene SET pov_character_id = ?1 WHERE id = ?2",
                    rusqlite::params![c_id.as_str(), scene_id.as_str()],
                )
                .map_err(|e| e.to_string())?;
        }
        None => {
            db_guard
                .conn()
                .execute(
                    "UPDATE scene SET pov_character_id = NULL WHERE id = ?1",
                    rusqlite::params![scene_id.as_str()],
                )
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

// ----------------------------------------------------------------------
// Intake schema + autosuggest core helpers (M3 T14)
// ----------------------------------------------------------------------

/// Resolve a static intake schema id to its section list. Pure — no
/// state, no IO — but kept as a free function so the Tauri shim and the
/// tests share the same code path (no need for a tokio runtime in the
/// "unknown schema" test).
///
/// LSM v2.1 is currently the only supported schema. M4 will add World
/// Bible segment schemas against the same descriptor type.
fn intake_schema_core(schema_id: &str) -> Result<Vec<IntakeSchemaSection>, String> {
    if schema_id != "lsm-v2.1" {
        return Err(format!("unknown schema_id: {schema_id}"));
    }
    Ok(LSM_V2_1
        .iter()
        .map(|(section, fields)| IntakeSchemaSection {
            section: (*section).to_string(),
            fields: fields.to_vec(),
        })
        .collect())
}

async fn character_autosuggest_for_scene_core(
    db: Arc<Mutex<Db>>,
    scene_id: String,
    body_text: String,
) -> Result<Vec<AutosuggestResultDto>, String> {
    // Parse-and-discard: validates the scene id at the command boundary
    // so a malformed id errors loudly instead of silently being ignored.
    // The id itself is unused today (we autosuggest based on body text
    // alone) but a future implementation can use it to e.g. exclude
    // already-linked characters from suggestions.
    let _scene_id: Id = scene_id
        .parse()
        .map_err(|e: water_core::Error| e.to_string())?;

    // Hold the DB lock only for the SELECT — `list_all_with_aliases`
    // drains its statement into an owned `Vec<AutosuggestRow>` before
    // returning, so the regex scan that follows needs no DB access.
    // Drop the guard before the (potentially multi-KB body × N character)
    // regex scan so concurrent DB writers aren't blocked on autosave.
    // (This command runs on the 2s debounced autosave loop in F4.)
    let all_chars = {
        let db_guard = db.lock().await;
        CharacterStore::list_all_with_aliases(&db_guard).map_err(|e| e.to_string())?
    };
    let results = suggest_for_scene_body(&body_text, &all_chars);
    Ok(results
        .into_iter()
        .map(|r| AutosuggestResultDto {
            character_id: r.character_id.to_string(),
            full_name: r.full_name,
            mention_count: r.mention_count,
        })
        .collect())
}

// ----------------------------------------------------------------------
// Tauri command shims
// ----------------------------------------------------------------------

#[tauri::command]
pub async fn character_create(
    state: State<'_, AppState>,
) -> Result<CharacterIndexEntry, String> {
    let (db, root, project_id) = {
        let proj = state.project.lock().await;
        let project = proj.as_ref().ok_or("no project open")?;
        (
            project.db.clone(),
            project.root.clone(),
            project.project_id.clone(),
        )
    };
    character_create_core(db, root, project_id).await
}

#[tauri::command]
pub async fn character_read(
    state: State<'_, AppState>,
    id: String,
) -> Result<CharacterFile, String> {
    let (db, root) = {
        let proj = state.project.lock().await;
        let project = proj.as_ref().ok_or("no project open")?;
        (project.db.clone(), project.root.clone())
    };
    character_read_core(db, root, id).await
}

#[tauri::command]
pub async fn character_list(
    state: State<'_, AppState>,
) -> Result<Vec<CharacterIndexEntry>, String> {
    let db = {
        let proj = state.project.lock().await;
        let project = proj.as_ref().ok_or("no project open")?;
        project.db.clone()
    };
    character_list_core(db).await
}

#[tauri::command]
pub async fn character_update_field(
    state: State<'_, AppState>,
    id: String,
    field_id: String,
    value: serde_json::Value,
) -> Result<CharacterIndexEntry, String> {
    let (db, root, locks) = {
        let proj = state.project.lock().await;
        let project = proj.as_ref().ok_or("no project open")?;
        (
            project.db.clone(),
            project.root.clone(),
            project.character_write_locks.clone(),
        )
    };
    character_update_field_core(db, root, locks, id, field_id, value).await
}

#[tauri::command]
pub async fn character_delete(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let (db, root, locks) = {
        let proj = state.project.lock().await;
        let project = proj.as_ref().ok_or("no project open")?;
        (
            project.db.clone(),
            project.root.clone(),
            project.character_write_locks.clone(),
        )
    };
    character_delete_core(db, root, locks, id).await
}

#[tauri::command]
pub async fn character_link_to_scene(
    state: State<'_, AppState>,
    scene_id: String,
    character_id: String,
) -> Result<(), String> {
    let (db, scene_locks) = {
        let proj = state.project.lock().await;
        let project = proj.as_ref().ok_or("no project open")?;
        (project.db.clone(), project.scene_write_locks.clone())
    };
    character_link_to_scene_core(db, scene_locks, scene_id, character_id).await
}

#[tauri::command]
pub async fn character_unlink_from_scene(
    state: State<'_, AppState>,
    scene_id: String,
    character_id: String,
) -> Result<(), String> {
    let (db, scene_locks) = {
        let proj = state.project.lock().await;
        let project = proj.as_ref().ok_or("no project open")?;
        (project.db.clone(), project.scene_write_locks.clone())
    };
    character_unlink_from_scene_core(db, scene_locks, scene_id, character_id).await
}

#[tauri::command]
pub async fn character_set_pov(
    state: State<'_, AppState>,
    scene_id: String,
    character_id: Option<String>,
) -> Result<(), String> {
    let (db, scene_locks) = {
        let proj = state.project.lock().await;
        let project = proj.as_ref().ok_or("no project open")?;
        (project.db.clone(), project.scene_write_locks.clone())
    };
    character_set_pov_core(db, scene_locks, scene_id, character_id).await
}

/// Return the intake schema sections for `schema_id`. The renderer's
/// `ConversationalIntake` component reads this once on mount to render
/// questions one at a time.
///
/// Stateless — no project needs to be open. `async` only because the
/// Tauri macro infrastructure prefers async commands.
#[tauri::command]
pub async fn intake_schema(schema_id: String) -> Result<Vec<IntakeSchemaSection>, String> {
    intake_schema_core(&schema_id)
}

/// Scan `body_text` for character names + aliases and return the top
/// five hits. Called from the Scene Metadata sheet's autosave loop
/// (F4). `scene_id` is validated but not yet used for filtering — see
/// `character_autosuggest_for_scene_core` for the rationale.
#[tauri::command]
pub async fn character_autosuggest_for_scene(
    state: State<'_, AppState>,
    scene_id: String,
    body_text: String,
) -> Result<Vec<AutosuggestResultDto>, String> {
    let db = {
        let proj = state.project.lock().await;
        let project = proj.as_ref().ok_or("no project open")?;
        project.db.clone()
    };
    character_autosuggest_for_scene_core(db, scene_id, body_text).await
}

// ----------------------------------------------------------------------
// Tests
// ----------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;
    use water_core::{ManuscriptStore, NewScene, ProjectStore, SceneRow, SceneStore};

    /// Build a fresh project on disk + DB. Returns `(temp_dir, db, root,
    /// project_id, locks)` — the inputs every `_core` helper takes.
    ///
    /// We bypass the Tauri command layer because `tauri::State<'_, T>` has
    /// no public test constructor. The `_core` helpers in this file are
    /// the actual unit-under-test; the `#[tauri::command]` shims are just
    /// argument plumbing.
    async fn test_project() -> (TempDir, Arc<Mutex<Db>>, PathBuf, String, CharacterWriteLocks)
    {
        let dir = TempDir::new().unwrap();
        let root = dir.path().to_path_buf();
        std::fs::create_dir_all(root.join("characters")).unwrap();
        let db_raw = Db::open(root.join("project.db")).unwrap();
        let db = Arc::new(Mutex::new(db_raw));
        let project_id = {
            let g = db.lock().await;
            ProjectStore::new(&g).insert("TestProject").unwrap().id
        };
        (
            dir,
            db,
            root,
            project_id.to_string(),
            CharacterWriteLocks::new(),
        )
    }

    /// Same as `test_project` but also creates a manuscript and one scene
    /// so T13 scene-linkage tests have something to link characters to.
    /// Returns the existing tuple extended with `(scene_row, scene_locks)`.
    /// We bypass `scene_create` (the Tauri command) because that requires
    /// `tauri::State` and the snapshot scheduler — neither of which we
    /// need here. Going through `SceneStore::create` directly mirrors what
    /// the production command does for the bits we care about.
    async fn test_project_with_scene() -> (
        TempDir,
        Arc<Mutex<Db>>,
        PathBuf,
        String,
        CharacterWriteLocks,
        SceneRow,
        SceneWriteLocks,
    ) {
        let (dir, db, root, pid, char_locks) = test_project().await;
        let project_id: Id = pid.parse().unwrap();
        let scene = {
            let g = db.lock().await;
            let manuscript = ManuscriptStore::new(&g)
                .insert(&project_id, "Manuscript", 0)
                .unwrap();
            let store = SceneStore::new(&g, root.clone());
            store
                .create(NewScene {
                    manuscript_id: manuscript.id,
                    chapter_id: None,
                    name: "Scene 1".into(),
                    ordering: 0,
                })
                .unwrap()
        };
        (
            dir,
            db,
            root,
            pid,
            char_locks,
            scene,
            SceneWriteLocks::new(),
        )
    }

    /// Read `scene.pov_character_id` for assertions. Returns `None` when
    /// POV is NULL.
    async fn read_pov(db: &Arc<Mutex<Db>>, scene_id: &str) -> Option<String> {
        let g = db.lock().await;
        g.conn()
            .query_row(
                "SELECT pov_character_id FROM scene WHERE id = ?1",
                [scene_id],
                |r| r.get::<_, Option<String>>(0),
            )
            .unwrap()
    }

    #[tokio::test]
    async fn create_character_assigns_round_robin_hue() {
        let (_dir, db, root, pid, _locks) = test_project().await;
        let c1 = character_create_core(db.clone(), root.clone(), pid.clone())
            .await
            .unwrap();
        let c2 = character_create_core(db.clone(), root.clone(), pid.clone())
            .await
            .unwrap();
        let c3 = character_create_core(db.clone(), root.clone(), pid.clone())
            .await
            .unwrap();
        assert_eq!(c1.hue_token, "--water-hue-character-1");
        assert_eq!(c2.hue_token, "--water-hue-character-2");
        assert_eq!(c3.hue_token, "--water-hue-character-3");
        // Fresh character with no fields filled — completion is 0.
        assert_eq!(c1.completion, 0);
    }

    #[tokio::test]
    async fn create_character_persists_through_list() {
        let (_dir, db, root, pid, _locks) = test_project().await;
        let _ = character_create_core(db.clone(), root.clone(), pid.clone())
            .await
            .unwrap();
        let _ = character_create_core(db.clone(), root.clone(), pid.clone())
            .await
            .unwrap();
        let list = character_list_core(db.clone()).await.unwrap();
        assert_eq!(list.len(), 2);
    }

    #[tokio::test]
    async fn update_field_full_name_appends_old_name_to_aliases() {
        let (_dir, db, root, pid, locks) = test_project().await;
        let c = character_create_core(db.clone(), root.clone(), pid.clone())
            .await
            .unwrap();
        character_update_field_core(
            db.clone(),
            root.clone(),
            locks.clone(),
            c.id.clone(),
            "main.full_name".into(),
            json!("Marcus Vale"),
        )
        .await
        .unwrap();
        let updated = character_update_field_core(
            db.clone(),
            root.clone(),
            locks.clone(),
            c.id.clone(),
            "main.full_name".into(),
            json!("Marcus Tenebris"),
        )
        .await
        .unwrap();
        assert_eq!(updated.full_name, "Marcus Tenebris");

        let file = character_read_core(db.clone(), root.clone(), c.id.clone())
            .await
            .unwrap();
        let aliases = file
            .data
            .get("main")
            .and_then(|m| m.as_table())
            .and_then(|m| m.get("aliases"))
            .and_then(|v| v.as_array())
            .expect("aliases array");
        let names: Vec<&str> = aliases.iter().filter_map(|v| v.as_str()).collect();
        assert!(
            names.contains(&"Marcus Vale"),
            "rename cascade should push old name to aliases; got: {names:?}"
        );
    }

    #[tokio::test]
    async fn update_field_completion_pct_climbs_with_required_fields() {
        let (_dir, db, root, pid, locks) = test_project().await;
        let c = character_create_core(db.clone(), root.clone(), pid.clone())
            .await
            .unwrap();
        assert_eq!(c.completion, 0);
        let updated = character_update_field_core(
            db.clone(),
            root.clone(),
            locks.clone(),
            c.id.clone(),
            "main.full_name".into(),
            json!("Marcus"),
        )
        .await
        .unwrap();
        // 1 of 8 required filled = 12 (floor of 12.5).
        assert_eq!(updated.completion, 12);
    }

    #[tokio::test]
    async fn delete_cascades_to_scene_presence_and_pov() {
        // End-to-end cascade through the command surface: T13 commands
        // (link + set_pov) followed by the T12 delete cascade. The same
        // contract is also covered at the water-core unit-test level in
        // `delete_and_cascade_clears_scene_pov_and_presence`; this version
        // proves the Tauri command shims compose correctly.
        let (_dir, db, root, pid, char_locks, scene, scene_locks) =
            test_project_with_scene().await;
        let c = character_create_core(db.clone(), root.clone(), pid.clone())
            .await
            .unwrap();
        character_link_to_scene_core(
            db.clone(),
            scene_locks.clone(),
            scene.id.to_string(),
            c.id.clone(),
        )
        .await
        .unwrap();
        character_set_pov_core(
            db.clone(),
            scene_locks.clone(),
            scene.id.to_string(),
            Some(c.id.clone()),
        )
        .await
        .unwrap();

        // Sanity: POV is set, presence row exists.
        assert_eq!(
            read_pov(&db, scene.id.as_str()).await.as_deref(),
            Some(c.id.as_str()),
            "fixture precondition: POV should be set before delete",
        );
        let presence_before: i64 = db
            .lock()
            .await
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM scene_character_presence WHERE scene_id = ?1 AND character_id = ?2",
                rusqlite::params![scene.id.as_str(), c.id.as_str()],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(presence_before, 1, "fixture precondition: presence row should exist before delete");

        // Cascade through the T12 command.
        character_delete_core(db.clone(), root.clone(), char_locks.clone(), c.id.clone())
            .await
            .unwrap();

        // Both invariants hold post-delete.
        assert!(
            read_pov(&db, scene.id.as_str()).await.is_none(),
            "delete cascade should NULL the scene POV",
        );
        let presence_after: i64 = db
            .lock()
            .await
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM scene_character_presence WHERE scene_id = ?1 AND character_id = ?2",
                rusqlite::params![scene.id.as_str(), c.id.as_str()],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(presence_after, 0, "delete cascade should remove presence rows");
    }

    // ------------------------------------------------------------------
    // M3 T13: scene linkage
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn link_then_set_pov_succeeds() {
        let (_dir, db, root, pid, _char_locks, scene, scene_locks) =
            test_project_with_scene().await;
        let c = character_create_core(db.clone(), root.clone(), pid.clone())
            .await
            .unwrap();
        character_link_to_scene_core(
            db.clone(),
            scene_locks.clone(),
            scene.id.to_string(),
            c.id.clone(),
        )
        .await
        .unwrap();
        character_set_pov_core(
            db.clone(),
            scene_locks.clone(),
            scene.id.to_string(),
            Some(c.id.clone()),
        )
        .await
        .unwrap();
        let pov = read_pov(&db, scene.id.as_str()).await;
        assert_eq!(pov.as_deref(), Some(c.id.as_str()));
    }

    #[tokio::test]
    async fn set_pov_without_link_returns_error() {
        let (_dir, db, root, pid, _char_locks, scene, scene_locks) =
            test_project_with_scene().await;
        let c = character_create_core(db.clone(), root.clone(), pid.clone())
            .await
            .unwrap();
        let err = character_set_pov_core(
            db.clone(),
            scene_locks.clone(),
            scene.id.to_string(),
            Some(c.id.clone()),
        )
        .await
        .unwrap_err();
        assert!(
            err.contains("characters_present"),
            "expected presence-constraint error, got: {err}"
        );
        // POV must still be NULL.
        let pov = read_pov(&db, scene.id.as_str()).await;
        assert!(
            pov.is_none(),
            "POV should remain NULL on rejected set, got {pov:?}"
        );
    }

    // ------------------------------------------------------------------
    // M3 T14: intake schema + autosuggest
    // ------------------------------------------------------------------

    #[test]
    fn intake_schema_returns_lsm_v2_1_in_full() {
        // Asserts the command surfaces exactly what's in the source-of-truth
        // constant. The canonical `== 29` assertion lives in `intake.rs`
        // (`lsm_v2_1_has_29_fields_total`); duplicating it here would mean
        // two test failures with the same root cause on schema growth.
        let sections = intake_schema_core("lsm-v2.1").unwrap();
        let total: usize = sections.iter().map(|s| s.fields.len()).sum();
        let expected: usize = water_core::character::intake::LSM_V2_1
            .iter()
            .map(|(_, fields)| fields.len())
            .sum();
        assert_eq!(total, expected);
    }

    #[test]
    fn intake_schema_unknown_errors() {
        let err = intake_schema_core("garbage").unwrap_err();
        assert!(
            err.contains("unknown schema_id"),
            "expected 'unknown schema_id' in error, got: {err}"
        );
    }

    #[tokio::test]
    async fn autosuggest_excludes_zero_mention_chars() {
        let (_dir, db, root, pid, locks, scene, _scene_locks) =
            test_project_with_scene().await;
        let c1 = character_create_core(db.clone(), root.clone(), pid.clone())
            .await
            .unwrap();
        character_update_field_core(
            db.clone(),
            root.clone(),
            locks.clone(),
            c1.id.clone(),
            "main.full_name".into(),
            json!("Marcus"),
        )
        .await
        .unwrap();
        let c2 = character_create_core(db.clone(), root.clone(), pid.clone())
            .await
            .unwrap();
        character_update_field_core(
            db.clone(),
            root.clone(),
            locks.clone(),
            c2.id.clone(),
            "main.full_name".into(),
            json!("Talia"),
        )
        .await
        .unwrap();

        let results = character_autosuggest_for_scene_core(
            db.clone(),
            scene.id.to_string(),
            "Marcus walked in.".into(),
        )
        .await
        .unwrap();
        assert_eq!(results.len(), 1, "Talia has zero mentions; should be excluded");
        assert_eq!(results[0].full_name, "Marcus");
        assert_eq!(results[0].mention_count, 1);
    }

    #[tokio::test]
    async fn unlink_clears_pov_if_was_pov() {
        let (_dir, db, root, pid, _char_locks, scene, scene_locks) =
            test_project_with_scene().await;
        let c = character_create_core(db.clone(), root.clone(), pid.clone())
            .await
            .unwrap();
        character_link_to_scene_core(
            db.clone(),
            scene_locks.clone(),
            scene.id.to_string(),
            c.id.clone(),
        )
        .await
        .unwrap();
        character_set_pov_core(
            db.clone(),
            scene_locks.clone(),
            scene.id.to_string(),
            Some(c.id.clone()),
        )
        .await
        .unwrap();
        // Sanity: POV is set before we unlink.
        assert_eq!(
            read_pov(&db, scene.id.as_str()).await.as_deref(),
            Some(c.id.as_str())
        );
        character_unlink_from_scene_core(
            db.clone(),
            scene_locks.clone(),
            scene.id.to_string(),
            c.id.clone(),
        )
        .await
        .unwrap();
        let pov = read_pov(&db, scene.id.as_str()).await;
        assert!(
            pov.is_none(),
            "unlinking the POV character should auto-clear POV (spec § 20), got {pov:?}"
        );
    }
}
