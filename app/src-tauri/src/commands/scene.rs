use crate::state::AppState;
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
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

/// Renderer-facing snapshot of the scene's currently-set location (M4 T11).
/// Sent as part of `SceneMetadata` so the SceneMetadataSheet can render the
/// location pill (name + segment slug for hue/badge selection) without a
/// separate round-trip to `world_entry_read`.
///
/// `segment_slug` is the parent `world_segment.slug` (e.g. `"locations"`).
/// We include the slug rather than the segment id because the renderer
/// keys its segment-specific styling (hue token, label) by slug.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneLocationPayload {
    pub id: String,
    pub name: String,
    pub segment_slug: String,
}

/// Renderer-facing snapshot of a scene's character + location metadata
/// (M3 T21 + M4 T11). Returned by `scene_read_metadata` to populate the
/// SceneMetadataSheet without forcing the renderer to round-trip through
/// `scene_list` (which doesn't carry presence + POV) or `world_entry_read`
/// (which would require a second await on every sheet open).
///
/// `location` is `None` when `scene.location_id IS NULL`.
#[derive(Serialize, Debug, Clone)]
pub struct SceneMetadata {
    pub characters_present: Vec<String>,
    pub pov_character_id: Option<String>,
    pub location: Option<SceneLocationPayload>,
    /// Brief writer-supplied summary of what happens in the scene.
    /// Sourced from `scene.scene_goal`. `None` when unset.
    pub summary: Option<String>,
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

    // POV + summary in one round trip.
    let (pov_character_id, summary): (Option<String>, Option<String>) = conn
        .query_row(
            "SELECT pov_character_id, scene_goal FROM scene WHERE id = ?1",
            [scene_id.as_str()],
            |r| Ok((r.get::<_, Option<String>>(0)?, r.get::<_, Option<String>>(1)?)),
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

    // Resolve `scene.location_id` to a `{id, name, segment_slug}` payload
    // (M4 T11). LEFT JOIN through world_entry → world_segment so a single
    // round-trip returns either the populated payload or `None` (which
    // happens both when `location_id IS NULL` and — defensively — if the
    // FK is dangling, though the `ON DELETE SET NULL` clause + rebuild
    // orphan-reap should prevent that in practice).
    drop(stmt);
    let location: Option<SceneLocationPayload> = conn
        .query_row(
            "SELECT we.id, we.name, ws.slug
             FROM scene s
             JOIN world_entry we ON we.id = s.location_id
             JOIN world_segment ws ON ws.id = we.segment_id
             WHERE s.id = ?1",
            [scene_id.as_str()],
            |r| {
                Ok(SceneLocationPayload {
                    id: r.get(0)?,
                    name: r.get(1)?,
                    segment_slug: r.get(2)?,
                })
            },
        )
        .optional()
        .map_err(|e| e.to_string())?;

    Ok(SceneMetadata {
        characters_present,
        pov_character_id,
        location,
        summary,
    })
}

/// Persist a scene's summary. `None` / empty string clears it.
#[tauri::command]
pub async fn scene_set_summary(
    state: State<'_, AppState>,
    scene_id: String,
    summary: Option<String>,
) -> Result<(), String> {
    let (db, root) = {
        let proj = state.project.lock().await;
        let p = proj.as_ref().ok_or("no project open")?;
        (p.db.clone(), p.root.clone())
    };
    let scene_id: Id = scene_id
        .parse()
        .map_err(|e: water_core::Error| e.to_string())?;
    let g = db.lock().await;
    let store = SceneStore::new(&g, root);
    store
        .set_summary(&scene_id, summary.as_deref())
        .map_err(|e| e.to_string())
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
    let id: Id = id.parse().map_err(|e: water_core::Error| e.to_string())?;
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
    let id: Id = id.parse().map_err(|e: water_core::Error| e.to_string())?;
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
    let id: Id = id.parse().map_err(|e: water_core::Error| e.to_string())?;
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

/// Set or clear `scene.location_id`. Pass `Some(world_entry_id)` to
/// attach a location, or `None` to clear it. (M4 T11.)
///
/// FK integrity is enforced by the column's `REFERENCES world_entry(id)
/// ON DELETE SET NULL` clause — a non-existent `location_id` will trip
/// the FK and surface as a SQLite error here. The renderer is expected
/// to pick the id from a `world_entry_list` result, so a missing FK
/// indicates either a stale UI cache or a race with `world_entry_delete`.
async fn scene_set_location_core(
    db: Arc<Mutex<Db>>,
    scene_id: String,
    location_id: Option<String>,
) -> Result<(), String> {
    let scene_id: Id = scene_id
        .parse()
        .map_err(|e: water_core::Error| e.to_string())?;
    // Validate the location id at the command boundary so a malformed
    // string doesn't reach SQLite as an arbitrary text value (the FK
    // would silently accept it since SQLite doesn't type-check FK refs
    // beyond existence).
    let location_id: Option<Id> = match location_id {
        Some(s) => Some(s.parse().map_err(|e: water_core::Error| e.to_string())?),
        None => None,
    };
    let db_guard = db.lock().await;
    let conn = db_guard.conn();
    let result = match location_id.as_ref() {
        Some(loc) => conn.execute(
            "UPDATE scene SET location_id = ?1 WHERE id = ?2",
            (loc.as_str(), scene_id.as_str()),
        ),
        None => conn.execute(
            "UPDATE scene SET location_id = NULL WHERE id = ?1",
            [scene_id.as_str()],
        ),
    };
    result.map(|_| ()).map_err(|e| e.to_string())
}

/// Tauri-command wrapper around `scene_set_location_core`. Mirrors the
/// `scene_read_metadata` shape: lock the project guard, clone the db
/// handle, drop the guard, call `_core` (so the project lock isn't held
/// across the DB lock — KNOWN_FRAGILE #6 lock ordering).
#[tauri::command]
pub async fn scene_set_location(
    state: State<'_, AppState>,
    scene_id: String,
    location_id: Option<String>,
) -> Result<(), String> {
    let db = {
        let proj = state.project.lock().await;
        let project = proj.as_ref().ok_or("no project open")?;
        project.db.clone()
    };
    scene_set_location_core(db, scene_id, location_id).await
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
        assert!(meta.location.is_none(), "fresh scene must have no location");
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

    /// M4 T11: round-trip the scene→location FK through both write +
    /// read commands. Uses `WorldStore::seed_builtins` to pull in the
    /// `locations` collection segment (the only built-in collection),
    /// then creates a single entry, sets `scene.location_id` to it via
    /// `scene_set_location_core`, and asserts the read-metadata payload
    /// surfaces the `{id, name, segment_slug}` triple. Then clears it
    /// (with `None`) and asserts the field goes back to `None`.
    #[tokio::test]
    async fn scene_set_location_round_trip() {
        use water_core::WorldStore;

        let (_dir, db, root, scene) = test_project_with_scene().await;

        let (entry_id, expected_slug, expected_name) = {
            let g = db.lock().await;
            // Look up the project_id that `test_project_with_scene`
            // inserted (we don't have it returned by the fixture).
            let project_id_str: String = g
                .conn()
                .query_row(
                    "SELECT m.project_id FROM scene s \
                     JOIN manuscript m ON m.id = s.manuscript_id \
                     WHERE s.id = ?1",
                    [scene.id.as_str()],
                    |r| r.get(0),
                )
                .unwrap();
            let project_id: Id = project_id_str.parse().unwrap();
            let store = WorldStore::new(&g, root.clone());
            store.seed_builtins(&project_id).unwrap();
            let loc_seg = store
                .find_segment_by_slug(&project_id, "locations")
                .unwrap()
                .expect("seed_builtins must create a `locations` segment");
            let entry_id = store
                .create_entry(&loc_seg.id, "The Old Lighthouse")
                .unwrap();
            (entry_id.to_string(), loc_seg.slug, "The Old Lighthouse")
        };

        // Attach.
        scene_set_location_core(db.clone(), scene.id.to_string(), Some(entry_id.clone()))
            .await
            .unwrap();
        let meta = scene_read_metadata_core(db.clone(), scene.id.to_string())
            .await
            .unwrap();
        let loc = meta.location.expect("location must be set after attach");
        assert_eq!(loc.id, entry_id);
        assert_eq!(loc.name, expected_name);
        assert_eq!(loc.segment_slug, expected_slug);

        // Clear.
        scene_set_location_core(db.clone(), scene.id.to_string(), None)
            .await
            .unwrap();
        let meta = scene_read_metadata_core(db.clone(), scene.id.to_string())
            .await
            .unwrap();
        assert!(
            meta.location.is_none(),
            "location must be None after clear; got {:?}",
            meta.location
        );
    }
}
