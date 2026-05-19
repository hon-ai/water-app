//! `CharacterStore` — `TOML` on disk + `SQLite` index.
//!
//! M1 introduced `upsert/list/delete` for full-file overwrites. M3 adds
//! a richer surface used by the Tauri command layer:
//!
//! * [`CharacterStore::create`] — create an empty character with a
//!   pre-allocated hue token.
//! * [`CharacterStore::read`] — read the on-disk `.toml` into a
//!   `CharacterFile`.
//! * [`CharacterStore::list_index`] — flat list used by the index panel
//!   (carries `hue_token` + decoded `data_json` so the renderer can show
//!   role + completion).
//! * [`CharacterStore::update_field`] — per-field update for the
//!   Conversational Intake flow. Handles the `main.full_name` rename
//!   cascade into `main.aliases`. Runs inside a `SQLite` transaction so
//!   the on-disk write only sticks when the DB row also lands.
//! * [`CharacterStore::delete_and_cascade`] — soft delete: move `.toml`
//!   to `characters/.trash/` and cascade through
//!   `scene_character_presence` + null `scene.pov_character_id`.
//!
//! `next_hue_token` is exposed at module level so the command layer can
//! call it without instantiating a store.

pub mod autosuggest;
pub mod intake;
pub mod registry;

pub use autosuggest::{suggest_for_scene_body, AutosuggestResult, AutosuggestRow};
pub use intake::{completion_pct, REQUIRED_FIELD_IDS};
pub use registry::{CharacterRegistry, CharacterRegistryRow};

use crate::{Db, Error, Id, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

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

/// Flat row produced by [`CharacterStore::list_index`] — carries every
/// field the renderer's index view needs without a second round trip.
///
/// `role` is extracted from `data_json.main.role_in_story` at query time
/// (it's not a separate column). `data_json` is preserved so the command
/// layer can compute [`completion_pct`] without re-decoding.
#[derive(Debug, Clone)]
pub struct CharacterIndexRow {
    pub id: Id,
    pub full_name: String,
    pub role: Option<String>,
    pub hue_token: String,
    pub data_json: Value,
}

/// Input to [`CharacterStore::create`]. The hue is pre-allocated by the
/// caller via [`next_hue_token`] so the round-robin policy stays out of
/// the store.
#[derive(Debug, Clone)]
pub struct NewCharacter {
    pub project_id: Id,
    pub hue_token: String,
}

/// Round-robin hue allocator: returns the `--water-hue-character-N` token
/// (N in 1..=6) currently used by the fewest characters. Ties broken by
/// lowest index, so a freshly seeded project gets `1, 2, 3, ...` in order.
///
/// On any row with an unrecognized `hue_token` (e.g. legacy data) we simply
/// don't count it; the next allocation still goes to the genuinely-empty
/// bucket.
pub fn next_hue_token(db: &Db) -> Result<String> {
    let mut stmt = db
        .conn()
        .prepare("SELECT hue_token, COUNT(*) FROM character GROUP BY hue_token")?;
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?
        .collect::<std::result::Result<Vec<(String, i64)>, _>>()?;

    let mut by_index: [i64; 6] = [0; 6];
    for (token, count) in &rows {
        for i in 1..=6 {
            if token == &format!("--water-hue-character-{i}") {
                by_index[i - 1] = *count;
                break;
            }
        }
    }
    let min_index = by_index
        .iter()
        .enumerate()
        .min_by_key(|(_, c)| **c)
        .map_or(0, |(i, _)| i);
    Ok(format!("--water-hue-character-{}", min_index + 1))
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

    fn trash_dir(&self) -> PathBuf {
        self.dir().join(".trash")
    }

    /// Create a new (empty) character. The on-disk file is `characters/<id>.toml`
    /// with `schema_version = "lsm-v2.1"` and an empty `data` table. The DB
    /// row carries the same `data_json` ({}) plus the caller-supplied
    /// `hue_token`.
    ///
    /// The on-disk write happens BEFORE the DB insert. Then the DB insert
    /// is wrapped in `unchecked_transaction` so a rollback (e.g. unique
    /// constraint) cleans up the orphaned file. On the happy path the
    /// transaction commits and the file stays.
    pub fn create(&self, input: NewCharacter) -> Result<CharacterIndexRow> {
        std::fs::create_dir_all(self.dir())?;
        let id = Id::new();
        let path = self.dir().join(format!("{id}.toml"));
        let file = CharacterFile {
            id: id.clone(),
            name: String::new(),
            schema_version: "lsm-v2.1".into(),
            data: toml::Table::new(),
        };
        let text = toml::to_string_pretty(&file)?;
        std::fs::write(&path, text)?;
        let hash = crate::scene::hash_file(&path)?;
        let now = Utc::now().to_rfc3339();
        let data_json = "{}".to_string();

        let tx = self.db.conn().unchecked_transaction()?;
        let insert_result = tx.execute(
            "INSERT INTO character
             (id, project_id, name, schema_version, data_json, hue_token, file_path, file_hash, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)",
            (
                id.as_str(),
                input.project_id.as_str(),
                "",
                "lsm-v2.1",
                &data_json,
                &input.hue_token,
                path.to_string_lossy(),
                &hash,
                &now,
            ),
        );
        match insert_result {
            Ok(_) => {
                tx.commit()?;
                Ok(CharacterIndexRow {
                    id,
                    full_name: String::new(),
                    role: None,
                    hue_token: input.hue_token,
                    data_json: Value::Object(serde_json::Map::new()),
                })
            }
            Err(e) => {
                // Drop the orphaned file. Errors here are swallowed: we're
                // already in the error path; a leftover empty .toml is
                // surveyable and the rebuild path tolerates extras.
                let _ = std::fs::remove_file(&path);
                Err(e.into())
            }
        }
    }

    /// Read the on-disk `.toml` for a character. The DB is consulted only
    /// to resolve `id -> file_path`; everything else comes from disk.
    pub fn read(&self, id: &Id) -> Result<CharacterFile> {
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
        let text = std::fs::read_to_string(&path)?;
        let file: CharacterFile = toml::from_str(&text)?;
        Ok(file)
    }

    pub fn upsert(&self, project_id: &Id, file: CharacterFile) -> Result<CharacterRow> {
        std::fs::create_dir_all(self.dir())?;
        let path = self.dir().join(format!("{}.toml", file.id));
        let text = toml::to_string_pretty(&file)?;
        std::fs::write(&path, text)?;
        let hash = crate::scene::hash_file(&path)?;
        let now = Utc::now().to_rfc3339();

        let data_json = serde_json::to_string(&file.data)?;
        // ON CONFLICT must not clobber `hue_token` — once allocated it's
        // stable for the character's lifetime. Pre-M3 rows that came
        // through the v3 backfill have a populated hue; upsert leaves it
        // alone. New characters should go through `create`, which sets
        // hue_token explicitly; this upsert path keeps a default of
        // `--water-hue-character-1` only when inserting from a context
        // (test / rebuild) that hasn't allocated one.
        self.db.conn().execute(
            "INSERT INTO character (id, project_id, name, schema_version, data_json, hue_token, file_path, file_hash, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, COALESCE((SELECT hue_token FROM character WHERE id = ?1), '--water-hue-character-1'), ?6, ?7, ?8, ?8)
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

    /// Flat list for the index view. Ordered by `name COLLATE NOCASE` so
    /// the renderer doesn't need to re-sort.
    ///
    /// `role` is extracted from `data_json.main.role_in_story` at the call
    /// site so that the schema doesn't need a separate column. Missing /
    /// non-string values resolve to `None`.
    pub fn list_index(db: &Db) -> Result<Vec<CharacterIndexRow>> {
        let mut stmt = db.conn().prepare(
            "SELECT id, name, hue_token, data_json FROM character ORDER BY name COLLATE NOCASE",
        )?;
        let rows = stmt.query_map([], |row| {
            let id_s: String = row.get(0)?;
            let id = id_s.parse::<Id>().map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?;
            let full_name: String = row.get(1)?;
            let hue_token: String = row.get(2)?;
            let data_json_str: String = row.get(3)?;
            let data_json: Value = serde_json::from_str(&data_json_str).unwrap_or(Value::Null);
            let role = data_json
                .pointer("/main/role_in_story")
                .and_then(|v| v.as_str())
                .map(str::to_string);
            Ok(CharacterIndexRow {
                id,
                full_name,
                role,
                hue_token,
                data_json,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    /// Slim listing for the scene-character autosuggest scanner.
    /// Returns every character with its `full_name` (from
    /// `data_json.main.full_name`, falling back to the `name` SQL column
    /// when the JSON field is absent) and its aliases (string entries in
    /// `data_json.main.aliases`; non-string entries are dropped, empty
    /// strings filtered).
    ///
    /// Characters whose `full_name` AND aliases are all empty are
    /// excluded — they'd contribute zero matches and just inflate the
    /// scanner's iteration count.
    ///
    /// Mirrors the SQL-then-collect pattern used by [`Self::list_index`]
    /// so the `rusqlite::Statement` (which is `!Send`) doesn't cross any
    /// `.await` point.
    pub fn list_all_with_aliases(db: &Db) -> Result<Vec<AutosuggestRow>> {
        let mut stmt = db
            .conn()
            .prepare("SELECT id, name, data_json FROM character")?;
        let rows = stmt.query_map([], |row| {
            let id_s: String = row.get(0)?;
            let id = id_s.parse::<Id>().map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?;
            let name_col: String = row.get(1)?;
            let data_json_str: String = row.get(2)?;
            let data_json: Value = serde_json::from_str(&data_json_str).unwrap_or(Value::Null);
            let full_name = data_json
                .pointer("/main/full_name")
                .and_then(|v| v.as_str())
                .map_or(name_col, str::to_string);
            let aliases: Vec<String> = data_json
                .pointer("/main/aliases")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str())
                        .filter(|s| !s.is_empty())
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default();
            Ok(AutosuggestRow {
                character_id: id,
                full_name,
                aliases,
            })
        })?;
        let collected = rows.collect::<std::result::Result<Vec<_>, _>>()?;
        // Filter out rows that would contribute zero matches anyway.
        Ok(collected
            .into_iter()
            .filter(|r| !r.full_name.is_empty() || !r.aliases.is_empty())
            .collect())
    }

    /// Update a single field on a character. `field_id` is the dotted-path
    /// LSM v2.1 id (e.g. `"main.full_name"`), resolved to the JSON pointer
    /// `/main/full_name`. Intermediate objects are created as needed.
    ///
    /// **Rename cascade.** When `field_id == "main.full_name"` and the
    /// new string differs from the existing value, the old name is
    /// appended (de-duped, case-sensitive) to `main.aliases`. This matches
    /// spec § 20.
    ///
    /// The DB update + on-disk re-serialization run inside a single
    /// `SQLite` transaction. The on-disk write happens *after* the DB UPDATE
    /// succeeds but *before* the COMMIT — if the write fails, the
    /// transaction is rolled back (so the DB stays consistent with the
    /// pre-write on-disk content).
    ///
    /// # Errors
    /// Returns [`Error::NotFound`] when `id` doesn't match any character row,
    /// [`Error::Other`] when `field_id` is empty or attempts to descend
    /// through a non-object value, plus any IO/SQLite/JSON/TOML error that
    /// bubbles up from the underlying read/write/serialize pipeline.
    #[allow(clippy::too_many_lines)] // pipeline is naturally linear; splitting
                                      // would just move the same code into
                                      // helpers that aren't reused.
    pub fn update_field(
        &self,
        id: &Id,
        field_id: &str,
        value: &Value,
    ) -> Result<CharacterIndexRow> {
        let (file_path_s, data_json_str, hue_token, schema_version): (
            String,
            String,
            String,
            String,
        ) = self
            .db
            .conn()
            .query_row(
                "SELECT file_path, data_json, hue_token, schema_version FROM character WHERE id = ?1",
                [id.as_str()],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Error::NotFound(format!("character {id}")),
                other => Error::from(other),
            })?;
        let mut data_json: Value =
            serde_json::from_str(&data_json_str).unwrap_or_else(|_| Value::Object(serde_json::Map::new()));
        if !data_json.is_object() {
            data_json = Value::Object(serde_json::Map::new());
        }

        // Resolve dotted path -> JSON pointer; create intermediate objects.
        let segments: Vec<&str> = field_id.split('.').collect();
        if segments.is_empty() || segments.iter().any(|s| s.is_empty()) {
            return Err(Error::Other("empty field_id segment".into()));
        }
        let is_rename = field_id == "main.full_name";
        // Capture the OLD value before mutation so the cascade can compare.
        let old_full_name: Option<String> = data_json
            .pointer("/main/full_name")
            .and_then(|v| v.as_str())
            .map(str::to_string);

        // Walk + create missing intermediates. We use a two-pass strategy
        // (ensure-intermediates, then JSON-pointer-style set) because the
        // single-pass `cursor` walk runs afoul of NLL borrow-tracking when
        // the same `&mut` is re-projected through a loop body.
        ensure_path_objects(&mut data_json, &segments)?;
        {
            let last_seg = *segments
                .last()
                .ok_or_else(|| Error::Other("empty field_id".into()))?;
            let obj_ref = walk_to_parent_mut(&mut data_json, &segments)
                .ok_or_else(|| Error::Other(format!("field_id {field_id}: traversal failed")))?;
            obj_ref.insert(last_seg.to_string(), value.clone());
        }

        // Rename cascade: append OLD full_name to main.aliases if changed.
        let new_full_name_str = if is_rename {
            value.as_str().map(str::to_string)
        } else {
            None
        };
        if is_rename {
            if let (Some(old), Some(new)) = (&old_full_name, &new_full_name_str) {
                if !old.is_empty() && old != new {
                    let main = data_json
                        .as_object_mut()
                        .and_then(|o| o.get_mut("main"))
                        .and_then(Value::as_object_mut);
                    if let Some(main_obj) = main {
                        let aliases_entry = main_obj
                            .entry("aliases".to_string())
                            .or_insert_with(|| Value::Array(Vec::new()));
                        if let Some(arr) = aliases_entry.as_array_mut() {
                            let already_present = arr
                                .iter()
                                .any(|v| v.as_str().is_some_and(|s| s == old.as_str()));
                            if !already_present {
                                arr.push(Value::String(old.clone()));
                            }
                        }
                    }
                }
            }
        }

        // Determine the new `name` column value (mirrors main.full_name).
        let new_name_col: String = data_json
            .pointer("/main/full_name")
            .and_then(|v| v.as_str())
            .map(str::to_string)
            .unwrap_or_default();

        // Serialize both projections.
        let data_json_str = serde_json::to_string(&data_json)?;
        // The on-disk TOML uses CharacterFile's `#[serde(flatten)]` for
        // data, so we round-trip through the same struct used by `read`.
        // Convert serde_json::Value -> toml::Table by writing JSON and
        // parsing as TOML via a `serde_json::Value` -> `serde_json::Map` ->
        // `toml::Value` adapter. The simplest reliable conversion is to
        // re-serialize through `serde_json::to_value` -> ad hoc map walk.
        let data_table = json_value_to_toml_table(&data_json)?;
        let file = CharacterFile {
            id: id.clone(),
            name: new_name_col.clone(),
            schema_version: schema_version.clone(),
            data: data_table,
        };
        let toml_text = toml::to_string_pretty(&file)?;
        let now = Utc::now().to_rfc3339();

        let tx = self.db.conn().unchecked_transaction()?;
        tx.execute(
            "UPDATE character SET name = ?1, data_json = ?2, updated_at = ?3 WHERE id = ?4",
            (&new_name_col, &data_json_str, &now, id.as_str()),
        )?;
        // Write the file BEFORE COMMIT so disk failure rolls the DB back.
        std::fs::write(Path::new(&file_path_s), &toml_text)?;
        let hash = crate::scene::hash_file(Path::new(&file_path_s))?;
        tx.execute(
            "UPDATE character SET file_hash = ?1 WHERE id = ?2",
            (&hash, id.as_str()),
        )?;
        tx.commit()?;

        let role = data_json
            .pointer("/main/role_in_story")
            .and_then(|v| v.as_str())
            .map(str::to_string);
        Ok(CharacterIndexRow {
            id: id.clone(),
            full_name: new_name_col,
            role,
            hue_token,
            data_json,
        })
    }

    /// Soft delete: cascade through `scene_character_presence` (DELETE)
    /// and `scene.pov_character_id` (NULL), then move the `.toml` to
    /// `characters/.trash/<id>-<unix-ts>.toml`, then DELETE the character
    /// row. All DB changes run inside a single transaction.
    ///
    /// A missing/permission-denied file move is logged at warn level and
    /// does NOT abort the cascade — the DB delete still proceeds and the
    /// orphan toml (if any) can be hand-cleaned.
    pub fn delete_and_cascade(&self, id: &Id) -> Result<()> {
        let file_path_s: String = self
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

        let tx = self.db.conn().unchecked_transaction()?;
        tx.execute(
            "DELETE FROM scene_character_presence WHERE character_id = ?1",
            [id.as_str()],
        )?;
        tx.execute(
            "UPDATE scene SET pov_character_id = NULL WHERE pov_character_id = ?1",
            [id.as_str()],
        )?;
        tx.execute("DELETE FROM character WHERE id = ?1", [id.as_str()])?;

        // Move the .toml. This happens before commit so a failure to move
        // doesn't leave the DB inconsistent with disk — but we explicitly
        // tolerate "file already gone" since the rebuild path is the
        // ultimate source of truth.
        let src = Path::new(&file_path_s);
        if src.exists() {
            let trash = self.trash_dir();
            if let Err(e) = std::fs::create_dir_all(&trash) {
                tracing::warn!(error = %e, "could not create characters/.trash; skipping move");
            } else {
                let ts = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map_or(0, |d| d.as_secs());
                let dst = trash.join(format!("{id}-{ts}.toml"));
                if let Err(e) = std::fs::rename(src, &dst) {
                    tracing::warn!(error = %e, src = %src.display(), "failed to move character toml to .trash");
                }
            }
        }
        tx.commit()?;
        Ok(())
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

/// Ensure that the path described by `segments[..segments.len()-1]` exists
/// as nested objects inside `root`. Used by `update_field` so that
/// `walk_to_parent_mut` can rely on every intermediate being an object.
fn ensure_path_objects(root: &mut Value, segments: &[&str]) -> Result<()> {
    if segments.len() < 2 {
        return Ok(());
    }
    let parents = &segments[..segments.len() - 1];
    let mut cur = root;
    for seg in parents {
        let obj = cur.as_object_mut().ok_or_else(|| {
            Error::Other(format!("path segment `{seg}`: parent is not an object"))
        })?;
        if !obj.get(*seg).is_some_and(Value::is_object) {
            obj.insert((*seg).to_string(), Value::Object(serde_json::Map::new()));
        }
        cur = obj.get_mut(*seg).expect("just inserted as object");
    }
    Ok(())
}

/// Return a mutable reference to the parent object of `segments.last()`.
/// Assumes [`ensure_path_objects`] has already made every intermediate
/// segment a JSON object.
fn walk_to_parent_mut<'a>(
    root: &'a mut Value,
    segments: &[&str],
) -> Option<&'a mut serde_json::Map<String, Value>> {
    if segments.is_empty() {
        return None;
    }
    let parents = &segments[..segments.len() - 1];
    let mut cur = root;
    for seg in parents {
        cur = cur.as_object_mut()?.get_mut(*seg)?;
    }
    cur.as_object_mut()
}

/// Convert a `serde_json::Value` (which must be an object) to a
/// `toml::Table`. Used by `update_field` to round-trip the JSON-shaped
/// `data_json` into the TOML on-disk projection without a custom adapter.
///
/// Goes through a JSON string then a TOML-typed `serde_json::Value` mapping
/// — simple and correct for the LSM v2.1 shape (no Int64/Float64 quirks
/// because the schema is strings + arrays of strings).
fn json_value_to_toml_table(v: &Value) -> Result<toml::Table> {
    fn convert(v: &Value) -> Result<toml::Value> {
        Ok(match v {
            Value::Null => toml::Value::String(String::new()),
            Value::Bool(b) => toml::Value::Boolean(*b),
            Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    toml::Value::Integer(i)
                } else if let Some(f) = n.as_f64() {
                    toml::Value::Float(f)
                } else {
                    return Err(Error::Other(format!("unsupported number in JSON: {n}")));
                }
            }
            Value::String(s) => toml::Value::String(s.clone()),
            Value::Array(arr) => {
                let items: Result<Vec<toml::Value>> = arr.iter().map(convert).collect();
                toml::Value::Array(items?)
            }
            Value::Object(obj) => {
                let mut t = toml::Table::new();
                for (k, vv) in obj {
                    t.insert(k.clone(), convert(vv)?);
                }
                toml::Value::Table(t)
            }
        })
    }
    match v {
        Value::Object(obj) => {
            let mut t = toml::Table::new();
            for (k, vv) in obj {
                t.insert(k.clone(), convert(vv)?);
            }
            Ok(t)
        }
        _ => Err(Error::Other("expected JSON object at top level".into())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ProjectStore;
    use serde_json::json;

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

    // --- M3 T12 additions ---

    #[test]
    fn next_hue_token_empty_db_returns_character_1() {
        let (_dir, db, _) = setup();
        assert_eq!(next_hue_token(&db).unwrap(), "--water-hue-character-1");
    }

    #[test]
    fn create_assigns_round_robin_hues() {
        let (dir, db, p_id) = setup();
        let store = CharacterStore::new(&db, dir.path().to_path_buf());
        let mut hues = Vec::new();
        for _ in 0..3 {
            let hue = next_hue_token(&db).unwrap();
            let row = store
                .create(NewCharacter {
                    project_id: p_id.clone(),
                    hue_token: hue.clone(),
                })
                .unwrap();
            hues.push(row.hue_token);
        }
        assert_eq!(hues[0], "--water-hue-character-1");
        assert_eq!(hues[1], "--water-hue-character-2");
        assert_eq!(hues[2], "--water-hue-character-3");
    }

    #[test]
    fn create_writes_file_and_db_row() {
        let (dir, db, p_id) = setup();
        let store = CharacterStore::new(&db, dir.path().to_path_buf());
        let row = store
            .create(NewCharacter {
                project_id: p_id,
                hue_token: "--water-hue-character-1".into(),
            })
            .unwrap();
        // File exists and round-trips through read().
        let file = store.read(&row.id).unwrap();
        assert_eq!(file.schema_version, "lsm-v2.1");
        assert_eq!(file.name, "");
    }

    #[test]
    fn list_index_returns_hue_and_role() {
        let (dir, db, p_id) = setup();
        let store = CharacterStore::new(&db, dir.path().to_path_buf());
        let row = store
            .create(NewCharacter {
                project_id: p_id,
                hue_token: "--water-hue-character-2".into(),
            })
            .unwrap();
        store
            .update_field(&row.id, "main.role_in_story", &json!("protagonist"))
            .unwrap();
        store
            .update_field(&row.id, "main.full_name", &json!("Marcus"))
            .unwrap();
        let rows = CharacterStore::list_index(&db).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].hue_token, "--water-hue-character-2");
        assert_eq!(rows[0].role.as_deref(), Some("protagonist"));
        assert_eq!(rows[0].full_name, "Marcus");
    }

    #[test]
    fn update_field_sets_simple_value_and_persists_to_disk() {
        let (dir, db, p_id) = setup();
        let store = CharacterStore::new(&db, dir.path().to_path_buf());
        let row = store
            .create(NewCharacter {
                project_id: p_id,
                hue_token: "--water-hue-character-1".into(),
            })
            .unwrap();
        store
            .update_field(&row.id, "main.want", &json!("freedom"))
            .unwrap();
        // On-disk: re-read the TOML directly.
        let file = store.read(&row.id).unwrap();
        let want = file
            .data
            .get("main")
            .and_then(|m| m.as_table())
            .and_then(|m| m.get("want"))
            .and_then(|v| v.as_str())
            .unwrap();
        assert_eq!(want, "freedom");
    }

    #[test]
    fn update_field_full_name_appends_old_to_aliases_on_rename() {
        let (dir, db, p_id) = setup();
        let store = CharacterStore::new(&db, dir.path().to_path_buf());
        let row = store
            .create(NewCharacter {
                project_id: p_id,
                hue_token: "--water-hue-character-1".into(),
            })
            .unwrap();
        // First set: no cascade (old is empty).
        store
            .update_field(&row.id, "main.full_name", &json!("Marcus Vale"))
            .unwrap();
        // Rename: cascade should push "Marcus Vale" into aliases.
        store
            .update_field(&row.id, "main.full_name", &json!("Marcus Tenebris"))
            .unwrap();
        let file = store.read(&row.id).unwrap();
        let aliases = file
            .data
            .get("main")
            .and_then(|m| m.as_table())
            .and_then(|m| m.get("aliases"))
            .and_then(|v| v.as_array())
            .unwrap();
        let names: Vec<&str> = aliases.iter().filter_map(|v| v.as_str()).collect();
        assert!(names.contains(&"Marcus Vale"), "got: {names:?}");
        // Rename back to a name already in aliases must NOT duplicate.
        store
            .update_field(&row.id, "main.full_name", &json!("Marcus Vale"))
            .unwrap();
        let file2 = store.read(&row.id).unwrap();
        let aliases2 = file2
            .data
            .get("main")
            .and_then(|m| m.as_table())
            .and_then(|m| m.get("aliases"))
            .and_then(|v| v.as_array())
            .unwrap();
        let count_vale = aliases2
            .iter()
            .filter(|v| v.as_str() == Some("Marcus Vale"))
            .count();
        // "Marcus Tenebris" was just renamed into aliases on this last
        // step; "Marcus Vale" must appear at most once.
        assert!(count_vale <= 1);
    }

    #[test]
    fn update_field_no_cascade_when_full_name_unchanged() {
        let (dir, db, p_id) = setup();
        let store = CharacterStore::new(&db, dir.path().to_path_buf());
        let row = store
            .create(NewCharacter {
                project_id: p_id,
                hue_token: "--water-hue-character-1".into(),
            })
            .unwrap();
        store
            .update_field(&row.id, "main.full_name", &json!("Marcus"))
            .unwrap();
        store
            .update_field(&row.id, "main.full_name", &json!("Marcus"))
            .unwrap();
        let file = store.read(&row.id).unwrap();
        let aliases = file
            .data
            .get("main")
            .and_then(|m| m.as_table())
            .and_then(|m| m.get("aliases"));
        // Either aliases is absent or it's an empty array.
        match aliases {
            None => {}
            Some(toml::Value::Array(a)) => assert!(a.is_empty()),
            Some(other) => panic!("unexpected aliases shape: {other:?}"),
        }
    }

    #[test]
    fn update_field_updates_db_name_column_when_full_name_changes() {
        let (dir, db, p_id) = setup();
        let store = CharacterStore::new(&db, dir.path().to_path_buf());
        let row = store
            .create(NewCharacter {
                project_id: p_id,
                hue_token: "--water-hue-character-1".into(),
            })
            .unwrap();
        store
            .update_field(&row.id, "main.full_name", &json!("Marcus"))
            .unwrap();
        let name: String = db
            .conn()
            .query_row(
                "SELECT name FROM character WHERE id = ?1",
                [row.id.as_str()],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(name, "Marcus");
    }

    #[test]
    fn delete_and_cascade_removes_row_and_moves_file_to_trash() {
        let (dir, db, p_id) = setup();
        let store = CharacterStore::new(&db, dir.path().to_path_buf());
        let row = store
            .create(NewCharacter {
                project_id: p_id.clone(),
                hue_token: "--water-hue-character-1".into(),
            })
            .unwrap();
        let original_path = dir.path().join("characters").join(format!("{}.toml", row.id));
        assert!(original_path.exists());

        store.delete_and_cascade(&row.id).unwrap();

        // Row gone.
        let count: i64 = db
            .conn()
            .query_row("SELECT COUNT(*) FROM character", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 0);
        // File moved out of the live dir.
        assert!(!original_path.exists());
        // Something landed in characters/.trash/.
        let trash = dir.path().join("characters").join(".trash");
        let trash_count = std::fs::read_dir(&trash).map_or(0, std::iter::Iterator::count);
        assert!(trash_count >= 1, "expected at least one file in {trash:?}");
    }

    #[test]
    fn delete_and_cascade_clears_scene_pov_and_presence() {
        // Build a scene that references the character through pov + presence,
        // then assert delete cascades both away.
        let (dir, db, p_id) = setup();
        let store = CharacterStore::new(&db, dir.path().to_path_buf());
        let character = store
            .create(NewCharacter {
                project_id: p_id.clone(),
                hue_token: "--water-hue-character-1".into(),
            })
            .unwrap();
        // Insert a manuscript + scene directly via SQL (the SceneStore path
        // requires a full path setup we don't need here).
        let now = Utc::now().to_rfc3339();
        db.conn()
            .execute(
                "INSERT INTO manuscript (id, project_id, name, ordering, created_at, updated_at)
                 VALUES ('m1', ?1, 'M', 0, ?2, ?2)",
                (p_id.as_str(), &now),
            )
            .unwrap();
        db.conn()
            .execute(
                "INSERT INTO scene (id, manuscript_id, ordering, name, pov_character_id, file_path, created_at, updated_at)
                 VALUES ('s1', 'm1', 0, 'S', ?1, 'manuscript/scenes/s1.md', ?2, ?2)",
                (character.id.as_str(), &now),
            )
            .unwrap();
        db.conn()
            .execute(
                "INSERT INTO scene_character_presence (scene_id, character_id) VALUES ('s1', ?1)",
                [character.id.as_str()],
            )
            .unwrap();
        // Sanity: presence is in place.
        let pres_before: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM scene_character_presence WHERE character_id = ?1",
                [character.id.as_str()],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(pres_before, 1);

        store.delete_and_cascade(&character.id).unwrap();

        let pres_after: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM scene_character_presence WHERE character_id = ?1",
                [character.id.as_str()],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(pres_after, 0);
        let pov: Option<String> = db
            .conn()
            .query_row("SELECT pov_character_id FROM scene WHERE id = 's1'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert!(pov.is_none());
    }
}
