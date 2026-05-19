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
        if coh < 0.35 && div > 0.5 {
            Some(TriggerCandidate {
                trigger_id: self.id(),
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
        };
        let project = ProjectSnapshot::default();
        let characters = CharacterRegistry::empty();
        let ctx = TriggerContext {
            telemetry: &telem,
            analysis: &analysis,
            scene: &scene,
            project: &project,
            characters: &characters,
        };
        assert!(TopicDrift.evaluate(&ctx).is_some());
    }
}
