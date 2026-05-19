//! Conversational Intake schema descriptors.
//!
//! Each `IntakeField` describes one question in a schema. The
//! `ConversationalIntake` renderer reads these via the Tauri command
//! `intake_schema` and walks them one at a time. Per-answer commits write
//! back through `character_update_field`.
//!
//! LSM v2.1 is the only schema in M3. M4 will add World Bible segment
//! schemas against the same `IntakeField` type.

use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case", tag = "type", content = "options")]
pub enum IntakeFieldKind {
    ShortText,
    LongText,
    StringList,
    Choice(&'static [&'static str]),
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct IntakeField {
    pub id: &'static str,
    pub section: &'static str,
    pub label: &'static str,
    pub prompt_question: &'static str,
    pub helper: Option<&'static str>,
    pub examples: &'static [&'static str],
    pub kind: IntakeFieldKind,
    pub optional_skip: bool,
}

// ---- main (12 fields) ----

pub const LSM_MAIN: &[IntakeField] = &[
    IntakeField {
        id: "main.full_name",
        section: "main",
        label: "Full name",
        prompt_question: "What is this character's full name?",
        helper: None,
        examples: &["Marcus Vale", "Ada Thorne"],
        kind: IntakeFieldKind::ShortText,
        optional_skip: false,
    },
    IntakeField {
        id: "main.aliases",
        section: "main",
        label: "Aliases",
        prompt_question: "What other names is this character known by? (Nicknames, titles, pen names.)",
        helper: None,
        examples: &[],
        kind: IntakeFieldKind::StringList,
        optional_skip: true,
    },
    IntakeField {
        id: "main.age",
        section: "main",
        label: "Age",
        prompt_question: "How old are they at the start of the story?",
        helper: None,
        examples: &["32", "early 40s", "ageless"],
        kind: IntakeFieldKind::ShortText,
        optional_skip: true,
    },
    IntakeField {
        id: "main.pronouns",
        section: "main",
        label: "Pronouns",
        prompt_question: "What pronouns?",
        helper: None,
        examples: &["she/her", "they/them", "he/him"],
        kind: IntakeFieldKind::ShortText,
        optional_skip: true,
    },
    IntakeField {
        id: "main.role_in_story",
        section: "main",
        label: "Role in story",
        prompt_question: "What role does this character play in the story?",
        helper: None,
        examples: &[],
        kind: IntakeFieldKind::Choice(&["protagonist", "antagonist", "supporting", "mentor", "foil", "other"]),
        optional_skip: false,
    },
    IntakeField {
        id: "main.want",
        section: "main",
        label: "Want",
        prompt_question: "What do they want? What are they consciously pursuing?",
        helper: None,
        examples: &[],
        kind: IntakeFieldKind::LongText,
        optional_skip: false,
    },
    IntakeField {
        id: "main.need",
        section: "main",
        label: "Need",
        prompt_question: "What do they actually need? What would heal them, even if they don't see it?",
        helper: None,
        examples: &[],
        kind: IntakeFieldKind::LongText,
        optional_skip: false,
    },
    IntakeField {
        id: "main.ghost_wound",
        section: "main",
        label: "Ghost wound",
        prompt_question: "What past event still haunts them? What unhealed thing shapes who they are today?",
        helper: None,
        examples: &[],
        kind: IntakeFieldKind::LongText,
        optional_skip: true,
    },
    IntakeField {
        id: "main.lie_they_believe",
        section: "main",
        label: "Lie they believe",
        prompt_question: "What false belief do they hold about themselves or the world? What story do they tell themselves that isn't quite true?",
        helper: None,
        examples: &[],
        kind: IntakeFieldKind::LongText,
        optional_skip: false,
    },
    IntakeField {
        id: "main.truth",
        section: "main",
        label: "Truth",
        prompt_question: "What truth would set them free if they could see it?",
        helper: None,
        examples: &[],
        kind: IntakeFieldKind::LongText,
        optional_skip: true,
    },
    IntakeField {
        id: "main.fatal_flaw",
        section: "main",
        label: "Fatal flaw",
        prompt_question: "What character trait will most likely undo them in this story?",
        helper: None,
        examples: &[],
        kind: IntakeFieldKind::LongText,
        optional_skip: true,
    },
    IntakeField {
        id: "main.strength",
        section: "main",
        label: "Strength",
        prompt_question: "What is their greatest virtue or capacity?",
        helper: None,
        examples: &[],
        kind: IntakeFieldKind::LongText,
        optional_skip: true,
    },
];

// ---- bonus_traits (8 fields) ----

pub const LSM_BONUS_TRAITS: &[IntakeField] = &[
    IntakeField {
        id: "bonus_traits.voice",
        section: "bonus_traits",
        label: "Voice",
        prompt_question: "How would you describe their voice? (Cadence, register, tone — not what they say but how they sound.)",
        helper: None,
        examples: &["spare, weather-worn, with quiet warmth", "clipped and precise, like a lawyer"],
        kind: IntakeFieldKind::LongText,
        optional_skip: false,
    },
    IntakeField {
        id: "bonus_traits.tells",
        section: "bonus_traits",
        label: "Tells",
        prompt_question: "What do they do without realizing it? (Physical or verbal tells.)",
        helper: None,
        examples: &[],
        kind: IntakeFieldKind::StringList,
        optional_skip: true,
    },
    IntakeField {
        id: "bonus_traits.habits",
        section: "bonus_traits",
        label: "Habits",
        prompt_question: "What recurring small actions or rituals shape their day?",
        helper: None,
        examples: &[],
        kind: IntakeFieldKind::StringList,
        optional_skip: true,
    },
    IntakeField {
        id: "bonus_traits.speech_patterns",
        section: "bonus_traits",
        label: "Speech patterns",
        prompt_question: "What phrases, fillers, or quirks of speech recur in their dialogue?",
        helper: None,
        examples: &[],
        kind: IntakeFieldKind::StringList,
        optional_skip: true,
    },
    IntakeField {
        id: "bonus_traits.physicality",
        section: "bonus_traits",
        label: "Physicality",
        prompt_question: "How do they move? How do they hold themselves in a room?",
        helper: None,
        examples: &[],
        kind: IntakeFieldKind::LongText,
        optional_skip: true,
    },
    IntakeField {
        id: "bonus_traits.preferences",
        section: "bonus_traits",
        label: "Preferences",
        prompt_question: "Any strong likes, dislikes, or aesthetic preferences? (One per line: `coffee: bitter, no sugar`.)",
        helper: None,
        examples: &[],
        kind: IntakeFieldKind::StringList,
        optional_skip: true,
    },
    IntakeField {
        id: "bonus_traits.fears",
        section: "bonus_traits",
        label: "Fears",
        prompt_question: "What are they most afraid of? (Not phobias — the real fears.)",
        helper: None,
        examples: &[],
        kind: IntakeFieldKind::StringList,
        optional_skip: false,
    },
    IntakeField {
        id: "bonus_traits.values",
        section: "bonus_traits",
        label: "Values",
        prompt_question: "What do they hold sacred? What would they refuse to compromise on?",
        helper: None,
        examples: &[],
        kind: IntakeFieldKind::StringList,
        optional_skip: false,
    },
];

// ---- arc (5 fields) ----

pub const LSM_ARC: &[IntakeField] = &[
    IntakeField {
        id: "arc.starting_state",
        section: "arc",
        label: "Starting state",
        prompt_question:
            "Where is this character emotionally / morally / situationally when the story begins?",
        helper: None,
        examples: &[],
        kind: IntakeFieldKind::LongText,
        optional_skip: true,
    },
    IntakeField {
        id: "arc.ending_state",
        section: "arc",
        label: "Ending state",
        prompt_question: "Where are they by the end?",
        helper: None,
        examples: &[],
        kind: IntakeFieldKind::LongText,
        optional_skip: true,
    },
    IntakeField {
        id: "arc.inciting_change",
        section: "arc",
        label: "Inciting change",
        prompt_question: "What event in the early story knocks them out of equilibrium?",
        helper: None,
        examples: &[],
        kind: IntakeFieldKind::LongText,
        optional_skip: true,
    },
    IntakeField {
        id: "arc.midpoint_shift",
        section: "arc",
        label: "Midpoint shift",
        prompt_question: "What changes at the midpoint? What do they finally see, or refuse?",
        helper: None,
        examples: &[],
        kind: IntakeFieldKind::LongText,
        optional_skip: true,
    },
    IntakeField {
        id: "arc.climax_choice",
        section: "arc",
        label: "Climax choice",
        prompt_question: "What choice defines them at the climax?",
        helper: None,
        examples: &[],
        kind: IntakeFieldKind::LongText,
        optional_skip: true,
    },
];

// ---- perspectives (4 fields) ----

pub const LSM_PERSPECTIVES: &[IntakeField] = &[
    IntakeField {
        id: "perspectives.self_view",
        section: "perspectives",
        label: "Self view",
        prompt_question: "How do they see themselves?",
        helper: None,
        examples: &[],
        kind: IntakeFieldKind::LongText,
        optional_skip: true,
    },
    IntakeField {
        id: "perspectives.others_view",
        section: "perspectives",
        label: "Others view",
        prompt_question: "How do other characters in the story see them?",
        helper: None,
        examples: &[],
        kind: IntakeFieldKind::LongText,
        optional_skip: true,
    },
    IntakeField {
        id: "perspectives.narrator_view",
        section: "perspectives",
        label: "Narrator view",
        prompt_question: "How does the narrative voice (whether explicit or implicit) frame them?",
        helper: None,
        examples: &[],
        kind: IntakeFieldKind::LongText,
        optional_skip: true,
    },
    IntakeField {
        id: "perspectives.antagonist_view",
        section: "perspectives",
        label: "Antagonist view",
        prompt_question: "How would their antagonist describe them?",
        helper: None,
        examples: &[],
        kind: IntakeFieldKind::LongText,
        optional_skip: true,
    },
];

pub const LSM_V2_1: &[(&str, &[IntakeField])] = &[
    ("main", LSM_MAIN),
    ("bonus_traits", LSM_BONUS_TRAITS),
    ("arc", LSM_ARC),
    ("perspectives", LSM_PERSPECTIVES),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lsm_v2_1_has_29_fields_total() {
        let total =
            LSM_MAIN.len() + LSM_BONUS_TRAITS.len() + LSM_ARC.len() + LSM_PERSPECTIVES.len();
        assert_eq!(total, 29);
    }

    #[test]
    fn lsm_v2_1_has_8_required_fields() {
        let required = LSM_MAIN
            .iter()
            .chain(LSM_BONUS_TRAITS)
            .chain(LSM_ARC)
            .chain(LSM_PERSPECTIVES)
            .filter(|f| !f.optional_skip)
            .count();
        assert_eq!(required, 8);
    }

    #[test]
    fn lsm_v2_1_field_ids_are_unique() {
        let mut all: Vec<&str> = LSM_V2_1
            .iter()
            .flat_map(|(_, fields)| fields.iter().map(|f| f.id))
            .collect();
        all.sort_unstable();
        let original_len = all.len();
        all.dedup();
        assert_eq!(all.len(), original_len, "duplicate field id in LSM_V2_1");
    }

    #[test]
    fn lsm_v2_1_field_ids_are_dotted_paths() {
        for (section_name, fields) in LSM_V2_1 {
            for field in *fields {
                let prefix = format!("{section_name}.");
                assert!(
                    field.id.starts_with(&prefix),
                    "field {} should start with {}",
                    field.id,
                    prefix
                );
                assert_eq!(field.section, *section_name);
            }
        }
    }
}
