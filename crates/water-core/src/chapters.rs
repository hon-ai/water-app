//! manuscript/chapters.toml — ordered list of named scene groupings.

use crate::{Error, Id, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

pub const FILE_NAME: &str = "chapters.toml";
pub const CURRENT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChaptersFile {
    pub schema_version: u32,
    #[serde(default)]
    pub chapter: Vec<Chapter>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Chapter {
    pub id: Id,
    pub name: String,
    pub ordering: i64,
    #[serde(default)]
    pub scene_ids: Vec<Id>,
}

impl ChaptersFile {
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            schema_version: CURRENT_SCHEMA_VERSION,
            chapter: Vec::new(),
        }
    }

    pub fn read<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path_display = path.as_ref().display().to_string();
        let text = std::fs::read_to_string(&path)
            .map_err(|e| Error::InvalidProject(format!("read {path_display}: {e}")))?;
        let parsed: Self = toml::from_str(&text)
            .map_err(|e| Error::InvalidProject(format!("parse {path_display}: {e}")))?;
        if parsed.schema_version > CURRENT_SCHEMA_VERSION {
            return Err(Error::InvalidProject(format!(
                "{path_display} has schema_version {} (we are {})",
                parsed.schema_version, CURRENT_SCHEMA_VERSION
            )));
        }
        Ok(parsed)
    }

    pub fn write<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let text = toml::to_string_pretty(self)?;
        std::fs::write(path, text)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_round_trip() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join(FILE_NAME);
        ChaptersFile::empty().write(&path).unwrap();
        let loaded = ChaptersFile::read(&path).unwrap();
        assert_eq!(loaded, ChaptersFile::empty());
    }

    #[test]
    fn populated_round_trip() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join(FILE_NAME);
        let f = ChaptersFile {
            schema_version: 1,
            chapter: vec![Chapter {
                id: Id::new(),
                name: "Part One".into(),
                ordering: 0,
                scene_ids: vec![Id::new(), Id::new()],
            }],
        };
        f.write(&path).unwrap();
        let loaded = ChaptersFile::read(&path).unwrap();
        assert_eq!(loaded, f);
    }
}
