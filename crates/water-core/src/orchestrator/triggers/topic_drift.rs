use crate::orchestrator::feedback::{loosen_above, loosen_below};
use crate::orchestrator::{SpeakerTrack, Trigger, TriggerCandidate, TriggerContext};

pub struct TopicDrift;

impl Trigger for TopicDrift {
    fn id(&self) -> &'static str {
        "topic_drift"
    }

    fn evaluate(&self, ctx: &TriggerContext<'_>) -> Option<TriggerCandidate> {
        if ctx.telemetry.cursor_classification
            == crate::orchestrator::CursorClassification::MidSentence
        {
            return None;
        }
        let coh = ctx.analysis.coherence?;
        let div = ctx.analysis.divergence?;
        let s = ctx.tuning.sensitivity_for(self.id());
        let coh_bar = loosen_below(0.35, s);
        let div_bar = loosen_above(0.5, s);
        if coh < coh_bar && div > div_bar {
            Some(TriggerCandidate {
                trigger_id: self.id().to_string(),
                priority: 7.0,
                preferred_track: SpeakerTrack::Either,
                reason: format!("coherence={coh:.2} divergence={div:.2}"),
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
    fn fires_on_low_coherence_high_divergence() {
        let telem = TypingTelemetry {
            idle_for_ms: 3000,
            cursor_classification: CursorClassification::AtParagraphEnd,
            block_id: "^bk-0001".to_string(),
            recent_word_delta: 0,
            structural_inflection: StructuralInflection::None,
        };
        let analysis = AnalysisSnapshot {
            coherence: Some(0.2),
            divergence: Some(0.7),
            ..Default::default()
        };
        let scene = SceneSnapshot {
            id: Id::new(),
            pov_character_id: None,
            location_id: None,
            characters_present: vec![],
            word_count: 500,
            seconds_since_last_pill: 60,
            scene_ordering: None,
            manuscript_scene_count: None,
        };
        let project = ProjectSnapshot::default();
        let characters = CharacterRegistry::empty();
        let ctx = TriggerContext {
            telemetry: &telem,
            analysis: &analysis,
            scene: &scene,
            project: &project,
            characters: &characters,
            world_registry: crate::orchestrator::test_util::test_world_registry(),
            prompts: crate::orchestrator::test_util::test_prompts(),
            tuning: crate::orchestrator::test_util::test_tuning(),
        };
        assert!(TopicDrift.evaluate(&ctx).is_some());
    }
}
