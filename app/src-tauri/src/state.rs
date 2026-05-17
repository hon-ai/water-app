//! Process-wide Tauri state. Wrapped in tokio locks because tauri::State is
//! `&` to a single shared value across commands. `OpenProject` is never
//! constructed via `Default` (the DB requires a path) — the state holds an
//! `Option<OpenProject>` so the "no project open" state is the `None` arm.

use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use water_core::{llm::LlmRouter, Db, SnapshotScheduler};

pub struct OpenProject {
    pub root: PathBuf,
    /// Wrapped in `Arc<Mutex<Db>>` so subsystems (snapshot scheduler, sidecar
    /// supervisor) that need to hold the DB across tasks can share ownership.
    /// `Db` is `Send + !Sync` (`rusqlite::Connection` contains `RefCell`), so
    /// `tokio::sync::Mutex` is the only correct sharing primitive here.
    pub db: Arc<Mutex<Db>>,
    pub default_manuscript_id: String,
    /// Per-project snapshot scheduler. Lives as long as the project is open.
    /// On `close_project`, we fire OnClose snapshots then stop the task.
    pub scheduler: SnapshotScheduler,
}

// `tokio::sync::Mutex` (not `RwLock`) because `Db: Send + !Sync`
// (rusqlite::Connection contains RefCell). `tauri::State<T>` requires
// `T: Send + Sync`, and `Mutex<T>: Sync` needs only `T: Send`.
pub struct AppState {
    pub project: Mutex<Option<OpenProject>>,
    pub router: Mutex<Option<Arc<LlmRouter>>>,
}

impl AppState {
    #[must_use]
    pub fn new() -> Self {
        Self {
            project: Mutex::new(None),
            router: Mutex::new(None),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
