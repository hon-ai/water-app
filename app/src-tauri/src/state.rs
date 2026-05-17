//! Process-wide Tauri state. Wrapped in tokio locks because tauri::State is
//! `&` to a single shared value across commands. `OpenProject` is never
//! constructed via `Default` (the DB requires a path) — the state holds an
//! `Option<OpenProject>` so the "no project open" state is the `None` arm.
//!
//! Note: `water_core::Db` wraps a `rusqlite::Connection`, which is `Send` but
//! `!Sync`. `tokio::sync::RwLock<T>: Sync` requires `T: Send + Sync`, but
//! `tokio::sync::Mutex<T>: Sync` only requires `T: Send`. We therefore use
//! `Mutex` here so that `AppState: Send + Sync` (required by `tauri::State`).

use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use water_core::llm::LlmRouter;
use water_core::Db;

pub struct OpenProject {
    pub root: PathBuf,
    pub db: Db,
    pub default_manuscript_id: String,
}

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
