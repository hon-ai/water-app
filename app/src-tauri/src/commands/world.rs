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
use water_core::world::{
    templates::WorldTemplateSchema, WorldEntryFile, WorldSegmentRow, WorldSingleDocFile, WorldStore,
};
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

/// Unified renderer-facing projection of one world doc (single-doc segment
/// OR a collection entry). Mirrors `water_core::world::WorldEntryFile`
/// shape on the wire but with `id`/`segment_id` stringified.
///
/// For single-doc segments `aliases` is always `vec![]` (the data model
/// has no alias concept for them); for collection entries it carries the
/// per-entry alias list. Section keys (`"main"`, `"lists"`, …) land at
/// top level via `#[serde(flatten)]`, matching the on-disk TOML shape.
#[derive(Debug, Serialize, Deserialize)]
pub struct WorldEntryFilePayload {
    pub id: String,
    pub segment_id: String,
    pub schema_version: String,
    pub name: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(flatten)]
    pub data: serde_json::Map<String, serde_json::Value>,
}

impl From<WorldSingleDocFile> for WorldEntryFilePayload {
    fn from(f: WorldSingleDocFile) -> Self {
        Self {
            id: f.id.to_string(),
            // Single-doc files don't track their own segment_id internally
            // (the segment_id is the lookup key); the caller fills it in.
            segment_id: String::new(),
            schema_version: f.schema_version,
            name: f.name,
            aliases: Vec::new(),
            data: f.data,
        }
    }
}

impl From<WorldEntryFile> for WorldEntryFilePayload {
    fn from(f: WorldEntryFile) -> Self {
        Self {
            id: f.id.to_string(),
            segment_id: f.segment_id.to_string(),
            schema_version: f.schema_version,
            name: f.name,
            aliases: f.aliases,
            data: f.data,
        }
    }
}

/// Body of `world_single_doc_update_field`.
#[derive(Debug, Deserialize)]
pub struct UpdateSingleDocRequest {
    pub segment_id: String,
    pub field_id: String,
    pub value: serde_json::Value,
}

/// Renderer-facing projection of a `WorldEntryIndexRow`. Stringifies the
/// `id`/`segment_id` for JSON.
#[derive(Debug, Serialize, Deserialize)]
pub struct WorldEntryIndexPayload {
    pub id: String,
    pub segment_id: String,
    pub name: String,
    pub preview: String,
}

/// Body of `world_entry_create`.
#[derive(Debug, Deserialize)]
pub struct CreateEntryRequest {
    pub segment_id: String,
    pub name: String,
}

/// Body of `world_entry_update_field`.
#[derive(Debug, Deserialize)]
pub struct UpdateEntryFieldRequest {
    pub entry_id: String,
    pub field_id: String,
    pub value: serde_json::Value,
}

/// Body of `world_entry_update_aliases`.
#[derive(Debug, Deserialize)]
pub struct UpdateAliasesRequest {
    pub entry_id: String,
    pub aliases: Vec<String>,
}

/// Body of `world_autosuggest`. `scene_id` is currently unused but is
/// accepted as a parameter so the renderer can wire the scene context
/// in now; presence-aware filtering arrives in a later task.
#[derive(Debug, Deserialize)]
pub struct AutosuggestRequest {
    pub scene_id: String,
    pub paragraph: String,
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

// ----- Single-doc commands (Task 9) -----

async fn world_single_doc_read_core(
    db: Arc<Mutex<Db>>,
    project_root: PathBuf,
    segment_id: String,
) -> Result<WorldEntryFilePayload, String> {
    let seg_id: Id = segment_id
        .parse()
        .map_err(|e: water_core::Error| e.to_string())?;
    let db_guard = db.lock().await;
    let store = WorldStore::new(&db_guard, project_root);
    let file = store.read_single_doc(&seg_id).map_err(|e| e.to_string())?;
    // `WorldSingleDocFile::From` leaves `segment_id` empty since the type
    // doesn't carry it; backfill from the caller-supplied id so the
    // renderer always sees a populated field.
    let mut payload: WorldEntryFilePayload = file.into();
    payload.segment_id = seg_id.to_string();
    Ok(payload)
}

async fn world_single_doc_update_field_core(
    db: Arc<Mutex<Db>>,
    project_root: PathBuf,
    req: UpdateSingleDocRequest,
) -> Result<(), String> {
    let seg_id: Id = req
        .segment_id
        .parse()
        .map_err(|e: water_core::Error| e.to_string())?;
    let db_guard = db.lock().await;
    let store = WorldStore::new(&db_guard, project_root);
    store
        .update_single_doc_field(&seg_id, &req.field_id, &req.value)
        .map_err(|e| e.to_string())
}

// ----- Entry CRUD + autosuggest (Task 10) -----

async fn world_entry_list_core(
    db: Arc<Mutex<Db>>,
    project_root: PathBuf,
    segment_id: String,
) -> Result<Vec<WorldEntryIndexPayload>, String> {
    let seg_id: Id = segment_id
        .parse()
        .map_err(|e: water_core::Error| e.to_string())?;
    let db_guard = db.lock().await;
    let store = WorldStore::new(&db_guard, project_root);
    let rows = store.list_entries(&seg_id).map_err(|e| e.to_string())?;
    Ok(rows
        .into_iter()
        .map(|r| WorldEntryIndexPayload {
            id: r.id.to_string(),
            segment_id: r.segment_id.to_string(),
            name: r.name,
            preview: r.preview,
        })
        .collect())
}

async fn world_entry_read_core(
    db: Arc<Mutex<Db>>,
    project_root: PathBuf,
    entry_id: String,
) -> Result<WorldEntryFilePayload, String> {
    let entry_id: Id = entry_id
        .parse()
        .map_err(|e: water_core::Error| e.to_string())?;
    let db_guard = db.lock().await;
    let store = WorldStore::new(&db_guard, project_root);
    let file = store.read_entry(&entry_id).map_err(|e| e.to_string())?;
    Ok(file.into())
}

async fn world_entry_create_core(
    db: Arc<Mutex<Db>>,
    project_root: PathBuf,
    req: CreateEntryRequest,
) -> Result<String, String> {
    let seg_id: Id = req
        .segment_id
        .parse()
        .map_err(|e: water_core::Error| e.to_string())?;
    let db_guard = db.lock().await;
    let store = WorldStore::new(&db_guard, project_root);
    store
        .create_entry(&seg_id, &req.name)
        .map(|id| id.to_string())
        .map_err(|e| e.to_string())
}

async fn world_entry_update_field_core(
    db: Arc<Mutex<Db>>,
    project_root: PathBuf,
    req: UpdateEntryFieldRequest,
) -> Result<(), String> {
    let entry_id: Id = req
        .entry_id
        .parse()
        .map_err(|e: water_core::Error| e.to_string())?;
    let db_guard = db.lock().await;
    let store = WorldStore::new(&db_guard, project_root);
    store
        .update_entry_field(&entry_id, &req.field_id, &req.value)
        .map_err(|e| e.to_string())
}

async fn world_entry_update_aliases_core(
    db: Arc<Mutex<Db>>,
    project_root: PathBuf,
    req: UpdateAliasesRequest,
) -> Result<(), String> {
    let entry_id: Id = req
        .entry_id
        .parse()
        .map_err(|e: water_core::Error| e.to_string())?;
    let db_guard = db.lock().await;
    let store = WorldStore::new(&db_guard, project_root);
    store
        .update_entry_aliases(&entry_id, &req.aliases)
        .map_err(|e| e.to_string())
}

async fn world_entry_delete_if_empty_core(
    db: Arc<Mutex<Db>>,
    project_root: PathBuf,
    entry_id: String,
) -> Result<bool, String> {
    let entry_id: Id = entry_id
        .parse()
        .map_err(|e: water_core::Error| e.to_string())?;
    let db_guard = db.lock().await;
    let store = WorldStore::new(&db_guard, project_root);
    store
        .delete_entry_if_empty(&entry_id)
        .map_err(|e| e.to_string())
}

async fn world_entry_delete_core(
    db: Arc<Mutex<Db>>,
    project_root: PathBuf,
    entry_id: String,
) -> Result<(), String> {
    let entry_id: Id = entry_id
        .parse()
        .map_err(|e: water_core::Error| e.to_string())?;
    let db_guard = db.lock().await;
    let store = WorldStore::new(&db_guard, project_root);
    store.delete_entry(&entry_id).map_err(|e| e.to_string())
}

/// Autosuggest for a paragraph: tokenizes on word boundaries (everything
/// not alphanumeric and not an apostrophe is a separator), lowercases,
/// dedups via `HashSet`, then for every token looks up `find_by_token`
/// against a freshly-built `WorldRegistry`. Matches are deduped by entry
/// id (a multi-token match like "the pell" → entry "Pell" only emits the
/// entry once) and filtered to `locations`-slug entries — M4 scope is
/// locations only; other segment slugs are intentionally excluded so the
/// autosuggest pill UI doesn't surface character-bible-style data in the
/// location chip lane.
///
/// `scene_id` is accepted for future presence-aware filtering (matches
/// the character autosuggest surface convention) and currently unused.
pub async fn world_autosuggest_core(
    db: Arc<Mutex<Db>>,
    project_root: PathBuf,
    project_id: String,
    req: AutosuggestRequest,
) -> Result<Vec<WorldEntryIndexPayload>, String> {
    use water_core::world::WorldRegistry;
    let project_id: Id = project_id
        .parse()
        .map_err(|e: water_core::Error| e.to_string())?;
    let reg = {
        let db_guard = db.lock().await;
        WorldRegistry::from_db(&db_guard, &project_id, project_root)
    }
    .map_err(|e| e.to_string())?;

    let tokens: std::collections::HashSet<String> = req
        .paragraph
        .split(|c: char| !c.is_alphanumeric() && c != '\'')
        .filter(|s| !s.is_empty())
        .map(str::to_lowercase)
        .collect();

    let mut hits: std::collections::HashMap<Id, WorldEntryIndexPayload> =
        std::collections::HashMap::new();
    for token in &tokens {
        for id in reg.find_by_token(token) {
            if let Some(snap) = reg.by_id(id) {
                if snap.segment_slug != "locations" {
                    continue;
                }
                hits.entry(id.clone())
                    .or_insert_with(|| WorldEntryIndexPayload {
                        id: snap.id.to_string(),
                        segment_id: snap.segment_id.to_string(),
                        name: snap.name.clone(),
                        preview: String::new(),
                    });
            }
        }
    }
    // Reserved for future filtering — see doc comment.
    let _ = req.scene_id;
    Ok(hits.into_values().collect())
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

// ----- Single-doc command shims (Task 9) -----

#[tauri::command]
pub async fn world_single_doc_read(
    state: State<'_, AppState>,
    segment_id: String,
) -> Result<WorldEntryFilePayload, String> {
    let (db, root) = {
        let proj = state.project.lock().await;
        let project = proj.as_ref().ok_or("no project open")?;
        (project.db.clone(), project.root.clone())
    };
    world_single_doc_read_core(db, root, segment_id).await
}

#[tauri::command]
pub async fn world_single_doc_update_field(
    state: State<'_, AppState>,
    req: UpdateSingleDocRequest,
) -> Result<(), String> {
    let (db, root) = {
        let proj = state.project.lock().await;
        let project = proj.as_ref().ok_or("no project open")?;
        (project.db.clone(), project.root.clone())
    };
    world_single_doc_update_field_core(db, root, req).await
}

// ----- Entry CRUD + autosuggest shims (Task 10) -----

#[tauri::command]
pub async fn world_entry_list(
    state: State<'_, AppState>,
    segment_id: String,
) -> Result<Vec<WorldEntryIndexPayload>, String> {
    let (db, root) = {
        let proj = state.project.lock().await;
        let project = proj.as_ref().ok_or("no project open")?;
        (project.db.clone(), project.root.clone())
    };
    world_entry_list_core(db, root, segment_id).await
}

#[tauri::command]
pub async fn world_entry_read(
    state: State<'_, AppState>,
    entry_id: String,
) -> Result<WorldEntryFilePayload, String> {
    let (db, root) = {
        let proj = state.project.lock().await;
        let project = proj.as_ref().ok_or("no project open")?;
        (project.db.clone(), project.root.clone())
    };
    world_entry_read_core(db, root, entry_id).await
}

#[tauri::command]
pub async fn world_entry_create(
    state: State<'_, AppState>,
    req: CreateEntryRequest,
) -> Result<String, String> {
    let (db, root) = {
        let proj = state.project.lock().await;
        let project = proj.as_ref().ok_or("no project open")?;
        (project.db.clone(), project.root.clone())
    };
    world_entry_create_core(db, root, req).await
}

#[tauri::command]
pub async fn world_entry_update_field(
    state: State<'_, AppState>,
    req: UpdateEntryFieldRequest,
) -> Result<(), String> {
    let (db, root) = {
        let proj = state.project.lock().await;
        let project = proj.as_ref().ok_or("no project open")?;
        (project.db.clone(), project.root.clone())
    };
    world_entry_update_field_core(db, root, req).await
}

#[tauri::command]
pub async fn world_entry_update_aliases(
    state: State<'_, AppState>,
    req: UpdateAliasesRequest,
) -> Result<(), String> {
    let (db, root) = {
        let proj = state.project.lock().await;
        let project = proj.as_ref().ok_or("no project open")?;
        (project.db.clone(), project.root.clone())
    };
    world_entry_update_aliases_core(db, root, req).await
}

#[tauri::command]
pub async fn world_entry_delete_if_empty(
    state: State<'_, AppState>,
    entry_id: String,
) -> Result<bool, String> {
    let (db, root) = {
        let proj = state.project.lock().await;
        let project = proj.as_ref().ok_or("no project open")?;
        (project.db.clone(), project.root.clone())
    };
    world_entry_delete_if_empty_core(db, root, entry_id).await
}

#[tauri::command]
pub async fn world_entry_delete(
    state: State<'_, AppState>,
    entry_id: String,
) -> Result<(), String> {
    let (db, root) = {
        let proj = state.project.lock().await;
        let project = proj.as_ref().ok_or("no project open")?;
        (project.db.clone(), project.root.clone())
    };
    world_entry_delete_core(db, root, entry_id).await
}

#[tauri::command]
pub async fn world_autosuggest(
    state: State<'_, AppState>,
    req: AutosuggestRequest,
) -> Result<Vec<WorldEntryIndexPayload>, String> {
    let (db, root, project_id) = {
        let proj = state.project.lock().await;
        let project = proj.as_ref().ok_or("no project open")?;
        (
            project.db.clone(),
            project.root.clone(),
            project.project_id.clone(),
        )
    };
    world_autosuggest_core(db, root, project_id, req).await
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

    // ----- Task 9 + 10 helpers -----

    /// Like [`test_project`] but also resolves and returns the `locations`
    /// segment id. Returned tuple:
    /// `(temp_dir, db, root, project_id, locations_segment_id)`. The two
    /// `Id` values are stringified so they pass straight into the _core
    /// helpers without further conversion.
    async fn mk_with_loc(
    ) -> (TempDir, Arc<Mutex<Db>>, PathBuf, String, String) {
        let (dir, db, root, project_id) = test_project().await;
        let loc_id = {
            let g = db.lock().await;
            let pid: Id = project_id.parse().unwrap();
            WorldStore::new(&g, root.clone())
                .find_segment_by_slug(&pid, "locations")
                .unwrap()
                .expect("locations segment must be seeded")
                .id
                .to_string()
        };
        (dir, db, root, project_id, loc_id)
    }

    // ----- Task 9: single-doc round-trip -----

    #[tokio::test]
    async fn world_single_doc_round_trip() {
        let (_dir, db, root, project_id) = test_project().await;
        let segs = world_segment_list_core(db.clone(), root.clone(), project_id)
            .await
            .unwrap();
        let concept = segs.iter().find(|s| s.slug == "concept").unwrap();

        world_single_doc_update_field_core(
            db.clone(),
            root.clone(),
            UpdateSingleDocRequest {
                segment_id: concept.id.clone(),
                field_id: "main.core_premise".to_string(),
                value: serde_json::json!("A test premise"),
            },
        )
        .await
        .unwrap();

        let file = world_single_doc_read_core(db, root, concept.id.clone())
            .await
            .unwrap();
        assert_eq!(file.segment_id, concept.id);
        let main = file.data.get("main").unwrap().as_object().unwrap();
        assert_eq!(
            main.get("core_premise").unwrap().as_str().unwrap(),
            "A test premise"
        );
    }

    // ----- Task 10: entry CRUD + autosuggest -----

    #[tokio::test]
    async fn entry_create_then_list() {
        let (_dir, db, root, _p, seg) = mk_with_loc().await;
        let id = world_entry_create_core(
            db.clone(),
            root.clone(),
            CreateEntryRequest {
                segment_id: seg.clone(),
                name: "Pell".to_string(),
            },
        )
        .await
        .unwrap();
        let list = world_entry_list_core(db, root, seg).await.unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, id);
    }

    #[tokio::test]
    async fn entry_delete_if_empty_returns_true_for_blank() {
        let (_dir, db, root, _p, seg) = mk_with_loc().await;
        let id = world_entry_create_core(
            db.clone(),
            root.clone(),
            CreateEntryRequest {
                segment_id: seg,
                name: String::new(),
            },
        )
        .await
        .unwrap();
        let reaped = world_entry_delete_if_empty_core(db, root, id).await.unwrap();
        assert!(reaped);
    }

    #[tokio::test]
    async fn autosuggest_matches_alias_case_insensitive() {
        let (_dir, db, root, p, seg) = mk_with_loc().await;
        let id = world_entry_create_core(
            db.clone(),
            root.clone(),
            CreateEntryRequest {
                segment_id: seg,
                name: "The Pell Library".to_string(),
            },
        )
        .await
        .unwrap();
        world_entry_update_aliases_core(
            db.clone(),
            root.clone(),
            UpdateAliasesRequest {
                entry_id: id.clone(),
                aliases: vec!["Pell".to_string()],
            },
        )
        .await
        .unwrap();

        let hits = world_autosuggest_core(
            db,
            root,
            p,
            AutosuggestRequest {
                scene_id: "noop".to_string(),
                paragraph: "She walked past pell at dusk.".to_string(),
            },
        )
        .await
        .unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, id);
    }
}
