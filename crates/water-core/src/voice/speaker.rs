//! Speaker trait + persona-backed implementation.
//!
//! A `Speaker` is the runtime-visible interface for any voice that can speak
//! in the editor — currently personas (`PersonaSpeaker`); characters will
//! arrive in a later task. Speakers are loaded from TOML at startup and held
//! as `Arc<dyn Speaker>` so the router can shuffle them cheaply.

use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Kind discriminator for [`Speaker`]. Persona = built-in voice (Echo,
/// Architect, …). Character = user-defined character speaker (later task).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpeakerKind {
    Persona,
    Character,
}

/// A voice that can be asked to speak. Implementations are immutable after
/// construction; per-project overrides (rename) are applied at load time.
pub trait Speaker: Send + Sync {
    fn id(&self) -> &str;
    fn kind(&self) -> SpeakerKind;
    fn display_name(&self) -> &str;
    fn hue_token(&self) -> &str;
    fn prompt_fragment(&self) -> &str;
    fn anti_loop_threshold(&self) -> f32 {
        0.70
    }
    fn cooldown_ms(&self) -> u64 {
        45_000
    }
}

#[derive(Debug, Deserialize)]
struct PersonaToml {
    version: String,
    id: String,
    display_name: String,
    hue_token: String,
    #[serde(default = "default_threshold")]
    anti_loop_threshold: f32,
    #[serde(default = "default_cooldown")]
    cooldown_ms: u64,
    prompt: PersonaPrompt,
}

#[derive(Debug, Deserialize)]
struct PersonaPrompt {
    voice_profile: String,
}

fn default_threshold() -> f32 {
    0.70
}
fn default_cooldown() -> u64 {
    45_000
}

/// Persona speaker: a built-in voice loaded from a TOML manifest.
#[derive(Debug, Clone)]
pub struct PersonaSpeaker {
    id: String,
    display_name: String,
    hue_token: String,
    prompt_fragment: String,
    anti_loop_threshold: f32,
    cooldown_ms: u64,
}

impl PersonaSpeaker {
    /// Parse a persona TOML manifest into a `PersonaSpeaker`.
    pub fn from_toml_str(s: &str) -> Result<Self, String> {
        let parsed: PersonaToml = toml::from_str(s).map_err(|e| e.to_string())?;
        if parsed.version != "1" {
            return Err(format!(
                "unsupported persona TOML version: {}",
                parsed.version
            ));
        }
        Ok(Self {
            id: parsed.id,
            display_name: parsed.display_name,
            hue_token: parsed.hue_token,
            prompt_fragment: parsed.prompt.voice_profile,
            anti_loop_threshold: parsed.anti_loop_threshold,
            cooldown_ms: parsed.cooldown_ms,
        })
    }

    /// Override the display name (per-project rename).
    #[must_use]
    pub fn with_display_name(mut self, name: String) -> Self {
        self.display_name = name;
        self
    }
}

impl Speaker for PersonaSpeaker {
    fn id(&self) -> &str {
        &self.id
    }
    fn kind(&self) -> SpeakerKind {
        SpeakerKind::Persona
    }
    fn display_name(&self) -> &str {
        &self.display_name
    }
    fn hue_token(&self) -> &str {
        &self.hue_token
    }
    fn prompt_fragment(&self) -> &str {
        &self.prompt_fragment
    }
    fn anti_loop_threshold(&self) -> f32 {
        self.anti_loop_threshold
    }
    fn cooldown_ms(&self) -> u64 {
        self.cooldown_ms
    }
}

/// Convenient alias for shared, type-erased speakers.
pub type SpeakerArc = Arc<dyn Speaker>;

/// Character speaker: a user-defined voice constructed from a project's
/// `character` table row.
///
/// **T3 stub.** Identity + hue + sensible defaults are wired here so the
/// `CharacterRegistry` can hand back `SpeakerArc`s today. T4 will fill
/// `prompt_fragment` by rendering a voice template against the sheet data.
#[derive(Debug, Clone)]
pub struct CharacterSpeaker {
    id: String,
    display_name: String,
    hue_token: String,
    prompt_fragment: String,
    anti_loop_threshold: f32,
    cooldown_ms: u64,
}

impl CharacterSpeaker {
    /// Construct from a `CharacterRegistryRow`. Renders the LSM v2.1 sheet
    /// data through the built-in character voice template at
    /// `prompts/speakers/character/template.toml`.
    #[must_use]
    pub fn from_row(row: &crate::character::registry::CharacterRegistryRow) -> Self {
        let template = crate::voice::character_template::CharacterTemplate::load_builtin();
        let prompt_fragment = template.render(&row.data);
        Self {
            id: row.id.as_str().to_string(),
            display_name: row.name.clone(),
            hue_token: row.hue_token.clone(),
            prompt_fragment,
            anti_loop_threshold: 0.70,
            cooldown_ms: 60_000, // slightly longer than personas (45s)
        }
    }
}

impl Speaker for CharacterSpeaker {
    fn id(&self) -> &str {
        &self.id
    }
    fn kind(&self) -> SpeakerKind {
        SpeakerKind::Character
    }
    fn display_name(&self) -> &str {
        &self.display_name
    }
    fn hue_token(&self) -> &str {
        &self.hue_token
    }
    fn prompt_fragment(&self) -> &str {
        &self.prompt_fragment
    }
    fn anti_loop_threshold(&self) -> f32 {
        self.anti_loop_threshold
    }
    fn cooldown_ms(&self) -> u64 {
        self.cooldown_ms
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ECHO: &str = include_str!("../../../../prompts/speakers/persona/echo.toml");

    #[test]
    fn parses_echo_toml() {
        let s = PersonaSpeaker::from_toml_str(ECHO).unwrap();
        assert_eq!(s.id(), "echo");
        assert_eq!(s.display_name(), "Echo");
        assert_eq!(s.hue_token(), "--water-hue-muse");
        assert!(s.prompt_fragment().contains("listening through fog"));
        assert!((s.anti_loop_threshold() - 0.70).abs() < 1e-5);
        assert_eq!(s.cooldown_ms(), 45_000);
    }

    #[test]
    fn rename_overrides_display_name() {
        let s = PersonaSpeaker::from_toml_str(ECHO)
            .unwrap()
            .with_display_name("Muse".to_string());
        assert_eq!(s.display_name(), "Muse");
        assert_eq!(s.id(), "echo");
    }

    #[test]
    fn rejects_wrong_version() {
        let bad = r#"
version = "99"
id = "x"
display_name = "X"
hue_token = "--water-hue-muse"
[prompt]
voice_profile = "y"
"#;
        assert!(PersonaSpeaker::from_toml_str(bad).is_err());
    }
}
