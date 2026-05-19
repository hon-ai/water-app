//! `idle_pause_with_present_character` — fires when the writer pauses
//! during a scene with characters present. Allows a character voice to
//! gently surface during quiet writing moments.
//!
//! Threshold tuning per spec § 13.

use crate::orchestrator::{
    CursorClassification, SpeakerTrack, Trigger, TriggerCandidate, TriggerContext,
};

pub struct IdlePauseWithPresentCharacter;

impl Trigger for IdlePauseWithPresentCharacter {
    fn id(&self) -> &'static str {
        "idle_pause_with_present_character"
    }

    fn evaluate(&self, ctx: &TriggerContext<'_>) -> Option<TriggerCandidate> {
        if ctx.telemetry.cursor_classification == CursorClassification::MidSentence {
            return None;
        }
        if ctx.telemetry.idle_for_ms < 8_000 {
            return None;
        }
        if ctx.scene.characters_present.is_empty() {
            return None;
        }
        if ctx.scene.seconds_since_last_pill < 60 {
            return None;
        }
        Some(TriggerCandidate {
            trigger_id: self.id(),
            priority: 4.0,
            preferred_track: SpeakerTrack::Character,
            reason: "idle_with_present_character".to_string(),
            block_target_id: Some(ctx.telemetry.block_id.clone()),
            requires_confirmation: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestrator::*;
    use crate::Id;

    struct Fixture {
        telem: TypingTelemetry,
        analysis: AnalysisSnapshot,
        scene: SceneSnapshot,
        project: ProjectSnapshot,
    }

    impl Fixture {
        fn ctx(&self) -> TriggerContext<'_> {
            TriggerContext {
                telemetry: &self.telem,
                analysis: &self.analysis,
                scene: &self.scene,
                project: &self.project,
            }
        }
    }

    fn fixture(
        idle_ms: u64,
        characters_present: Vec<Id>,
        seconds_since_last_pill: u64,
        cursor: CursorClassification,
    ) -> Fixture {
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
            pov_character_id: characters_present.first().cloned(),
            location_id: None,
            characters_present,
            word_count: 500,
            seconds_since_last_pill,
        };
        Fixture {
            telem,
            analysis,
            scene,
            project: ProjectSnapshot::default(),
        }
    }

    #[test]
    fn fires_when_idle_and_chars_present_and_no_recent_pill() {
        let fx = fixture(
            9000,
            vec![Id::new()],
            60,
            CursorClassification::AtParagraphEnd,
        );
        let cand = IdlePauseWithPresentCharacter.evaluate(&fx.ctx()).unwrap();
        assert_eq!(cand.trigger_id, "idle_pause_with_present_character");
        assert!((cand.priority - 4.0).abs() < 1e-5);
        assert_eq!(cand.preferred_track, SpeakerTrack::Character);
    }

    #[test]
    fn does_not_fire_when_not_idle() {
        let fx = fixture(
            5000,
            vec![Id::new()],
            60,
            CursorClassification::AtParagraphEnd,
        );
        assert!(IdlePauseWithPresentCharacter.evaluate(&fx.ctx()).is_none());
    }

    #[test]
    fn does_not_fire_when_no_chars_present() {
        let fx = fixture(9000, vec![], 60, CursorClassification::AtParagraphEnd);
        assert!(IdlePauseWithPresentCharacter.evaluate(&fx.ctx()).is_none());
    }

    #[test]
    fn does_not_fire_when_recent_pill() {
        let fx = fixture(
            9000,
            vec![Id::new()],
            30,
            CursorClassification::AtParagraphEnd,
        );
        assert!(IdlePauseWithPresentCharacter.evaluate(&fx.ctx()).is_none());
    }

    #[test]
    fn does_not_fire_mid_sentence() {
        let fx = fixture(9000, vec![Id::new()], 60, CursorClassification::MidSentence);
        assert!(IdlePauseWithPresentCharacter.evaluate(&fx.ctx()).is_none());
    }
}
