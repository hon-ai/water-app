//! `character_dissonance` Stage 1 — lemma-overlap gate.
//!
//! For each present character, computes Jaccard lemma overlap between the
//! just-finished paragraph and three character fields:
//!   - `bonus_traits.values` (joined)
//!   - `bonus_traits.fears` (joined)
//!   - `main.lie_they_believe`
//!
//! If overlap >= 0.30 for any field, fires a `TriggerCandidate` carrying a
//! `requires_confirmation` request. Stage 2 (LLM yes/no) happens in the
//! orchestrator service (M3 T11).

use crate::orchestrator::lemma_overlap::overlap;
use crate::orchestrator::{
    CursorClassification, SpeakerTrack, Trigger, TriggerCandidate, TriggerContext,
};

const GATE_THRESHOLD: f32 = 0.30;

pub struct CharacterDissonance;

impl Trigger for CharacterDissonance {
    fn id(&self) -> &'static str {
        "character_dissonance"
    }

    fn evaluate(&self, ctx: &TriggerContext<'_>) -> Option<TriggerCandidate> {
        if ctx.telemetry.cursor_classification == CursorClassification::MidSentence {
            return None;
        }
        let paragraph = ctx.analysis.last_block_text.as_deref()?;
        for char_id in &ctx.scene.characters_present {
            let Some(_speaker) = ctx.characters.by_id(char_id.as_str()) else {
                continue;
            };
            let Some(row) = ctx.characters.list().iter().find(|r| r.id == *char_id) else {
                continue;
            };
            let sheet = &row.data;
            let candidates: &[(&'static str, String)] = &[
                ("values", read_list_joined(sheet, "bonus_traits", "values")),
                ("fears", read_list_joined(sheet, "bonus_traits", "fears")),
                (
                    "lie_they_believe",
                    read_str(sheet, "main", "lie_they_believe"),
                ),
            ];
            for (field_label, field_value) in candidates {
                if field_value.is_empty() {
                    continue;
                }
                let ovl = overlap(paragraph, field_value);
                if ovl >= GATE_THRESHOLD {
                    // Render Stage-2 confirmation from the built-in
                    // `pill_dissonance_check` TOML. If rendering fails
                    // (only possible if the id is somehow missing), drop
                    // the candidate — better silent than a half-built
                    // confirmation surface.
                    let req = ctx
                        .prompts
                        .render_confirmation_request(
                            "pill_dissonance_check",
                            &[
                                ("full_name", row.name.as_str()),
                                ("field_label", field_label),
                                ("field_value", field_value.as_str()),
                                ("paragraph_text", paragraph),
                            ],
                        )
                        .ok()?;
                    return Some(TriggerCandidate {
                        trigger_id: self.id().to_string(),
                        priority: 5.5,
                        preferred_track: SpeakerTrack::Character,
                        reason: format!("dissonance_gate field={field_label} overlap={ovl:.2}"),
                        block_target_id: Some(ctx.telemetry.block_id.clone()),
                        requires_confirmation: Some(req),
                    });
                }
            }
        }
        None
    }
}

fn read_str(sheet: &serde_json::Value, section: &str, key: &str) -> String {
    sheet
        .get(section)
        .and_then(|s| s.get(key))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string()
}

fn read_list_joined(sheet: &serde_json::Value, section: &str, key: &str) -> String {
    let arr = sheet
        .get(section)
        .and_then(|s| s.get(key))
        .and_then(|v| v.as_array());
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::character::registry::{CharacterRegistry, CharacterRegistryRow};
    use crate::orchestrator::*;
    use crate::Id;

    fn build_ctx(
        last_block: Option<&str>,
        character_data: serde_json::Value,
        cursor: CursorClassification,
    ) -> (
        TypingTelemetry,
        AnalysisSnapshot,
        SceneSnapshot,
        ProjectSnapshot,
        CharacterRegistry,
    ) {
        let char_id: Id = "01HE000000000000000000000C".parse().unwrap();
        let telem = TypingTelemetry {
            idle_for_ms: 4000,
            cursor_classification: cursor,
            block_id: "^bk-0001".to_string(),
            recent_word_delta: 0,
            structural_inflection: StructuralInflection::None,
        };
        let analysis = AnalysisSnapshot {
            last_block_text: last_block.map(str::to_string),
            ..AnalysisSnapshot::default()
        };
        let scene = SceneSnapshot {
            id: Id::new(),
            pov_character_id: Some(char_id.clone()),
            location_id: None,
            characters_present: vec![char_id.clone()],
            word_count: 500,
            seconds_since_last_pill: 60,
        };
        let mut reg = CharacterRegistry::empty();
        reg.insert_for_test(CharacterRegistryRow {
            id: char_id,
            name: "Marcus".to_string(),
            hue_token: "--water-hue-character-1".to_string(),
            data: character_data,
        });
        (telem, analysis, scene, ProjectSnapshot::default(), reg)
    }

    fn make_ctx<'a>(
        telem: &'a TypingTelemetry,
        analysis: &'a AnalysisSnapshot,
        scene: &'a SceneSnapshot,
        project: &'a ProjectSnapshot,
        registry: &'a CharacterRegistry,
    ) -> TriggerContext<'a> {
        TriggerContext {
            telemetry: telem,
            analysis,
            scene,
            project,
            characters: registry,
            world_registry: crate::orchestrator::test_util::test_world_registry(),
            prompts: crate::orchestrator::test_util::test_prompts(),
        }
    }

    #[test]
    fn fires_when_paragraph_overlaps_character_values() {
        // Crafted to clear the 0.30 Jaccard threshold after stopword + suffix
        // stripping. Tokens roughly: paragraph -> {loyalty, show, up, mean},
        // field -> {loyalty, show, up}; overlap >= 0.30.
        let (telem, analysis, scene, project, registry) = build_ctx(
            Some("Loyalty means showing up."),
            serde_json::json!({
                "bonus_traits": { "values": ["loyalty", "showing up"] }
            }),
            CursorClassification::AtParagraphEnd,
        );
        let ctx = make_ctx(&telem, &analysis, &scene, &project, &registry);
        let cand = CharacterDissonance
            .evaluate(&ctx)
            .expect("expected fire on values-overlap");
        assert_eq!(cand.trigger_id, "character_dissonance");
        assert!(
            cand.requires_confirmation.is_some(),
            "stage 2 LLM confirmation required"
        );
        assert_eq!(cand.preferred_track, SpeakerTrack::Character);
    }

    #[test]
    fn does_not_fire_when_no_overlap() {
        let (telem, analysis, scene, project, registry) = build_ctx(
            Some("The rain fell softly on the roof."),
            serde_json::json!({
                "bonus_traits": { "values": ["loyalty", "showing up"] }
            }),
            CursorClassification::AtParagraphEnd,
        );
        let ctx = make_ctx(&telem, &analysis, &scene, &project, &registry);
        assert!(CharacterDissonance.evaluate(&ctx).is_none());
    }

    #[test]
    fn does_not_fire_mid_sentence() {
        let (telem, analysis, scene, project, registry) = build_ctx(
            Some("He had always valued loyalty above all things"),
            serde_json::json!({"bonus_traits": {"values": ["loyalty"]}}),
            CursorClassification::MidSentence,
        );
        let ctx = make_ctx(&telem, &analysis, &scene, &project, &registry);
        assert!(CharacterDissonance.evaluate(&ctx).is_none());
    }

    #[test]
    fn does_not_fire_without_last_block_text() {
        let (telem, analysis, scene, project, registry) = build_ctx(
            None,
            serde_json::json!({"bonus_traits":{"values":["loyalty"]}}),
            CursorClassification::AtParagraphEnd,
        );
        let ctx = make_ctx(&telem, &analysis, &scene, &project, &registry);
        assert!(CharacterDissonance.evaluate(&ctx).is_none());
    }
}
