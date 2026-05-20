//! M4 World/Setting Bible Tauri commands (Task 8).
//!
//! Each `#[tauri::command]` is a thin shim over a `_core` helper that takes
//! the raw `(db, root, project_id, ...)` inputs. This split exists so the
//! integration tests in this file can drive the same code path without
//! constructing a `tauri::State<'_, AppState>` (which has no public test
//! constructor in current Tauri).
//!
//! Lock-ordering note: world segment mutations touch DB rows only (no
//! per-segment write locks yet — segment-template edits are not on the
//! hot keystroke loop the way `character_update_field` is). The
//! single-entry/single-doc field-edit commands will be added in a later
//! task and at that point will need their own per-entry locks.

use crate::state::AppState;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::State;
use tokio::sync::Mutex;
use water_core::world::{templates::WorldTemplateSchema, WorldSegmentRow, WorldStore};
use water_core::{Db, Id};

/// Renderer-facing projection of a world segment row. Mirrors
/// `water_core::world::WorldSegmentRow` but stringifies `id` for JSON
/// friendliness (the renderer treats it as opaque), matching the
/// character command surface convention.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct WorldSegmentPayload {
    pub id: String,
    pub slug: String,
    pub name: String,
    pub ordering: i64,
    pub is_collection: bool,
    pub hue_token: String,
    pub hidden: bool,
    pub has_template_override: bool,
}

impl From<WorldSegmentRow> for WorldSegmentPayload {
    fn from(r: WorldSegmentRow) -> Self {
        Self {
            id: r.id.to_string(),
            slug: r.slug,
            name: r.name,
            ordering: r.ordering,
            is_collection: r.is_collection,
            hue_token: r.hue_token,
            hidden: r.hidden,
            has_template_override: r.has_template_override,
        }
    }
}

/// Body of `world_segment_create`. `template` carries the full
/// `WorldTemplateSchema` so the renderer can ship a customized schema
/// at create-time (the default user-add flow seeds an empty-fields
/// template, but the surface admits arbitrary schemas).
#[derive(Debug, Deserialize)]
pub struct CreateSegmentRequest {
    pub name: String,
    pub is_collection: bool,
    pub template: WorldTemplateSchema,
}

/// Body of `world_segment_update_template`.
#[derive(Debug, Deserialize)]
pub struct UpdateTemplateRequest {
    pub segment_id: String,
    pub template: WorldTemplateSchema,
}

/// Body of `world_segment_set_hidden`.
#[derive(Debug, Deserialize)]
pub struct SetHiddenRequest {
    pub segment_id: String,
    pub hidden: bool,
}

// ----------------------------------------------------------------------
// Core helpers (testable without Tauri State)
// ----------------------------------------------------------------------

async fn world_segment_list_core(
    db: Arc<Mutex<Db>>,
    root: PathBuf,
    project_id: String,
) -> Result<Vec<WorldSegmentPayload>, String> {
    let project_id: Id = project_id
        .parse()
        .map_err(|e: water_core::Error| e.to_string())?;
    let db_guard = db.lock().await;
    let store = WorldStore::new(&db_guard, root);
    store
        .list_segments(&project_id)
        .map(|rows| rows.into_iter().map(Into::into).collect())
        .map_err(|e| e.to_string())
}

async fn world_segment_create_core(
    db: Arc<Mutex<Db>>,
    root: PathBuf,
    project_id: String,
    req: CreateSegmentRequest,
) -> Result<String, String> {
    let project_id: Id = project_id
        .parse()
        .map_err(|e: water_core::Error| e.to_string())?;
    let db_guard = db.lock().await;
    let store = WorldStore::new(&db_guard, root);
    store
        .create_user_segment(&project_id, &req.name, req.is_collection, &req.template)
        .map(|id| id.to_string())
        .map_err(|e| e.to_string())
}

async fn world_segment_update_template_core(
    db: Arc<Mutex<Db>>,
    root: PathBuf,
    req: UpdateTemplateRequest,
) -> Result<(), String> {
    let seg_id: Id = req
        .segment_id
        .parse()
        .map_err(|e: water_core::Error| e.to_string())?;
    let db_guard = db.lock().await;
    let store = WorldStore::new(&db_guard, root);
    store
        .update_segment_template(&seg_id, &req.template)
        .map_err(|e| e.to_string())
}

async fn world_segment_set_hidden_core(
    db: Arc<Mutex<Db>>,
    root: PathBuf,
    req: SetHiddenRequest,
) -> Result<(), String> {
    let seg_id: Id = req
        .segment_id
        .parse()
        .map_err(|e: water_core::Error| e.to_string())?;
    let db_guard = db.lock().await;
    let store = WorldStore::new(&db_guard, root);
    store
        .set_segment_hidden(&seg_id, req.hidden)
        .map_err(|e| e.to_string())
}

async fn world_segment_delete_core(
    db: Arc<Mutex<Db>>,
    root: PathBuf,
    segment_id: String,
) -> Result<(), String> {
    let seg_id: Id = segment_id
        .parse()
        .map_err(|e: water_core::Error| e.to_string())?;
    let db_guard = db.lock().await;
    let store = WorldStore::new(&db_guard, root);
    store
        .delete_user_segment(&seg_id)
        .map_err(|e| e.to_string())
}

async fn world_intake_schema_core(
    db: Arc<Mutex<Db>>,
    segment_id: String,
) -> Result<WorldTemplateSchema, String> {
    let seg_id: Id = segment_id
        .parse()
        .map_err(|e: water_core::Error| e.to_string())?;
    let db_guard = db.lock().await;
    water_core::world::templates::effective_template(&db_guard, &seg_id)
        .map_err(|e| e.to_string())
}

// ----------------------------------------------------------------------
// Tauri command shims
// ----------------------------------------------------------------------

#[tauri::command]
pub async fn world_segment_list(
    state: State<'_, AppState>,
) -> Result<Vec<WorldSegmentPayload>, String> {
    let (db, root, project_id) = {
        let proj = state.project.lock().await;
        let project = proj.as_ref().ok_or("no project open")?;
        (
            project.db.clone(),
            project.root.clone(),
            project.project_id.clone(),
        )
    };
    world_segment_list_core(db, root, project_id).await
}

#[tauri::command]
pub async fn world_segment_create(
    state: State<'_, AppState>,
    req: CreateSegmentRequest,
) -> Result<String, String> {
    let (db, root, project_id) = {
        let proj = state.project.lock().await;
        let project = proj.as_ref().ok_or("no project open")?;
        (
            project.db.clone(),
            project.root.clone(),
            project.project_id.clone(),
        )
    };
    world_segment_create_core(db, root, project_id, req).await
}

#[tauri::command]
pub async fn world_segment_update_template(
    state: State<'_, AppState>,
    req: UpdateTemplateRequest,
) -> Result<(), String> {
    let (db, root) = {
        let proj = state.project.lock().await;
        let project = proj.as_ref().ok_or("no project open")?;
        (project.db.clone(), project.root.clone())
    };
    world_segment_update_template_core(db, root, req).await
}

#[tauri::command]
pub async fn world_segment_set_hidden(
    state: State<'_, AppState>,
    req: SetHiddenRequest,
) -> Result<(), String> {
    let (db, root) = {
        let proj = state.project.lock().await;
        let project = proj.as_ref().ok_or("no project open")?;
        (project.db.clone(), project.root.clone())
    };
    world_segment_set_hidden_core(db, root, req).await
}

#[tauri::command]
pub async fn world_segment_delete(
    state: State<'_, AppState>,
    segment_id: String,
) -> Result<(), String> {
    let (db, root) = {
        let proj = state.project.lock().await;
        let project = proj.as_ref().ok_or("no project open")?;
        (project.db.clone(), project.root.clone())
    };
    world_segment_delete_core(db, root, segment_id).await
}

#[tauri::command]
pub async fn world_intake_schema(
    state: State<'_, AppState>,
    segment_id: String,
) -> Result<WorldTemplateSchema, String> {
    let db = {
        let proj = state.project.lock().await;
        let project = proj.as_ref().ok_or("no project open")?;
        project.db.clone()
    };
    world_intake_schema_core(db, segment_id).await
}

// ----------------------------------------------------------------------
// Tests
// ----------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use water_core::world::templates::{WorldTemplateField, WorldTemplateFieldKind};
    use water_core::ProjectStore;

    /// Build a fresh project on disk + DB and seed the six built-in world
    /// segments. Returns `(temp_dir, db, root, project_id)` — the inputs
    /// every `_core` helper takes. Mirrors `character::tests::test_project`.
    async fn test_project() -> (TempDir, Arc<Mutex<Db>>, PathBuf, String) {
        let dir = TempDir::new().unwrap();
        let root = dir.path().to_path_buf();
        std::fs::create_dir_all(root.join("world")).unwrap();
        let db_raw = Db::open(root.join("project.db")).unwrap();
        let db = Arc::new(Mutex::new(db_raw));
        let project_id = {
            let g = db.lock().await;
            let pid = ProjectStore::new(&g).insert("TestProject").unwrap().id;
            WorldStore::new(&g, root.clone())
                .seed_builtins(&pid)
                .unwrap();
            pid
        };
        (dir, db, root, project_id.to_string())
    }

    #[tokio::test]
    async fn world_segment_list_returns_six_builtins() {
        let (_dir, db, root, project_id) = test_project().await;
        let segs = world_segment_list_core(db, root, project_id).await.unwrap();
        assert_eq!(segs.len(), 6, "expected 6 built-in segments, got {}", segs.len());
        // Sanity-check that the canonical slugs are present.
        let slugs: Vec<&str> = segs.iter().map(|s| s.slug.as_str()).collect();
        assert!(slugs.contains(&"concept"));
        assert!(slugs.contains(&"locations"));
        assert!(slugs.contains(&"history"));
    }

    #[tokio::test]
    async fn world_segment_create_then_list_includes_new() {
        let (_dir, db, root, project_id) = test_project().await;
        let req = CreateSegmentRequest {
            name: "Magic".to_string(),
            is_collection: false,
            template: WorldTemplateSchema {
                id: "magic".to_string(),
                label: "Magic".to_string(),
                fields: vec![WorldTemplateField {
                    id: "main.rules".to_string(),
                    label: "Rules".to_string(),
                    prompt_question: "What are the rules?".to_string(),
                    kind: WorldTemplateFieldKind::LongText,
                    optional_skip: false,
                }],
            },
        };
        let new_id = world_segment_create_core(db.clone(), root.clone(), project_id.clone(), req)
            .await
            .unwrap();
        assert!(!new_id.is_empty(), "new segment id should be non-empty");

        let segs = world_segment_list_core(db, root, project_id).await.unwrap();
        assert_eq!(segs.len(), 7);
        let magic = segs.iter().find(|s| s.name == "Magic").expect("Magic segment should exist");
        assert!(magic.has_template_override, "user-created segment must have template_json override");
    }

    #[tokio::test]
    async fn world_intake_schema_returns_builtin_for_concept() {
        let (_dir, db, root, project_id) = test_project().await;
        let segs = world_segment_list_core(db.clone(), root, project_id)
            .await
            .unwrap();
        let concept = segs
            .iter()
            .find(|s| s.slug == "concept")
            .expect("concept segment must be seeded");
        let schema = world_intake_schema_core(db, concept.id.clone()).await.unwrap();
        assert_eq!(schema.id, "concept");
        assert!(
            schema.fields.iter().any(|f| f.id == "main.core_premise"),
            "concept built-in schema must include main.core_premise"
        );
    }
}
