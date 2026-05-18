use crate::orchestrator::{
    SpeakerTrack, StructuralInflection, Trigger, TriggerCandidate, TriggerContext,
};

pub struct StructuralInflectionTrigger;

impl Trigger for StructuralInflectionTrigger {
    fn id(&self) -> &'static str {
        "structural_inflection"
    }

    fn evaluate(&self, ctx: &TriggerContext<'_>) -> Option<TriggerCandidate> {
        if ctx.telemetry.cursor_classification
            == crate::orchestrator::CursorClassification::MidSentence
        {
            return None;
        }
        let kind = ctx.telemetry.structural_inflection;
        if kind == StructuralInflection::None {
            return None;
        }
        // Priority multiplier: 1.5 if scene metadata is set AND inflection
        // deviates from it; 0.6 if corresponding metadata is null.
        let multiplier = match kind {
            StructuralInflection::PovChange => {
                if ctx.scene.pov_character_id.is_some() {
                    1.5
                } else {
                    0.6
                }
            }
            StructuralInflection::LocationChange => {
                if ctx.scene.location_id.is_some() {
                    1.5
                } else {
                    0.6
                }
            }
            // User-initiated; always full priority.
            StructuralInflection::NewScene | StructuralInflection::NewChapter => 1.0,
            StructuralInflection::None => return None,
        };
        let base_priority = 5.5_f32;
        let track = match kind {
            StructuralInflection::LocationChange => SpeakerTrack::Persona, // Cartographer
            _ => {
                if ctx.scene.pov_character_id.is_some() {
                    SpeakerTrack::Character
                } else {
                    SpeakerTrack::Persona
                }
            }
        };
        Some(TriggerCandidate {
            trigger_id: self.id(),
            priority: base_priority * multiplier,
            preferred_track: track,
            reason: format!("{kind:?}"),
            block_target_id: Some(ctx.telemetry.block_id.clone()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestrator::*;
    use crate::Id;

    fn base_ctx(
        infl: StructuralInflection,
        pov: Option<Id>,
        loc: Option<Id>,
    ) -> (
        TypingTelemetry,
        AnalysisSnapshot,
        SceneSnapshot,
        ProjectSnapshot,
    ) {
        let telem = TypingTelemetry {
            idle_for_ms: 3000,
            cursor_classification: CursorClassification::AtParagraphEnd,
            block_id: "^bk-0001".to_string(),
            recent_word_delta: 0,
            structural_inflection: infl,
        };
        let analysis = AnalysisSnapshot::default();
        let scene = SceneSnapshot {
            id: Id::new(),
            pov_character_id: pov,
            location_id: loc,
            characters_present: vec![],
            word_count: 500,
            seconds_since_last_pill: 60,
        };
        (telem, analysis, scene, ProjectSnapshot::default())
    }

    #[test]
    fn pov_change_with_set_pov_is_high_priority() {
        let (telem, analysis, scene, project) =
            base_ctx(StructuralInflection::PovChange, Some(Id::new()), None);
        let ctx = TriggerContext {
            telemetry: &telem,
            analysis: &analysis,
            scene: &scene,
            project: &project,
        };
        let cand = StructuralInflectionTrigger.evaluate(&ctx).unwrap();
        assert!(cand.priority > 7.0, "got priority {}", cand.priority);
    }

    #[test]
    fn pov_change_with_null_pov_is_low_priority() {
        let (telem, analysis, scene, project) =
            base_ctx(StructuralInflection::PovChange, None, None);
        let ctx = TriggerContext {
            telemetry: &telem,
            analysis: &analysis,
            scene: &scene,
            project: &project,
        };
        let cand = StructuralInflectionTrigger.evaluate(&ctx).unwrap();
        assert!(cand.priority < 4.0, "got priority {}", cand.priority);
    }

    #[test]
    fn none_does_not_fire() {
        let (telem, analysis, scene, project) = base_ctx(StructuralInflection::None, None, None);
        let ctx = TriggerContext {
            telemetry: &telem,
            analysis: &analysis,
            scene: &scene,
            project: &project,
        };
        assert!(StructuralInflectionTrigger.evaluate(&ctx).is_none());
    }
}
