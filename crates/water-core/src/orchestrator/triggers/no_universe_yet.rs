use crate::orchestrator::{SpeakerTrack, Trigger, TriggerCandidate, TriggerContext};

pub struct NoUniverseYet;

impl Trigger for NoUniverseYet {
    fn id(&self) -> &'static str {
        "no_universe_yet"
    }

    fn evaluate(&self, ctx: &TriggerContext<'_>) -> Option<TriggerCandidate> {
        if ctx.telemetry.cursor_classification
            == crate::orchestrator::CursorClassification::MidSentence
        {
            return None;
        }
        if ctx.project.character_count == 0
            && ctx.project.world_entry_count == 0
            && ctx.scene.word_count > 200
        {
            Some(TriggerCandidate {
                trigger_id: self.id().to_string(),
                priority: 4.5,
                preferred_track: SpeakerTrack::Persona, // Chorus
                reason: "eliciting_mode".to_string(),
                block_target_id: None,
                requires_confirmation: None,
            })
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::character::registry::CharacterRegistry;
    use crate::orchestrator::*;
    use crate::Id;

    #[test]
    fn fires_when_project_is_empty_and_text_has_grown() {
        let telem = TypingTelemetry {
            idle_for_ms: 3000,
            cursor_classification: CursorClassification::AtParagraphEnd,
            block_id: "^bk-0001".to_string(),
            recent_word_delta: 0,
            structural_inflection: StructuralInflection::None,
        };
        let analysis = AnalysisSnapshot::default();
        let scene = SceneSnapshot {
            id: Id::new(),
            pov_character_id: None,
            location_id: None,
            characters_present: vec![],
            word_count: 250,
            seconds_since_last_pill: 60,
        };
        let project = ProjectSnapshot {
            character_count: 0,
            world_entry_count: 0,
        };
        let characters = CharacterRegistry::empty();
        let ctx = TriggerContext {
            telemetry: &telem,
            analysis: &analysis,
            scene: &scene,
            project: &project,
            characters: &characters,
        };
        assert!(NoUniverseYet.evaluate(&ctx).is_some());
    }

    #[test]
    fn does_not_fire_when_project_has_characters() {
        let telem = TypingTelemetry {
            idle_for_ms: 3000,
            cursor_classification: CursorClassification::AtParagraphEnd,
            block_id: "^bk-0001".to_string(),
            recent_word_delta: 0,
            structural_inflection: StructuralInflection::None,
        };
        let analysis = AnalysisSnapshot::default();
        let scene = SceneSnapshot {
            id: Id::new(),
            pov_character_id: None,
            location_id: None,
            characters_present: vec![],
            word_count: 250,
            seconds_since_last_pill: 60,
        };
        let project = ProjectSnapshot {
            character_count: 2,
            world_entry_count: 0,
        };
        let characters = CharacterRegistry::empty();
        let ctx = TriggerContext {
            telemetry: &telem,
            analysis: &analysis,
            scene: &scene,
            project: &project,
            characters: &characters,
        };
        assert!(NoUniverseYet.evaluate(&ctx).is_none());
    }
}
