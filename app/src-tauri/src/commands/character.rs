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
    character::{completion_pct, next_hue_token, CharacterFile, CharacterStore, NewCharacter},
    CharacterWriteLocks, Db, Id,
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

// ----------------------------------------------------------------------
// Tests
// ----------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;
    use water_core::ProjectStore;

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
    #[ignore = "depends on T13 (character_link_to_scene + character_set_pov commands)"]
    async fn delete_cascades_to_scene_presence_and_pov() {
        // T13 will land character_link_to_scene + character_set_pov; this
        // placeholder asserts the cascade contract end-to-end through the
        // command surface. The same cascade is already covered at the
        // water-core unit-test level in `delete_and_cascade_clears_scene_pov_and_presence`.
    }
}
