//! `idle_pause` — companion to `idle_pause_with_present_character`
//! that fires when the writer is idle WITHOUT requiring a linked
//! character in the scene. Lower priority (3.0 vs 4.0) so the
//! character-aware version wins whenever both gates open.
//!
//! Reason for existing: a tester opening a fresh project, writing
//! two paragraphs in an unlinked scene, pausing — would otherwise
//! see nothing surface from the orchestrator. The character gate on
//! `idle_pause_with_present_character` was correct for full-metadata
//! projects but turned out to be silent-failure for the bare-bones
//! "I just want to try the app" path. This trigger fills that gap by
//! letting a persona (Chorus by default; the voice router picks)
//! surface an ambient observation.
//!
//! Longer idle gate (12 s vs 8 s) than the character version so we
//! prefer the richer character voice when it's available — only
//! falling through to ambient mode after a real pause.

use crate::orchestrator::{
    CursorClassification, SpeakerTrack, Trigger, TriggerCandidate, TriggerContext,
};

pub struct IdlePause;

impl Trigger for IdlePause {
    fn id(&self) -> &'static str {
        "idle_pause"
    }

    fn evaluate(&self, ctx: &TriggerContext<'_>) -> Option<TriggerCandidate> {
        if ctx.telemetry.cursor_classification == CursorClassification::MidSentence {
            return None;
        }
        // 6 s idle — short enough that the council surfaces
        // routinely while the writer pauses, long enough that
        // a typing burst with a single mid-thought period
        // doesn't trip it.
        if ctx.telemetry.idle_for_ms < 6_000 {
            return None;
        }
        if ctx.scene.seconds_since_last_pill < 60 {
            return None;
        }
        // Need *some* prose under the cursor. Low threshold so
        // even a sentence or two is enough to elicit an ambient
        // pill — the cold-start "council of personas" experience
        // depends on this firing early, not on the writer
        // pre-loading 200 words first.
        if ctx.scene.word_count < 15 {
            return None;
        }
        Some(TriggerCandidate {
            trigger_id: self.id().to_string(),
            // Same as `idle_pause_with_present_character` (4.0).
            // The character-aware version is registered before this
            // one in `builtin_triggers`, so when both gates open
            // `pick_best_trigger` keeps the character pick — but
            // this fires standalone when no characters are linked.
            priority: 4.0,
            preferred_track: SpeakerTrack::Persona,
            reason: "idle_pause_ambient".to_string(),
            block_target_id: Some(ctx.telemetry.block_id.clone()),
            requires_confirmation: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::character::registry::CharacterRegistry;
    use crate::orchestrator::*;
    use crate::Id;

    fn ctx_with(
        idle_ms: u64,
        characters_present: Vec<Id>,
        word_count: u32,
        cursor: CursorClassification,
    ) -> (
        TypingTelemetry,
        AnalysisSnapshot,
        SceneSnapshot,
        ProjectSnapshot,
        CharacterRegistry,
    ) {
        let telem = TypingTelemetry {
            idle_for_ms: idle_ms,
            cursor_classification: cursor,
            block_id: "^bk-0001".to_string(),
            recent_word_delta: 0,
            structural_inflection: StructuralInflection::None,
        };
        let analysis = AnalysisSnapshot::default();
        let scene = SceneSnapshot {
            id: Id::new(),
            pov_character_id: None,
            location_id: None,
            characters_present,
            word_count,
            seconds_since_last_pill: 60,
            scene_ordering: None,
            manuscript_scene_count: None,
        };
        (
            telem,
            analysis,
            scene,
            ProjectSnapshot::default(),
            CharacterRegistry::empty(),
        )
    }

    fn make_ctx<'a>(
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
    fn fires_on_idle_pause_without_characters() {
        let (t, a, s, p, c) = ctx_with(7_000, vec![], 120, CursorClassification::AtParagraphEnd);
        let cand = IdlePause.evaluate(&make_ctx(&t, &a, &s, &p, &c)).unwrap();
        assert_eq!(cand.trigger_id, "idle_pause");
        assert!((cand.priority - 4.0).abs() < 1e-5);
        assert_eq!(cand.preferred_track, SpeakerTrack::Persona);
    }

    #[test]
    fn does_not_fire_mid_sentence() {
        let (t, a, s, p, c) = ctx_with(7_000, vec![], 120, CursorClassification::MidSentence);
        assert!(IdlePause.evaluate(&make_ctx(&t, &a, &s, &p, &c)).is_none());
    }

    #[test]
    fn does_not_fire_under_idle_threshold() {
        let (t, a, s, p, c) = ctx_with(4_000, vec![], 120, CursorClassification::AtParagraphEnd);
        assert!(IdlePause.evaluate(&make_ctx(&t, &a, &s, &p, &c)).is_none());
    }

    #[test]
    fn does_not_fire_on_tiny_scene() {
        let (t, a, s, p, c) = ctx_with(7_000, vec![], 10, CursorClassification::AtParagraphEnd);
        assert!(IdlePause.evaluate(&make_ctx(&t, &a, &s, &p, &c)).is_none());
    }
}
