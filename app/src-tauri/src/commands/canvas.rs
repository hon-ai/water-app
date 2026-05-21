//! Macro Spatial Canvas Tauri commands (M6 Task 4).
//!
//! Four commands following the `_core` async-fn extraction pattern:
//!
//! - [`scene_canvas_list`] — returns one row per scene with canvas
//!   position + group label. The renderer applies auto-flow on the
//!   `None` slots locally.
//! - [`scene_canvas_set_position`] — debounced from the renderer.
//! - [`scene_canvas_set_group`] — `None` clears.
//! - [`scene_canvas_reset_all`] — clears every position in the open
//!   project. Renderer falls back to auto-flow after this.

use crate::state::AppState;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::State;
use tokio::sync::Mutex;
use std::collections::HashMap;
use water_core::{Db, Id, SceneStore};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Presence {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SceneCanvasRow {
    pub id: String,
    pub name: String,
    pub manuscript_ordering: i64,
    pub canvas_x: Option<f32>,
    pub canvas_y: Option<f32>,
    pub canvas_group: Option<String>,
    pub word_count: i64,
    /// M6 lanes: POV character id (NULL if unset). The renderer
    /// uses this as the *primary* lane in character mode.
    pub pov_character_id: Option<String>,
    /// Display name of the POV character (LEFT JOIN). NULL when
    /// pov_character_id is unset.
    pub pov_character_name: Option<String>,
    /// M6 lanes: primary location entry id (NULL if unset).
    pub location_id: Option<String>,
    /// Display name of the primary location entry.
    pub location_name: Option<String>,
    /// All characters present in the scene (from
    /// `scene_character_presence`). The POV character is included
    /// when present; the renderer dedupes against `pov_character_id`.
    pub character_presences: Vec<Presence>,
    /// All locations the scene touches (from
    /// `scene_location_presence`). Primary `location_id` is included
    /// when present; the renderer dedupes against it.
    pub location_presences: Vec<Presence>,
}

#[tauri::command]
pub async fn scene_canvas_list(
    state: State<'_, AppState>,
) -> Result<Vec<SceneCanvasRow>, String> {
    let (db, project_id) = {
        let proj = state.project.lock().await;
        let p = proj.as_ref().ok_or("no project open")?;
        (p.db.clone(), p.project_id.clone())
    };
    scene_canvas_list_core(db, project_id).await
}

pub async fn scene_canvas_list_core(
    db: Arc<Mutex<Db>>,
    project_id: String,
) -> Result<Vec<SceneCanvasRow>, String> {
    let project_id: Id = project_id
        .parse()
        .map_err(|e: water_core::Error| e.to_string())?;
    let g = db.lock().await;
    let mut stmt = g
        .conn()
        .prepare(
            "SELECT scene.id, scene.name, scene.ordering,
                    scene.canvas_x, scene.canvas_y, scene.canvas_group,
                    scene.word_count,
                    scene.pov_character_id, character.name,
                    scene.location_id, world_entry.name
             FROM scene
             JOIN manuscript ON manuscript.id = scene.manuscript_id
             LEFT JOIN character ON character.id = scene.pov_character_id
             LEFT JOIN world_entry ON world_entry.id = scene.location_id
             WHERE manuscript.project_id = ?1
             ORDER BY scene.ordering ASC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([project_id.as_str()], |r| {
            #[allow(clippy::cast_possible_truncation)]
            Ok(SceneCanvasRow {
                id: r.get::<_, String>(0)?,
                name: r.get::<_, String>(1)?,
                manuscript_ordering: r.get::<_, i64>(2)?,
                canvas_x: r.get::<_, Option<f64>>(3)?.map(|v| v as f32),
                canvas_y: r.get::<_, Option<f64>>(4)?.map(|v| v as f32),
                canvas_group: r.get::<_, Option<String>>(5)?,
                word_count: r.get::<_, i64>(6)?,
                pov_character_id: r.get::<_, Option<String>>(7)?,
                pov_character_name: r.get::<_, Option<String>>(8)?,
                location_id: r.get::<_, Option<String>>(9)?,
                location_name: r.get::<_, Option<String>>(10)?,
                character_presences: Vec::new(),
                location_presences: Vec::new(),
            })
        })
        .map_err(|e| e.to_string())?;
    let mut out: Vec<SceneCanvasRow> = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| e.to_string())?);
    }
    drop(stmt);

    // Bulk-load presence tables for this project so the renderer can
    // do multi-lane layout without per-scene IPC. Two extra queries
    // is cheaper than N joins.
    let chars = load_presence(
        g.conn(),
        "SELECT scp.scene_id, character.id, character.name
         FROM scene_character_presence scp
         JOIN scene ON scene.id = scp.scene_id
         JOIN manuscript ON manuscript.id = scene.manuscript_id
         JOIN character ON character.id = scp.character_id
         WHERE manuscript.project_id = ?1",
        project_id.as_str(),
    )?;
    let locs = load_presence(
        g.conn(),
        "SELECT slp.scene_id, world_entry.id, world_entry.name
         FROM scene_location_presence slp
         JOIN scene ON scene.id = slp.scene_id
         JOIN manuscript ON manuscript.id = scene.manuscript_id
         JOIN world_entry ON world_entry.id = slp.location_id
         WHERE manuscript.project_id = ?1",
        project_id.as_str(),
    )?;
    for row in &mut out {
        // Start each presence list with the primary (POV / location_id)
        // so the renderer's first entry is always the primary lane.
        let mut cs: Vec<Presence> = Vec::new();
        if let (Some(id), Some(name)) =
            (row.pov_character_id.clone(), row.pov_character_name.clone())
        {
            cs.push(Presence { id, name });
        }
        if let Some(extra) = chars.get(&row.id) {
            for p in extra {
                if !cs.iter().any(|x| x.id == p.id) {
                    cs.push(p.clone());
                }
            }
        }
        row.character_presences = cs;

        let mut ls: Vec<Presence> = Vec::new();
        if let (Some(id), Some(name)) =
            (row.location_id.clone(), row.location_name.clone())
        {
            ls.push(Presence { id, name });
        }
        if let Some(extra) = locs.get(&row.id) {
            for p in extra {
                if !ls.iter().any(|x| x.id == p.id) {
                    ls.push(p.clone());
                }
            }
        }
        row.location_presences = ls;
    }
    Ok(out)
}

fn load_presence(
    conn: &rusqlite::Connection,
    sql: &str,
    project_id: &str,
) -> Result<HashMap<String, Vec<Presence>>, String> {
    let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([project_id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                Presence {
                    id: r.get::<_, String>(1)?,
                    name: r.get::<_, String>(2)?,
                },
            ))
        })
        .map_err(|e| e.to_string())?;
    let mut out: HashMap<String, Vec<Presence>> = HashMap::new();
    for r in rows {
        let (scene_id, p) = r.map_err(|e| e.to_string())?;
        out.entry(scene_id).or_default().push(p);
    }
    Ok(out)
}

#[tauri::command]
pub async fn scene_canvas_set_position(
    state: State<'_, AppState>,
    scene_id: String,
    x: Option<f32>,
    y: Option<f32>,
) -> Result<(), String> {
    let (db, root) = {
        let proj = state.project.lock().await;
        let p = proj.as_ref().ok_or("no project open")?;
        (p.db.clone(), p.root.clone())
    };
    scene_canvas_set_position_core(db, root, scene_id, x, y).await
}

pub async fn scene_canvas_set_position_core(
    db: Arc<Mutex<Db>>,
    project_root: PathBuf,
    scene_id: String,
    x: Option<f32>,
    y: Option<f32>,
) -> Result<(), String> {
    let scene_id: Id = scene_id
        .parse()
        .map_err(|e: water_core::Error| e.to_string())?;
    let g = db.lock().await;
    let store = SceneStore::new(&g, project_root);
    store
        .set_canvas_position(&scene_id, x, y)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn scene_canvas_set_group(
    state: State<'_, AppState>,
    scene_id: String,
    group: Option<String>,
) -> Result<(), String> {
    let (db, root) = {
        let proj = state.project.lock().await;
        let p = proj.as_ref().ok_or("no project open")?;
        (p.db.clone(), p.root.clone())
    };
    scene_canvas_set_group_core(db, root, scene_id, group).await
}

pub async fn scene_canvas_set_group_core(
    db: Arc<Mutex<Db>>,
    project_root: PathBuf,
    scene_id: String,
    group: Option<String>,
) -> Result<(), String> {
    let scene_id: Id = scene_id
        .parse()
        .map_err(|e: water_core::Error| e.to_string())?;
    let g = db.lock().await;
    let store = SceneStore::new(&g, project_root);
    store
        .set_canvas_group(&scene_id, group.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn scene_canvas_reset_all(state: State<'_, AppState>) -> Result<(), String> {
    let (db, project_id) = {
        let proj = state.project.lock().await;
        let p = proj.as_ref().ok_or("no project open")?;
        (p.db.clone(), p.project_id.clone())
    };
    scene_canvas_reset_all_core(db, project_id).await
}

pub async fn scene_canvas_reset_all_core(
    db: Arc<Mutex<Db>>,
    project_id: String,
) -> Result<(), String> {
    let project_id: Id = project_id
        .parse()
        .map_err(|e: water_core::Error| e.to_string())?;
    let g = db.lock().await;
    SceneStore::new(&g, std::path::PathBuf::new())
        .reset_all_canvas_positions(&project_id)
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use water_core::{ManuscriptStore, NewScene, ProjectStore};

    async fn seed() -> (TempDir, Arc<Mutex<Db>>, String, String, String) {
        let dir = TempDir::new().unwrap();
        let db_raw = Db::open(dir.path().join("project.db")).unwrap();
        let project = ProjectStore::new(&db_raw).insert("P").unwrap();
        let manuscript = ManuscriptStore::new(&db_raw)
            .insert(&project.id, "M", 0)
            .unwrap();
        let scene = SceneStore::new(&db_raw, dir.path().to_path_buf())
            .create(NewScene {
                manuscript_id: manuscript.id,
                chapter_id: None,
                name: "S".into(),
                ordering: 0,
            })
            .unwrap();
        (
            dir,
            Arc::new(Mutex::new(db_raw)),
            project.id.to_string(),
            scene.id.to_string(),
            "_unused".to_string(),
        )
    }

    #[tokio::test]
    async fn list_returns_single_scene_with_null_canvas_fields() {
        let (_dir, db, project_id, _scene_id, _) = seed().await;
        let rows = scene_canvas_list_core(db, project_id).await.unwrap();
        assert_eq!(rows.len(), 1);
        assert!(rows[0].canvas_x.is_none());
        assert!(rows[0].canvas_y.is_none());
    }

    #[tokio::test]
    async fn set_position_persists() {
        let (dir, db, project_id, scene_id, _) = seed().await;
        scene_canvas_set_position_core(
            db.clone(),
            dir.path().to_path_buf(),
            scene_id.clone(),
            Some(120.0),
            Some(80.0),
        )
        .await
        .unwrap();
        let rows = scene_canvas_list_core(db, project_id).await.unwrap();
        assert!((rows[0].canvas_x.unwrap() - 120.0).abs() < 1e-5);
        assert!((rows[0].canvas_y.unwrap() - 80.0).abs() < 1e-5);
    }

    #[tokio::test]
    async fn reset_all_clears_every_scene_position() {
        let (dir, db, project_id, scene_id, _) = seed().await;
        scene_canvas_set_position_core(
            db.clone(),
            dir.path().to_path_buf(),
            scene_id,
            Some(50.0),
            Some(50.0),
        )
        .await
        .unwrap();
        scene_canvas_reset_all_core(db.clone(), project_id.clone())
            .await
            .unwrap();
        let rows = scene_canvas_list_core(db, project_id).await.unwrap();
        assert!(rows[0].canvas_x.is_none());
        assert!(rows[0].canvas_y.is_none());
    }

    #[tokio::test]
    async fn set_group_round_trips() {
        let (dir, db, project_id, scene_id, _) = seed().await;
        scene_canvas_set_group_core(
            db.clone(),
            dir.path().to_path_buf(),
            scene_id,
            Some("Act II".to_string()),
        )
        .await
        .unwrap();
        let rows = scene_canvas_list_core(db, project_id).await.unwrap();
        assert_eq!(rows[0].canvas_group.as_deref(), Some("Act II"));
    }
}
