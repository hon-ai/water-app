use crate::orchestrator::{SpeakerTrack, Trigger, TriggerCandidate, TriggerContext};

/// Threshold below which the trigger considers the scene "drifting."
/// Used for both the M5 heat-derived `heat_coherence_tail` and the
/// legacy `analysis.flow` heuristic — same gate, two data sources.
const FLOW_DIP_THRESHOLD: f32 = 0.4;

/// Minimum gap since the last pill. Prevents back-to-back drift pills
/// during a sustained low-coherence patch.
const MIN_SECONDS_SINCE_LAST_PILL: u64 = 30;

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
        // M5: prefer the heat-derived trailing coherence average over
        // the legacy `flow` heuristic. Same gate semantics — both
        // signals measure "how connected is the writing." When
        // `heat_coherence_tail` is None (heat hasn't computed yet),
        // fall back to the M2 heuristic.
        let (signal, source) = match ctx.analysis.heat_coherence_tail {
            Some(c) => (c, "heat_coherence_tail"),
            None => (ctx.analysis.flow?, "flow"),
        };
        if signal < FLOW_DIP_THRESHOLD
            && ctx.scene.seconds_since_last_pill >= MIN_SECONDS_SINCE_LAST_PILL
        {
            Some(TriggerCandidate {
                trigger_id: self.id().to_string(),
                priority: 6.0,
                preferred_track: SpeakerTrack::Persona,
                reason: format!("{source}={signal:.2}"),
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
        assert!(SceneFlowDip.evaluate(&ctx).is_none());
    }

    fn mk_telem() -> TypingTelemetry {
        TypingTelemetry {
            idle_for_ms: 3000,
            cursor_classification: CursorClassification::AtParagraphEnd,
            block_id: "^bk-0001".to_string(),
            recent_word_delta: 0,
            structural_inflection: StructuralInflection::None,
        }
    }

    fn mk_scene(seconds_since_last_pill: u64) -> SceneSnapshot {
        SceneSnapshot {
            id: Id::new(),
            pov_character_id: None,
            location_id: None,
            characters_present: vec![],
            word_count: 500,
            seconds_since_last_pill,
        }
    }

    #[test]
    fn m5_prefers_heat_coherence_tail_over_legacy_flow() {
        let telem = mk_telem();
        let analysis = AnalysisSnapshot {
            flow: Some(0.9),
            heat_coherence_tail: Some(0.1),
            ..Default::default()
        };
        let scene = mk_scene(60);
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
        let cand = SceneFlowDip.evaluate(&ctx).expect("should fire");
        assert!(cand.reason.contains("heat_coherence_tail"), "got {:?}", cand.reason);
    }

    #[test]
    fn m5_falls_back_to_legacy_flow_when_heat_absent() {
        let telem = mk_telem();
        let analysis = AnalysisSnapshot {
            flow: Some(0.2),
            heat_coherence_tail: None,
            ..Default::default()
        };
        let scene = mk_scene(60);
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
        let cand = SceneFlowDip.evaluate(&ctx).expect("should fire");
        assert!(cand.reason.contains("flow=") && !cand.reason.contains("heat"),
            "got {:?}", cand.reason);
    }

    #[test]
    fn m5_does_not_fire_when_heat_coherence_tail_above_floor() {
        let telem = mk_telem();
        let analysis = AnalysisSnapshot {
            flow: Some(0.1),
            heat_coherence_tail: Some(0.7),
            ..Default::default()
        };
        let scene = mk_scene(60);
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
        assert!(SceneFlowDip.evaluate(&ctx).is_none());
    }
}
