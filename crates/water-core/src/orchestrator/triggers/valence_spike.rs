use crate::orchestrator::{SpeakerTrack, Trigger, TriggerCandidate, TriggerContext};

pub struct ValenceSpike;

impl Trigger for ValenceSpike {
    fn id(&self) -> &'static str {
        "valence_spike"
    }

    #[allow(clippy::cast_precision_loss)]
    fn evaluate(&self, ctx: &TriggerContext<'_>) -> Option<TriggerCandidate> {
        if ctx.telemetry.cursor_classification
            == crate::orchestrator::CursorClassification::MidSentence
        {
            return None;
        }
        let current = *ctx.analysis.valence_history.last()?;
        if ctx.analysis.valence_history.len() < 3 {
            return None;
        }
        let scene_mean: f32 = ctx.analysis.valence_history.iter().sum::<f32>()
            / ctx.analysis.valence_history.len() as f32;
        let delta = (current - scene_mean).abs();
        if delta > 0.4 {
            let track = if ctx.scene.characters_present.is_empty() {
                SpeakerTrack::Persona
            } else {
                SpeakerTrack::Character
            };
            Some(TriggerCandidate {
                trigger_id: self.id().to_string(),
                priority: 6.5,
                preferred_track: track,
                reason: format!("valence_delta={delta:.2}"),
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
    fn fires_on_large_valence_swing() {
        let telem = TypingTelemetry {
            idle_for_ms: 3000,
            cursor_classification: CursorClassification::AtParagraphEnd,
            block_id: "^bk-0001".to_string(),
            recent_word_delta: 0,
            structural_inflection: StructuralInflection::None,
        };
        let analysis = AnalysisSnapshot {
            valence_history: vec![0.1, 0.1, 0.1, 0.7], // mean ~0.25, delta ~0.45
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
            world_registry: crate::orchestrator::test_util::test_world_registry(),
            prompts: crate::orchestrator::test_util::test_prompts(),
        };
        assert!(ValenceSpike.evaluate(&ctx).is_some());
    }
}
