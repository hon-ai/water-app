use crate::state::AppState;
use serde::Serialize;
use tauri::State;
use water_core::{Id, NewScene, SceneFile, SceneStore};

#[derive(Serialize)]
pub struct SceneInfo {
    pub id: String,
    pub name: String,
    pub ordering: i64,
    pub word_count: i64,
}

#[tauri::command]
pub async fn scene_create(state: State<'_, AppState>, name: String) -> Result<SceneInfo, String> {
    let proj = state.project.lock().await;
    let project = proj.as_ref().ok_or("no project open")?;
    let manuscript_id: Id = project
        .default_manuscript_id
        .parse()
        .map_err(|e: water_core::Error| e.to_string())?;
    let store = SceneStore::new(&project.db, project.root.clone());
    // Ordering at end of manuscript.
    let count: i64 = project
        .db
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM scene WHERE manuscript_id = ?1",
            [manuscript_id.as_str()],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    let row = store
        .create(NewScene {
            manuscript_id,
            chapter_id: None,
            name,
            ordering: count,
        })
        .map_err(|e| e.to_string())?;
    Ok(SceneInfo {
        id: row.id.to_string(),
        name: row.name,
        ordering: row.ordering,
        word_count: row.word_count,
    })
}

#[tauri::command]
pub async fn scene_read(state: State<'_, AppState>, id: String) -> Result<String, String> {
    let proj = state.project.lock().await;
    let project = proj.as_ref().ok_or("no project open")?;
    let id: Id = id.parse().map_err(|e: water_core::Error| e.to_string())?;
    let store = SceneStore::new(&project.db, project.root.clone());
    let file: SceneFile = store.read(&id).map_err(|e| e.to_string())?;
    Ok(file.body)
}

#[tauri::command]
pub async fn scene_write_body(
    state: State<'_, AppState>,
    id: String,
    body: String,
) -> Result<SceneInfo, String> {
    let proj = state.project.lock().await;
    let project = proj.as_ref().ok_or("no project open")?;
    let id: Id = id.parse().map_err(|e: water_core::Error| e.to_string())?;
    let store = SceneStore::new(&project.db, project.root.clone());
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
    let proj = state.project.lock().await;
    let project = proj.as_ref().ok_or("no project open")?;
    let manuscript_id: Id = project
        .default_manuscript_id
        .parse()
        .map_err(|e: water_core::Error| e.to_string())?;
    let store = SceneStore::new(&project.db, project.root.clone());
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
