//! Character registry — populated from the `character` table on project
//! open. Sibling of `voice::PersonaRegistry`. Used by the voice router
//! when a character-track trigger fires.
//!
//! Hue assignment happens in `CharacterStore::upsert` (via the v3 backfill
//! and future insert logic); this registry just reads what's already
//! stored in the `hue_token` column.

use crate::voice::speaker::{CharacterSpeaker, SpeakerArc};
use crate::{Db, Id};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

/// One row of the character registry: identity + hue + JSON-decoded sheet
/// data. Distinct from `crate::character::CharacterRow` (which is the
/// disk-projection used by `CharacterStore`); this row is the *runtime*
/// projection consumed by the voice subsystem.
#[derive(Debug, Clone)]
pub struct CharacterRegistryRow {
    pub id: Id,
    pub name: String,
    pub hue_token: String,
    /// JSON-decoded sheet data. Sourced from the `character.data_json` column.
    pub data: serde_json::Value,
}

/// Holds every character defined in the project, keyed by id, plus an
/// ordered list for LRU selection.
#[derive(Default)]
pub struct CharacterRegistry {
    by_id: HashMap<String, SpeakerArc>,
    rows: Vec<CharacterRegistryRow>,
}

impl std::fmt::Debug for CharacterRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // `SpeakerArc` is `Arc<dyn Speaker>` (no Debug bound), so we render
        // the row list and elide `by_id` (it's redundant with `rows`).
        f.debug_struct("CharacterRegistry")
            .field("rows", &self.rows)
            .finish_non_exhaustive()
    }
}

impl CharacterRegistry {
    /// Load every character from the project DB. Each row becomes a
    /// `CharacterSpeaker` (currently a stub — T4 fills the prompt
    /// fragment) indexed by its `Id`.
    pub fn from_db(db: &Db) -> Result<Self, String> {
        let mut stmt = db
            .conn()
            .prepare("SELECT id, name, hue_token, data_json FROM character ORDER BY created_at")
            .map_err(|e| e.to_string())?;
        let rows_iter = stmt
            .query_map([], |r| {
                let id_str: String = r.get(0)?;
                let id = id_str.parse::<Id>().map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    )
                })?;
                let data_json: String = r.get(3)?;
                Ok(CharacterRegistryRow {
                    id,
                    name: r.get(1)?,
                    hue_token: r.get(2)?,
                    data: serde_json::from_str(&data_json).unwrap_or(serde_json::Value::Null),
                })
            })
            .map_err(|e| e.to_string())?;

        let mut rows: Vec<CharacterRegistryRow> = Vec::new();
        let mut by_id: HashMap<String, SpeakerArc> = HashMap::new();
        for row in rows_iter {
            let row = row.map_err(|e| e.to_string())?;
            let speaker: SpeakerArc = Arc::new(CharacterSpeaker::from_row(&row));
            by_id.insert(row.id.as_str().to_string(), speaker);
            rows.push(row);
        }
        Ok(Self { by_id, rows })
    }

    /// Build an empty registry. Useful for tests + projects that haven't
    /// defined any characters yet.
    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    /// Look up a character by id.
    #[must_use]
    pub fn by_id(&self, id: &str) -> Option<SpeakerArc> {
        self.by_id.get(id).cloned()
    }

    /// Test-only helper: insert a fully-formed `CharacterRegistryRow`
    /// (and its derived `CharacterSpeaker`) directly into the registry,
    /// bypassing the DB load path.
    #[cfg(test)]
    pub fn insert_for_test(&mut self, row: CharacterRegistryRow) {
        let speaker: SpeakerArc = Arc::new(CharacterSpeaker::from_row(&row));
        self.by_id.insert(row.id.as_str().to_string(), speaker);
        self.rows.push(row);
    }

    /// All characters, in `created_at` order.
    #[must_use]
    pub fn list(&self) -> &[CharacterRegistryRow] {
        &self.rows
    }

    /// Case-sensitive name lookup. Matches against the SQL `name` column
    /// (which mirrors `main.full_name` after [`CharacterStore::update_field`]).
    /// Returns the first row whose `name` equals `token` exactly.
    ///
    /// Used by [`crate::world::collision::resolve_token_kind`] (M4 § 6.2)
    /// to detect character-vs-world name collisions. Case sensitivity is
    /// intentional and mirrors the M3 autosuggest convention; world
    /// lookups are case-insensitive by contrast.
    #[must_use]
    pub fn find_by_name(&self, token: &str) -> Option<&CharacterRegistryRow> {
        self.rows.iter().find(|r| r.name == token)
    }

    /// Returns the least-recently-used character from `present`, skipping
    /// characters whose cooldown hasn't elapsed since their last emit.
    ///
    /// Selection rules:
    /// * Only ids in `present` that resolve in this registry are eligible.
    /// * A candidate is *skipped* if `now - cooldowns[id] < cooldown_ms`.
    /// * Among eligibles, the one with the **oldest** last-emit wins; a
    ///   character that has never emitted (absent from `cooldowns`) counts
    ///   as oldest of all. Ties broken by `present` order via stable sort.
    #[must_use]
    pub fn pick_lru_present(
        &self,
        present: &[Id],
        cooldowns: &HashMap<String, Instant>,
        now: Instant,
    ) -> Option<SpeakerArc> {
        let mut candidates: Vec<(SpeakerArc, Option<Instant>)> = present
            .iter()
            .filter_map(|id| {
                self.by_id(id.as_str()).map(|s| {
                    let last = cooldowns.get(s.id()).copied();
                    (s, last)
                })
            })
            .filter(|(s, last)| match last {
                Some(t) => {
                    u64::try_from(now.saturating_duration_since(*t).as_millis()).unwrap_or(u64::MAX)
                        >= s.cooldown_ms()
                }
                None => true,
            })
            .collect();
        // `Option<Instant>` natural ordering treats `None` as less than any
        // `Some(_)`, which is exactly the "never-emitted = oldest" rule we
        // want. Stable sort preserves `present` order on ties.
        candidates.sort_by_key(|(_, last)| *last);
        candidates.into_iter().next().map(|(s, _)| s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voice::speaker::SpeakerKind;
    use tempfile::TempDir;

    fn fresh_db() -> (TempDir, Db) {
        let dir = TempDir::new().unwrap();
        let db = Db::open(dir.path().join("project.db")).unwrap();
        // Seed a project so character.project_id FK is valid.
        db.conn()
            .execute(
                "INSERT INTO project (id, name, created_at, updated_at)
                 VALUES ('p1', 'P', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
                rusqlite::params![],
            )
            .unwrap();
        (dir, db)
    }

    fn insert_character(db: &Db, id: &str, name: &str, hue: &str, created_at: &str) {
        db.conn()
            .execute(
                "INSERT INTO character
                 (id, project_id, name, schema_version, data_json, hue_token, file_path, created_at, updated_at)
                 VALUES (?1, 'p1', ?2, 'lsm-v2.1', '{}', ?3, ?4, ?5, ?5)",
                rusqlite::params![id, name, hue, format!("characters/{id}.toml"), created_at],
            )
            .unwrap();
    }

    #[test]
    fn from_db_loads_zero_characters() {
        let (_tmp, db) = fresh_db();
        let reg = CharacterRegistry::from_db(&db).unwrap();
        assert_eq!(reg.list().len(), 0);
    }

    #[test]
    fn from_db_loads_one_character() {
        let (_tmp, db) = fresh_db();
        insert_character(
            &db,
            "01HE000000000000000000000A",
            "Marcus",
            "--water-hue-character-1",
            "2026-01-01T00:00:00Z",
        );
        let reg = CharacterRegistry::from_db(&db).unwrap();
        assert_eq!(reg.list().len(), 1);
        assert_eq!(reg.list()[0].name, "Marcus");
        assert_eq!(reg.list()[0].hue_token, "--water-hue-character-1");
        let speaker = reg.by_id("01HE000000000000000000000A").unwrap();
        assert_eq!(speaker.display_name(), "Marcus");
        assert_eq!(speaker.kind(), SpeakerKind::Character);
        assert_eq!(speaker.hue_token(), "--water-hue-character-1");
    }

    #[test]
    fn speaker_prompt_fragment_includes_voice() {
        let (_tmp, db) = fresh_db();
        let data_json = serde_json::json!({
            "main": { "full_name": "Marcus", "role_in_story": "protagonist", "want": "w", "need": "n", "lie_they_believe": "l" },
            "bonus_traits": { "voice": "spare, weather-worn", "fears": ["x"], "values": ["y"] }
        }).to_string();
        db.conn().execute(
            "INSERT INTO character (id, project_id, name, schema_version, data_json, hue_token, file_path, created_at, updated_at)
             VALUES ('01HE000000000000000000000A', 'p1', 'Marcus', 'lsm-v2.1', ?1, '--water-hue-character-1', 'characters/x.toml', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
            rusqlite::params![data_json],
        ).unwrap();
        let reg = CharacterRegistry::from_db(&db).unwrap();
        let speaker = reg.by_id("01HE000000000000000000000A").unwrap();
        assert!(speaker.prompt_fragment().contains("spare, weather-worn"));
        assert!(speaker.prompt_fragment().contains("Marcus"));
    }

    #[test]
    fn pick_lru_present_returns_least_recently_used() {
        let (_tmp, db) = fresh_db();
        insert_character(
            &db,
            "01HE000000000000000000000A",
            "A",
            "--water-hue-character-1",
            "2026-01-01T00:00:00Z",
        );
        insert_character(
            &db,
            "01HE000000000000000000000B",
            "B",
            "--water-hue-character-2",
            "2026-01-02T00:00:00Z",
        );
        let reg = CharacterRegistry::from_db(&db).unwrap();
        let present = vec![
            "01HE000000000000000000000A".parse::<Id>().unwrap(),
            "01HE000000000000000000000B".parse::<Id>().unwrap(),
        ];
        let now = Instant::now();
        let mut cooldowns: HashMap<String, Instant> = HashMap::new();
        // B just emitted; A has never emitted. A wins (never-emitted = oldest).
        cooldowns.insert("01HE000000000000000000000B".into(), now);
        let pick = reg.pick_lru_present(&present, &cooldowns, now);
        assert!(pick.is_some());
        assert_eq!(pick.unwrap().id(), "01HE000000000000000000000A");
    }
}
