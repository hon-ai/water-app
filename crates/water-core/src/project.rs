//! Project + Manuscript CRUD against the `SQLite` index.

use crate::{Db, Error, Id, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: Id,
    pub name: String,
    pub default_manuscript_id: Option<Id>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manuscript {
    pub id: Id,
    pub project_id: Id,
    pub name: String,
    pub ordering: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub struct ProjectStore<'a> {
    db: &'a Db,
}

impl<'a> ProjectStore<'a> {
    #[must_use]
    pub fn new(db: &'a Db) -> Self {
        Self { db }
    }

    pub fn insert(&self, name: &str) -> Result<Project> {
        let now = Utc::now();
        let p = Project {
            id: Id::new(),
            name: name.to_owned(),
            default_manuscript_id: None,
            created_at: now,
            updated_at: now,
        };
        self.db.conn().execute(
            "INSERT INTO project (id, name, default_manuscript_id, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            (
                p.id.as_str(),
                &p.name,
                p.default_manuscript_id.as_ref().map(Id::as_str),
                p.created_at.to_rfc3339(),
                p.updated_at.to_rfc3339(),
            ),
        )?;
        Ok(p)
    }

    pub fn get(&self, id: &Id) -> Result<Project> {
        self.db
            .conn()
            .query_row(
                "SELECT id, name, default_manuscript_id, created_at, updated_at
                 FROM project WHERE id = ?1",
                [id.as_str()],
                row_to_project,
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Error::NotFound(format!("project {id}")),
                other => other.into(),
            })
    }

    pub fn set_default_manuscript(&self, project_id: &Id, manuscript_id: &Id) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let n = self.db.conn().execute(
            "UPDATE project SET default_manuscript_id = ?2, updated_at = ?3 WHERE id = ?1",
            (project_id.as_str(), manuscript_id.as_str(), now),
        )?;
        if n == 0 {
            return Err(Error::NotFound(format!("project {project_id}")));
        }
        Ok(())
    }
}

pub struct ManuscriptStore<'a> {
    db: &'a Db,
}

impl<'a> ManuscriptStore<'a> {
    #[must_use]
    pub fn new(db: &'a Db) -> Self {
        Self { db }
    }

    pub fn insert(&self, project_id: &Id, name: &str, ordering: i64) -> Result<Manuscript> {
        let now = Utc::now();
        let m = Manuscript {
            id: Id::new(),
            project_id: project_id.clone(),
            name: name.to_owned(),
            ordering,
            created_at: now,
            updated_at: now,
        };
        self.db.conn().execute(
            "INSERT INTO manuscript (id, project_id, name, ordering, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            (
                m.id.as_str(),
                m.project_id.as_str(),
                &m.name,
                m.ordering,
                m.created_at.to_rfc3339(),
                m.updated_at.to_rfc3339(),
            ),
        )?;
        Ok(m)
    }

    pub fn list(&self, project_id: &Id) -> Result<Vec<Manuscript>> {
        let mut stmt = self.db.conn().prepare(
            "SELECT id, project_id, name, ordering, created_at, updated_at
             FROM manuscript WHERE project_id = ?1 ORDER BY ordering",
        )?;
        let rows = stmt.query_map([project_id.as_str()], row_to_manuscript)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }
}

fn row_to_project(row: &rusqlite::Row<'_>) -> rusqlite::Result<Project> {
    let id: String = row.get(0)?;
    let name: String = row.get(1)?;
    let default_manuscript_id: Option<String> = row.get(2)?;
    let created_at: String = row.get(3)?;
    let updated_at: String = row.get(4)?;
    Ok(Project {
        id: parse_id(&id)?,
        name,
        default_manuscript_id: default_manuscript_id.as_deref().map(parse_id).transpose()?,
        created_at: parse_dt(&created_at)?,
        updated_at: parse_dt(&updated_at)?,
    })
}

fn row_to_manuscript(row: &rusqlite::Row<'_>) -> rusqlite::Result<Manuscript> {
    let id: String = row.get(0)?;
    let project_id: String = row.get(1)?;
    let name: String = row.get(2)?;
    let ordering: i64 = row.get(3)?;
    let created_at: String = row.get(4)?;
    let updated_at: String = row.get(5)?;
    Ok(Manuscript {
        id: parse_id(&id)?,
        project_id: parse_id(&project_id)?,
        name,
        ordering,
        created_at: parse_dt(&created_at)?,
        updated_at: parse_dt(&updated_at)?,
    })
}

fn parse_id(s: &str) -> rusqlite::Result<Id> {
    s.parse::<Id>().map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
    })
}

fn parse_dt(s: &str) -> rusqlite::Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .map(|d| d.with_timezone(&Utc))
        .map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn fresh_db() -> Db {
        Db::open_in_memory().unwrap()
    }

    #[test]
    fn insert_and_get_project_round_trip() {
        let db = fresh_db();
        let store = ProjectStore::new(&db);
        let p = store.insert("Test Project").unwrap();
        let got = store.get(&p.id).unwrap();
        assert_eq!(got.id, p.id);
        assert_eq!(got.name, "Test Project");
        assert!(got.default_manuscript_id.is_none());
    }

    #[test]
    fn get_missing_returns_not_found() {
        let db = fresh_db();
        let store = ProjectStore::new(&db);
        let err = store.get(&Id::new()).unwrap_err();
        assert!(matches!(err, Error::NotFound(_)));
    }

    #[test]
    fn manuscript_insert_and_list() {
        let db = fresh_db();
        let p = ProjectStore::new(&db).insert("P").unwrap();
        let ms = ManuscriptStore::new(&db);
        let m1 = ms.insert(&p.id, "First", 0).unwrap();
        let m2 = ms.insert(&p.id, "Second", 1).unwrap();
        let list = ms.list(&p.id).unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].id, m1.id);
        assert_eq!(list[1].id, m2.id);
    }

    #[test]
    fn set_default_manuscript_updates_project() {
        let db = fresh_db();
        let ps = ProjectStore::new(&db);
        let p = ps.insert("P").unwrap();
        let m = ManuscriptStore::new(&db).insert(&p.id, "M", 0).unwrap();
        ps.set_default_manuscript(&p.id, &m.id).unwrap();
        let p2 = ps.get(&p.id).unwrap();
        assert_eq!(p2.default_manuscript_id, Some(m.id));
    }
}
