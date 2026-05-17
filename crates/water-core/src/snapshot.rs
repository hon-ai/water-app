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
}
