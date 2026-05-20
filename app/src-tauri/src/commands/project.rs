use crate::events::{emit, SidecarStatusPayload};
use crate::state::{AppState, OpenProject};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, State};
use tokio::sync::Mutex;
use water_core::{
    chapters::ChaptersFile, rebuild_from_truth, repair, water_toml::WaterToml, ActiveScene,
    CharacterWriteLocks, Db, ManuscriptStore, ProjectStore, SceneWriteLocks, Sidecar, SidecarSpec,
    SidecarStatus, SidecarSupervisor, SnapshotScheduler,
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
    app: AppHandle,
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
    std::fs::create_dir_all(root.join("manuscript").join("scenes")).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(root.join("characters")).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(root.join("world")).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(root.join("snapshots")).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(root.join(".water").join("cache")).map_err(|e| e.to_string())?;

    let db_path = root.join("project.db");
    let mut db_raw = Db::open(&db_path).map_err(|e| e.to_string())?;
    // `Db::open` already migrates to latest; this is a defensive idempotent
    // call so behavior is unchanged if `Db::open` is ever refactored to not
    // auto-migrate.
    water_core::migrations::run_pending(&mut db_raw)?;
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
        (
            project.id.clone(),
            project.name.clone(),
            manuscript.id.clone(),
        )
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
    let (sidecar, supervisor) = boot_sidecar_for_project(&app).await;
    let orchestrator =
        spawn_orchestrator_for_project(&app, &state, &db, &project_id, root.clone()).await;

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
        project_id: project_id.to_string(),
        default_manuscript_id: manuscript_id.to_string(),
        scheduler,
        sidecar,
        supervisor,
        scene_write_locks: SceneWriteLocks::new(),
        character_write_locks: CharacterWriteLocks::new(),
        world_write_locks: crate::state::WorldWriteLocks::new(),
        orchestrator,
    });
    Ok(info)
}

#[tauri::command]
pub async fn open_project(
    app: AppHandle,
    state: State<'_, AppState>,
    root: String,
) -> Result<OpenProjectInfo, String> {
    let root = PathBuf::from(root);
    let water = WaterToml::read(&root).map_err(|e| e.to_string())?;
    let db_path = root.join("project.db");

    let (mut db_raw, default_manuscript_id) = if db_path.exists() {
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

    // `Db::open` (and `rebuild_from_truth` which goes through it) already
    // migrates to latest; this defensive idempotent call ensures schema is
    // current even if those paths are refactored to skip auto-migration.
    water_core::migrations::run_pending(&mut db_raw)?;

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
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|e| e.to_string())?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        collected
    };
    for (id_s, path_s) in rows {
        let scene_id: water_core::Id =
            id_s.parse().map_err(|e: water_core::Error| e.to_string())?;
        scheduler
            .register(ActiveScene {
                scene_id,
                file_path: std::path::PathBuf::from(path_s),
            })
            .await;
    }

    let (sidecar, supervisor) = boot_sidecar_for_project(&app).await;
    let orchestrator =
        spawn_orchestrator_for_project(&app, &state, &db, &water.project_id, root.clone()).await;

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
        project_id: water.project_id.to_string(),
        default_manuscript_id,
        scheduler,
        sidecar,
        supervisor,
        scene_write_locks: SceneWriteLocks::new(),
        character_write_locks: CharacterWriteLocks::new(),
        world_write_locks: crate::state::WorldWriteLocks::new(),
        orchestrator,
    });
    Ok(info)
}

#[tauri::command]
pub async fn close_project(state: State<'_, AppState>) -> Result<(), String> {
    let mut g = state.project.lock().await;
    if let Some(project) = g.take() {
        // Tell the orchestrator to wind down first so any in-flight
        // generate task finishes against the still-live router/app rather
        // than racing the rest of close.
        if let Some(orch) = &project.orchestrator {
            orch.send(crate::orchestrator_service::OrchestratorRequest::Shutdown)
                .await;
        }
        // Fire OnClose snapshots for all registered scenes, then stop the loop.
        let _ = project.scheduler.on_close().await;
        let _ = project.scheduler.stop().await;
        if let Some(sup) = project.supervisor {
            sup.stop();
        }
        // `project.sidecar` (Arc<Sidecar>) drops here; kill_on_drop(true)
        // on the underlying tokio::process::Child terminates the uvicorn
        // worker. If another Arc clone is held by the supervisor's forwarder
        // task, the worker terminates when the LAST clone drops — which
        // happens within the same tokio runtime tick once sup.stop() runs.
        drop(project.sidecar);
    }
    Ok(())
}

/// Build the persona registry from the per-project DB and spawn the
/// orchestrator service. Wired into both `create_project` and `open_project`.
///
/// Returns `None` only if `PersonaRegistry::from_db` fails — which is
/// effectively a corrupt-DB condition. We log and continue without an
/// orchestrator rather than blocking project open; the renderer just sees
/// no pills.
async fn spawn_orchestrator_for_project(
    app: &AppHandle,
    state: &State<'_, AppState>,
    db: &Arc<Mutex<water_core::Db>>,
    project_id: &water_core::Id,
    project_root: PathBuf,
) -> Option<crate::orchestrator_service::OrchestratorHandle> {
    // Personas + characters + world are all loaded under a single DB-lock
    // acquisition so we don't race a writer between the reads. The project
    // lock is NOT held here (see callers in `create_project`/`open_project`)
    // — they construct `OpenProject` only after this helper returns, so
    // lock-ordering (KNOWN_FRAGILE #6: project before DB) is honored by
    // virtue of not holding the project lock at all on this path.
    let (personas, characters, world_registry) = {
        let g = db.lock().await;
        let personas = match water_core::voice::registry::PersonaRegistry::from_db(&g) {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(error = %e, "persona registry load failed; orchestrator disabled");
                return None;
            }
        };
        // CharacterRegistry load failures are non-fatal: a corrupt or empty
        // character table just yields no characters, and the orchestrator
        // still runs (persona-track triggers fire as usual). M3 T7's
        // `character_dissonance` correctly returns None when the registry
        // is empty.
        let characters = match water_core::character::registry::CharacterRegistry::from_db(&g) {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    "character registry load failed; using empty registry"
                );
                water_core::character::registry::CharacterRegistry::empty()
            }
        };
        // WorldRegistry load failures are non-fatal for the same reason:
        // an empty registry means no world-track context, but persona +
        // character routing still works. M4 Task 13 wired the snapshot
        // through `TriggerContext`; Task 17's `world_drift` trigger
        // returns None on empty.
        let world_registry =
            match water_core::world::WorldRegistry::from_db(&g, project_id, project_root.clone()) {
                Ok(r) => r,
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        "world registry load failed; using empty registry"
                    );
                    water_core::world::WorldRegistry::default()
                }
            };
        (personas, characters, world_registry)
    };
    // The orchestrator holds a CLONE of AppState.router (Arc<Mutex<...>>),
    // so any `provider_test` reconfig published into that slot is observed
    // on the orchestrator's next LLM dispatch without restarting the
    // project.
    let shared: crate::orchestrator_service::SharedRouter = state.router.clone();
    let handle = crate::orchestrator_service::OrchestratorService::start(
        app.clone(),
        shared,
        personas,
        characters,
        world_registry,
        project_root,
    );
    Some(handle)
}

fn sanitize_dir_name(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == ' ' || *c == '-' || *c == '_')
        .collect::<String>()
        .trim()
        .replace(' ', "-")
}

/// Resolve the sidecar workspace path in dev mode.
///
/// In dev, the working dir is `app/src-tauri/`, so the sidecar lives at
/// `../../sidecar`. Resolved at compile time via `CARGO_MANIFEST_DIR` for
/// stability. For packaged builds we'll need `tauri::path::resource_dir`;
/// that lands with the packaging work outside M1.1.
fn dev_sidecar_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("sidecar")
}

/// Best-effort sidecar boot. Returns `(None, None)` if `uv` is missing or
/// the spawn fails — we log and continue without blocking the project open.
async fn boot_sidecar_for_project(
    app: &AppHandle,
) -> (Option<Arc<Sidecar>>, Option<SidecarSupervisor>) {
    let uv_path = match which::which("uv") {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(error = %e, "uv not found on PATH; sidecar disabled for this session");
            return (None, None);
        }
    };
    let spec = SidecarSpec {
        working_dir: dev_sidecar_dir(),
        uv_bin: uv_path,
        port: 18765,
        host: "127.0.0.1".into(),
        boot_timeout: Duration::from_secs(20),
    };
    let sc = match Sidecar::spawn(spec).await {
        Ok(sc) => Arc::new(sc),
        Err(e) => {
            tracing::warn!(error = %e, "sidecar failed to spawn; continuing without sidecar");
            return (None, None);
        }
    };
    // 5s health-poll cadence matches the previous behavior; backoff inside
    // the supervisor decouples retry pacing from this interval on failures.
    let (sup, mut rx) = SidecarSupervisor::start(sc.clone(), Duration::from_secs(5));
    let app_clone = app.clone();
    tokio::spawn(async move {
        loop {
            if rx.changed().await.is_err() {
                break;
            }
            let evt = rx.borrow().clone();
            // Typed emit through the event bus. Mirrors
            // `WaterEventPayloads["sidecar:status"]` in `events.ts`.
            let payload = SidecarStatusPayload {
                status: match evt.status {
                    SidecarStatus::Loading => "loading".to_string(),
                    SidecarStatus::Ready => "ready".to_string(),
                    SidecarStatus::Error => "error".to_string(),
                },
                detail: evt.detail,
            };
            // Best-effort: errors here are usually "no window yet" during
            // early boot — the renderer can still pull diagnostics_status.
            let _ = emit(&app_clone, "sidecar:status", payload);
        }
    });
    (Some(sc), Some(sup))
}
