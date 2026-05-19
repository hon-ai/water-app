//! `WorldStore` — world segments + entries on disk and in the index.

pub mod templates;

use crate::{Db, Id, Result};
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldSegmentRow {
    pub id: Id,
    pub name: String,
    pub ordering: i64,
    pub is_collection: bool,
    pub slug: String,
    pub hue_token: String,
    pub hidden: bool,
    /// Computed: true iff `template_json IS NOT NULL` (segment has a user override).
    pub has_template_override: bool,
}

/// Canonical slugs of the six built-in world segments.
///
/// Used by [`WorldStore::delete_user_segment`] to refuse deletion of built-ins.
const BUILTIN_SLUGS: &[&str] = &[
    "concept",
    "locations",
    "politics_and_social",
    "culture",
    "world",
    "history",
];

/// Returns the current unix timestamp in seconds as a string.
///
/// Used as `created_at` / `updated_at` for world segments. Returns `"0"` if the
/// system clock predates the UNIX epoch (effectively impossible).
fn current_timestamp_string() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    secs.to_string()
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
            "SELECT id, name, ordering, is_collection, slug, hue_token, hidden,
                    CASE WHEN template_json IS NULL THEN 0 ELSE 1 END AS has_override
             FROM world_segment WHERE project_id = ?1 ORDER BY ordering",
        )?;
        let rows = stmt.query_map([project_id.as_str()], |row| {
            let id: String = row.get(0)?;
            let id = id.parse::<Id>().map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?;
            Ok(WorldSegmentRow {
                id,
                name: row.get(1)?,
                ordering: row.get(2)?,
                is_collection: row.get::<_, i64>(3)? != 0,
                slug: row.get(4)?,
                hue_token: row.get(5)?,
                hidden: row.get::<_, i64>(6)? != 0,
                has_template_override: row.get::<_, i64>(7)? != 0,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    /// Reads a single segment by id, returning all v4 columns including
    /// the computed `has_template_override` flag.
    pub fn read_segment(&self, segment_id: &Id) -> Result<WorldSegmentRow> {
        let mut stmt = self.db.conn().prepare(
            "SELECT id, name, ordering, is_collection, slug, hue_token, hidden,
                    CASE WHEN template_json IS NULL THEN 0 ELSE 1 END
             FROM world_segment WHERE id = ?1",
        )?;
        stmt.query_row([segment_id.as_str()], |row| {
            let id: String = row.get(0)?;
            let id = id.parse::<Id>().map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?;
            Ok(WorldSegmentRow {
                id,
                name: row.get(1)?,
                ordering: row.get(2)?,
                is_collection: row.get::<_, i64>(3)? != 0,
                slug: row.get(4)?,
                hue_token: row.get(5)?,
                hidden: row.get::<_, i64>(6)? != 0,
                has_template_override: row.get::<_, i64>(7)? != 0,
            })
        })
        .map_err(Into::into)
    }

    /// Finds a segment by `(project_id, slug)`. Returns `Ok(None)` if no match.
    pub fn find_segment_by_slug(
        &self,
        project_id: &Id,
        slug: &str,
    ) -> Result<Option<WorldSegmentRow>> {
        let segs = self.list_segments(project_id)?;
        Ok(segs.into_iter().find(|s| s.slug == slug))
    }

    /// Seeds the 6 built-in world segments for `project_id`.
    ///
    /// Idempotent: each canonical slug is inserted at most once per project. On
    /// subsequent calls, slugs already present are skipped (the existing row's
    /// `name`/`ordering`/`hue_token`/`hidden` are left untouched).
    ///
    /// Hue tokens are assigned round-robin against `--water-hue-world-1..6` in
    /// slug order (so a fresh project gets exactly hues 1..6).
    ///
    /// Note: this writes the `slug` column directly via raw INSERT because the
    /// M1-era [`Self::upsert_segment`] does not persist the slug column (the v4
    /// backfill only fires once at migration time).
    pub fn seed_builtins(&self, project_id: &Id) -> Result<()> {
        use crate::world::templates::built_in_templates;
        let now = current_timestamp_string();
        for (idx, t) in built_in_templates().iter().enumerate() {
            let existing: Option<String> = self
                .db
                .conn()
                .query_row(
                    "SELECT id FROM world_segment WHERE project_id = ?1 AND slug = ?2",
                    (project_id.as_str(), t.slug),
                    |r| r.get(0),
                )
                .optional()?;
            if existing.is_some() {
                continue;
            }
            let id = Id::new();
            let hue = format!("--water-hue-world-{}", (idx % 6) + 1);
            self.db.conn().execute(
                "INSERT INTO world_segment
                 (id, project_id, name, ordering, is_collection, slug, hue_token, hidden, created_at, updated_at, template_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, ?8, ?8, NULL)",
                (
                    id.as_str(),
                    project_id.as_str(),
                    t.display_name,
                    i64::try_from(idx).unwrap_or(i64::MAX),
                    i64::from(t.is_collection),
                    t.slug,
                    &hue,
                    &now,
                ),
            )?;
        }
        Ok(())
    }

    /// Creates a user-added segment with a custom template override.
    ///
    /// The new segment is placed at `MAX(ordering) + 1` and gets a round-robin
    /// hue token. The `slug` column is left empty (user-added segments are not
    /// resolvable by built-in slug). Returns the new segment id.
    pub fn create_user_segment(
        &self,
        project_id: &Id,
        name: &str,
        is_collection: bool,
        template: &crate::world::templates::WorldTemplateSchema,
    ) -> Result<Id> {
        let id = Id::new();
        let now = current_timestamp_string();
        let json = serde_json::to_string(template)?;
        let next_ord: i64 = self.db.conn().query_row(
            "SELECT COALESCE(MAX(ordering), -1) + 1 FROM world_segment WHERE project_id = ?1",
            [project_id.as_str()],
            |r| r.get(0),
        )?;
        let count: i64 = self.db.conn().query_row(
            "SELECT COUNT(*) FROM world_segment WHERE project_id = ?1",
            [project_id.as_str()],
            |r| r.get(0),
        )?;
        let hue = format!("--water-hue-world-{}", (count % 6) + 1);

        self.db.conn().execute(
            "INSERT INTO world_segment
             (id, project_id, name, ordering, is_collection, slug, hue_token, hidden, created_at, updated_at, template_json)
             VALUES (?1, ?2, ?3, ?4, ?5, '', ?6, 0, ?7, ?7, ?8)",
            (
                id.as_str(),
                project_id.as_str(),
                name,
                next_ord,
                i64::from(is_collection),
                &hue,
                &now,
                &json,
            ),
        )?;
        Ok(id)
    }

    /// Replaces (or sets) the user-override template JSON for a segment and
    /// bumps `updated_at`.
    pub fn update_segment_template(
        &self,
        segment_id: &Id,
        template: &crate::world::templates::WorldTemplateSchema,
    ) -> Result<()> {
        let json = serde_json::to_string(template)?;
        let now = current_timestamp_string();
        self.db.conn().execute(
            "UPDATE world_segment SET template_json = ?1, updated_at = ?2 WHERE id = ?3",
            (&json, &now, segment_id.as_str()),
        )?;
        Ok(())
    }

    /// Toggles the `hidden` flag on a segment. Hidden segments stay in the DB
    /// (and remain reachable by `list_segments`) but the UI filters them out.
    pub fn set_segment_hidden(&self, segment_id: &Id, hidden: bool) -> Result<()> {
        let now = current_timestamp_string();
        self.db.conn().execute(
            "UPDATE world_segment SET hidden = ?1, updated_at = ?2 WHERE id = ?3",
            (i64::from(hidden), &now, segment_id.as_str()),
        )?;
        Ok(())
    }

    /// Deletes a user-added segment. Refuses to delete any of the six canonical
    /// built-ins — callers should use [`Self::set_segment_hidden`] instead.
    ///
    /// # Errors
    ///
    /// Returns [`crate::Error::Other`] if `segment_id` resolves to a built-in
    /// slug (precondition violation, not a "not found").
    pub fn delete_user_segment(&self, segment_id: &Id) -> Result<()> {
        let slug: String = self.db.conn().query_row(
            "SELECT slug FROM world_segment WHERE id = ?1",
            [segment_id.as_str()],
            |r| r.get(0),
        )?;
        if BUILTIN_SLUGS.contains(&slug.as_str()) {
            return Err(crate::Error::Other(format!(
                "cannot delete built-in segment '{slug}' — use set_segment_hidden instead"
            )));
        }
        self.db.conn().execute(
            "DELETE FROM world_segment WHERE id = ?1",
            [segment_id.as_str()],
        )?;
        Ok(())
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

    #[test]
    fn seed_builtins_inserts_six_segments_idempotently() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        let p = ProjectStore::new(&db).insert("P").unwrap();
        let store = WorldStore::new(&db, dir.path().to_path_buf());

        store.seed_builtins(&p.id).unwrap();
        let segs = store.list_segments(&p.id).unwrap();
        assert_eq!(segs.len(), 6, "expected 6 built-in segments; got {}", segs.len());

        // Second call must be idempotent.
        store.seed_builtins(&p.id).unwrap();
        let segs2 = store.list_segments(&p.id).unwrap();
        assert_eq!(segs2.len(), 6);
    }

    #[test]
    fn seed_builtins_assigns_unique_hue_tokens_round_robin() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        let p = ProjectStore::new(&db).insert("P").unwrap();
        let store = WorldStore::new(&db, dir.path().to_path_buf());
        store.seed_builtins(&p.id).unwrap();

        let hues: Vec<String> = db
            .conn()
            .prepare("SELECT hue_token FROM world_segment WHERE project_id = ?1 ORDER BY ordering")
            .unwrap()
            .query_map([p.id.as_str()], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<std::result::Result<_, _>>()
            .unwrap();

        assert_eq!(
            hues,
            vec![
                "--water-hue-world-1",
                "--water-hue-world-2",
                "--water-hue-world-3",
                "--water-hue-world-4",
                "--water-hue-world-5",
                "--water-hue-world-6",
            ]
        );
    }

    #[test]
    fn seed_builtins_sets_correct_slugs_and_is_collection_flags() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        let p = ProjectStore::new(&db).insert("P").unwrap();
        let store = WorldStore::new(&db, dir.path().to_path_buf());
        store.seed_builtins(&p.id).unwrap();

        let mut stmt = db
            .conn()
            .prepare("SELECT slug, is_collection FROM world_segment WHERE project_id = ?1 ORDER BY ordering")
            .unwrap();
        let rows: Vec<(String, bool)> = stmt
            .query_map([p.id.as_str()], |row| {
                let s: String = row.get(0)?;
                let c: i64 = row.get(1)?;
                Ok((s, c != 0))
            })
            .unwrap()
            .collect::<std::result::Result<_, _>>()
            .unwrap();

        assert_eq!(
            rows,
            vec![
                ("concept".to_string(), false),
                ("locations".to_string(), true),
                ("politics_and_social".to_string(), false),
                ("culture".to_string(), false),
                ("world".to_string(), false),
                ("history".to_string(), false),
            ]
        );
    }

    #[test]
    fn find_segment_by_slug_returns_some_for_builtin() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        let p = ProjectStore::new(&db).insert("P").unwrap();
        let store = WorldStore::new(&db, dir.path().to_path_buf());
        store.seed_builtins(&p.id).unwrap();

        let found = store.find_segment_by_slug(&p.id, "locations").unwrap();
        assert!(found.is_some());
        let s = found.unwrap();
        assert!(s.is_collection);
    }

    #[test]
    fn find_segment_by_slug_returns_none_for_unknown() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        let p = ProjectStore::new(&db).insert("P").unwrap();
        let store = WorldStore::new(&db, dir.path().to_path_buf());
        store.seed_builtins(&p.id).unwrap();

        assert!(store.find_segment_by_slug(&p.id, "nonexistent").unwrap().is_none());
    }

    #[test]
    fn create_user_segment_persists_template_json() {
        use crate::world::templates::{WorldTemplateField, WorldTemplateFieldKind, WorldTemplateSchema};
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        let p = ProjectStore::new(&db).insert("P").unwrap();
        let store = WorldStore::new(&db, dir.path().to_path_buf());

        let custom = WorldTemplateSchema {
            id: "magic_systems".to_string(),
            label: "Magic Systems".to_string(),
            fields: vec![WorldTemplateField {
                id: "main.name".to_string(),
                label: "System Name".to_string(),
                prompt_question: "What's this system called?".to_string(),
                kind: WorldTemplateFieldKind::ShortText,
                optional_skip: false,
            }],
        };
        let id = store
            .create_user_segment(&p.id, "Magic Systems", true, &custom)
            .unwrap();

        let json: String = db
            .conn()
            .query_row(
                "SELECT template_json FROM world_segment WHERE id = ?1",
                [id.as_str()],
                |r| r.get(0),
            )
            .unwrap();
        let parsed: WorldTemplateSchema = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.label, "Magic Systems");
    }

    #[test]
    fn set_segment_hidden_toggles_flag() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        let p = ProjectStore::new(&db).insert("P").unwrap();
        let store = WorldStore::new(&db, dir.path().to_path_buf());
        store.seed_builtins(&p.id).unwrap();
        let s = store.find_segment_by_slug(&p.id, "history").unwrap().unwrap();
        store.set_segment_hidden(&s.id, true).unwrap();
        let again = store.find_segment_by_slug(&p.id, "history").unwrap().unwrap();
        assert!(again.hidden);
        store.set_segment_hidden(&s.id, false).unwrap();
        let third = store.find_segment_by_slug(&p.id, "history").unwrap().unwrap();
        assert!(!third.hidden);
    }

    #[test]
    fn delete_user_segment_refuses_builtin() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        let p = ProjectStore::new(&db).insert("P").unwrap();
        let store = WorldStore::new(&db, dir.path().to_path_buf());
        store.seed_builtins(&p.id).unwrap();
        let s = store.find_segment_by_slug(&p.id, "concept").unwrap().unwrap();
        let err = store.delete_user_segment(&s.id).unwrap_err();
        assert!(err.to_string().contains("built-in"));
    }

    #[test]
    fn delete_user_segment_removes_user_added() {
        use crate::world::templates::{WorldTemplateField, WorldTemplateFieldKind, WorldTemplateSchema};
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        let p = ProjectStore::new(&db).insert("P").unwrap();
        let store = WorldStore::new(&db, dir.path().to_path_buf());
        let custom = WorldTemplateSchema {
            id: "test".to_string(),
            label: "Test".to_string(),
            fields: vec![WorldTemplateField {
                id: "main.thing".to_string(),
                label: "Thing".to_string(),
                prompt_question: "?".to_string(),
                kind: WorldTemplateFieldKind::ShortText,
                optional_skip: false,
            }],
        };
        let id = store.create_user_segment(&p.id, "Test", false, &custom).unwrap();
        store.delete_user_segment(&id).unwrap();
        let segs = store.list_segments(&p.id).unwrap();
        assert!(segs.iter().all(|s| s.id != id));
    }
}
