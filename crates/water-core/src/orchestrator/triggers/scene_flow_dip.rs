use crate::orchestrator::{SpeakerTrack, Trigger, TriggerCandidate, TriggerContext};

pub struct SceneFlowDip;

impl Trigger for SceneFlowDip {
    fn id(&self) -> &'static str {
        "scene_flow_dip"
    }

    fn evaluate(&self, ctx: &TriggerContext<'_>) -> Option<TriggerCandidate> {
        if ctx.telemetry.cursor_classification
            == crate::orchestrator::CursorClassification::MidSentence
        {
            return None;
        }
        let flow = ctx.analysis.flow?;
        if flow < 0.4 && ctx.scene.seconds_since_last_pill >= 30 {
            Some(TriggerCandidate {
                trigger_id: self.id(),
                priority: 6.0,
                preferred_track: SpeakerTrack::Persona,
                reason: format!("flow={flow:.2}"),
                block_target_id: None,
            })
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestrator::*;
    use crate::Id;

    #[test]
    fn fires_on_low_flow_sustained() {
        let telem = TypingTelemetry {
            idle_for_ms: 3000,
            cursor_classification: CursorClassification::AtParagraphEnd,
            block_id: "^bk-0001".to_string(),
            recent_word_delta: 0,
            structural_inflection: StructuralInflection::None,
        };
        let analysis = AnalysisSnapshot {
            flow: Some(0.3),
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
        let ctx = TriggerContext {
            telemetry: &telem,
            analysis: &analysis,
            scene: &scene,
            project: &project,
        };
        assert!(SceneFlowDip.evaluate(&ctx).is_some());
    }

    #[test]
    fn does_not_fire_when_pill_recent() {
        let telem = TypingTelemetry {
            idle_for_ms: 3000,
            cursor_classification: CursorClassification::AtParagraphEnd,
            block_id: "^bk-0001".to_string(),
            recent_word_delta: 0,
            structural_inflection: StructuralInflection::None,
        };
        let analysis = AnalysisSnapshot {
            flow: Some(0.3),
            ..Default::default()
        };
        let scene = SceneSnapshot {
            id: Id::new(),
            pov_character_id: None,
            location_id: None,
            characters_present: vec![],
            word_count: 500,
            seconds_since_last_pill: 5,
        };
        let project = ProjectSnapshot::default();
        let ctx = TriggerContext {
            telemetry: &telem,
            analysis: &analysis,
            scene: &scene,
            project: &project,
        };
        assert!(SceneFlowDip.evaluate(&ctx).is_none());
    }
}
