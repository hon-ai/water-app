//! water.toml — project root metadata, the human-readable companion to `project.db`.

use crate::{Error, Id, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

pub const FILE_NAME: &str = "water.toml";
pub const CURRENT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WaterToml {
    pub schema_version: u32,
    pub project_id: Id,
    pub name: String,
    pub default_manuscript_id: Option<Id>,
    pub created_at: String,
    pub updated_at: String,
}

impl WaterToml {
    #[must_use]
    pub fn new(name: &str, project_id: Id, manuscript_id: Id) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            schema_version: CURRENT_SCHEMA_VERSION,
            project_id,
            name: name.to_owned(),
            default_manuscript_id: Some(manuscript_id),
            created_at: now.clone(),
            updated_at: now,
        }
    }

    pub fn read<P: AsRef<Path>>(dir: P) -> Result<Self> {
        let path = dir.as_ref().join(FILE_NAME);
        let text = std::fs::read_to_string(&path)
            .map_err(|e| Error::InvalidProject(format!("read {}: {e}", path.display())))?;
        let parsed: Self = toml::from_str(&text)
            .map_err(|e| Error::InvalidProject(format!("parse {}: {e}", path.display())))?;
        if parsed.schema_version > CURRENT_SCHEMA_VERSION {
            return Err(Error::InvalidProject(format!(
                "project {} requires Water schema version {} (we are {})",
                parsed.name, parsed.schema_version, CURRENT_SCHEMA_VERSION
            )));
        }
        Ok(parsed)
    }

    pub fn write<P: AsRef<Path>>(&self, dir: P) -> Result<()> {
        let path = dir.as_ref().join(FILE_NAME);
        let text = toml::to_string_pretty(self)?;
        std::fs::write(path, text)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let original = WaterToml::new("My Book", Id::new(), Id::new());
        original.write(dir.path()).unwrap();
        let loaded = WaterToml::read(dir.path()).unwrap();
        assert_eq!(loaded, original);
    }

    #[test]
    fn rejects_future_schema_version() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(FILE_NAME),
            "schema_version = 99\nproject_id = \"01H8AA0000000000000000AAAA\"\nname = \"X\"\ncreated_at = \"2026-01-01T00:00:00Z\"\nupdated_at = \"2026-01-01T00:00:00Z\"\n",
        )
        .unwrap();
        let err = WaterToml::read(dir.path()).unwrap_err();
        assert!(matches!(err, Error::InvalidProject(_)));
    }
}
