//! Read and write a single scene `.md` file: `YAML` frontmatter + `Markdown` body.

use crate::{Error, Id, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

// `Eq` dropped in M6: f32 doesn't impl Eq (NaN). The struct gains
// `canvas_x` / `canvas_y` as Option<f32>; PartialEq still works.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SceneFrontmatter {
    /// Obsidian-readability marker. Always serializes as `true`; tells
    /// an Obsidian user who's opened the Water project folder as a
    /// vault which of their markdown files are Water scenes vs.
    /// notes/lore they might keep alongside. Defaults to `true` on
    /// read so legacy scene files (created before this field landed)
    /// stay correctly tagged after their next save.
    #[serde(default = "default_water_scene", rename = "water_scene")]
    pub water_scene: bool,
    pub id: Id,
    pub name: String,
    pub chapter_id: Option<Id>,
    pub order: i64,
    #[serde(default)]
    pub pov_character_id: Option<Id>,
    #[serde(default)]
    pub characters_present: Vec<Id>,
    #[serde(default)]
    pub location_id: Option<Id>,
    #[serde(default)]
    pub scene_goal: Option<String>,
    #[serde(default = "default_status")]
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub word_count: i64,
    /// M6: position in the macro spatial canvas. `None` = unplaced;
    /// the renderer auto-flows unplaced scenes on first paint.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canvas_x: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canvas_y: Option<f32>,
    /// M6: optional group label ("Act II", etc.). Pure visual hint
    /// (per spec § 8: not a separate model entity in v1).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canvas_group: Option<String>,
}

fn default_status() -> String {
    "draft".to_owned()
}

fn default_water_scene() -> bool {
    true
}

// `Eq` dropped in M6 — `SceneFrontmatter` now carries `Option<f32>`
// canvas position which is PartialEq but not Eq.
#[derive(Debug, Clone, PartialEq)]
pub struct SceneFile {
    pub frontmatter: SceneFrontmatter,
    pub body: String,
}

const DELIMITER: &str = "---";

impl SceneFile {
    pub fn parse(text: &str) -> Result<Self> {
        let trimmed = text.trim_start_matches('\u{feff}');
        let trimmed = trimmed.strip_prefix(DELIMITER).ok_or_else(|| {
            Error::InvalidProject("scene .md must start with `---` frontmatter".into())
        })?;
        // Skip optional CR/LF after opening ---.
        let trimmed = trimmed.trim_start_matches('\r').trim_start_matches('\n');
        let end = trimmed.find("\n---").ok_or_else(|| {
            Error::InvalidProject("scene .md missing closing `---` for frontmatter".into())
        })?;
        let yaml = &trimmed[..end];
        let rest = &trimmed[end + 4..]; // skip "\n---"
        let body = rest
            .trim_start_matches('\r')
            .trim_start_matches('\n')
            .to_owned();
        let frontmatter: SceneFrontmatter = serde_yaml::from_str(yaml)?;
        Ok(Self { frontmatter, body })
    }

    pub fn read<P: AsRef<Path>>(path: P) -> Result<Self> {
        let text = std::fs::read_to_string(path)?;
        Self::parse(&text)
    }

    #[allow(clippy::inherent_to_string)]
    pub fn to_string(&self) -> Result<String> {
        let yaml = serde_yaml::to_string(&self.frontmatter)?;
        let mut out = String::with_capacity(yaml.len() + self.body.len() + 16);
        out.push_str(DELIMITER);
        out.push('\n');
        out.push_str(&yaml);
        if !yaml.ends_with('\n') {
            out.push('\n');
        }
        out.push_str(DELIMITER);
        out.push('\n');
        if !self.body.is_empty() {
            out.push('\n');
            out.push_str(&self.body);
            if !self.body.ends_with('\n') {
                out.push('\n');
            }
        }
        Ok(out)
    }

    pub fn write<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let text = self.to_string()?;
        std::fs::write(path, text)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn sample() -> SceneFile {
        SceneFile {
            frontmatter: SceneFrontmatter {
                water_scene: true,
                id: "01H8X400000000000000000000".parse().unwrap(),
                name: "Test".into(),
                chapter_id: None,
                order: 0,
                pov_character_id: None,
                characters_present: vec![],
                location_id: None,
                scene_goal: None,
                status: "draft".into(),
                created_at: "2026-05-16T09:00:00+00:00".into(),
                updated_at: "2026-05-16T09:00:00+00:00".into(),
                word_count: 0,
                canvas_x: None,
                canvas_y: None,
                canvas_group: None,
            },
            body: "Hello.\n".into(),
        }
    }

    #[test]
    fn round_trip_text() {
        let s = sample();
        let text = s.to_string().unwrap();
        let parsed = SceneFile::parse(&text).unwrap();
        assert_eq!(parsed, s);
    }

    #[test]
    fn parse_rejects_missing_opening_delimiter() {
        let err = SceneFile::parse("no frontmatter").unwrap_err();
        assert!(matches!(err, Error::InvalidProject(_)));
    }

    #[test]
    fn parse_rejects_missing_closing_delimiter() {
        let err = SceneFile::parse("---\nname: X\n").unwrap_err();
        assert!(matches!(err, Error::InvalidProject(_)));
    }

    #[test]
    fn round_trip_through_disk() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("01H8X4.md");
        let s = sample();
        s.write(&path).unwrap();
        let loaded = SceneFile::read(&path).unwrap();
        assert_eq!(loaded, s);
    }
}
