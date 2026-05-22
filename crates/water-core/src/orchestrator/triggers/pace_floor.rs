use crate::orchestrator::{SpeakerTrack, Trigger, TriggerCandidate, TriggerContext};

/// Threshold below which the writer is judged "stalled." Same value used
/// for both the M5 heat-derived `heat_pace_tail` and the legacy
/// `analysis.pace` heuristic — the M5 swap is a data-source change, not
/// a semantic change.
const PACE_FLOOR_THRESHOLD: f32 = 0.3;

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
        // M5: prefer the heat-derived trailing pacing average over the
        // per-tick `pace` heuristic. When `heat_pace_tail` is None
        // (heat hasn't computed yet, or HeatStore is empty), fall back
        // to the M2 heuristic so brand-new scenes don't go silent.
        let (pace, source) = match ctx.analysis.heat_pace_tail {
            Some(p) => (p, "heat_pace_tail"),
            None => (ctx.analysis.pace?, "pace"),
        };
        // recent_word_delta is words in last 10s; convert to last 3 min by
        // requiring sustained low pace (caller's debounce ensures this).
        if pace < PACE_FLOOR_THRESHOLD && ctx.telemetry.recent_word_delta.unsigned_abs() < 40 {
            Some(TriggerCandidate {
                trigger_id: self.id().to_string(),
                priority: 5.0,
                preferred_track: SpeakerTrack::Persona,
                reason: format!(
                    "{source}={pace:.2} word_delta={}",
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

    fn mk_telem(recent_word_delta: i32) -> TypingTelemetry {
        TypingTelemetry {
            idle_for_ms: 3000,
            cursor_classification: CursorClassification::AtParagraphEnd,
            block_id: "^bk-0001".to_string(),
            recent_word_delta,
            structural_inflection: StructuralInflection::None,
        }
    }

    fn mk_scene() -> SceneSnapshot {
        SceneSnapshot {
            id: Id::new(),
            pov_character_id: None,
            location_id: None,
            characters_present: vec![],
            word_count: 500,
            seconds_since_last_pill: 60,
            scene_ordering: None,
            manuscript_scene_count: None,
        }
    }

    fn mk_ctx<'a>(
        telem: &'a TypingTelemetry,
        analysis: &'a AnalysisSnapshot,
        scene: &'a SceneSnapshot,
        project: &'a ProjectSnapshot,
        characters: &'a CharacterRegistry,
    ) -> TriggerContext<'a> {
        TriggerContext {
            telemetry: telem,
            analysis,
            scene,
            project,
            characters,
            world_registry: crate::orchestrator::test_util::test_world_registry(),
            prompts: crate::orchestrator::test_util::test_prompts(),
            tuning: crate::orchestrator::test_util::test_tuning(),
        }
    }

    #[test]
    fn fires_on_low_pace_and_low_word_delta() {
        let telem = mk_telem(10);
        let analysis = AnalysisSnapshot {
            pace: Some(0.2),
            ..Default::default()
        };
        let scene = mk_scene();
        let project = ProjectSnapshot::default();
        let characters = CharacterRegistry::empty();
        let ctx = mk_ctx(&telem, &analysis, &scene, &project, &characters);
        assert!(PaceFloor.evaluate(&ctx).is_some());
    }

    #[test]
    fn m5_prefers_heat_pace_tail_over_legacy_pace() {
        // Heat says slow (0.1); legacy says fast (0.9). Trigger should
        // fire because heat_pace_tail takes precedence. The reason
        // string should cite the heat source so debug surfaces are honest.
        let telem = mk_telem(10);
        let analysis = AnalysisSnapshot {
            pace: Some(0.9),
            heat_pace_tail: Some(0.1),
            ..Default::default()
        };
        let scene = mk_scene();
        let project = ProjectSnapshot::default();
        let characters = CharacterRegistry::empty();
        let ctx = mk_ctx(&telem, &analysis, &scene, &project, &characters);
        let cand = PaceFloor.evaluate(&ctx).expect("should fire");
        assert!(
            cand.reason.contains("heat_pace_tail"),
            "reason should cite the heat source; got {:?}",
            cand.reason,
        );
    }

    #[test]
    fn m5_falls_back_to_legacy_pace_when_heat_absent() {
        // No heat yet (HeatStore empty); the trigger should still fire
        // on the legacy heuristic. Reason cites the legacy field name.
        let telem = mk_telem(10);
        let analysis = AnalysisSnapshot {
            pace: Some(0.2),
            heat_pace_tail: None,
            ..Default::default()
        };
        let scene = mk_scene();
        let project = ProjectSnapshot::default();
        let characters = CharacterRegistry::empty();
        let ctx = mk_ctx(&telem, &analysis, &scene, &project, &characters);
        let cand = PaceFloor.evaluate(&ctx).expect("should fire");
        assert!(
            cand.reason.contains("pace=") && !cand.reason.contains("heat"),
            "reason should cite the legacy source; got {:?}",
            cand.reason,
        );
    }

    #[test]
    fn m5_does_not_fire_when_heat_pace_tail_above_floor() {
        // Heat reports healthy pace (0.7); the legacy field reports
        // stalled (0.1). Heat wins → no fire.
        let telem = mk_telem(10);
        let analysis = AnalysisSnapshot {
            pace: Some(0.1),
            heat_pace_tail: Some(0.7),
            ..Default::default()
        };
        let scene = mk_scene();
        let project = ProjectSnapshot::default();
        let characters = CharacterRegistry::empty();
        let ctx = mk_ctx(&telem, &analysis, &scene, &project, &characters);
        assert!(PaceFloor.evaluate(&ctx).is_none());
    }
}
