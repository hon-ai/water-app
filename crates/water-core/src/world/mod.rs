//! `WorldStore` — world segments + entries on disk and in the index.

pub mod templates;

use crate::{Db, Id, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldSegmentRow {
    pub id: Id,
    pub name: String,
    pub ordering: i64,
    pub is_collection: bool,
}

pub struct WorldStore<'a> {
    db: &'a Db,
    project_root: PathBuf,
}

impl<'a> WorldStore<'a> {
    #[must_use]
    pub fn new(db: &'a Db, project_root: PathBuf) -> Self {
        Self { db, project_root }
    }

    /// Upserts a world segment.
    ///
    /// The `slug` parameter is overloaded: if it parses as a valid ULID, it's
    /// used as the segment id and the call is idempotent via `ON CONFLICT(id)`
    /// — the existing row's `name`/`ordering`/`is_collection` are updated. If `slug`
    /// is anything else (e.g. a human label like `"concept"`), a fresh ULID is
    /// generated and a new row is inserted on every call.
    ///
    /// Callers that want idempotent upsert MUST pass a stable ULID as `slug`.
    /// The M1 surface is permissive; M2 may tighten this.
    pub fn upsert_segment(
        &self,
        project_id: &Id,
        slug: &str,
        name: &str,
        ordering: i64,
        is_collection: bool,
    ) -> Result<Id> {
        let id: Id = slug.parse::<Id>().unwrap_or_else(|_| Id::new());
        self.db.conn().execute(
            "INSERT INTO world_segment (id, project_id, name, ordering, is_collection)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(id) DO UPDATE SET name = excluded.name,
                                            ordering = excluded.ordering,
                                            is_collection = excluded.is_collection",
            (
                id.as_str(),
                project_id.as_str(),
                name,
                ordering,
                i64::from(is_collection),
            ),
        )?;
        Ok(id)
    }

    pub fn list_segments(&self, project_id: &Id) -> Result<Vec<WorldSegmentRow>> {
        let mut stmt = self.db.conn().prepare(
            "SELECT id, name, ordering, is_collection FROM world_segment
             WHERE project_id = ?1 ORDER BY ordering",
        )?;
        let rows = stmt.query_map([project_id.as_str()], |row| {
            let id: String = row.get(0)?;
            let name: String = row.get(1)?;
            let ordering: i64 = row.get(2)?;
            let is_collection: i64 = row.get(3)?;
            let id = id.parse::<Id>().map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?;
            Ok(WorldSegmentRow {
                id,
                name,
                ordering,
                is_collection: is_collection != 0,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    #[must_use]
    pub fn project_root(&self) -> &PathBuf {
        &self.project_root
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ProjectStore;

    #[test]
    fn upsert_and_list_segments() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        let p = ProjectStore::new(&db).insert("P").unwrap();
        let store = WorldStore::new(&db, dir.path().to_path_buf());
        store
            .upsert_segment(&p.id, "concept", "Concept", 0, false)
            .unwrap();
        store
            .upsert_segment(&p.id, "locations", "Locations", 1, true)
            .unwrap();
        let list = store.list_segments(&p.id).unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].name, "Concept");
        assert!(!list[0].is_collection);
        assert!(list[1].is_collection);
    }

    #[test]
    fn upsert_segment_with_ulid_slug_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        let p = ProjectStore::new(&db).insert("P").unwrap();
        let store = WorldStore::new(&db, dir.path().to_path_buf());
        // Use a fresh ULID as the slug so the ON CONFLICT(id) path fires.
        let ulid_slug = Id::new();
        let id1 = store
            .upsert_segment(&p.id, ulid_slug.as_str(), "First", 0, false)
            .unwrap();
        let id2 = store
            .upsert_segment(&p.id, ulid_slug.as_str(), "Renamed", 5, true)
            .unwrap();
        assert_eq!(id1, id2, "same ULID slug must yield same id");
        let list = store.list_segments(&p.id).unwrap();
        assert_eq!(list.len(), 1, "second upsert must update, not insert");
        assert_eq!(list[0].name, "Renamed");
        assert_eq!(list[0].ordering, 5);
        assert!(list[0].is_collection);
    }
}
