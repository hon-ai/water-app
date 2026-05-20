//! Deterministic auto-flow layout for the macro spatial canvas.
//!
//! When the writer opens the canvas surface for the first time (or
//! resets all positions via the right-click action), every scene
//! gets a default position from this layout. The renderer then lets
//! the writer drag freely; subsequent positions land in the DB +
//! frontmatter via `SceneStore::set_canvas_position`.

use crate::{Id, SceneRow};

/// How many cards we lay out per row before wrapping. Picked so an
/// 8-scene act fits comfortably across a 1920 px-wide canvas at
/// zoom = 1.0 (8 × 240 = 1920).
pub const CARDS_PER_ROW: usize = 8;

/// Horizontal spacing between cards in canvas-space units.
/// Card is 200 × 100; spacing leaves a 40 px gutter for breathing
/// room + reading-order overlay arc clearance.
pub const CARD_SPACING_X: f32 = 240.0;

/// Vertical spacing — 100 px card + 40 px row gap.
pub const CARD_SPACING_Y: f32 = 140.0;

/// Produce `(scene_id, x, y)` triples for the input scenes, in the
/// order provided. Layout is **strictly index-ordered** — the
/// caller is responsible for sorting by manuscript ordering first.
/// Wraps every `CARDS_PER_ROW` cards into a new row.
///
/// Empty input yields an empty vec.
#[must_use]
pub fn auto_flow(scenes: &[SceneRow]) -> Vec<(Id, f32, f32)> {
    scenes
        .iter()
        .enumerate()
        .map(|(ix, s)| {
            #[allow(clippy::cast_precision_loss)]
            let col = (ix % CARDS_PER_ROW) as f32;
            #[allow(clippy::cast_precision_loss)]
            let row = (ix / CARDS_PER_ROW) as f32;
            (s.id.clone(), col * CARD_SPACING_X, row * CARD_SPACING_Y)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Id;

    fn mk_scene(name: &str) -> SceneRow {
        SceneRow {
            id: Id::new(),
            manuscript_id: Id::new(),
            chapter_id: None,
            ordering: 0,
            name: name.to_string(),
            word_count: 0,
            file_path: std::path::PathBuf::from(format!("scenes/{name}.md")),
            file_hash: Some("h".to_string()),
        }
    }

    #[test]
    fn empty_input_yields_empty_vec() {
        assert!(auto_flow(&[]).is_empty());
    }

    #[test]
    fn single_scene_lands_at_origin() {
        let s = mk_scene("A");
        let out = auto_flow(&[s.clone()]);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].0, s.id);
        assert!((out[0].1 - 0.0).abs() < 1e-5);
        assert!((out[0].2 - 0.0).abs() < 1e-5);
    }

    #[test]
    fn second_scene_lands_one_column_right() {
        let scenes = vec![mk_scene("A"), mk_scene("B")];
        let out = auto_flow(&scenes);
        assert!((out[1].1 - CARD_SPACING_X).abs() < 1e-5);
        assert!((out[1].2 - 0.0).abs() < 1e-5);
    }

    #[test]
    fn ninth_scene_wraps_to_second_row() {
        // 8 cards per row → 9th lands at column 0, row 1.
        let scenes: Vec<SceneRow> =
            (0..9).map(|i| mk_scene(&format!("S{i}"))).collect();
        let out = auto_flow(&scenes);
        assert_eq!(out.len(), 9);
        assert!((out[8].1 - 0.0).abs() < 1e-5);
        assert!((out[8].2 - CARD_SPACING_Y).abs() < 1e-5);
    }

    #[test]
    fn order_is_preserved_from_input() {
        // The caller is responsible for sorting; auto_flow must use
        // the input order verbatim. Build scenes in a specific
        // sequence and assert the same sequence comes back.
        let scenes: Vec<SceneRow> =
            (0..3).map(|i| mk_scene(&format!("S{i}"))).collect();
        let out = auto_flow(&scenes);
        for (in_s, out_t) in scenes.iter().zip(out.iter()) {
            assert_eq!(in_s.id, out_t.0);
        }
    }
}
