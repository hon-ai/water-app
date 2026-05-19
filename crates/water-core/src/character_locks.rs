//! Per-character write-lock registry. `CharacterStore::update_field` (and any
//! future character-mutation path) acquires the lock for a given character
//! before any disk I/O, so rapid Conversational-Intake updates don't tear
//! the on-disk `.toml`.
//!
//! Mirrors `SceneWriteLocks` 1:1 — locks are created lazily on first acquire
//! and never removed (small per-character overhead; project lifetime).

use crate::Id;
use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, OwnedMutexGuard};

#[derive(Clone, Default)]
pub struct CharacterWriteLocks {
    inner: Arc<DashMap<String, Arc<Mutex<()>>>>,
}

impl CharacterWriteLocks {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns an owned guard. Drop the guard to release the lock.
    pub async fn acquire(&self, character_id: &Id) -> OwnedMutexGuard<()> {
        let key = character_id.as_str().to_string();
        let lock = self
            .inner
            .entry(key)
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .value()
            .clone();
        lock.lock_owned().await
    }
}
