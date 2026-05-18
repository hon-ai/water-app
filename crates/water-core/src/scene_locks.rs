//! Per-scene write-lock registry. Both `SceneStore::rename` and
//! `SceneStore::write_body` acquire the lock for a given scene before any
//! disk I/O, so concurrent rename+body writes don't tear the file.
//!
//! Locks are created lazily on first acquire and never removed (small
//! per-scene overhead; project lifetime).

use crate::Id;
use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, OwnedMutexGuard};

#[derive(Clone, Default)]
pub struct SceneWriteLocks {
    inner: Arc<DashMap<String, Arc<Mutex<()>>>>,
}

impl SceneWriteLocks {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns an owned guard. Drop the guard to release the lock.
    pub async fn acquire(&self, scene_id: &Id) -> OwnedMutexGuard<()> {
        let key = scene_id.as_str().to_string();
        let lock = self
            .inner
            .entry(key)
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .value()
            .clone();
        lock.lock_owned().await
    }
}
