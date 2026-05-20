//! Heatmap Tauri commands (M5 Task 11).
//!
//! Two surface commands today:
//! - [`heat_read`] returns the cached heat rows for one scene, grouped
//!   by metric kind. Empty for scenes that haven't been recomputed yet.
//! - [`heat_set_metric_enabled`] persists the writer's metric-picker
//!   state to a per-project settings file so toggles survive restarts.
//!
//! Both follow the `_core` async-fn extraction pattern: the Tauri
//! command unpacks `State<AppState>` and calls the `_core` fn so tests
//! can drive the function directly.

use crate::state::AppState;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tauri::State;
use tokio::sync::Mutex;
use water_core::{heat::HeatStore, HeatMetricKind, HeatRow, Id};

#[derive(Debug, Serialize, Deserialize)]
pub struct HeatReadResponse {
    /// One vec per metric kind, in the canonical iteration order from
    /// `HeatMetricKind::all()`. Empty vecs mean the metric hasn't been
    /// computed yet (or is currently disabled).
    pub metrics: HashMap<String, Vec<HeatRow>>,
}

/// Read every cached metric for `scene_id`. Returns a map keyed by
/// the metric kind's stored string form (`"pacing"`, `"valence"`, …).
/// Useful for the renderer's initial paint — subsequent updates come
/// via the `heat:updated` event.
#[tauri::command]
pub async fn heat_read(
    state: State<'_, AppState>,
    scene_id: String,
) -> Result<HeatReadResponse, String> {
    let db = {
        let proj = state.project.lock().await;
        proj.as_ref().ok_or("no project open")?.db.clone()
    };
    heat_read_core(db, scene_id).await
}

pub async fn heat_read_core(
    db: Arc<Mutex<water_core::Db>>,
    scene_id: String,
) -> Result<HeatReadResponse, String> {
    let scene_id: Id = scene_id.parse().map_err(|e: water_core::Error| e.to_string())?;
    let g = db.lock().await;
    let store = HeatStore::new(&g);
    let mut metrics = HashMap::with_capacity(5);
    for kind in HeatMetricKind::all() {
        let rows = store.read(&scene_id, kind).map_err(|e| e.to_string())?;
        metrics.insert(kind.as_str().to_string(), rows);
    }
    Ok(HeatReadResponse { metrics })
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MetricEnabledMap {
    /// Map keyed by `HeatMetricKind::as_str()`. Missing keys default
    /// to `false` for LLM-required metrics and `true` for local metrics.
    pub enabled: HashMap<String, bool>,
}

/// Persist a per-project metric-enabled toggle. The settings file
/// lives at `<project_root>/.water/heat_metrics.json` and is read by
/// `heat_read_settings` (Phase E renderer mount).
#[tauri::command]
pub async fn heat_set_metric_enabled(
    state: State<'_, AppState>,
    kind: String,
    enabled: bool,
) -> Result<(), String> {
    let root = {
        let proj = state.project.lock().await;
        proj.as_ref().ok_or("no project open")?.root.clone()
    };
    heat_set_metric_enabled_core(&root, &kind, enabled)
}

pub fn heat_set_metric_enabled_core(
    root: &Path,
    kind: &str,
    enabled: bool,
) -> Result<(), String> {
    // Validate the metric name before touching disk so we don't write
    // a settings file that contains arbitrary keys.
    if HeatMetricKind::from_str(kind).is_none() {
        return Err(format!("unknown heat metric: {kind:?}"));
    }
    let path = root.join(".water").join("heat_metrics.json");
    let mut map = read_settings_map(&path).unwrap_or_default();
    map.enabled.insert(kind.to_string(), enabled);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(&map).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())?;
    Ok(())
}

/// Read the persisted per-project metric-enabled map. Returns an empty
/// map (renderer applies defaults) on first launch or any read error.
#[tauri::command]
pub async fn heat_read_settings(state: State<'_, AppState>) -> Result<MetricEnabledMap, String> {
    let root = {
        let proj = state.project.lock().await;
        proj.as_ref().ok_or("no project open")?.root.clone()
    };
    Ok(read_settings_map(&root.join(".water").join("heat_metrics.json")).unwrap_or_default())
}

fn read_settings_map(path: &Path) -> Option<MetricEnabledMap> {
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

impl Default for MetricEnabledMap {
    fn default() -> Self {
        Self {
            enabled: HashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use water_core::{Db, ManuscriptStore, NewScene, ProjectStore, SceneStore};

    async fn seed_db() -> (TempDir, Arc<Mutex<Db>>, String) {
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
        let scene_id = scene.id.to_string();
        (dir, Arc::new(Mutex::new(db_raw)), scene_id)
    }

    #[tokio::test]
    async fn heat_read_returns_empty_metrics_for_brand_new_scene() {
        let (_dir, db, scene_id) = seed_db().await;
        let resp = heat_read_core(db, scene_id).await.unwrap();
        for kind in HeatMetricKind::all() {
            let rows = resp.metrics.get(kind.as_str()).expect("metric key present");
            assert!(rows.is_empty(), "{} should be empty on fresh scene", kind.as_str());
        }
    }

    #[tokio::test]
    async fn heat_read_returns_cached_rows_after_write_batch() {
        let (_dir, db, scene_id) = seed_db().await;
        let scene_id_parsed: Id = scene_id.parse().unwrap();
        {
            let g = db.lock().await;
            HeatStore::new(&g)
                .write_batch(
                    &scene_id_parsed,
                    HeatMetricKind::Pacing,
                    &[(0, 0.5, "h0"), (1, 0.8, "h1")],
                )
                .unwrap();
        }
        let resp = heat_read_core(db, scene_id).await.unwrap();
        let rows = resp.metrics.get("pacing").unwrap();
        assert_eq!(rows.len(), 2);
        assert!((rows[0].value - 0.5).abs() < 1e-5);
        assert!((rows[1].value - 0.8).abs() < 1e-5);
    }

    #[tokio::test]
    async fn heat_read_returns_error_on_malformed_scene_id() {
        let (_dir, db, _scene_id) = seed_db().await;
        let err = heat_read_core(db, "not-a-ulid".into()).await.unwrap_err();
        assert!(!err.is_empty());
    }

    #[test]
    fn heat_set_metric_enabled_rejects_unknown_metric() {
        let dir = TempDir::new().unwrap();
        let err =
            heat_set_metric_enabled_core(dir.path(), "topicality", true).unwrap_err();
        assert!(err.contains("topicality"));
    }

    #[test]
    fn heat_set_metric_enabled_persists_and_round_trips() {
        let dir = TempDir::new().unwrap();
        heat_set_metric_enabled_core(dir.path(), "pacing", true).unwrap();
        heat_set_metric_enabled_core(dir.path(), "valence", false).unwrap();
        let path = dir.path().join(".water").join("heat_metrics.json");
        let map = read_settings_map(&path).unwrap();
        assert_eq!(map.enabled.get("pacing"), Some(&true));
        assert_eq!(map.enabled.get("valence"), Some(&false));
    }

    #[test]
    fn heat_set_metric_enabled_creates_water_dir() {
        let dir = TempDir::new().unwrap();
        let water_dir = dir.path().join(".water");
        assert!(!water_dir.exists());
        heat_set_metric_enabled_core(dir.path(), "pacing", true).unwrap();
        assert!(water_dir.exists());
        assert!(water_dir.join("heat_metrics.json").exists());
    }
}
