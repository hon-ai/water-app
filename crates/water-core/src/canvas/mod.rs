//! Macro Spatial Canvas — M6.
//!
//! Position storage lives on the `scene` row (canvas_x/canvas_y/
//! canvas_group, v6 migration). This module provides the layout
//! helper that produces an initial position for scenes that haven't
//! been placed yet.

pub mod layout;

pub use layout::{auto_flow, CARDS_PER_ROW, CARD_SPACING_X, CARD_SPACING_Y};
