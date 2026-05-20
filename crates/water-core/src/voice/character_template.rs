//! Character voice template rendering.
//!
//! Takes an LSM v2.1 sheet (as `serde_json::Value`) and renders the voice
//! template at `prompts/speakers/character/template.toml`. Missing fields
//! cause their entire sentence to be omitted (missing-field policy per
//! M3 spec § 10).
//!
//! M4 Task 25: the renderer signature now takes a
//! [`crate::world::WorldRegistry`] and a [`crate::voice::SceneContext`]
//! in preparation for `{{world.location_*}}` token injection (Task 26).
//! The arguments are accepted but not yet consumed; Task 26 wires the
//! token substitutions and updates the bundled template.

use crate::voice::SceneContext;
use crate::world::WorldRegistry;
use serde::Deserialize;

const TEMPLATE_TOML: &str = include_str!("../../../../prompts/speakers/character/template.toml");

#[derive(Debug, Deserialize)]
struct TemplateFile {
    #[allow(dead_code)]
    version: String,
    #[serde(default)]
    #[allow(dead_code)]
    schema_version: String,
    prompt: TemplatePrompt,
}

#[derive(Debug, Deserialize)]
struct TemplatePrompt {
    voice_profile: String,
}

#[derive(Debug, Clone)]
pub struct CharacterTemplate {
    /// The raw `voice_profile` string with `{{placeholder}}` markers.
    raw: String,
}

impl CharacterTemplate {
    /// Load the built-in template at compile time.
    ///
    /// # Panics
    /// Panics if the built-in template TOML at
    /// `prompts/speakers/character/template.toml` fails to parse. This is
    /// a programmer error (the file is bundled at compile time) and would
    /// be caught by `cargo test`.
    #[must_use]
    pub fn load_builtin() -> Self {
        let parsed: TemplateFile =
            toml::from_str(TEMPLATE_TOML).expect("built-in character template must parse");
        Self {
            raw: parsed.prompt.voice_profile,
        }
    }

    /// Render the template with the given LSM v2.1 sheet data. Missing
    /// fields cause their entire sentence to be omitted.
    ///
    /// When `scene_context.location_id` references an entry in the
    /// `locations` segment of `world_registry`, the rendered prompt
    /// gains a Setting block built from that entry's `main.name`,
    /// `main.type`, and `main.sensory_detail`. Any missing piece drops
    /// its line via the existing line-based omission rule.
    #[must_use]
    pub fn render(
        &self,
        sheet: &serde_json::Value,
        world_registry: &WorldRegistry,
        scene_context: &SceneContext,
    ) -> String {
        let main = sheet.get("main").unwrap_or(&serde_json::Value::Null);
        let bonus = sheet
            .get("bonus_traits")
            .unwrap_or(&serde_json::Value::Null);

        let (loc_name, loc_type, loc_sensory) =
            location_tokens(world_registry, scene_context);

        let substitutions: &[(&str, String)] = &[
            ("full_name", read_str(main, "full_name")),
            (
                "role_descriptor",
                role_descriptor(read_str(main, "role_in_story").as_str()),
            ),
            ("want", read_str(main, "want")),
            ("need", read_str(main, "need")),
            ("lie_they_believe", read_str(main, "lie_they_believe")),
            ("ghost_wound", read_str(main, "ghost_wound")),
            ("fatal_flaw", read_str(main, "fatal_flaw")),
            ("voice", read_str(bonus, "voice")),
            (
                "speech_patterns",
                read_list_joined(bonus, "speech_patterns"),
            ),
            ("fears", read_list_joined(bonus, "fears")),
            ("values", read_list_joined(bonus, "values")),
            ("world.location_name", loc_name),
            ("world.location_type", loc_type),
            ("world.location_sensory_detail", loc_sensory),
        ];

        render_with_omission(&self.raw, substitutions)
    }
}

/// Resolve the `{{world.location_*}}` substitutions for the rendering
/// pass. Returns `(name, type, sensory_detail)` — each is empty when the
/// information isn't available, which causes its template line to drop
/// via the line-based omission rule.
fn location_tokens(
    world_registry: &WorldRegistry,
    scene_context: &SceneContext,
) -> (String, String, String) {
    let Some(loc_id) = scene_context.location_id.as_ref() else {
        return (String::new(), String::new(), String::new());
    };
    let Some(snap) = world_registry.by_id(loc_id) else {
        return (String::new(), String::new(), String::new());
    };
    if snap.segment_slug != "locations" {
        return (String::new(), String::new(), String::new());
    }
    let main = snap.data.get("main").unwrap_or(&serde_json::Value::Null);
    let name = if snap.name.trim().is_empty() {
        String::new()
    } else {
        snap.name.clone()
    };
    (name, read_str(main, "type"), read_str(main, "sensory_detail"))
}

fn read_str(obj: &serde_json::Value, key: &str) -> String {
    obj.get(key)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string()
}

fn read_list_joined(obj: &serde_json::Value, key: &str) -> String {
    let arr = obj.get(key).and_then(|v| v.as_array());
    arr.map(|items| {
        items
            .iter()
            .filter_map(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(", ")
    })
    .unwrap_or_default()
}

fn role_descriptor(role: &str) -> String {
    match role {
        "protagonist" => "You are the protagonist of this story.".to_string(),
        "antagonist" => "You are an antagonist in this story.".to_string(),
        "supporting" => "You are a supporting character in this story.".to_string(),
        "mentor" => "You are a mentor figure in this story.".to_string(),
        "foil" => "You are a foil character in this story.".to_string(),
        _ => String::new(),
    }
}

fn render_with_omission(template: &str, subs: &[(&str, String)]) -> String {
    let mut out_lines: Vec<String> = Vec::new();
    for line in template.lines() {
        let mut keep = true;
        let mut rendered = line.to_string();
        for (key, value) in subs {
            let marker = format!("{{{{{key}}}}}");
            if rendered.contains(&marker) {
                if value.is_empty() {
                    keep = false;
                    break;
                }
                rendered = rendered.replace(&marker, value);
            }
        }
        if keep {
            out_lines.push(rendered);
        }
    }
    // Collapse runs of empty lines to a single empty line.
    let mut collapsed: Vec<String> = Vec::new();
    let mut prev_empty = false;
    for line in out_lines {
        let is_empty = line.trim().is_empty();
        if is_empty && prev_empty {
            continue;
        }
        collapsed.push(line);
        prev_empty = is_empty;
    }
    collapsed.join("\n").trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voice::SceneContext;
    use crate::world::WorldRegistry;
    use serde_json::json;

    #[test]
    fn renders_full_sheet() {
        let t = CharacterTemplate::load_builtin();
        let sheet = json!({
            "main": {
                "full_name": "Marcus Vale",
                "role_in_story": "protagonist",
                "want": "to be seen as the man his father wasn't",
                "need": "to forgive himself for the night of the fire",
                "lie_they_believe": "If I just work hard enough, I can outrun what I did",
                "ghost_wound": "The fire he failed to stop when he was 15",
                "fatal_flaw": "He refuses to ask for help",
            },
            "bonus_traits": {
                "voice": "spare, weather-worn, with quiet warmth",
                "speech_patterns": ["You know what I mean", "It's fine"],
                "fears": ["losing his sister", "being seen as weak"],
                "values": ["loyalty", "showing up when it matters"],
            },
        });
        let rendered = t.render(&sheet, &WorldRegistry::default(), &SceneContext::empty());
        assert!(rendered.contains("Marcus Vale"));
        assert!(rendered.contains("protagonist of this story"));
        assert!(rendered.contains("spare, weather-worn"));
        assert!(rendered.contains("losing his sister, being seen as weak"));
        assert!(rendered.contains("loyalty, showing up when it matters"));
        assert!(rendered.contains("The fire he failed to stop"));
    }

    #[test]
    fn omits_sentences_with_missing_fields() {
        let t = CharacterTemplate::load_builtin();
        let sheet = json!({
            "main": {
                "full_name": "Ada",
                "role_in_story": "supporting",
                "want": "to retire",
                "need": "to face the regret she's been hiding",
                "lie_they_believe": "She has plenty of time",
                // ghost_wound + fatal_flaw absent
            },
            "bonus_traits": {
                "voice": "clipped",
                // speech_patterns, fears, values absent
            },
        });
        let rendered = t.render(&sheet, &WorldRegistry::default(), &SceneContext::empty());
        assert!(rendered.contains("Ada"));
        assert!(rendered.contains("clipped"));
        assert!(!rendered.contains("{{"), "no unresolved placeholders");
        assert!(
            !rendered.contains("What still haunts you"),
            "ghost_wound line dropped"
        );
        assert!(
            !rendered.contains("Your fatal flaw"),
            "fatal_flaw line dropped"
        );
        assert!(
            !rendered.contains("You often say things like"),
            "speech_patterns line dropped"
        );
        assert!(!rendered.contains("What you fear"), "fears line dropped");
        assert!(
            !rendered.contains("What you hold sacred"),
            "values line dropped"
        );
    }

    #[test]
    fn unknown_role_omits_descriptor() {
        let t = CharacterTemplate::load_builtin();
        let sheet = json!({
            "main": { "full_name": "X", "role_in_story": "weirdo", "want": "w", "need": "n", "lie_they_believe": "l" },
            "bonus_traits": { "voice": "v" },
        });
        let rendered = t.render(&sheet, &WorldRegistry::default(), &SceneContext::empty());
        assert!(rendered.contains('X'), "name still appears elsewhere");
        assert!(!rendered.contains("weirdo"));
    }

    /// Helper: seed a project + create a location entry with the given main
    /// fields. Returns (db, project_root, project_id, entry_id).
    #[cfg(test)]
    fn mk_location(
        name: &str,
        ty: &str,
        sensory: &str,
    ) -> (
        crate::Db,
        tempfile::TempDir,
        crate::Id,
        crate::Id,
    ) {
        let dir = tempfile::tempdir().unwrap();
        let db = crate::Db::open_in_memory().unwrap();
        let p = crate::ProjectStore::new(&db).insert("P").unwrap();
        let store = crate::world::WorldStore::new(&db, dir.path().to_path_buf());
        store.seed_builtins(&p.id).unwrap();
        let loc = store
            .find_segment_by_slug(&p.id, "locations")
            .unwrap()
            .unwrap();
        let entry_id = store.create_entry(&loc.id, name).unwrap();
        if !ty.is_empty() {
            store
                .update_entry_field(&entry_id, "main.type", &serde_json::json!(ty))
                .unwrap();
        }
        if !sensory.is_empty() {
            store
                .update_entry_field(
                    &entry_id,
                    "main.sensory_detail",
                    &serde_json::json!(sensory),
                )
                .unwrap();
        }
        (db, dir, p.id, entry_id)
    }

    fn baseline_sheet() -> serde_json::Value {
        json!({
            "main": {
                "full_name": "Aren",
                "role_in_story": "protagonist",
                "want": "w",
                "need": "n",
                "lie_they_believe": "l",
            },
            "bonus_traits": { "voice": "v" },
        })
    }

    #[test]
    fn includes_location_sensory_detail_when_scene_has_location_id() {
        let (db, dir, project_id, entry_id) = mk_location(
            "The Pell Library",
            "library",
            "Dust thick enough to read fingertips in",
        );
        let world_reg =
            crate::world::WorldRegistry::from_db(&db, &project_id, dir.path().to_path_buf())
                .unwrap();
        let scene_ctx = SceneContext {
            scene_id: crate::Id::new(),
            location_id: Some(entry_id),
            pov_character_id: None,
            characters_present: vec![],
        };

        let rendered =
            CharacterTemplate::load_builtin().render(&baseline_sheet(), &world_reg, &scene_ctx);

        assert!(
            rendered.contains("The Pell Library"),
            "location name missing from rendered prompt:\n{rendered}"
        );
        assert!(
            rendered.contains("library"),
            "location type missing from rendered prompt:\n{rendered}"
        );
        assert!(
            rendered.contains("Dust thick enough to read fingertips in"),
            "sensory_detail missing from rendered prompt:\n{rendered}"
        );
        assert!(
            !rendered.contains("{{world."),
            "world tokens left unrendered:\n{rendered}"
        );
    }

    #[test]
    fn omits_location_lines_when_scene_location_id_is_none() {
        let world_reg = WorldRegistry::default();
        let scene_ctx = SceneContext::empty();
        let rendered =
            CharacterTemplate::load_builtin().render(&baseline_sheet(), &world_reg, &scene_ctx);

        assert!(
            !rendered.contains("{{world."),
            "world tokens left unrendered:\n{rendered}"
        );
        // The "Setting:" prefix used for the location block must not appear
        // when there is no location to render.
        assert!(
            !rendered.contains("Setting:"),
            "setting block leaked through with no location:\n{rendered}"
        );
    }

    #[test]
    fn omits_location_lines_when_location_id_points_to_unknown_entry() {
        let (db, dir, project_id, _entry_id) = mk_location("Pell", "library", "dust");
        let world_reg =
            crate::world::WorldRegistry::from_db(&db, &project_id, dir.path().to_path_buf())
                .unwrap();
        let scene_ctx = SceneContext {
            scene_id: crate::Id::new(),
            location_id: Some(crate::Id::new()), // unknown id
            pov_character_id: None,
            characters_present: vec![],
        };

        let rendered =
            CharacterTemplate::load_builtin().render(&baseline_sheet(), &world_reg, &scene_ctx);

        assert!(
            !rendered.contains("Pell"),
            "unknown-id should not leak the existing location:\n{rendered}"
        );
        assert!(
            !rendered.contains("{{world."),
            "world tokens left unrendered:\n{rendered}"
        );
        assert!(
            !rendered.contains("Setting:"),
            "setting block leaked through with unknown location:\n{rendered}"
        );
    }

}
