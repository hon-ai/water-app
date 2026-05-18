//! Persona registry: loads from `prompts/speakers/persona/*.toml`. The
//! per-project rename override is read from the `settings` table and
//! applied at load time.

use super::speaker::{PersonaSpeaker, SpeakerArc};
use crate::Db;
use rusqlite::OptionalExtension;
use std::sync::Arc;

const PERSONA_FILES: &[(&str, &str)] = &[
    (
        "echo",
        include_str!("../../../../prompts/speakers/persona/echo.toml"),
    ),
    (
        "architect",
        include_str!("../../../../prompts/speakers/persona/architect.toml"),
    ),
    (
        "editor",
        include_str!("../../../../prompts/speakers/persona/editor.toml"),
    ),
    (
        "cartographer",
        include_str!("../../../../prompts/speakers/persona/cartographer.toml"),
    ),
    (
        "chorus",
        include_str!("../../../../prompts/speakers/persona/chorus.toml"),
    ),
];

/// Holds the five built-in persona speakers, with per-project renames
/// applied at construction time.
#[derive(Default)]
pub struct PersonaRegistry {
    personas: Vec<SpeakerArc>,
}

impl PersonaRegistry {
    /// Load all built-in personas, applying any `persona.rename.<id>`
    /// override stored in the `settings` table of `db`.
    pub fn from_db(db: &Db) -> Result<Self, String> {
        let mut personas: Vec<SpeakerArc> = Vec::with_capacity(PERSONA_FILES.len());
        for (id, toml) in PERSONA_FILES {
            let base =
                PersonaSpeaker::from_toml_str(toml).map_err(|e| format!("persona {id}: {e}"))?;
            let key = format!("persona.rename.{id}");
            let rename: Option<String> = db
                .conn()
                .query_row(
                    "SELECT value_json FROM settings WHERE key = ?1",
                    [key.as_str()],
                    |r| r.get(0),
                )
                .optional()
                .map_err(|e| e.to_string())?;
            let speaker = if let Some(name_json) = rename {
                let name: String = serde_json::from_str(&name_json).map_err(|e| e.to_string())?;
                base.with_display_name(name)
            } else {
                base
            };
            personas.push(Arc::new(speaker) as SpeakerArc);
        }
        Ok(Self { personas })
    }

    /// All loaded personas, in built-in order.
    #[must_use]
    pub fn list(&self) -> &[SpeakerArc] {
        &self.personas
    }

    /// Look up a persona by id (e.g. `"echo"`).
    #[must_use]
    pub fn by_id(&self, id: &str) -> Option<SpeakerArc> {
        self.personas.iter().find(|s| s.id() == id).cloned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn fresh_db() -> (TempDir, Db) {
        let dir = TempDir::new().unwrap();
        let db = Db::open(dir.path().join("project.db")).unwrap();
        (dir, db)
    }

    #[test]
    fn loads_five_built_in_personas() {
        let (_t, db) = fresh_db();
        let reg = PersonaRegistry::from_db(&db).unwrap();
        let ids: Vec<&str> = reg.list().iter().map(|s| s.id()).collect();
        assert_eq!(
            ids,
            vec!["echo", "architect", "editor", "cartographer", "chorus"]
        );
    }

    #[test]
    fn rename_via_settings_overrides_display_name() {
        let (_t, db) = fresh_db();
        db.conn()
            .execute(
                "INSERT INTO settings (key, value_json) VALUES (?1, ?2)",
                rusqlite::params!["persona.rename.echo", "\"Whisper\""],
            )
            .unwrap();
        let reg = PersonaRegistry::from_db(&db).unwrap();
        let echo = reg.by_id("echo").unwrap();
        assert_eq!(echo.display_name(), "Whisper");
    }
}
