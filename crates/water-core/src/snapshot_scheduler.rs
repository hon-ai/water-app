//! Snapshot scheduler — async task that takes hourly, on-close, and manual
//! snapshots and prunes per the retention policy.

use crate::snapshot::{SnapshotStore, SnapshotTrigger};
use crate::{Db, Error, Id, Result};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, Mutex};

// `Duration::from_hours` is unstable on stable Rust; use seconds.
#[allow(clippy::duration_suboptimal_units)]
const HOURLY_INTERVAL: Duration = Duration::from_secs(3600);

#[derive(Debug, Clone)]
pub struct ActiveScene {
    pub scene_id: Id,
    pub file_path: PathBuf,
}

enum Cmd {
    Manual(Id),
    PreRestore(Id),
    OnClose,
    Stop,
}

pub struct SnapshotScheduler {
    tx: mpsc::Sender<Cmd>,
    active: Arc<Mutex<Vec<ActiveScene>>>,
}

impl SnapshotScheduler {
    /// Spawn the scheduler. Returns the handle. Caller must keep it alive.
    #[must_use]
    pub fn spawn(db: Arc<Mutex<Db>>, project_root: PathBuf) -> Self {
        let (tx, mut rx) = mpsc::channel::<Cmd>(32);
        let active: Arc<Mutex<Vec<ActiveScene>>> = Arc::new(Mutex::new(Vec::new()));
        let active_clone = active.clone();

        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(HOURLY_INTERVAL);
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            loop {
                tokio::select! {
                    _ = ticker.tick() => {
                        let scenes = active_clone.lock().await.clone();
                        for s in scenes {
                            let _ = take_one(&db, &project_root, &s, SnapshotTrigger::Hourly).await;
                        }
                    }
                    cmd = rx.recv() => {
                        match cmd {
                            Some(Cmd::Manual(scene_id)) => {
                                let active = active_clone.lock().await.clone();
                                if let Some(s) = active.iter().find(|a| a.scene_id == scene_id).cloned() {
                                    let _ = take_one(&db, &project_root, &s, SnapshotTrigger::Manual).await;
                                }
                            }
                            Some(Cmd::PreRestore(scene_id)) => {
                                let active = active_clone.lock().await.clone();
                                if let Some(s) = active.iter().find(|a| a.scene_id == scene_id).cloned() {
                                    let _ = take_one(&db, &project_root, &s, SnapshotTrigger::PreRestore).await;
                                }
                            }
                            Some(Cmd::OnClose) => {
                                let scenes = active_clone.lock().await.clone();
                                for s in scenes {
                                    let _ = take_one(&db, &project_root, &s, SnapshotTrigger::OnClose).await;
                                }
                            }
                            Some(Cmd::Stop) | None => break,
                        }
                    }
                }
            }
        });

        Self { tx, active }
    }

    pub async fn register(&self, scene: ActiveScene) {
        let mut g = self.active.lock().await;
        if !g.iter().any(|s| s.scene_id == scene.scene_id) {
            g.push(scene);
        }
    }

    pub async fn unregister(&self, scene_id: &Id) {
        let mut g = self.active.lock().await;
        g.retain(|s| &s.scene_id != scene_id);
    }

    pub async fn request_manual(&self, scene_id: Id) -> Result<()> {
        self.tx
            .send(Cmd::Manual(scene_id))
            .await
            .map_err(|e| Error::Other(format!("scheduler closed: {e}")))
    }

    pub async fn request_pre_restore(&self, scene_id: Id) -> Result<()> {
        self.tx
            .send(Cmd::PreRestore(scene_id))
            .await
            .map_err(|e| Error::Other(format!("scheduler closed: {e}")))
    }

    pub async fn on_close(&self) -> Result<()> {
        self.tx
            .send(Cmd::OnClose)
            .await
            .map_err(|e| Error::Other(format!("scheduler closed: {e}")))
    }

    pub async fn stop(&self) -> Result<()> {
        self.tx
            .send(Cmd::Stop)
            .await
            .map_err(|e| Error::Other(format!("scheduler closed: {e}")))
    }
}

async fn take_one(
    db: &Arc<Mutex<Db>>,
    project_root: &Path,
    scene: &ActiveScene,
    trigger: SnapshotTrigger,
) -> Result<()> {
    let db_guard = db.lock().await;
    let store = SnapshotStore::new(&db_guard, project_root.to_path_buf());
    store.take(&scene.scene_id, &scene.file_path, trigger)?;
    store.prune(&scene.scene_id, chrono::Utc::now())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ManuscriptStore, NewScene, ProjectStore, SceneStore};

    fn fixture() -> (tempfile::TempDir, Arc<Mutex<Db>>, Id, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        let p = ProjectStore::new(&db).insert("P").unwrap();
        let m = ManuscriptStore::new(&db).insert(&p.id, "M", 0).unwrap();
        let ss = SceneStore::new(&db, dir.path().to_path_buf());
        let scene = ss
            .create(NewScene {
                manuscript_id: m.id,
                chapter_id: None,
                name: "S".into(),
                ordering: 0,
            })
            .unwrap();
        ss.write_body(&scene.id, "hello").unwrap();
        let scene_path = dir
            .path()
            .join("manuscript")
            .join("scenes")
            .join(format!("{}.md", scene.id));
        (dir, Arc::new(Mutex::new(db)), scene.id, scene_path)
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn manual_request_takes_a_snapshot() {
        let (dir, db, scene_id, scene_path) = fixture();
        let scheduler = SnapshotScheduler::spawn(db.clone(), dir.path().to_path_buf());
        scheduler
            .register(ActiveScene {
                scene_id: scene_id.clone(),
                file_path: scene_path,
            })
            .await;
        scheduler.request_manual(scene_id.clone()).await.unwrap();
        // give the task a moment to process
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        let count: i64 = db
            .lock()
            .await
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM snapshot WHERE scene_id = ?1",
                [scene_id.as_str()],
                |r| r.get(0),
            )
            .unwrap();
        assert!(
            count >= 1,
            "expected at least one snapshot row, got {count}"
        );
        scheduler.stop().await.unwrap();
    }
}
