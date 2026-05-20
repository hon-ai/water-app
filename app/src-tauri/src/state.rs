//! Process-wide Tauri state. Wrapped in tokio locks because tauri::State is
//! `&` to a single shared value across commands. `OpenProject` is never
//! constructed via `Default` (the DB requires a path) — the state holds an
//! `Option<OpenProject>` so the "no project open" state is the `None` arm.

use dashmap::DashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use water_core::{
    llm::LlmRouter, CharacterWriteLocks, Db, SceneWriteLocks, Sidecar, SidecarSupervisor,
    SnapshotScheduler,
};

/// Per-`world_entry` write-lock registry. Parallel to `SceneWriteLocks` /
/// `CharacterWriteLocks` but kept as a raw `DashMap<Id, Arc<Mutex<()>>>`
/// rather than a typed wrapper in `water-core` because M4's write paths
/// (the world Tauri commands) already live in this crate, so the wrapper
/// would have no other call sites. If a `WorldStore`-level write path is
/// ever added in `water-core`, lift this into a `world_locks.rs` mirror
/// of `character_locks.rs` and re-export.
///
/// Same lock-ordering rule as the other registries (KNOWN_FRAGILE #6):
/// `project lock → drop → world write-lock → db lock`.
pub type WorldWriteLocks = DashMap<water_core::Id, Arc<Mutex<()>>>;

pub struct OpenProject {
    pub root: PathBuf,
    /// Wrapped in `Arc<Mutex<Db>>` so subsystems (snapshot scheduler, sidecar
    /// supervisor) that need to hold the DB across tasks can share ownership.
    /// `Db` is `Send + !Sync` (`rusqlite::Connection` contains `RefCell`), so
    /// `tokio::sync::Mutex` is the only correct sharing primitive here.
    pub db: Arc<Mutex<Db>>,
    /// The owning `project.id`. Needed by character CRUD commands (the
    /// `character` table's FK requires a `project_id` on insert). M1
    /// projects are single-project per DB; this is just the id of the
    /// row created by `ProjectStore::insert` at create/open time.
    pub project_id: String,
    pub default_manuscript_id: String,
    /// Per-project snapshot scheduler. Lives as long as the project is open.
    /// On `close_project`, we fire OnClose snapshots then stop the task.
    pub scheduler: SnapshotScheduler,
    /// Spawned sidecar process. `None` if `uv` was unavailable or the spawn
    /// failed — we don't block project open on the sidecar.
    pub sidecar: Option<Arc<Sidecar>>,
    /// Watches the sidecar's /health and emits status events. `None` whenever
    /// `sidecar` is `None`.
    pub supervisor: Option<SidecarSupervisor>,
    /// Per-scene write locks shared by all command handlers that touch
    /// `SceneStore::rename` or `SceneStore::write_body`. Prevents the
    /// whole-file write race documented in KNOWN_FRAGILE #7.
    pub scene_write_locks: SceneWriteLocks,
    /// Per-character write locks. `character_update_field` is called once
    /// per keystroke in the Conversational Intake flow; without
    /// serialization a fast typist can land two `update_field` invocations
    /// concurrently and tear the on-disk `.toml`. Same lock-ordering rule
    /// as scene writes: acquire the per-character lock BEFORE the DB lock.
    pub character_write_locks: CharacterWriteLocks,
    /// Per-`world_entry` write locks. The M4 world commands
    /// (`world_entry_update_field`, `world_single_doc_update_field`,
    /// `world_entry_update_aliases`, etc.) mutate on-disk `.toml` files
    /// and the `world_entry.data_json` mirror column under one BEGIN /
    /// COMMIT transaction. Concurrent updates against the same entry id
    /// would race the file write — the per-entry lock serializes them.
    /// Same lock-ordering rule (KNOWN_FRAGILE #6): acquire BEFORE the DB
    /// lock.
    ///
    /// Allow-dead-code while the field is scaffolding: the M4 plan adds
    /// the registry now (Phase B Task 12) so the wiring for the write
    /// commands (Phase C / later) can pull it from `OpenProject`
    /// without another struct-touch commit.
    #[allow(dead_code)]
    pub world_write_locks: WorldWriteLocks,
    /// Per-project orchestrator. `None` only on the (currently unreachable)
    /// path where service spawn fails; in practice this is always `Some`
    /// once `open_project`/`create_project` returns. Dropped on
    /// `close_project` which terminates the service loop via `Shutdown`.
    pub orchestrator: Option<crate::orchestrator_service::OrchestratorHandle>,
}

// `tokio::sync::Mutex` (not `RwLock`) because `Db: Send + !Sync`
// (rusqlite::Connection contains RefCell). `tauri::State<T>` requires
// `T: Send + Sync`, and `Mutex<T>: Sync` needs only `T: Send`.
//
// `router` is wrapped in `Arc<Mutex<...>>` (rather than a bare `Mutex`) so
// the `OrchestratorService` can hold a clone of the SAME slot. Reconfigs
// from `provider_test` are then visible to the orchestrator without
// restarting the project — see `orchestrator_service::SharedRouter`.
pub struct AppState {
    pub project: Mutex<Option<OpenProject>>,
    pub router: Arc<Mutex<Option<Arc<LlmRouter>>>>,
}

impl AppState {
    #[must_use]
    pub fn new() -> Self {
        Self {
            project: Mutex::new(None),
            router: Arc::new(Mutex::new(None)),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
