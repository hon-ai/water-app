use crate::state::AppState;
use serde::Serialize;
use std::sync::Arc;
use tauri::State;
use tokio::sync::Mutex;
use water_core::{Db, Id, NewScene, SceneFile, SceneStore};

#[derive(Serialize)]
pub struct SceneInfo {
    pub id: String,
    pub name: String,
    pub ordering: i64,
    pub word_count: i64,
}

/// Renderer-facing snapshot of a scene's character metadata (M3 T21).
/// Returned by `scene_read_metadata` to populate the SceneMetadataSheet
/// without forcing the renderer to round-trip through scene_list (which
/// doesn't carry presence + POV today).
#[derive(Serialize, Debug, Clone)]
pub struct SceneMetadata {
    pub characters_present: Vec<String>,
    pub pov_character_id: Option<String>,
}

async fn scene_read_metadata_core(
    db: Arc<Mutex<Db>>,
    scene_id: String,
) -> Result<SceneMetadata, String> {
    let scene_id: Id = scene_id
        .parse()
        .map_err(|e: water_core::Error| e.to_string())?;
    let db_guard = db.lock().await;
    let conn = db_guard.conn();

    // POV is nullable (SQLite NULL → Option::None).
    let pov_character_id: Option<String> = conn
        .query_row(
            "SELECT pov_character_id FROM scene WHERE id = ?1",
            [scene_id.as_str()],
            |r| r.get::<_, Option<String>>(0),
        )
        .map_err(|e| e.to_string())?;

    // Presence rows. No ordering guarantee on the wire — the renderer
    // displays them via a checkbox list keyed by character id, so order
    // doesn't matter (and indexing into characters_present by position
    // would be a bug regardless).
    let mut stmt = conn
        .prepare("SELECT character_id FROM scene_character_presence WHERE scene_id = ?1")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([scene_id.as_str()], |r| r.get::<_, String>(0))
        .map_err(|e| e.to_string())?;
    let mut characters_present: Vec<String> = Vec::new();
    for row in rows {
        characters_present.push(row.map_err(|e| e.to_string())?);
    }

    Ok(SceneMetadata {
        characters_present,
        pov_character_id,
    })
}

#[tauri::command]
pub async fn scene_create(state: State<'_, AppState>, name: String) -> Result<SceneInfo, String> {
    let proj = state.project.lock().await;
    let project = proj.as_ref().ok_or("no project open")?;
    let manuscript_id: Id = project
        .default_manuscript_id
        .parse()
        .map_err(|e: water_core::Error| e.to_string())?;
    let db = project.db.clone();
    let root = project.root.clone();

    let row = {
        let db_guard = db.lock().await;
        let store = SceneStore::new(&db_guard, root.clone());
        let count: i64 = db_guard
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM scene WHERE manuscript_id = ?1",
                [manuscript_id.as_str()],
                |r| r.get(0),
            )
            .map_err(|e| e.to_string())?;
        store
            .create(NewScene {
                manuscript_id,
                chapter_id: None,
                name,
                ordering: count,
            })
            .map_err(|e| e.to_string())?
    };

    // Register the new scene with the scheduler so hourly + on-close snapshots
    // include it. We're still holding the project guard, which is the easiest
    // way to reach project.scheduler without restructuring ownership.
    project
        .scheduler
        .register(water_core::ActiveScene {
            scene_id: row.id.clone(),
            file_path: row.file_path.clone(),
        })
        .await;

    Ok(SceneInfo {
        id: row.id.to_string(),
        name: row.name,
        ordering: row.ordering,
        word_count: row.word_count,
    })
}

#[tauri::command]
pub async fn scene_read(state: State<'_, AppState>, id: String) -> Result<String, String> {
    let (db, root) = {
        let proj = state.project.lock().await;
        let project = proj.as_ref().ok_or("no project open")?;
        (project.db.clone(), project.root.clone())
    };
    let id: Id = id
        .parse()
        .map_err(|e: water_core::Error| e.to_string())?;
    let db_guard = db.lock().await;
    let store = SceneStore::new(&db_guard, root);
    let file: SceneFile = store.read(&id).map_err(|e| e.to_string())?;
    Ok(file.body)
}

#[tauri::command]
pub async fn scene_write_body(
    state: State<'_, AppState>,
    id: String,
    body: String,
) -> Result<SceneInfo, String> {
    let (db, root, locks) = {
        let proj = state.project.lock().await;
        let project = proj.as_ref().ok_or("no project open")?;
        (
            project.db.clone(),
            project.root.clone(),
            project.scene_write_locks.clone(),
        )
    };
    let id: Id = id
        .parse()
        .map_err(|e: water_core::Error| e.to_string())?;
    // Per-scene write lock: serializes `rename` + `write_body` so concurrent
    // flushes don't tear the scene file (KNOWN_FRAGILE #7). Acquired BEFORE
    // the DB lock so the lock ordering matches `scene_rename`.
    let _write_guard = locks.acquire(&id).await;
    let db_guard = db.lock().await;
    let store = SceneStore::new(&db_guard, root);
    let row = store.write_body(&id, &body).map_err(|e| e.to_string())?;
    Ok(SceneInfo {
        id: row.id.to_string(),
        name: row.name,
        ordering: row.ordering,
        word_count: row.word_count,
    })
}

#[tauri::command]
pub async fn scene_list(state: State<'_, AppState>) -> Result<Vec<SceneInfo>, String> {
    let (db, root, manuscript_id) = {
        let proj = state.project.lock().await;
        let project = proj.as_ref().ok_or("no project open")?;
        let manuscript_id: Id = project
            .default_manuscript_id
            .parse()
            .map_err(|e: water_core::Error| e.to_string())?;
        (project.db.clone(), project.root.clone(), manuscript_id)
    };
    let db_guard = db.lock().await;
    let store = SceneStore::new(&db_guard, root);
    let rows = store.list(&manuscript_id).map_err(|e| e.to_string())?;
    Ok(rows
        .into_iter()
        .map(|r| SceneInfo {
            id: r.id.to_string(),
            name: r.name,
            ordering: r.ordering,
            word_count: r.word_count,
        })
        .collect())
}

/// Read the per-scene character metadata (`characters_present` +
/// `pov_character_id`). Used by the SceneMetadataSheet (M3 T21) — the
/// scene-list command intentionally doesn't carry these fields because
/// most callers (sidebar, autosave) don't need them.
#[tauri::command]
pub async fn scene_read_metadata(
    state: State<'_, AppState>,
    id: String,
) -> Result<SceneMetadata, String> {
    let db = {
        let proj = state.project.lock().await;
        let project = proj.as_ref().ok_or("no project open")?;
        project.db.clone()
    };
    scene_read_metadata_core(db, id).await
}

#[tauri::command]
pub async fn scene_rename(
    state: State<'_, AppState>,
    id: String,
    name: String,
) -> Result<SceneInfo, String> {
    let (db, root, locks) = {
        let proj = state.project.lock().await;
        let project = proj.as_ref().ok_or("no project open")?;
        (
            project.db.clone(),
            project.root.clone(),
            project.scene_write_locks.clone(),
        )
    };
    let id: Id = id
        .parse()
        .map_err(|e: water_core::Error| e.to_string())?;
    // Per-scene write lock: serializes `rename` + `write_body` so concurrent
    // flushes don't tear the scene file (KNOWN_FRAGILE #7).
    // Acquired BEFORE the DB lock so the lock ordering matches `scene_write_body`.
    // Both commands acquire project_lock -> (drop) -> scene_write_lock -> db_lock.
    let _write_guard = locks.acquire(&id).await;
    let db_guard = db.lock().await;
    let store = SceneStore::new(&db_guard, root);
    let row = store.rename(&id, &name).map_err(|e| e.to_string())?;
    Ok(SceneInfo {
        id: row.id.to_string(),
        name: row.name,
        ordering: row.ordering,
        word_count: row.word_count,
    })
}

// ----------------------------------------------------------------------
// Tests for `scene_read_metadata_core` (M3 T21).
//
// We exercise only the `_core` helper — the `#[tauri::command]` shim is
// pure argument plumbing (see `commands/character.rs::tests` for the
// same rationale). The fixture here is intentionally a minimal local
// copy of `character::tests::test_project_with_scene` rather than a
// shared module: keeping it private to this file matches the existing
// pattern and avoids a cross-file refactor for two small tests.
// ----------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;
    use water_core::{ManuscriptStore, ProjectStore, SceneRow};

    async fn test_project_with_scene() -> (TempDir, Arc<Mutex<Db>>, PathBuf, SceneRow) {
        let dir = TempDir::new().unwrap();
        let root = dir.path().to_path_buf();
        std::fs::create_dir_all(root.join("characters")).unwrap();
        let db_raw = Db::open(root.join("project.db")).unwrap();
        let db = Arc::new(Mutex::new(db_raw));
        let scene = {
            let g = db.lock().await;
            let project = ProjectStore::new(&g).insert("TestProject").unwrap();
            let manuscript = ManuscriptStore::new(&g)
                .insert(&project.id, "Manuscript", 0)
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
        (dir, db, root, scene)
    }

    #[tokio::test]
    async fn read_metadata_empty_for_fresh_scene() {
        let (_dir, db, _root, scene) = test_project_with_scene().await;
        let meta = scene_read_metadata_core(db.clone(), scene.id.to_string())
            .await
            .unwrap();
        assert!(meta.characters_present.is_empty());
        assert!(meta.pov_character_id.is_none());
    }

    #[tokio::test]
    async fn read_metadata_reflects_presence_and_pov() {
        use water_core::character::{next_hue_token, CharacterStore, NewCharacter};

        let (_dir, db, root, scene) = test_project_with_scene().await;

        // Seed a character via the real store (so `created_at`/`updated_at`/
        // `file_path` are populated correctly), then write the presence +
        // POV rows directly — both foreign-key columns we exercise here
        // (`scene_character_presence.character_id`, `scene.pov_character_id`)
        // only reference `character.id`, so the read surface under test
        // doesn't care about other character columns.
        let char_id = {
            let g = db.lock().await;
            let conn = g.conn();
            // project_id via scene → manuscript chain.
            let project_id_str: String = conn
                .query_row(
                    "SELECT m.project_id FROM scene s \
                     JOIN manuscript m ON m.id = s.manuscript_id \
                     WHERE s.id = ?1",
                    [scene.id.as_str()],
                    |r| r.get(0),
                )
                .unwrap();
            let project_id: Id = project_id_str.parse().unwrap();
            let hue = next_hue_token(&g).unwrap();
            let store = CharacterStore::new(&g, root.clone());
            let row = store
                .create(NewCharacter {
                    project_id,
                    hue_token: hue,
                })
                .unwrap();
            conn.execute(
                "INSERT INTO scene_character_presence (scene_id, character_id) VALUES (?1, ?2)",
                rusqlite::params![scene.id.as_str(), row.id.as_str()],
            )
            .unwrap();
            conn.execute(
                "UPDATE scene SET pov_character_id = ?1 WHERE id = ?2",
                rusqlite::params![row.id.as_str(), scene.id.as_str()],
            )
            .unwrap();
            row.id.to_string()
        };

        let meta = scene_read_metadata_core(db.clone(), scene.id.to_string())
            .await
            .unwrap();
        assert_eq!(meta.characters_present, vec![char_id.clone()]);
        assert_eq!(meta.pov_character_id.as_deref(), Some(char_id.as_str()));
    }
}
