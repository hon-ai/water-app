//! `WorldStore` — world segments + entries on disk and in the index.
//!
//! Split out from `world/mod.rs` ahead of Task 5 (collection CRUD) so each
//! submodule stays under ~700 lines.

use crate::{Db, Error, Id, Result};
use chrono::Utc;
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

/// On-disk shape for a single-doc segment. Section keys (e.g. `"main"`,
/// `"lists"`) land at top level via `#[serde(flatten)]`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldSingleDocFile {
    pub id: Id,
    pub schema_version: String,
    pub name: String,
    #[serde(flatten)]
    pub data: serde_json::Map<String, serde_json::Value>,
}

/// On-disk shape for one row of a collection segment (e.g. one location).
/// Section keys (`"main"`, `"lists"`, ...) land at top level via
/// `#[serde(flatten)]`. `aliases` defaults to an empty vec so entries
/// written before Task 5 still round-trip.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldEntryFile {
    pub id: Id,
    pub segment_id: Id,
    pub schema_version: String,
    pub name: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(flatten)]
    pub data: serde_json::Map<String, serde_json::Value>,
}

/// Lightweight row used by `list_entries` — name + a one-line preview
/// derived from the entry's `[main]` section.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldEntryIndexRow {
    pub id: Id,
    pub segment_id: Id,
    pub name: String,
    pub preview: String,
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
    ///
    /// # Errors
    ///
    /// Returns [`Error::NotFound`] if `segment_id` is unknown.
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
        .map_err(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => {
                Error::NotFound(format!("world_segment {}", segment_id.as_str()))
            }
            other => Error::from(other),
        })
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
    /// `name`/`ordering`/`hue_token`/`hidden` are left untouched). Wrapped in a
    /// transaction so SELECT-then-INSERT races between concurrent callers
    /// cannot both observe "no existing row" and both insert.
    ///
    /// Hue tokens are assigned round-robin against `--water-hue-world-1..6` in
    /// slug order (so a fresh project gets exactly hues 1..6).
    ///
    /// Note: this writes the `slug` column directly via raw INSERT because the
    /// M1-era [`Self::upsert_segment`] does not persist the slug column (the v4
    /// backfill only fires once at migration time).
    pub fn seed_builtins(&self, project_id: &Id) -> Result<()> {
        use crate::world::templates::built_in_templates;
        let tx = self.db.conn().unchecked_transaction()?;
        let now = Utc::now().to_rfc3339();
        for (idx, t) in built_in_templates().iter().enumerate() {
            let existing: Option<String> = tx
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
            tx.execute(
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
        tx.commit()?;
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
        let now = Utc::now().to_rfc3339();
        let json = serde_json::to_string(template)?;

        let tx = self.db.conn().unchecked_transaction()?;

        // Single round-trip for both derived values; eliminates the race where
        // MAX(ordering) and COUNT(*) could observe different snapshots.
        let (next_ord, count): (i64, i64) = tx.query_row(
            "SELECT COALESCE(MAX(ordering), -1) + 1, COUNT(*) FROM world_segment WHERE project_id = ?1",
            [project_id.as_str()],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        let hue = format!("--water-hue-world-{}", (count % 6) + 1);

        tx.execute(
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
        tx.commit()?;
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
        let now = Utc::now().to_rfc3339();
        self.db.conn().execute(
            "UPDATE world_segment SET template_json = ?1, updated_at = ?2 WHERE id = ?3",
            (&json, &now, segment_id.as_str()),
        )?;
        Ok(())
    }

    /// Toggles the `hidden` flag on a segment. Hidden segments stay in the DB
    /// (and remain reachable by `list_segments`) but the UI filters them out.
    pub fn set_segment_hidden(&self, segment_id: &Id, hidden: bool) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        self.db.conn().execute(
            "UPDATE world_segment SET hidden = ?1, updated_at = ?2 WHERE id = ?3",
            (i64::from(hidden), &now, segment_id.as_str()),
        )?;
        Ok(())
    }

    /// Deletes a user-added segment. Refuses to delete any of the six canonical
    /// built-ins — callers should use [`Self::set_segment_hidden`] instead.
    ///
    /// The built-in check derives from [`crate::world::templates::built_in_templates`]
    /// at call time so adding a 7th built-in doesn't silently bypass the guard.
    ///
    /// # Errors
    ///
    /// - [`Error::NotFound`] if `segment_id` is unknown.
    /// - [`Error::Other`] if `segment_id` resolves to a built-in slug
    ///   (precondition violation, not a "not found").
    pub fn delete_user_segment(&self, segment_id: &Id) -> Result<()> {
        let slug: String = self
            .db
            .conn()
            .query_row(
                "SELECT slug FROM world_segment WHERE id = ?1",
                [segment_id.as_str()],
                |r| r.get(0),
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => {
                    Error::NotFound(format!("world_segment {}", segment_id.as_str()))
                }
                other => Error::from(other),
            })?;
        let is_builtin = crate::world::templates::built_in_templates()
            .iter()
            .any(|t| t.slug == slug);
        if is_builtin {
            return Err(Error::Other(format!(
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

    /// Returns the single-doc segment's data, lazily materializing an empty
    /// row the first time the segment is read.
    ///
    /// # Errors
    /// - [`Error::Other`] if the segment is a collection (callers should use
    ///   the collection APIs instead).
    /// - [`Error::NotFound`] if `segment_id` is unknown.
    pub fn read_single_doc(&self, segment_id: &Id) -> Result<WorldSingleDocFile> {
        self.read_single_doc_with_segment(segment_id).map(|(f, _)| f)
    }

    /// Internal helper that returns both the single-doc file and the resolved
    /// segment row, so callers that need both (notably
    /// [`Self::update_single_doc_field`]) avoid a redundant `read_segment` round
    /// trip.
    ///
    /// The SELECT-then-INSERT lazy-materialization is wrapped in a
    /// transaction so concurrent callers cannot both observe "no row" and
    /// both INSERT (same race `seed_builtins` guards against).
    fn read_single_doc_with_segment(
        &self,
        segment_id: &Id,
    ) -> Result<(WorldSingleDocFile, WorldSegmentRow)> {
        let seg = self.read_segment(segment_id)?;
        if seg.is_collection {
            return Err(Error::Other(format!(
                "segment {} is a collection; use list_entries / read_entry instead",
                seg.slug
            )));
        }

        let schema_version = format!("{}@1", seg.slug);
        let tx = self.db.conn().unchecked_transaction()?;

        let row: Option<(String, String, String)> = tx
            .query_row(
                "SELECT id, name, data_json FROM world_entry WHERE segment_id = ?1 LIMIT 1",
                [segment_id.as_str()],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;

        if let Some((id_str, name, data_json)) = row {
            tx.commit()?;
            let id = id_str
                .parse::<Id>()
                .map_err(|e| Error::Other(format!("invalid id in world_entry: {e}")))?;
            let data: serde_json::Map<String, serde_json::Value> =
                serde_json::from_str(&data_json).unwrap_or_default();
            let file = WorldSingleDocFile {
                id,
                schema_version,
                name: if name.is_empty() { seg.name.clone() } else { name },
                data,
            };
            return Ok((file, seg));
        }

        // Lazy create — still inside the transaction so a concurrent reader
        // either sees no row (and creates it itself behind our COMMIT) or
        // sees this row.
        let id = Id::new();
        let now = Utc::now().to_rfc3339();
        let file_path = format!("world/{}.toml", seg.slug);
        tx.execute(
            "INSERT INTO world_entry (id, segment_id, name, data_json, file_path, file_hash, aliases_json, schema_version, created_at, updated_at)
             VALUES (?1, ?2, ?3, '{}', ?4, '', '[]', ?5, ?6, ?6)",
            (
                id.as_str(),
                segment_id.as_str(),
                &seg.name,
                &file_path,
                &schema_version,
                &now,
            ),
        )?;
        tx.commit()?;

        let file = WorldSingleDocFile {
            id,
            schema_version,
            name: seg.name.clone(),
            data: serde_json::Map::new(),
        };
        Ok((file, seg))
    }

    /// Updates one field in a single-doc segment by dotted path (e.g.
    /// `"main.core_premise"` or `"lists.themes"`). Writes the new TOML to
    /// disk, recomputes `file_hash`, and updates the DB row.
    ///
    /// Transaction shape (mirrors `character::update_field`):
    ///
    /// 1. Read the row (lazily materializing it if missing).
    /// 2. Apply the dotted mutation in-memory.
    /// 3. Render the new TOML and prepare `data_json`.
    /// 4. BEGIN → UPDATE `data_json`/`updated_at` → `fs::write` →
    ///    `hash_file` → UPDATE `file_hash` → COMMIT.
    ///
    /// If `fs::write` fails between the two UPDATEs the transaction rolls
    /// back the `data_json` change, so the DB still mirrors what's on
    /// disk. If `fs::write` succeeds but the COMMIT fails (e.g. connection
    /// drops mid-tx), the file is "ahead" of the DB; the next rebuild
    /// detects this via the `file_hash` mismatch and reconciles.
    ///
    /// # Errors
    /// - [`Error::Other`] if `field_id` is not a well-formed `section.leaf`
    ///   pair, if the section exists with a non-object value, or if
    ///   mkdir/write/hash/serialization fails.
    /// - [`Error::NotFound`] if `segment_id` is unknown.
    pub fn update_single_doc_field(
        &self,
        segment_id: &Id,
        field_id: &str,
        value: &serde_json::Value,
    ) -> Result<()> {
        let (mut file, seg) = self.read_single_doc_with_segment(segment_id)?;
        apply_dotted_mutation(&mut file.data, field_id, value.clone())?;

        let file_path = format!("world/{}.toml", seg.slug);
        let disk_path = self.project_root.join(&file_path);

        if let Some(parent) = disk_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| Error::Other(format!("mkdir {}: {e}", parent.display())))?;
        }
        let toml_text = render_single_doc_toml(&file)?;
        let now = Utc::now().to_rfc3339();
        let data_json = serde_json::Value::Object(file.data.clone()).to_string();

        let tx = self.db.conn().unchecked_transaction()?;

        tx.execute(
            "UPDATE world_entry SET data_json = ?1, updated_at = ?2 WHERE id = ?3",
            (&data_json, &now, file.id.as_str()),
        )?;

        std::fs::write(&disk_path, &toml_text)
            .map_err(|e| Error::Other(format!("write {}: {e}", disk_path.display())))?;

        let hash = crate::scene::hash_file(&disk_path)?;
        tx.execute(
            "UPDATE world_entry SET file_hash = ?1 WHERE id = ?2",
            (&hash, file.id.as_str()),
        )?;

        tx.commit()?;
        Ok(())
    }

    // ----- Collection-entry CRUD (Task 5) -----

    /// Lists entries for a collection segment, ordered by name. Each row
    /// includes a short `preview` derived from `[main]` (first non-empty
    /// string field, truncated to 80 chars).
    pub fn list_entries(&self, segment_id: &Id) -> Result<Vec<WorldEntryIndexRow>> {
        let mut stmt = self.db.conn().prepare(
            "SELECT id, name, data_json FROM world_entry WHERE segment_id = ?1 ORDER BY name",
        )?;
        let rows = stmt.query_map([segment_id.as_str()], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;
        let mut out = Vec::new();
        for r in rows {
            let (id_str, name, data_json) = r?;
            let id = id_str
                .parse::<Id>()
                .map_err(|e| Error::Other(format!("invalid id in world_entry: {e}")))?;
            let data: serde_json::Value = match serde_json::from_str(&data_json) {
                Ok(v) => v,
                Err(e) => {
                    tracing::warn!(
                        target: "water::world",
                        entry_id = %id.as_str(),
                        error = %e,
                        "world_entry.data_json failed to parse; treating as empty for preview"
                    );
                    serde_json::Value::Null
                }
            };
            out.push(WorldEntryIndexRow {
                id,
                segment_id: segment_id.clone(),
                name,
                preview: compute_preview(&data),
            });
        }
        Ok(out)
    }

    /// Reads one collection entry by id.
    ///
    /// # Errors
    /// - [`Error::NotFound`] if `entry_id` is unknown.
    pub fn read_entry(&self, entry_id: &Id) -> Result<WorldEntryFile> {
        let (segment_id_str, name, data_json, aliases_json, schema_version): (
            String,
            String,
            String,
            String,
            String,
        ) = self
            .db
            .conn()
            .query_row(
                "SELECT segment_id, name, data_json, aliases_json, schema_version
                 FROM world_entry WHERE id = ?1",
                [entry_id.as_str()],
                |r| {
                    Ok((
                        r.get(0)?,
                        r.get(1)?,
                        r.get(2)?,
                        r.get(3)?,
                        r.get(4)?,
                    ))
                },
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => {
                    Error::NotFound(format!("world_entry {}", entry_id.as_str()))
                }
                other => Error::from(other),
            })?;
        let segment_id = segment_id_str
            .parse::<Id>()
            .map_err(|e| Error::Other(format!("invalid segment_id in world_entry: {e}")))?;
        let data: serde_json::Map<String, serde_json::Value> =
            match serde_json::from_str(&data_json) {
                Ok(m) => m,
                Err(e) => {
                    tracing::warn!(
                        target: "water::world",
                        entry_id = %entry_id.as_str(),
                        error = %e,
                        "world_entry.data_json failed to parse; treating as empty map"
                    );
                    serde_json::Map::new()
                }
            };
        let aliases: Vec<String> = match serde_json::from_str(&aliases_json) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(
                    target: "water::world",
                    entry_id = %entry_id.as_str(),
                    error = %e,
                    "world_entry.aliases_json failed to parse; treating as empty list"
                );
                Vec::new()
            }
        };
        Ok(WorldEntryFile {
            id: entry_id.clone(),
            segment_id,
            schema_version,
            name,
            aliases,
            data,
        })
    }

    /// Creates a new entry in a collection segment.
    ///
    /// Empty `name` is intentionally allowed — Task 29's Chorus-stub flow
    /// (`create_entry_seeded(&loc.id, "", "main.sensory_detail", &snippet)`)
    /// depends on the empty-name path round-tripping cleanly so the orphan
    /// reaper can later collect abandoned stubs.
    ///
    /// # Errors
    /// - [`Error::Other`] if the segment is not a collection.
    /// - [`Error::NotFound`] if `segment_id` is unknown.
    pub fn create_entry(&self, segment_id: &Id, name: &str) -> Result<Id> {
        let seg = self.read_segment(segment_id)?;
        if !seg.is_collection {
            return Err(Error::Other(format!(
                "segment {} is not a collection; cannot create entry",
                seg.slug
            )));
        }
        let id = Id::new();
        let now = Utc::now().to_rfc3339();
        let file_path = format!("world/{}/{}.toml", seg.slug, id.as_str());
        let schema_version = format!("{}@1", seg.slug);
        self.db.conn().execute(
            "INSERT INTO world_entry
             (id, segment_id, name, data_json, file_path, file_hash, aliases_json, schema_version, created_at, updated_at)
             VALUES (?1, ?2, ?3, '{}', ?4, '', '[]', ?5, ?6, ?6)",
            (
                id.as_str(),
                segment_id.as_str(),
                name,
                &file_path,
                &schema_version,
                &now,
            ),
        )?;
        Ok(id)
    }

    /// Creates an entry and immediately writes one field's seed value.
    /// Used by the Chorus-pin stub handler (Task 29) which materializes a
    /// draft entry from a generated snippet.
    ///
    /// If the seed write fails (e.g. invalid `seed_field_id`), the entry
    /// row created by the initial `create_entry` is rolled back via a
    /// best-effort `delete_entry` so no orphan persists. If the cleanup
    /// itself fails the original seed-write error is still returned (the
    /// seed failure is the user-visible cause); the cleanup failure is
    /// surfaced via `tracing::warn!`.
    pub fn create_entry_seeded(
        &self,
        segment_id: &Id,
        name: &str,
        seed_field_id: &str,
        seed_value: &str,
    ) -> Result<Id> {
        let id = self.create_entry(segment_id, name)?;
        match self.update_entry_field(
            &id,
            seed_field_id,
            &serde_json::Value::String(seed_value.to_string()),
        ) {
            Ok(()) => Ok(id),
            Err(e) => {
                if let Err(cleanup_err) = self.delete_entry(&id) {
                    tracing::warn!(
                        target: "water::world",
                        entry_id = %id.as_str(),
                        cleanup_error = %cleanup_err,
                        original_error = %e,
                        "create_entry_seeded: failed to delete orphan after seed-write failure"
                    );
                }
                Err(e)
            }
        }
    }

    /// Updates one field in a collection entry by dotted `section.leaf`
    /// path. If `field_id == "main.name"`, the entry's `name` column is
    /// also updated (rename-cascade). Non-string values for `main.name`
    /// are rejected up front so the cascade can never produce a bad row.
    ///
    /// Transaction shape mirrors [`Self::update_single_doc_field`]:
    /// `BEGIN` → `UPDATE data_json` (+ optionally `name`) + `updated_at` →
    /// `fs::write` → `hash_file` → `UPDATE file_hash` → `COMMIT`.
    ///
    /// # Panics
    /// Will not panic in practice: the only `expect` is gated behind the
    /// `main.name` `is_string` guard at the top of the function.
    ///
    /// # Errors
    /// - [`Error::Other`] if `field_id == "main.name"` and `value` is not a
    ///   JSON string; if `field_id` fails dotted-path validation; if
    ///   mkdir/write/hash/serialization fails.
    /// - [`Error::NotFound`] if `entry_id` is unknown.
    pub fn update_entry_field(
        &self,
        entry_id: &Id,
        field_id: &str,
        value: &serde_json::Value,
    ) -> Result<()> {
        // Rename-cascade guard — checked BEFORE any DB work so a bad call
        // never leaves a half-mutated state.
        if field_id == "main.name" && !value.is_string() {
            return Err(Error::Other(
                "main.name must be a string (rename-cascade guard)".to_string(),
            ));
        }

        let mut file = self.read_entry(entry_id)?;
        apply_dotted_mutation(&mut file.data, field_id, value.clone())?;

        let seg = self.read_segment(&file.segment_id)?;
        let file_path = format!("world/{}/{}.toml", seg.slug, entry_id.as_str());
        let disk_path = self.project_root.join(&file_path);

        let new_name = if field_id == "main.name" {
            // Guarded above — safe to unwrap.
            let s = value
                .as_str()
                .expect("main.name string guard checked above")
                .to_string();
            file.name.clone_from(&s);
            Some(s)
        } else {
            None
        };

        if let Some(parent) = disk_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| Error::Other(format!("mkdir {}: {e}", parent.display())))?;
        }
        let toml_text = render_entry_toml(&file)?;
        let now = Utc::now().to_rfc3339();
        let data_json = serde_json::Value::Object(file.data.clone()).to_string();

        let tx = self.db.conn().unchecked_transaction()?;

        if let Some(name) = &new_name {
            tx.execute(
                "UPDATE world_entry SET data_json = ?1, name = ?2, updated_at = ?3 WHERE id = ?4",
                (&data_json, name, &now, entry_id.as_str()),
            )?;
        } else {
            tx.execute(
                "UPDATE world_entry SET data_json = ?1, updated_at = ?2 WHERE id = ?3",
                (&data_json, &now, entry_id.as_str()),
            )?;
        }

        std::fs::write(&disk_path, &toml_text)
            .map_err(|e| Error::Other(format!("write {}: {e}", disk_path.display())))?;

        let hash = crate::scene::hash_file(&disk_path)?;
        tx.execute(
            "UPDATE world_entry SET file_hash = ?1 WHERE id = ?2",
            (&hash, entry_id.as_str()),
        )?;

        tx.commit()?;
        Ok(())
    }

    /// Replaces the entry's alias list (DB + disk) using the same
    /// transactional pattern as [`Self::update_entry_field`].
    pub fn update_entry_aliases(&self, entry_id: &Id, aliases: &[String]) -> Result<()> {
        let mut file = self.read_entry(entry_id)?;
        file.aliases = aliases.to_vec();

        let seg = self.read_segment(&file.segment_id)?;
        let file_path = format!("world/{}/{}.toml", seg.slug, entry_id.as_str());
        let disk_path = self.project_root.join(&file_path);

        if let Some(parent) = disk_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| Error::Other(format!("mkdir {}: {e}", parent.display())))?;
        }
        let toml_text = render_entry_toml(&file)?;
        let now = Utc::now().to_rfc3339();
        let aliases_json = serde_json::to_string(aliases)?;

        let tx = self.db.conn().unchecked_transaction()?;
        tx.execute(
            "UPDATE world_entry SET aliases_json = ?1, updated_at = ?2 WHERE id = ?3",
            (&aliases_json, &now, entry_id.as_str()),
        )?;
        std::fs::write(&disk_path, &toml_text)
            .map_err(|e| Error::Other(format!("write {}: {e}", disk_path.display())))?;
        let hash = crate::scene::hash_file(&disk_path)?;
        tx.execute(
            "UPDATE world_entry SET file_hash = ?1 WHERE id = ?2",
            (&hash, entry_id.as_str()),
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Deletes a collection entry — removes the DB row and (if present)
    /// the on-disk TOML file.
    ///
    /// # Errors
    /// - [`Error::NotFound`] if `entry_id` is unknown.
    pub fn delete_entry(&self, entry_id: &Id) -> Result<()> {
        let file_path: String = self
            .db
            .conn()
            .query_row(
                "SELECT file_path FROM world_entry WHERE id = ?1",
                [entry_id.as_str()],
                |r| r.get(0),
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => {
                    Error::NotFound(format!("world_entry {}", entry_id.as_str()))
                }
                other => Error::from(other),
            })?;
        let disk_path = self.project_root.join(&file_path);
        if disk_path.exists() {
            std::fs::remove_file(&disk_path)
                .map_err(|e| Error::Other(format!("remove {}: {e}", disk_path.display())))?;
        }
        self.db.conn().execute(
            "DELETE FROM world_entry WHERE id = ?1",
            [entry_id.as_str()],
        )?;
        Ok(())
    }

    /// Orphan-draft reaper: deletes the entry iff its `name`, `aliases`,
    /// and every section in `data` are all empty. Returns `true` if the
    /// entry was deleted, `false` if it still contained content.
    ///
    /// Wraps [`Self::delete_entry`] for the actual removal.
    ///
    /// # Errors
    /// - [`Error::NotFound`] if `entry_id` is unknown.
    pub fn delete_entry_if_empty(&self, entry_id: &Id) -> Result<bool> {
        let file = self.read_entry(entry_id)?;
        if is_world_entry_empty(&file) {
            self.delete_entry(entry_id)?;
            return Ok(true);
        }
        Ok(false)
    }
}

/// Applies a dotted `section.leaf` mutation against a single-doc data map,
/// creating the section object if it doesn't exist yet. Module-local so
/// Task 5 can reuse it for collection entries.
///
/// Strictly validates `field_id`: it must contain exactly one `.`, with
/// non-empty `section` and `leaf` parts. Multi-segment paths (e.g.
/// `"main.a.b"`) are rejected so we don't silently produce keys with dots
/// in their name.
fn apply_dotted_mutation(
    data: &mut serde_json::Map<String, serde_json::Value>,
    field_id: &str,
    value: serde_json::Value,
) -> Result<()> {
    let (section, leaf) = field_id
        .split_once('.')
        .ok_or_else(|| Error::Other(format!("field_id '{field_id}' is not dotted")))?;
    if section.is_empty() || leaf.is_empty() || leaf.contains('.') {
        return Err(Error::Other(format!(
            "field_id '{field_id}' must be 'section.leaf' with no further dots"
        )));
    }
    let section_obj = data
        .entry(section.to_string())
        .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
    let map = section_obj
        .as_object_mut()
        .ok_or_else(|| Error::Other(format!("section '{section}' is not an object")))?;
    map.insert(leaf.to_string(), value);
    Ok(())
}

/// Serializes a `WorldSingleDocFile` to pretty TOML.
fn render_single_doc_toml(file: &WorldSingleDocFile) -> Result<String> {
    toml::to_string_pretty(file).map_err(|e| Error::Other(format!("toml render: {e}")))
}

/// Serializes a `WorldEntryFile` to pretty TOML.
fn render_entry_toml(file: &WorldEntryFile) -> Result<String> {
    toml::to_string_pretty(file).map_err(|e| Error::Other(format!("toml render: {e}")))
}

/// Picks the first non-empty string in `data["main"]` (if any) and
/// truncates to 80 chars. Falls back to an empty string when there's
/// nothing presentable.
fn compute_preview(data: &serde_json::Value) -> String {
    if let Some(main) = data.get("main").and_then(serde_json::Value::as_object) {
        for v in main.values() {
            if let Some(s) = v.as_str() {
                if !s.trim().is_empty() {
                    return s.chars().take(80).collect();
                }
            }
        }
    }
    String::new()
}

/// True iff the entry has no name, no aliases, and every section in
/// `data` is recursively empty. Used by [`WorldStore::delete_entry_if_empty`].
fn is_world_entry_empty(file: &WorldEntryFile) -> bool {
    if !file.name.is_empty() {
        return false;
    }
    if !file.aliases.is_empty() {
        return false;
    }
    file.data.values().all(is_value_empty)
}

/// Recursive "is this JSON value content-free?" predicate. Numbers and
/// bools always count as non-empty (an explicit `false` or `0` is still
/// authored content); strings, arrays, and objects empty out by
/// shape/contents.
fn is_value_empty(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Null => true,
        serde_json::Value::String(s) => s.is_empty(),
        serde_json::Value::Array(a) => a.is_empty() || a.iter().all(is_value_empty),
        serde_json::Value::Object(m) => m.is_empty() || m.values().all(is_value_empty),
        serde_json::Value::Bool(_) | serde_json::Value::Number(_) => false,
    }
}
