//! Built-in world segment templates.
//!
//! Each [`BuiltInTemplate`] is a fixed schema shipped with the application.
//! User-customized segments persist their template in
//! `world_segment.template_json`; lookup precedence is user-override -> built-in.
//!
//! Field-id convention (matches M3 `LSM_V2_1`):
//! - `main.<key>` for scalars and long-text.
//! - `lists.<key>` for `string_list` kinds.
//!
//! On disk the corresponding TOML uses `[main]` and `[lists]` sections via
//! `#[serde(flatten)]` on the section enum.
//!
//! ## Why these types are M4-owned (not reused from `character::intake`)
//!
//! M3's `IntakeField` is `&'static str`-backed for compile-time schema
//! definition and only derives `Serialize`. M4 needs to carry runtime-loaded
//! schemas (from `template_json` or user-authored templates) which requires
//! owned strings + `Deserialize`. The two type families are intentionally
//! separate; both serialize to the same JSON shape so the TS-side
//! `IntakeSchemaSection` type consumes both transparently.

use crate::{Db, Error, Id, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum WorldTemplateFieldKind {
    ShortText,
    LongText,
    StringList,
    Choice { options: Vec<String> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldTemplateField {
    pub id: String,
    pub label: String,
    pub prompt_question: String,
    pub kind: WorldTemplateFieldKind,
    pub optional_skip: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorldTemplateSchema {
    pub id: String,
    pub label: String,
    pub fields: Vec<WorldTemplateField>,
}

pub struct BuiltInTemplate {
    pub slug: &'static str,
    pub display_name: &'static str,
    pub is_collection: bool,
    pub schema: WorldTemplateSchema,
}

fn field(
    id: &str,
    label: &str,
    prompt: &str,
    kind: WorldTemplateFieldKind,
    optional: bool,
) -> WorldTemplateField {
    WorldTemplateField {
        id: id.to_string(),
        label: label.to_string(),
        prompt_question: prompt.to_string(),
        kind,
        optional_skip: optional,
    }
}

fn section(id: &str, label: &str, fields: Vec<WorldTemplateField>) -> WorldTemplateSchema {
    WorldTemplateSchema {
        id: id.to_string(),
        label: label.to_string(),
        fields,
    }
}

/// Returns the 6 built-in world segment templates in their canonical order.
///
/// The order here is the order they're seeded into `world_segment` by
/// `seed_builtins` (Task 3), which is also the order the UI displays them.
#[must_use]
#[allow(clippy::too_many_lines)] // static schema data; splitting hurts readability
pub fn built_in_templates() -> Vec<BuiltInTemplate> {
    vec![
        BuiltInTemplate {
            slug: "concept",
            display_name: "Concept",
            is_collection: false,
            schema: section(
                "concept",
                "Concept",
                vec![
                    field(
                        "main.core_premise",
                        "Core Premise",
                        "What's the core premise of this story?",
                        WorldTemplateFieldKind::LongText,
                        false,
                    ),
                    field(
                        "main.genre",
                        "Genre",
                        "What genre does this sit in?",
                        WorldTemplateFieldKind::ShortText,
                        false,
                    ),
                    field(
                        "main.tone",
                        "Tone",
                        "What's the dominant tone?",
                        WorldTemplateFieldKind::ShortText,
                        false,
                    ),
                    field(
                        "lists.themes",
                        "Themes",
                        "What themes do you want to explore?",
                        WorldTemplateFieldKind::StringList,
                        true,
                    ),
                    field(
                        "lists.inspirations",
                        "Inspirations",
                        "What works inspired this?",
                        WorldTemplateFieldKind::StringList,
                        true,
                    ),
                ],
            ),
        },
        BuiltInTemplate {
            slug: "locations",
            display_name: "Locations",
            is_collection: true,
            schema: section(
                "locations",
                "Location",
                vec![
                    field(
                        "main.name",
                        "Name",
                        "What's this place called?",
                        WorldTemplateFieldKind::ShortText,
                        false,
                    ),
                    field(
                        "main.type",
                        "Type",
                        "What kind of place is it (city, library, ruin, etc.)?",
                        WorldTemplateFieldKind::ShortText,
                        false,
                    ),
                    field(
                        "main.sensory_detail",
                        "Sensory Detail",
                        "What does it look, smell, sound like?",
                        WorldTemplateFieldKind::LongText,
                        false,
                    ),
                    field(
                        "lists.notable_features",
                        "Notable Features",
                        "What features are worth remembering?",
                        WorldTemplateFieldKind::StringList,
                        true,
                    ),
                    field(
                        "main.significance",
                        "Significance",
                        "Why does this place matter to the story?",
                        WorldTemplateFieldKind::LongText,
                        true,
                    ),
                ],
            ),
        },
        BuiltInTemplate {
            slug: "politics_and_social",
            display_name: "Politics & Social",
            is_collection: false,
            schema: section(
                "politics_and_social",
                "Politics & Social",
                vec![
                    field(
                        "main.governance",
                        "Governance",
                        "Who rules and how?",
                        WorldTemplateFieldKind::LongText,
                        false,
                    ),
                    field(
                        "lists.factions",
                        "Factions",
                        "What major factions exist?",
                        WorldTemplateFieldKind::StringList,
                        true,
                    ),
                    field(
                        "main.conflicts",
                        "Conflicts",
                        "What conflicts shape the political landscape?",
                        WorldTemplateFieldKind::LongText,
                        false,
                    ),
                    field(
                        "main.hierarchies",
                        "Hierarchies",
                        "What social hierarchies are in play?",
                        WorldTemplateFieldKind::LongText,
                        true,
                    ),
                    field(
                        "lists.taboos",
                        "Taboos",
                        "What's forbidden, shameful, or dangerous?",
                        WorldTemplateFieldKind::StringList,
                        true,
                    ),
                ],
            ),
        },
        BuiltInTemplate {
            slug: "culture",
            display_name: "Culture",
            is_collection: false,
            schema: section(
                "culture",
                "Culture",
                vec![
                    field(
                        "main.languages",
                        "Languages",
                        "What languages are spoken?",
                        WorldTemplateFieldKind::LongText,
                        true,
                    ),
                    field(
                        "main.religions",
                        "Religions",
                        "What religions or belief systems exist?",
                        WorldTemplateFieldKind::LongText,
                        true,
                    ),
                    field(
                        "main.art_and_ritual",
                        "Art & Ritual",
                        "What art forms and rituals matter?",
                        WorldTemplateFieldKind::LongText,
                        true,
                    ),
                    field(
                        "main.daily_life",
                        "Daily Life",
                        "What does ordinary daily life look like?",
                        WorldTemplateFieldKind::LongText,
                        false,
                    ),
                ],
            ),
        },
        BuiltInTemplate {
            slug: "world",
            display_name: "World",
            is_collection: false,
            schema: section(
                "world",
                "World",
                vec![
                    field(
                        "main.era",
                        "Era",
                        "What time period or era is this?",
                        WorldTemplateFieldKind::ShortText,
                        false,
                    ),
                    field(
                        "main.technology_level",
                        "Technology Level",
                        "Where does technology sit?",
                        WorldTemplateFieldKind::ShortText,
                        false,
                    ),
                    field(
                        "main.magic_or_speculative_rules",
                        "Magic / Speculative Rules",
                        "What rules govern magic or speculative elements (if any)?",
                        WorldTemplateFieldKind::LongText,
                        true,
                    ),
                    field(
                        "main.geography",
                        "Geography",
                        "What does the geography look like?",
                        WorldTemplateFieldKind::LongText,
                        true,
                    ),
                ],
            ),
        },
        BuiltInTemplate {
            slug: "history",
            display_name: "History",
            is_collection: false,
            schema: section(
                "history",
                "History",
                vec![
                    field(
                        "lists.timeline_beats",
                        "Timeline Beats",
                        "What major events anchor the timeline?",
                        WorldTemplateFieldKind::StringList,
                        true,
                    ),
                    field(
                        "lists.legends",
                        "Legends",
                        "What stories does this world tell itself?",
                        WorldTemplateFieldKind::StringList,
                        true,
                    ),
                    field(
                        "lists.unresolved_threads",
                        "Unresolved Threads",
                        "What mysteries or unresolved threads linger?",
                        WorldTemplateFieldKind::StringList,
                        true,
                    ),
                ],
            ),
        },
    ]
}

/// Resolves the active template for a segment: user override if
/// `template_json` is non-null, else the built-in default looked up by slug.
///
/// Errors if the segment has no override AND no matching built-in.
pub fn effective_template(db: &Db, segment_id: &Id) -> Result<WorldTemplateSchema> {
    let (slug, template_json): (String, Option<String>) = db.conn().query_row(
        "SELECT slug, template_json FROM world_segment WHERE id = ?1",
        [segment_id.as_str()],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;

    if let Some(json) = template_json {
        let parsed: WorldTemplateSchema = serde_json::from_str(&json)
            .map_err(|e| Error::Other(format!("template_json parse: {e}")))?;
        return Ok(parsed);
    }

    for t in built_in_templates() {
        if t.slug == slug {
            return Ok(t.schema);
        }
    }

    Err(Error::Other(format!(
        "no template: segment {} has slug '{slug}' which is not a built-in and has no template_json override",
        segment_id.as_str()
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_templates_has_six_segments() {
        let all = built_in_templates();
        assert_eq!(all.len(), 6, "expected 6 built-in segments; got {}", all.len());
    }

    #[test]
    fn built_in_template_slugs_are_canonical() {
        let slugs: Vec<&str> = built_in_templates().iter().map(|t| t.slug).collect();
        assert_eq!(
            slugs,
            vec![
                "concept",
                "locations",
                "politics_and_social",
                "culture",
                "world",
                "history",
            ]
        );
    }

    #[test]
    fn only_locations_is_collection() {
        for t in built_in_templates() {
            if t.slug == "locations" {
                assert!(t.is_collection, "locations should be is_collection=true");
            } else {
                assert!(
                    !t.is_collection,
                    "{} should be is_collection=false",
                    t.slug
                );
            }
        }
    }

    #[test]
    fn concept_template_has_expected_fields() {
        let t = built_in_templates()
            .into_iter()
            .find(|t| t.slug == "concept")
            .unwrap();
        let ids: Vec<&str> = t.schema.fields.iter().map(|f| f.id.as_str()).collect();
        assert_eq!(
            ids,
            vec![
                "main.core_premise",
                "main.genre",
                "main.tone",
                "lists.themes",
                "lists.inspirations",
            ]
        );
    }

    #[test]
    fn locations_template_uses_main_name_for_canonical_name() {
        let t = built_in_templates()
            .into_iter()
            .find(|t| t.slug == "locations")
            .unwrap();
        assert!(
            t.schema.fields.iter().any(|f| f.id == "main.name"),
            "locations template must have a main.name field for rename-cascade"
        );
    }

    #[test]
    fn history_template_has_three_list_fields() {
        let t = built_in_templates()
            .into_iter()
            .find(|t| t.slug == "history")
            .unwrap();
        for f in &t.schema.fields {
            assert!(
                matches!(f.kind, WorldTemplateFieldKind::StringList),
                "history field {} should be string_list",
                f.id
            );
        }
        assert_eq!(t.schema.fields.len(), 3);
    }

    #[test]
    fn effective_template_returns_built_in_when_override_is_null() {
        use crate::world::WorldStore;
        use crate::{Db, ProjectStore};
        let db = Db::open_in_memory().unwrap();
        let p = ProjectStore::new(&db).insert("P").unwrap();
        let store = WorldStore::new(&db, std::path::PathBuf::from("/tmp"));
        let id = store
            .upsert_segment(&p.id, "concept", "Concept", 0, false)
            .unwrap();
        // upsert_segment writes name + ordering + is_collection but does not
        // populate slug; force-set it so the lookup matches a built-in.
        db.conn()
            .execute(
                "UPDATE world_segment SET slug = 'concept' WHERE id = ?1",
                [id.as_str()],
            )
            .unwrap();
        let schema = effective_template(&db, &id).unwrap();
        assert_eq!(schema.id, "concept");
        assert!(schema.fields.iter().any(|f| f.id == "main.core_premise"));
    }

    #[test]
    fn effective_template_returns_override_when_present() {
        use crate::world::WorldStore;
        use crate::{Db, ProjectStore};
        let db = Db::open_in_memory().unwrap();
        let p = ProjectStore::new(&db).insert("P").unwrap();
        let store = WorldStore::new(&db, std::path::PathBuf::from("/tmp"));
        let id = store
            .upsert_segment(&p.id, "concept", "Concept", 0, false)
            .unwrap();
        // Write an override.
        let custom = WorldTemplateSchema {
            id: "concept".to_string(),
            label: "Custom Concept".to_string(),
            fields: vec![field(
                "main.tagline",
                "Tagline",
                "One-sentence tagline?",
                WorldTemplateFieldKind::ShortText,
                false,
            )],
        };
        let json = serde_json::to_string(&custom).unwrap();
        db.conn()
            .execute(
                "UPDATE world_segment SET template_json = ?1 WHERE id = ?2",
                (&json, id.as_str()),
            )
            .unwrap();
        let schema = effective_template(&db, &id).unwrap();
        assert_eq!(schema.label, "Custom Concept");
        assert!(schema.fields.iter().any(|f| f.id == "main.tagline"));
    }

    #[test]
    fn effective_template_errors_when_slug_unknown_and_no_override() {
        use crate::world::WorldStore;
        use crate::{Db, ProjectStore};
        let db = Db::open_in_memory().unwrap();
        let p = ProjectStore::new(&db).insert("P").unwrap();
        let store = WorldStore::new(&db, std::path::PathBuf::from("/tmp"));
        // Insert a segment with an unknown slug and no template override.
        let id = store
            .upsert_segment(&p.id, "fictional_kingdoms", "Fictional Kingdoms", 9, true)
            .unwrap();
        // Force-clear the slug-derived backfill to simulate user-added-without-template error state.
        db.conn()
            .execute(
                "UPDATE world_segment SET slug = 'fictional_kingdoms', template_json = NULL WHERE id = ?1",
                [id.as_str()],
            )
            .unwrap();
        let err = effective_template(&db, &id).unwrap_err();
        assert!(
            err.to_string().contains("no template"),
            "expected 'no template' in error; got {err}"
        );
    }
}
