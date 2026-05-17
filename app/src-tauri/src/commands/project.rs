use crate::state::{AppState, OpenProject};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::State;
use tokio::sync::Mutex;
use water_core::{
    chapters::ChaptersFile, rebuild_from_truth, repair, water_toml::WaterToml, ActiveScene, Db,
    ManuscriptStore, ProjectStore, SnapshotScheduler,
};

#[derive(Serialize)]
pub struct OpenProjectInfo {
    pub root: String,
    pub name: String,
    pub project_id: String,
    pub default_manuscript_id: String,
}

#[tauri::command]
pub async fn create_project(
    state: State<'_, AppState>,
    parent_dir: String,
    name: String,
) -> Result<OpenProjectInfo, String> {
    let parent = PathBuf::from(&parent_dir);
    let safe = sanitize_dir_name(&name);
    let root = parent.join(format!("{safe}.water"));
    if root.exists() {
        return Err(format!("directory already exists: {}", root.display()));
    }
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(root.join("manuscript").join("scenes"))
        .map_err(|e| e.to_string())?;
    std::fs::create_dir_all(root.join("characters")).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(root.join("world")).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(root.join("snapshots")).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(root.join(".water").join("cache")).map_err(|e| e.to_string())?;

    let db_path = root.join("project.db");
    let db_raw = Db::open(&db_path).map_err(|e| e.to_string())?;
    let db = Arc::new(Mutex::new(db_raw));

    let (project_id, project_name, manuscript_id) = {
        let db_guard = db.lock().await;
        let project = ProjectStore::new(&db_guard)
            .insert(&name)
            .map_err(|e| e.to_string())?;
        let manuscript = ManuscriptStore::new(&db_guard)
            .insert(&project.id, "Manuscript", 0)
            .map_err(|e| e.to_string())?;
        ProjectStore::new(&db_guard)
            .set_default_manuscript(&project.id, &manuscript.id)
            .map_err(|e| e.to_string())?;
        (project.id.clone(), project.name.clone(), manuscript.id.clone())
    };

    WaterToml {
        schema_version: 1,
        project_id: project_id.clone(),
        name: project_name.clone(),
        default_manuscript_id: Some(manuscript_id.clone()),
        created_at: chrono::Utc::now().to_rfc3339(),
        updated_at: chrono::Utc::now().to_rfc3339(),
    }
    .write(&root)
    .map_err(|e| e.to_string())?;

    ChaptersFile::empty()
        .write(root.join("manuscript").join("chapters.toml"))
        .map_err(|e| e.to_string())?;

    let scheduler = SnapshotScheduler::spawn(db.clone(), root.clone());

    let info = OpenProjectInfo {
        root: root.to_string_lossy().to_string(),
        name: project_name,
        project_id: project_id.to_string(),
        default_manuscript_id: manuscript_id.to_string(),
    };
    let mut g = state.project.lock().await;
    *g = Some(OpenProject {
        root,
        db,
        default_manuscript_id: manuscript_id.to_string(),
        scheduler,
    });
    Ok(info)
}

#[tauri::command]
pub async fn open_project(
    state: State<'_, AppState>,
    root: String,
) -> Result<OpenProjectInfo, String> {
    let root = PathBuf::from(root);
    let water = WaterToml::read(&root).map_err(|e| e.to_string())?;
    let db_path = root.join("project.db");

    let (db_raw, default_manuscript_id) = if db_path.exists() {
        let db_raw = Db::open(&db_path).map_err(|e| e.to_string())?;
        let manuscript_id = water
            .default_manuscript_id
            .as_ref()
            .map(|id| id.to_string())
            .unwrap_or_default();
        (db_raw, manuscript_id)
    } else {
        let (db_raw, _stats) = rebuild_from_truth(&root).map_err(|e| e.to_string())?;
        let manuscript_id = water
            .default_manuscript_id
            .as_ref()
            .map(|id| id.to_string())
            .unwrap_or_default();
        (db_raw, manuscript_id)
    };

    let db = Arc::new(Mutex::new(db_raw));
    {
        let db_guard = db.lock().await;
        repair::run(&db_guard, &root).map_err(|e| e.to_string())?;
    }

    let scheduler = SnapshotScheduler::spawn(db.clone(), root.clone());

    // Register every existing scene so hourly + on-close snapshots fire for them.
    // The DB query is scoped in its own block so the `Statement` (which is
    // `!Send`) is fully dropped before the `.await` calls in the registration
    // loop below — otherwise the compiler conservatively considers it held
    // across awaits and the future stops being `Send`.
    let rows: Vec<(String, String)> = {
        let db_guard = db.lock().await;
        let mut stmt = db_guard
            .conn()
            .prepare("SELECT id, file_path FROM scene")
            .map_err(|e| e.to_string())?;
        let collected: Vec<(String, String)> = stmt
            .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
            .map_err(|e| e.to_string())?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        collected
    };
    for (id_s, path_s) in rows {
        let scene_id: water_core::Id = id_s
            .parse()
            .map_err(|e: water_core::Error| e.to_string())?;
        scheduler
            .register(ActiveScene {
                scene_id,
                file_path: std::path::PathBuf::from(path_s),
            })
            .await;
    }

    let info = OpenProjectInfo {
        root: root.to_string_lossy().to_string(),
        name: water.name.clone(),
        project_id: water.project_id.to_string(),
        default_manuscript_id: default_manuscript_id.clone(),
    };
    let mut g = state.project.lock().await;
    *g = Some(OpenProject {
        root,
        db,
        default_manuscript_id,
        scheduler,
    });
    Ok(info)
}

#[tauri::command]
pub async fn close_project(state: State<'_, AppState>) -> Result<(), String> {
    let mut g = state.project.lock().await;
    if let Some(project) = g.take() {
        // Fire OnClose snapshots for all registered scenes, then stop the loop.
        let _ = project.scheduler.on_close().await;
        let _ = project.scheduler.stop().await;
        // `project` (and its db Arc) drops here; the scheduler's spawned task
        // exits once the channel closes from stop().
    }
    Ok(())
}

fn sanitize_dir_name(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == ' ' || *c == '-' || *c == '_')
        .collect::<String>()
        .trim()
        .replace(' ', "-")
}
