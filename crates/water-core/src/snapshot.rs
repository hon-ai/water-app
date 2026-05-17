//! Per-scene snapshots: `zstd`-compressed copies of .md files with DB rows.

use crate::{Db, Error, Id, Result};
use chrono::Utc;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotTrigger {
    Autosave,
    Hourly,
    OnClose,
    PreRestore,
    Manual,
}

impl SnapshotTrigger {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Autosave => "autosave",
            Self::Hourly => "hourly",
            Self::OnClose => "on-close",
            Self::PreRestore => "pre-restore",
            Self::Manual => "manual",
        }
    }
}

#[derive(Debug, Clone)]
pub struct SnapshotRow {
    pub id: Id,
    pub scene_id: Id,
    pub taken_at: String,
    pub trigger: SnapshotTrigger,
    pub file_path: PathBuf,
    pub byte_size: i64,
}

pub struct SnapshotStore<'a> {
    db: &'a Db,
    project_root: PathBuf,
}

fn parse_id(s: &str) -> rusqlite::Result<Id> {
    s.parse::<Id>().map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
    })
}

impl<'a> SnapshotStore<'a> {
    #[must_use]
    pub fn new(db: &'a Db, project_root: PathBuf) -> Self {
        Self { db, project_root }
    }

    fn dir_for(&self, scene_id: &Id) -> PathBuf {
        self.project_root.join("snapshots").join(scene_id.as_str())
    }

    /// Take a snapshot of `source_scene_md` and record it.
    pub fn take(
        &self,
        scene_id: &Id,
        source_scene_md: &Path,
        trigger: SnapshotTrigger,
    ) -> Result<SnapshotRow> {
        let bytes = std::fs::read(source_scene_md)?;
        let compressed = zstd::encode_all(bytes.as_slice(), 3)
            .map_err(|e| Error::Other(format!("zstd encode: {e}")))?;
        let dir = self.dir_for(scene_id);
        std::fs::create_dir_all(&dir)?;
        let id = Id::new();
        let ts = Utc::now();
        let filename = format!("{}.zst", ts.format("%Y-%m-%dT%H-%M-%S%.3f"));
        let path = dir.join(filename);
        std::fs::write(&path, &compressed)?;
        let byte_size = i64::try_from(compressed.len()).unwrap_or(i64::MAX);
        let ts_str = ts.to_rfc3339();

        self.db.conn().execute(
            "INSERT INTO snapshot (id, scene_id, taken_at, trigger, file_path, byte_size)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            (
                id.as_str(),
                scene_id.as_str(),
                &ts_str,
                trigger.as_str(),
                path.to_string_lossy(),
                byte_size,
            ),
        )?;
        Ok(SnapshotRow {
            id,
            scene_id: scene_id.clone(),
            taken_at: ts_str,
            trigger,
            file_path: path,
            byte_size,
        })
    }

    pub fn list(&self, scene_id: &Id) -> Result<Vec<SnapshotRow>> {
        let mut stmt = self.db.conn().prepare(
            "SELECT id, scene_id, taken_at, trigger, file_path, byte_size
             FROM snapshot WHERE scene_id = ?1 ORDER BY taken_at DESC",
        )?;
        let rows = stmt.query_map([scene_id.as_str()], |row| {
            let id: String = row.get(0)?;
            let scene_id: String = row.get(1)?;
            let taken_at: String = row.get(2)?;
            let trigger: String = row.get(3)?;
            let file_path: String = row.get(4)?;
            let byte_size: i64 = row.get(5)?;
            let trig = match trigger.as_str() {
                "autosave" => SnapshotTrigger::Autosave,
                "hourly" => SnapshotTrigger::Hourly,
                "on-close" => SnapshotTrigger::OnClose,
                "pre-restore" => SnapshotTrigger::PreRestore,
                _ => SnapshotTrigger::Manual,
            };
            Ok(SnapshotRow {
                id: parse_id(&id)?,
                scene_id: parse_id(&scene_id)?,
                taken_at,
                trigger: trig,
                file_path: PathBuf::from(file_path),
                byte_size,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn read_decompressed(&self, id: &Id) -> Result<Vec<u8>> {
        let path: String = self
            .db
            .conn()
            .query_row(
                "SELECT file_path FROM snapshot WHERE id = ?1",
                [id.as_str()],
                |r| r.get(0),
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Error::NotFound(format!("snapshot {id}")),
                other => Error::from(other),
            })?;
        let bytes = std::fs::read(Path::new(&path))?;
        let plain = zstd::decode_all(bytes.as_slice())
            .map_err(|e| Error::Other(format!("zstd decode: {e}")))?;
        Ok(plain)
    }

    /// Apply the v1 retention policy. Returns the number of rows deleted.
    ///
    /// - All snapshots taken within the last 24h are kept.
    /// - Between 24h and 7d, the newest snapshot per `UTC` hour is kept.
    /// - Between 7d and 90d, the newest snapshot per `UTC` day is kept.
    /// - Older than 90d, the newest snapshot per `ISO` week is kept.
    pub fn prune(&self, scene_id: &Id, now: chrono::DateTime<chrono::Utc>) -> Result<usize> {
        use chrono::Datelike;
        let rows = self.list(scene_id)?;
        let mut to_keep: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
        let mut seen_bucket: std::collections::HashSet<(u8, String)> =
            std::collections::HashSet::new();

        for r in &rows {
            let ts = chrono::DateTime::parse_from_rfc3339(&r.taken_at)
                .map_err(|e| Error::Other(format!("snapshot timestamp: {e}")))?
                .with_timezone(&chrono::Utc);
            let age = now.signed_duration_since(ts);
            if age <= chrono::Duration::hours(24) {
                to_keep.insert(r.id.to_string());
            } else if age <= chrono::Duration::days(7) {
                let bucket = (1u8, ts.format("%Y-%m-%dT%H").to_string());
                if seen_bucket.insert(bucket) {
                    to_keep.insert(r.id.to_string());
                }
            } else if age <= chrono::Duration::days(90) {
                let bucket = (2u8, ts.format("%Y-%m-%d").to_string());
                if seen_bucket.insert(bucket) {
                    to_keep.insert(r.id.to_string());
                }
            } else {
                let iso = ts.iso_week();
                let bucket = (3u8, format!("{}-W{:02}", iso.year(), iso.week()));
                if seen_bucket.insert(bucket) {
                    to_keep.insert(r.id.to_string());
                }
            }
        }

        let mut deleted = 0usize;
        for r in rows {
            if !to_keep.contains(r.id.as_str()) {
                std::fs::remove_file(&r.file_path).ok();
                self.db
                    .conn()
                    .execute("DELETE FROM snapshot WHERE id = ?1", [r.id.as_str()])?;
                deleted += 1;
            }
        }
        Ok(deleted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ManuscriptStore, NewScene, ProjectStore, SceneStore};

    fn fixture() -> (tempfile::TempDir, Db, Id, Id) {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        let p = ProjectStore::new(&db).insert("P").unwrap();
        let m = ManuscriptStore::new(&db).insert(&p.id, "M", 0).unwrap();
        let ss = SceneStore::new(&db, dir.path().to_path_buf());
        let scene = ss
            .create(NewScene {
                manuscript_id: m.id.clone(),
                chapter_id: None,
                name: "S".into(),
                ordering: 0,
            })
            .unwrap();
        ss.write_body(&scene.id, "first body").unwrap();
        (dir, db, m.id, scene.id)
    }

    #[test]
    fn take_writes_compressed_file_and_row() {
        let (dir, db, _m_id, s_id) = fixture();
        let store = SnapshotStore::new(&db, dir.path().to_path_buf());
        let scene_path = dir
            .path()
            .join("manuscript")
            .join("scenes")
            .join(format!("{s_id}.md"));
        let snap = store
            .take(&s_id, &scene_path, SnapshotTrigger::Manual)
            .unwrap();
        assert!(snap.file_path.exists());
        let list = store.list(&s_id).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].trigger, SnapshotTrigger::Manual);
    }

    #[test]
    fn read_decompressed_matches_original() {
        let (dir, db, _m_id, s_id) = fixture();
        let store = SnapshotStore::new(&db, dir.path().to_path_buf());
        let scene_path = dir
            .path()
            .join("manuscript")
            .join("scenes")
            .join(format!("{s_id}.md"));
        let original = std::fs::read(&scene_path).unwrap();
        let snap = store
            .take(&s_id, &scene_path, SnapshotTrigger::Hourly)
            .unwrap();
        let restored = store.read_decompressed(&snap.id).unwrap();
        assert_eq!(restored, original);
    }

    #[test]
    fn prune_keeps_recent_drops_redundant_hourly() {
        // Manually craft snapshot rows at well-known timestamps so we can
        // assert exact retention behaviour. We don't write real files.
        let (dir, db, _m_id, s_id) = fixture();
        // Insert 30 rows at 5-minute intervals over the last 3 hours (all
        // within the 24h window so all are kept).
        let now = chrono::Utc::now();
        for i in 0i64..30 {
            let ts = now - chrono::Duration::minutes(i * 5);
            db.conn()
                .execute(
                    "INSERT INTO snapshot (id, scene_id, taken_at, trigger, file_path, byte_size)
                     VALUES (?1, ?2, ?3, 'autosave', '/tmp/x', 1)",
                    (Id::new().as_str(), s_id.as_str(), ts.to_rfc3339()),
                )
                .unwrap();
        }
        // Insert 10 rows in the 24h..7d window, all at the same hour (only
        // one per hour should survive). Anchor to the start of an hour so
        // the 3-minute intervals (0..27) never cross an hour boundary.
        let base = (now - chrono::Duration::days(2))
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_utc();
        for i in 0i64..10 {
            let ts = base + chrono::Duration::minutes(i * 3);
            db.conn()
                .execute(
                    "INSERT INTO snapshot (id, scene_id, taken_at, trigger, file_path, byte_size)
                     VALUES (?1, ?2, ?3, 'autosave', '/tmp/x', 1)",
                    (Id::new().as_str(), s_id.as_str(), ts.to_rfc3339()),
                )
                .unwrap();
        }

        let store = SnapshotStore::new(&db, dir.path().to_path_buf());
        let pruned = store.prune(&s_id, now).unwrap();
        // All 30 within last 24h kept; only 1 of the 10 same-hour group kept;
        // so total deleted = 9.
        assert_eq!(pruned, 9);
    }
}
