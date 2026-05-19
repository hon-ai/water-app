use crate::orchestrator::{SpeakerTrack, Trigger, TriggerCandidate, TriggerContext};

pub struct PaceFloor;

impl Trigger for PaceFloor {
    fn id(&self) -> &'static str {
        "pace_floor"
    }

    fn evaluate(&self, ctx: &TriggerContext<'_>) -> Option<TriggerCandidate> {
        if ctx.telemetry.cursor_classification
            == crate::orchestrator::CursorClassification::MidSentence
        {
            return None;
        }
        let pace = ctx.analysis.pace?;
        // recent_word_delta is words in last 10s; convert to last 3 min by
        // requiring sustained low pace (caller's debounce ensures this).
        if pace < 0.3 && ctx.telemetry.recent_word_delta.unsigned_abs() < 40 {
            Some(TriggerCandidate {
                trigger_id: self.id().to_string(),
                priority: 5.0,
                preferred_track: SpeakerTrack::Persona,
                reason: format!(
                    "pace={pace:.2} word_delta={}",
                    ctx.telemetry.recent_word_delta
                ),
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
    fn fires_on_low_pace_and_low_word_delta() {
        let telem = TypingTelemetry {
            idle_for_ms: 3000,
            cursor_classification: CursorClassification::AtParagraphEnd,
            block_id: "^bk-0001".to_string(),
            recent_word_delta: 10,
            structural_inflection: StructuralInflection::None,
        };
        let analysis = AnalysisSnapshot {
            pace: Some(0.2),
            ..Default::default()
        };
        let scene = SceneSnapshot {
            id: Id::new(),
            pov_character_id: None,
            location_id: None,
            characters_present: vec![],
            word_count: 500,
            seconds_since_last_pill: 60,
        };
        let project = ProjectSnapshot::default();
        let characters = CharacterRegistry::empty();
        let ctx = TriggerContext {
            telemetry: &telem,
            analysis: &analysis,
            scene: &scene,
            project: &project,
            characters: &characters,
            prompts: crate::orchestrator::test_util::test_prompts(),
        };
        assert!(PaceFloor.evaluate(&ctx).is_some());
    }
}
