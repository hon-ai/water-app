//! Phase 6 — arc_position derivation (UX_SPEC §F.3).
//!
//! Given a scene's 0-indexed `ordering` and the manuscript's total
//! scene count, return a labeled bucket placing the scene in the
//! story arc. Pure, deterministic, no DB access — the result drops
//! into the prompt context as a single line of framing.
//!
//! Buckets reflect the rough beats of a generic narrative arc; the
//! goal is *enough* signal for the model to know whether it's
//! reading the opening, the climb, the pivot, or the close —
//! without claiming any specific structural theory.

/// Bucket placing a scene in its manuscript's arc. Order of the
/// variants matches the timeline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArcPosition {
    OpeningSequence,
    RisingAction,
    MidpointPivot,
    ApproachingClimax,
    Climax,
    Resolution,
    /// Single-scene manuscript, or insufficient data to bucket.
    Standalone,
}

impl ArcPosition {
    /// Human-readable label, dropped into the prompt verbatim.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            ArcPosition::OpeningSequence => "opening sequence",
            ArcPosition::RisingAction => "rising action",
            ArcPosition::MidpointPivot => "midpoint pivot",
            ArcPosition::ApproachingClimax => "approaching climax",
            ArcPosition::Climax => "climax",
            ArcPosition::Resolution => "resolution",
            ArcPosition::Standalone => "standalone scene",
        }
    }
}

/// Compute the bucket for a 0-indexed scene `ordering` among
/// `total` scenes. Returns `Standalone` when `total <= 1`.
///
/// Bucket boundaries (in fractional position f = ordering / (total - 1)):
///   - f ≤ 0.10               → opening sequence
///   - 0.10 < f ≤ 0.40        → rising action
///   - 0.40 < f ≤ 0.55        → midpoint pivot
///   - 0.55 < f ≤ 0.80        → approaching climax
///   - 0.80 < f ≤ 0.92        → climax
///   - 0.92 < f               → resolution
///
/// The boundaries skew slightly toward "approaching climax" because
/// the model has the most to say *while the writer is still climbing*;
/// the climax + resolution windows are narrow because by then the
/// scene's tone is usually self-evident.
#[must_use]
pub fn arc_position(ordering: u32, total: u32) -> ArcPosition {
    if total <= 1 {
        return ArcPosition::Standalone;
    }
    #[allow(clippy::cast_precision_loss)]
    let f = ordering as f32 / (total - 1) as f32;
    if f <= 0.10 {
        ArcPosition::OpeningSequence
    } else if f <= 0.40 {
        ArcPosition::RisingAction
    } else if f <= 0.55 {
        ArcPosition::MidpointPivot
    } else if f <= 0.80 {
        ArcPosition::ApproachingClimax
    } else if f <= 0.92 {
        ArcPosition::Climax
    } else {
        ArcPosition::Resolution
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standalone_for_single_scene_manuscript() {
        assert_eq!(arc_position(0, 1), ArcPosition::Standalone);
        assert_eq!(arc_position(0, 0), ArcPosition::Standalone);
    }

    #[test]
    fn first_scene_is_opening() {
        assert_eq!(arc_position(0, 20), ArcPosition::OpeningSequence);
        assert_eq!(arc_position(1, 20), ArcPosition::OpeningSequence);
    }

    #[test]
    fn middle_is_midpoint() {
        assert_eq!(arc_position(10, 21), ArcPosition::MidpointPivot);
    }

    #[test]
    fn near_end_is_climax_then_resolution() {
        let total = 20;
        // ordering 17 → f = 17/19 ≈ 0.89 → climax
        assert_eq!(arc_position(17, total), ArcPosition::Climax);
        // ordering 19 (last) → f = 1.0 → resolution
        assert_eq!(arc_position(19, total), ArcPosition::Resolution);
    }

    #[test]
    fn three_scene_manuscript_buckets_cleanly() {
        // 3 scenes: f = 0, 0.5, 1.0
        assert_eq!(arc_position(0, 3), ArcPosition::OpeningSequence);
        assert_eq!(arc_position(1, 3), ArcPosition::MidpointPivot);
        assert_eq!(arc_position(2, 3), ArcPosition::Resolution);
    }

    #[test]
    fn label_strings_are_lowercase_and_stable() {
        // The label is dropped into the prompt verbatim; lowercasing
        // matches the rest of the system block's discipline.
        assert_eq!(ArcPosition::MidpointPivot.label(), "midpoint pivot");
        assert_eq!(ArcPosition::Climax.label(), "climax");
        assert!(ArcPosition::OpeningSequence
            .label()
            .chars()
            .all(|c| c.is_lowercase() || c.is_whitespace()));
    }
}
