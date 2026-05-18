//! `SceneStore` — manages on-disk scenes + the scene row in `SQLite`.

use crate::block::ensure_block_ids;
use crate::scene_md::{SceneFile, SceneFrontmatter};
use crate::{Db, Error, Id, Result};
use chrono::Utc;
use sha2::Digest;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct NewScene {
    pub manuscript_id: Id,
    pub chapter_id: Option<Id>,
    pub name: String,
    pub ordering: i64,
}

#[derive(Debug, Clone)]
pub struct SceneRow {
    pub id: Id,
    pub manuscript_id: Id,
    pub chapter_id: Option<Id>,
    pub ordering: i64,
    pub name: String,
    pub word_count: i64,
    pub file_path: PathBuf,
    pub file_hash: Option<String>,
}

pub struct SceneStore<'a> {
    db: &'a Db,
    project_root: PathBuf,
}

impl<'a> SceneStore<'a> {
    #[must_use]
    pub fn new(db: &'a Db, project_root: PathBuf) -> Self {
        Self { db, project_root }
    }

    fn scenes_dir(&self) -> PathBuf {
        self.project_root.join("manuscript").join("scenes")
    }

    pub fn create(&self, ns: NewScene) -> Result<SceneRow> {
        let id = Id::new();
        let now = Utc::now();
        let file_path = self.scenes_dir().join(format!("{id}.md"));
        std::fs::create_dir_all(self.scenes_dir())?;

        let frontmatter = SceneFrontmatter {
            id: id.clone(),
            name: ns.name.clone(),
            chapter_id: ns.chapter_id.clone(),
            order: ns.ordering,
            pov_character_id: None,
            characters_present: vec![],
            location_id: None,
            scene_goal: None,
            status: "draft".into(),
            created_at: now.to_rfc3339(),
            updated_at: now.to_rfc3339(),
            word_count: 0,
        };
        let file = SceneFile {
            frontmatter,
            body: String::new(),
        };
        file.write(&file_path)?;
        let file_hash = hash_file(&file_path)?;

        self.db.conn().execute(
            "INSERT INTO scene (id, manuscript_id, chapter_id, ordering, name, scene_goal, status,
                                word_count, file_path, file_hash, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, NULL, 'draft', 0, ?6, ?7, ?8, ?8)",
            (
                id.as_str(),
                ns.manuscript_id.as_str(),
                ns.chapter_id.as_ref().map(Id::as_str),
                ns.ordering,
                &ns.name,
                file_path.to_string_lossy(),
                &file_hash,
                now.to_rfc3339(),
            ),
        )?;

        Ok(SceneRow {
            id,
            manuscript_id: ns.manuscript_id,
            chapter_id: ns.chapter_id,
            ordering: ns.ordering,
            name: ns.name,
            word_count: 0,
            file_path,
            file_hash: Some(file_hash),
        })
    }

    pub fn read(&self, id: &Id) -> Result<SceneFile> {
        let path = self.path_for(id)?;
        SceneFile::read(path)
    }

    /// Write a new body. Ensures `^bk-XXXX` ids, recomputes word count,
    /// updates frontmatter timestamps + hash, and persists the `SceneFile`.
    pub fn write_body(&self, id: &Id, new_body: &str) -> Result<SceneRow> {
        let path = self.path_for(id)?;
        let mut file = SceneFile::read(&path)?;
        let (body_with_ids, _blocks) = ensure_block_ids(new_body);
        file.body = body_with_ids;
        let word_count = i64::try_from(count_words(&file.body)).unwrap_or(i64::MAX);
        file.frontmatter.word_count = word_count;
        file.frontmatter.updated_at = Utc::now().to_rfc3339();
        file.write(&path)?;
        let file_hash = hash_file(&path)?;

        self.db.conn().execute(
            "UPDATE scene SET word_count = ?2, file_hash = ?3, updated_at = ?4 WHERE id = ?1",
            (
                id.as_str(),
                word_count,
                &file_hash,
                &file.frontmatter.updated_at,
            ),
        )?;

        self.row(id)
    }

    // FIXME(KNOWN_FRAGILE #7): `move_to` follows the same read-modify-write
    // pattern as `rename` / `write_body` but is not currently gated by the
    // per-scene write lock. M2 Task 2 deliberately scoped the lock to the two
    // hot-path operations (autosave + title-blur). If a future bug shows a
    // scene_move racing with body autosave, gate this method too — same
    // pattern as the gated commands in app/src-tauri/src/commands/scene.rs.
    pub fn move_to(&self, id: &Id, new_chapter_id: Option<&Id>, new_ordering: i64) -> Result<()> {
        // Update DB
        let now = Utc::now().to_rfc3339();
        let n = self.db.conn().execute(
            "UPDATE scene SET chapter_id = ?2, ordering = ?3, updated_at = ?4 WHERE id = ?1",
            (
                id.as_str(),
                new_chapter_id.map(Id::as_str),
                new_ordering,
                &now,
            ),
        )?;
        if n == 0 {
            return Err(Error::NotFound(format!("scene {id}")));
        }
        // Update on-disk frontmatter to match.
        let path = self.path_for(id)?;
        let mut file = SceneFile::read(&path)?;
        file.frontmatter.chapter_id = new_chapter_id.cloned();
        file.frontmatter.order = new_ordering;
        file.frontmatter.updated_at = now;
        file.write(&path)?;
        let file_hash = hash_file(&path)?;
        self.db.conn().execute(
            "UPDATE scene SET file_hash = ?2 WHERE id = ?1",
            (id.as_str(), &file_hash),
        )?;
        Ok(())
    }

    /// Update the scene's name on disk + in the DB. Like `write_body`, refreshes
    /// `updated_at`, the file hash, and the matching DB row.
    ///
    /// # Errors
    /// Returns an `Error::NotFound` if no scene with the given id exists; otherwise
    /// propagates io / sqlite errors.
    pub fn rename(&self, id: &Id, new_name: &str) -> Result<SceneRow> {
        let path = self.path_for(id)?;
        let mut file = SceneFile::read(&path)?;
        new_name.clone_into(&mut file.frontmatter.name);
        file.frontmatter.updated_at = Utc::now().to_rfc3339();
        file.write(&path)?;
        let file_hash = hash_file(&path)?;

        self.db.conn().execute(
            "UPDATE scene SET name = ?2, file_hash = ?3, updated_at = ?4 WHERE id = ?1",
            (
                id.as_str(),
                new_name,
                &file_hash,
                &file.frontmatter.updated_at,
            ),
        )?;

        self.row(id)
    }

    pub fn list(&self, manuscript_id: &Id) -> Result<Vec<SceneRow>> {
        let mut stmt = self.db.conn().prepare(
            "SELECT id, manuscript_id, chapter_id, ordering, name, word_count, file_path, file_hash
             FROM scene WHERE manuscript_id = ?1 ORDER BY ordering",
        )?;
        let rows = stmt.query_map([manuscript_id.as_str()], row_to_scene_row)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    fn row(&self, id: &Id) -> Result<SceneRow> {
        self.db
            .conn()
            .query_row(
                "SELECT id, manuscript_id, chapter_id, ordering, name, word_count, file_path, file_hash
                 FROM scene WHERE id = ?1",
                [id.as_str()],
                row_to_scene_row,
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Error::NotFound(format!("scene {id}")),
                other => other.into(),
            })
    }

    fn path_for(&self, id: &Id) -> Result<PathBuf> {
        let path: String = self
            .db
            .conn()
            .query_row(
                "SELECT file_path FROM scene WHERE id = ?1",
                [id.as_str()],
                |r| r.get(0),
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Error::NotFound(format!("scene {id}")),
                other => other.into(),
            })?;
        Ok(PathBuf::from(path))
    }
}

fn parse_id(s: &str) -> rusqlite::Result<Id> {
    s.parse::<Id>().map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
    })
}

fn row_to_scene_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<SceneRow> {
    let id: String = row.get(0)?;
    let manuscript_id: String = row.get(1)?;
    let chapter_id: Option<String> = row.get(2)?;
    let ordering: i64 = row.get(3)?;
    let name: String = row.get(4)?;
    let word_count: i64 = row.get(5)?;
    let file_path: String = row.get(6)?;
    let file_hash: Option<String> = row.get(7)?;
    Ok(SceneRow {
        id: parse_id(&id)?,
        manuscript_id: parse_id(&manuscript_id)?,
        chapter_id: chapter_id.as_deref().map(parse_id).transpose()?,
        ordering,
        name,
        word_count,
        file_path: PathBuf::from(file_path),
        file_hash,
    })
}

fn count_words(s: &str) -> usize {
    s.split_whitespace()
        .filter(|w| !w.starts_with("^bk-"))
        .count()
}

pub(crate) fn hash_file<P: AsRef<Path>>(path: P) -> Result<String> {
    let bytes = std::fs::read(path)?;
    let digest = sha2::Sha256::digest(&bytes);
    Ok(format!("{digest:x}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ManuscriptStore, ProjectStore};
    use pretty_assertions::assert_eq;

    fn setup() -> (tempfile::TempDir, Db, Id) {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        let p = ProjectStore::new(&db).insert("P").unwrap();
        let m = ManuscriptStore::new(&db).insert(&p.id, "M", 0).unwrap();
        (dir, db, m.id)
    }

    #[test]
    fn create_writes_file_and_row() {
        let (dir, db, m_id) = setup();
        let store = SceneStore::new(&db, dir.path().to_path_buf());
        let scene = store
            .create(NewScene {
                manuscript_id: m_id,
                chapter_id: None,
                name: "S1".into(),
                ordering: 0,
            })
            .unwrap();
        assert!(scene.file_path.exists(), "scene file should exist on disk");
        let file = store.read(&scene.id).unwrap();
        assert_eq!(file.frontmatter.name, "S1");
        assert_eq!(file.body, "");
    }

    #[test]
    fn write_body_adds_block_ids_and_updates_word_count() {
        let (dir, db, m_id) = setup();
        let store = SceneStore::new(&db, dir.path().to_path_buf());
        let scene = store
            .create(NewScene {
                manuscript_id: m_id,
                chapter_id: None,
                name: "S1".into(),
                ordering: 0,
            })
            .unwrap();
        let row = store
            .write_body(&scene.id, "Hello there.\n\nAnother one.")
            .unwrap();
        assert_eq!(row.word_count, 4);
        let file = store.read(&scene.id).unwrap();
        assert!(file.body.contains("Hello there. ^bk-"));
        assert!(file.body.contains("Another one. ^bk-"));
    }

    #[test]
    fn move_to_updates_both_db_and_frontmatter() {
        let (dir, db, m_id) = setup();
        let store = SceneStore::new(&db, dir.path().to_path_buf());
        let scene = store
            .create(NewScene {
                manuscript_id: m_id.clone(),
                chapter_id: None,
                name: "S".into(),
                ordering: 0,
            })
            .unwrap();
        let hash_before = scene.file_hash.clone();
        store.move_to(&scene.id, None, 99).unwrap();
        let row = store.list(&m_id).unwrap();
        assert_eq!(row[0].ordering, 99);
        assert_ne!(
            row[0].file_hash, hash_before,
            "file_hash should refresh after move_to"
        );
        let file = store.read(&scene.id).unwrap();
        assert_eq!(file.frontmatter.order, 99);
    }

    #[test]
    fn rename_updates_both_db_and_frontmatter() {
        let (dir, db, m_id) = setup();
        let store = SceneStore::new(&db, dir.path().to_path_buf());
        let scene = store
            .create(NewScene {
                manuscript_id: m_id.clone(),
                chapter_id: None,
                name: "Original".into(),
                ordering: 0,
            })
            .unwrap();
        let hash_before = scene.file_hash.clone();
        let row = store.rename(&scene.id, "Renamed").unwrap();
        // DB side
        assert_eq!(row.name, "Renamed");
        assert_ne!(
            row.file_hash, hash_before,
            "file_hash should refresh after rename"
        );
        // List should also reflect the new name
        let list = store.list(&m_id).unwrap();
        assert_eq!(
            list.iter().find(|r| r.id == scene.id).unwrap().name,
            "Renamed"
        );
        // Frontmatter side
        let file = store.read(&scene.id).unwrap();
        assert_eq!(file.frontmatter.name, "Renamed");
    }

    #[tokio::test]
    async fn rename_and_write_body_are_serialized_per_scene() {
        use crate::scene_locks::SceneWriteLocks;
        use std::sync::Arc;

        let (_dir, db, root, scene_id) = setup_async_project_with_one_scene().await;

        let locks = SceneWriteLocks::new();
        let mut handles = Vec::new();
        for i in 0..50 {
            let db_c = Arc::clone(&db);
            let root_c = root.clone();
            let id_c = scene_id.clone();
            let locks_c = locks.clone();
            let new_name = format!("Renamed {i}");
            handles.push(tokio::spawn(async move {
                let _g = locks_c.acquire(&id_c).await;
                let g = db_c.lock().await;
                let store = SceneStore::new(&g, root_c.clone());
                store.rename(&id_c, &new_name).unwrap();
            }));
            let db_c2 = Arc::clone(&db);
            let root_c2 = root.clone();
            let id_c2 = scene_id.clone();
            let locks_c2 = locks.clone();
            let new_body = format!("Body iteration {i}\n");
            handles.push(tokio::spawn(async move {
                let _g = locks_c2.acquire(&id_c2).await;
                let g = db_c2.lock().await;
                let store = SceneStore::new(&g, root_c2.clone());
                store.write_body(&id_c2, &new_body).unwrap();
            }));
        }
        for h in handles {
            h.await.unwrap();
        }

        // Final state: DB row matches on-disk file, no torn write.
        let g = db.lock().await;
        let store = SceneStore::new(&g, root.clone());
        // Pull the row by listing — `row` is private, but `list` exposes the data.
        let rows = store
            .list(
                &g.conn()
                    .query_row(
                        "SELECT manuscript_id FROM scene WHERE id = ?1",
                        [scene_id.as_str()],
                        |r| r.get::<_, String>(0),
                    )
                    .unwrap()
                    .parse::<Id>()
                    .unwrap(),
            )
            .unwrap();
        let row = rows.iter().find(|r| r.id == scene_id).unwrap();
        let file = store.read(&scene_id).unwrap();
        assert!(row.name.starts_with("Renamed "));
        assert!(file.body.contains("Body iteration "));
        // File hash on disk matches the hash recorded in the DB row — proves the
        // last writer's row reflects the actual on-disk file (no torn write).
        let on_disk_hash = hash_file(
            root.join("manuscript/scenes")
                .join(format!("{}.md", scene_id.as_str())),
        )
        .unwrap();
        assert_eq!(row.file_hash.as_deref(), Some(on_disk_hash.as_str()));
    }

    async fn setup_async_project_with_one_scene() -> (
        tempfile::TempDir,
        std::sync::Arc<tokio::sync::Mutex<Db>>,
        std::path::PathBuf,
        Id,
    ) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_path_buf();
        std::fs::create_dir_all(root.join("manuscript").join("scenes")).unwrap();
        let db_raw = Db::open(root.join("project.db")).unwrap();
        let db = std::sync::Arc::new(tokio::sync::Mutex::new(db_raw));
        let scene_id = {
            let g = db.lock().await;
            let p = ProjectStore::new(&g).insert("P").unwrap();
            let m = ManuscriptStore::new(&g).insert(&p.id, "M", 0).unwrap();
            let store = SceneStore::new(&g, root.clone());
            store
                .create(NewScene {
                    manuscript_id: m.id,
                    chapter_id: None,
                    name: "Scene 1".into(),
                    ordering: 0,
                })
                .unwrap()
                .id
        };
        (dir, db, root, scene_id)
    }

    #[test]
    fn list_returns_scenes_in_ordering() {
        let (dir, db, m_id) = setup();
        let store = SceneStore::new(&db, dir.path().to_path_buf());
        store
            .create(NewScene {
                manuscript_id: m_id.clone(),
                chapter_id: None,
                name: "B".into(),
                ordering: 1,
            })
            .unwrap();
        store
            .create(NewScene {
                manuscript_id: m_id.clone(),
                chapter_id: None,
                name: "A".into(),
                ordering: 0,
            })
            .unwrap();
        let list = store.list(&m_id).unwrap();
        assert_eq!(
            list.iter().map(|s| s.name.clone()).collect::<Vec<_>>(),
            vec!["A", "B"]
        );
    }
}
