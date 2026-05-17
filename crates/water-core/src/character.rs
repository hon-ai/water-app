//! `CharacterStore` — `TOML` on disk + `SQLite` index.

use crate::{Db, Error, Id, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CharacterFile {
    pub id: Id,
    pub name: String,
    pub schema_version: String,
    #[serde(flatten)]
    pub data: toml::Table,
}

#[derive(Debug, Clone)]
pub struct CharacterRow {
    pub id: Id,
    pub name: String,
    pub file_path: PathBuf,
}

pub struct CharacterStore<'a> {
    db: &'a Db,
    project_root: PathBuf,
}

impl<'a> CharacterStore<'a> {
    #[must_use]
    pub fn new(db: &'a Db, project_root: PathBuf) -> Self {
        Self { db, project_root }
    }

    fn dir(&self) -> PathBuf {
        self.project_root.join("characters")
    }

    pub fn upsert(&self, project_id: &Id, file: CharacterFile) -> Result<CharacterRow> {
        std::fs::create_dir_all(self.dir())?;
        let path = self.dir().join(format!("{}.toml", file.id));
        let text = toml::to_string_pretty(&file)?;
        std::fs::write(&path, text)?;
        let hash = crate::scene::hash_file(&path)?;
        let now = Utc::now().to_rfc3339();

        let data_json = serde_json::to_string(&file.data)?;
        self.db.conn().execute(
            "INSERT INTO character (id, project_id, name, schema_version, data_json, file_path, file_hash, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)
             ON CONFLICT(id) DO UPDATE SET name = excluded.name, schema_version = excluded.schema_version,
                                            data_json = excluded.data_json, file_hash = excluded.file_hash,
                                            updated_at = excluded.updated_at",
            (
                file.id.as_str(),
                project_id.as_str(),
                &file.name,
                &file.schema_version,
                &data_json,
                path.to_string_lossy(),
                &hash,
                &now,
            ),
        )?;
        Ok(CharacterRow {
            id: file.id,
            name: file.name,
            file_path: path,
        })
    }

    pub fn list(&self, project_id: &Id) -> Result<Vec<CharacterRow>> {
        let mut stmt = self.db.conn().prepare(
            "SELECT id, name, file_path FROM character WHERE project_id = ?1 ORDER BY name",
        )?;
        let rows = stmt.query_map([project_id.as_str()], |row| {
            let id: String = row.get(0)?;
            let name: String = row.get(1)?;
            let file_path: String = row.get(2)?;
            let id = id.parse::<Id>().map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?;
            Ok(CharacterRow {
                id,
                name,
                file_path: PathBuf::from(file_path),
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn delete(&self, id: &Id) -> Result<()> {
        let path: String = self
            .db
            .conn()
            .query_row(
                "SELECT file_path FROM character WHERE id = ?1",
                [id.as_str()],
                |r| r.get(0),
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Error::NotFound(format!("character {id}")),
                other => Error::from(other),
            })?;
        std::fs::remove_file(Path::new(&path)).ok();
        self.db
            .conn()
            .execute("DELETE FROM character WHERE id = ?1", [id.as_str()])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ProjectStore;

    fn setup() -> (tempfile::TempDir, Db, Id) {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        let p = ProjectStore::new(&db).insert("P").unwrap();
        (dir, db, p.id)
    }

    fn sample_char() -> CharacterFile {
        let mut t = toml::Table::new();
        t.insert("note".into(), toml::Value::String("placeholder".into()));
        CharacterFile {
            id: Id::new(),
            name: "Maren".into(),
            schema_version: "lsm-v2.1".into(),
            data: t,
        }
    }

    #[test]
    fn upsert_creates_file_and_row() {
        let (dir, db, p_id) = setup();
        let store = CharacterStore::new(&db, dir.path().to_path_buf());
        let row = store.upsert(&p_id, sample_char()).unwrap();
        assert!(row.file_path.exists());
        assert_eq!(store.list(&p_id).unwrap().len(), 1);
    }

    #[test]
    fn upsert_is_idempotent_on_id() {
        let (dir, db, p_id) = setup();
        let store = CharacterStore::new(&db, dir.path().to_path_buf());
        let mut c = sample_char();
        store.upsert(&p_id, c.clone()).unwrap();
        c.name = "Renamed".into();
        store.upsert(&p_id, c.clone()).unwrap();
        let list = store.list(&p_id).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "Renamed");
    }

    #[test]
    fn delete_removes_file_and_row() {
        let (dir, db, p_id) = setup();
        let store = CharacterStore::new(&db, dir.path().to_path_buf());
        let row = store.upsert(&p_id, sample_char()).unwrap();
        store.delete(&row.id).unwrap();
        assert!(!row.file_path.exists());
        assert_eq!(store.list(&p_id).unwrap().len(), 0);
    }
}
