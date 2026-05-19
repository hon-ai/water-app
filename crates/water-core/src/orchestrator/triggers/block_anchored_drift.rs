use crate::orchestrator::{SpeakerTrack, Trigger, TriggerCandidate, TriggerContext};

pub struct BlockAnchoredDrift;

impl Trigger for BlockAnchoredDrift {
    fn id(&self) -> &'static str {
        "block_anchored_drift"
    }

    fn evaluate(&self, ctx: &TriggerContext<'_>) -> Option<TriggerCandidate> {
        if ctx.telemetry.cursor_classification
            == crate::orchestrator::CursorClassification::MidSentence
        {
            return None;
        }
        let m = ctx.analysis.block_metrics.get(&ctx.telemetry.block_id)?;
        let div = m.divergence.unwrap_or(0.0);
        let coh = m.coherence.unwrap_or(1.0);
        if div > 0.6 || coh < 0.35 {
            Some(TriggerCandidate {
                trigger_id: self.id().to_string(),
                priority: 8.0,
                preferred_track: SpeakerTrack::Either,
                reason: format!("divergence={div:.2} coherence={coh:.2}"),
                block_target_id: Some(ctx.telemetry.block_id.clone()),
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

    fn make_ctx(
        cursor: CursorClassification,
        block_id: &str,
        div: f32,
        coh: f32,
    ) -> (
        TypingTelemetry,
        AnalysisSnapshot,
        SceneSnapshot,
        ProjectSnapshot,
    ) {
        let telem = TypingTelemetry {
            idle_for_ms: 3000,
            cursor_classification: cursor,
            block_id: block_id.to_string(),
            recent_word_delta: 0,
            structural_inflection: StructuralInflection::None,
        };
        let mut block_metrics = std::collections::HashMap::new();
        block_metrics.insert(
            block_id.to_string(),
            BlockMetrics {
                flow: Some(0.5),
                coherence: Some(coh),
                divergence: Some(div),
            },
        );
        let analysis = AnalysisSnapshot {
            block_metrics,
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
        (telem, analysis, scene, project)
    }

    #[test]
    fn fires_on_high_divergence() {
        let (telem, analysis, scene, project) =
            make_ctx(CursorClassification::AtParagraphEnd, "^bk-0001", 0.75, 0.5);
        let characters = CharacterRegistry::empty();
        let ctx = TriggerContext {
            telemetry: &telem,
            analysis: &analysis,
            scene: &scene,
            project: &project,
            characters: &characters,
        };
        assert!(BlockAnchoredDrift.evaluate(&ctx).is_some());
    }

    #[test]
    fn fires_on_low_coherence() {
        let (telem, analysis, scene, project) =
            make_ctx(CursorClassification::AtParagraphEnd, "^bk-0001", 0.3, 0.2);
        let characters = CharacterRegistry::empty();
        let ctx = TriggerContext {
            telemetry: &telem,
            analysis: &analysis,
            scene: &scene,
            project: &project,
            characters: &characters,
        };
        assert!(BlockAnchoredDrift.evaluate(&ctx).is_some());
    }

    #[test]
    fn does_not_fire_mid_sentence() {
        let (telem, analysis, scene, project) =
            make_ctx(CursorClassification::MidSentence, "^bk-0001", 0.9, 0.1);
        let characters = CharacterRegistry::empty();
        let ctx = TriggerContext {
            telemetry: &telem,
            analysis: &analysis,
            scene: &scene,
            project: &project,
            characters: &characters,
        };
        assert!(BlockAnchoredDrift.evaluate(&ctx).is_none());
    }

    #[test]
    fn does_not_fire_when_metrics_normal() {
        let (telem, analysis, scene, project) =
            make_ctx(CursorClassification::AtParagraphEnd, "^bk-0001", 0.3, 0.7);
        let characters = CharacterRegistry::empty();
        let ctx = TriggerContext {
            telemetry: &telem,
            analysis: &analysis,
            scene: &scene,
            project: &project,
            characters: &characters,
        };
        assert!(BlockAnchoredDrift.evaluate(&ctx).is_none());
    }
}
