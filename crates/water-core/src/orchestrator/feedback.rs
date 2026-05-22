//! Adaptive trigger sensitivity — per-project reward learning.
//!
//! Each Stage-1 trigger has a sensitivity value in `[0.2, 0.8]` that
//! shifts its numeric thresholds. The sensitivity is learned from a
//! reward EMA over pill outcomes (pin / click / dismiss / evict),
//! attributed to whichever trigger fired the pill.
//!
//! The reward weights are *mode-aware*: a dismiss during a pour
//! burst is near-zero signal (the writer was deep in flow and
//! reflexively cleared the chrome), while a dismiss during a
//! reflect pause is a strong negative (she actually evaluated it).
//! See the writer-flow walkthrough that produced the constants
//! below.
//!
//! Storage: a single `trigger_feedback` row per `trigger_id`
//! (created v8). Cold-start: under `COLD_START_N` observations the
//! sensitivity blends with the 0.5 default so a handful of early
//! samples don't swing the system. Reset is a single TRUNCATE.

use crate::orchestrator::TypingTelemetry;
use crate::{Db, Result};
use rusqlite::params;
use std::collections::HashMap;

/// EMA learning rate. Each observation nudges `r_ema` by 10%.
/// After ~30 events the EMA is meaningfully responsive but still
/// stable to a single outlier.
pub const ALPHA: f32 = 0.1;

/// Scale factor mapping `r_ema` ([-1.5, +1.5] envelope from the
/// reward table) to a sensitivity *delta* around 0.5. Combined with
/// the floor/ceiling clamp, this caps the practical sensitivity
/// movement to ±0.3 from the default.
pub const SENSITIVITY_SCALE: f32 = 0.3;

/// Hard floor on learned sensitivity. A trigger the writer hates
/// should still fire occasionally — her instinct might be wrong for
/// the next scene.
pub const SENSITIVITY_FLOOR: f32 = 0.2;
pub const SENSITIVITY_CEILING: f32 = 0.8;
pub const SENSITIVITY_DEFAULT: f32 = 0.5;

/// Observation count below which we blend the learned sensitivity
/// with the default. Prevents swinging on a handful of samples.
pub const COLD_START_N: u32 = 10;

/// Writer state at the moment a pill emerged. Drives the reward
/// weights — dismissals in pour mode are near-zero signal; pins in
/// pour mode are strong positive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriterMode {
    Pour,
    Reflect,
}

impl WriterMode {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            WriterMode::Pour => "pour",
            WriterMode::Reflect => "reflect",
        }
    }
}

/// Classify the writer's current state from a telemetry tick.
///
/// Heuristic (per the writer-flow walkthrough):
///   pour:    recent_word_delta ≥ 15 in 10s   OR  idle_for_ms < 4000
///   reflect: idle_for_ms ≥ 4000  AND  recent_word_delta < 15
///
/// Falls to `Pour` on the boundary case (idle < 4s, delta < 15) —
/// the writer is *between bursts* and a wrong-mode classification
/// here biases toward the gentler reward signal, which is the safer
/// default.
#[must_use]
pub fn classify_writer_mode(t: &TypingTelemetry) -> WriterMode {
    if t.idle_for_ms >= 4000 && t.recent_word_delta < 15 {
        WriterMode::Reflect
    } else {
        WriterMode::Pour
    }
}

/// Pill lifecycle terminal events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PillOutcome {
    /// Writer pinned the pill (strongest positive).
    Pin,
    /// Writer clicked to deepen (rabbit hole) but did not pin.
    Click,
    /// Writer dismissed via the × button.
    Dismiss,
    /// FIFO eviction; the writer never interacted.
    Evict,
}

/// Compute the reward for a single (outcome, mode) pair.
///
/// The constants come directly from the writer-flow walkthrough:
/// pin during pour mode is a stronger signal because it took effort
/// to break flow; dismiss during pour is near-zero because pour-mode
/// dismissals are reflex, not evaluation.
#[must_use]
pub fn compute_reward(outcome: PillOutcome, mode: WriterMode) -> f32 {
    match (outcome, mode) {
        (PillOutcome::Pin, WriterMode::Pour) => 1.5,
        (PillOutcome::Pin, WriterMode::Reflect) => 1.0,
        (PillOutcome::Click, WriterMode::Pour) => 0.6,
        (PillOutcome::Click, WriterMode::Reflect) => 0.4,
        (PillOutcome::Dismiss, WriterMode::Pour) => -0.1,
        (PillOutcome::Dismiss, WriterMode::Reflect) => -0.7,
        (PillOutcome::Evict, WriterMode::Pour) => 0.0,
        (PillOutcome::Evict, WriterMode::Reflect) => -0.3,
    }
}

/// Map an `r_ema` value to a sensitivity in [0.2, 0.8], with the
/// cold-start blend toward 0.5 under `COLD_START_N` observations.
#[must_use]
pub fn r_ema_to_sensitivity(r_ema: f32, n_observations: u32) -> f32 {
    let raw = (SENSITIVITY_DEFAULT + SENSITIVITY_SCALE * r_ema)
        .clamp(SENSITIVITY_FLOOR, SENSITIVITY_CEILING);
    if n_observations >= COLD_START_N {
        raw
    } else {
        // Linear blend from default → raw as n approaches COLD_START_N.
        #[allow(clippy::cast_precision_loss)]
        let weight = n_observations as f32 / COLD_START_N as f32;
        SENSITIVITY_DEFAULT + weight * (raw - SENSITIVITY_DEFAULT)
    }
}

/// Magnitude of threshold shift from the symmetric center. With
/// sensitivity range [0.2, 0.8] (±0.3 from the 0.5 default) this
/// gives every threshold a ±0.12 swing — enough to be perceptible
/// to the writer over a session without making any trigger a
/// completely different animal at the extremes.
pub const THRESHOLD_SHIFT_MAGNITUDE: f32 = 0.4;

/// Map a sensitivity to an "above-this-fires" threshold (e.g.
/// `divergence > 0.6`). Higher sensitivity → lower threshold →
/// trigger fires more often.
#[must_use]
pub fn loosen_above(default: f32, sensitivity: f32) -> f32 {
    default - (sensitivity - SENSITIVITY_DEFAULT) * THRESHOLD_SHIFT_MAGNITUDE
}

/// Map a sensitivity to a "below-this-fires" threshold (e.g.
/// `coherence < 0.35`). Higher sensitivity → higher threshold →
/// trigger fires more often.
#[must_use]
pub fn loosen_below(default: f32, sensitivity: f32) -> f32 {
    default + (sensitivity - SENSITIVITY_DEFAULT) * THRESHOLD_SHIFT_MAGNITUDE
}

/// Read-only sensitivity lookup handed to every `Trigger::evaluate`
/// via `TriggerContext`. Absent trigger ids resolve to
/// `SENSITIVITY_DEFAULT`, so cold-boot and tests pass through
/// unchanged. Wraps a HashMap rather than exposing it so the
/// trigger-side call sites stay tidy.
#[derive(Debug, Default, Clone)]
pub struct TriggerTuning {
    sensitivities: HashMap<String, f32>,
}

impl TriggerTuning {
    #[must_use]
    pub fn new(sensitivities: HashMap<String, f32>) -> Self {
        Self { sensitivities }
    }

    #[must_use]
    pub fn sensitivity_for(&self, trigger_id: &str) -> f32 {
        self.sensitivities
            .get(trigger_id)
            .copied()
            .unwrap_or(SENSITIVITY_DEFAULT)
    }
}

/// A snapshot of one trigger's learned state. Returned by
/// `FeedbackStore::load_all` for the orchestrator's per-tick
/// sensitivity lookup.
#[derive(Debug, Clone)]
pub struct TriggerFeedback {
    pub trigger_id: String,
    pub r_ema: f32,
    pub sensitivity: f32,
    pub n_observations: u32,
    pub pour_observations: u32,
    pub reflect_observations: u32,
}

pub struct FeedbackStore<'a> {
    db: &'a Db,
}

impl<'a> FeedbackStore<'a> {
    #[must_use]
    pub fn new(db: &'a Db) -> Self {
        Self { db }
    }

    /// Apply one pill outcome to the trigger's learned state. Upserts
    /// the row if `trigger_id` is new. The EMA + observation counters
    /// + derived sensitivity all update atomically.
    pub fn record_outcome(
        &self,
        trigger_id: &str,
        outcome: PillOutcome,
        mode: WriterMode,
    ) -> Result<()> {
        let reward = compute_reward(outcome, mode);
        let conn = self.db.conn();

        // Read existing state (or use defaults).
        let existing: Option<(f32, u32, u32, u32)> = conn
            .query_row(
                "SELECT r_ema, n_observations, pour_observations, reflect_observations
                 FROM trigger_feedback WHERE trigger_id = ?1",
                params![trigger_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .ok();
        let (prev_ema, prev_n, prev_pour, prev_reflect) =
            existing.unwrap_or((0.0, 0, 0, 0));

        let next_ema = ALPHA * reward + (1.0 - ALPHA) * prev_ema;
        let next_n = prev_n + 1;
        let next_pour = prev_pour
            + match mode {
                WriterMode::Pour => 1,
                WriterMode::Reflect => 0,
            };
        let next_reflect = prev_reflect
            + match mode {
                WriterMode::Reflect => 1,
                WriterMode::Pour => 0,
            };
        let next_sens = r_ema_to_sensitivity(next_ema, next_n);
        let now = chrono::Utc::now().to_rfc3339();

        conn.execute(
            "INSERT INTO trigger_feedback
                (trigger_id, r_ema, sensitivity, n_observations,
                 pour_observations, reflect_observations, last_updated)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(trigger_id) DO UPDATE SET
                r_ema                = excluded.r_ema,
                sensitivity          = excluded.sensitivity,
                n_observations       = excluded.n_observations,
                pour_observations    = excluded.pour_observations,
                reflect_observations = excluded.reflect_observations,
                last_updated         = excluded.last_updated",
            params![
                trigger_id,
                next_ema,
                next_sens,
                next_n,
                next_pour,
                next_reflect,
                now,
            ],
        )?;
        Ok(())
    }

    /// Read all trigger sensitivities as a map. Triggers with no row
    /// are simply absent; the orchestrator treats absent triggers as
    /// `SENSITIVITY_DEFAULT`.
    pub fn load_sensitivities(&self) -> Result<HashMap<String, f32>> {
        let conn = self.db.conn();
        let mut stmt = conn.prepare("SELECT trigger_id, sensitivity FROM trigger_feedback")?;
        let rows = stmt.query_map([], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, f32>(1)?))
        })?;
        let mut out = HashMap::new();
        for row in rows {
            let (id, s) = row?;
            out.insert(id, s);
        }
        Ok(out)
    }

    /// Read all trigger feedback rows. For audit / settings UIs only;
    /// the hot path uses `load_sensitivities`.
    pub fn load_all(&self) -> Result<Vec<TriggerFeedback>> {
        let conn = self.db.conn();
        let mut stmt = conn.prepare(
            "SELECT trigger_id, r_ema, sensitivity, n_observations,
                    pour_observations, reflect_observations
             FROM trigger_feedback
             ORDER BY trigger_id",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(TriggerFeedback {
                trigger_id: r.get(0)?,
                r_ema: r.get(1)?,
                sensitivity: r.get(2)?,
                n_observations: r.get(3)?,
                pour_observations: r.get(4)?,
                reflect_observations: r.get(5)?,
            })
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// Nuke all learned state. Bound to the "reset trigger learning"
    /// button in Settings.
    pub fn reset(&self) -> Result<()> {
        self.db.conn().execute("DELETE FROM trigger_feedback", [])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestrator::{CursorClassification, StructuralInflection};

    fn telem(idle_ms: u64, delta: i32) -> TypingTelemetry {
        TypingTelemetry {
            idle_for_ms: idle_ms,
            cursor_classification: CursorClassification::AtParagraphEnd,
            block_id: "^bk-0001".to_string(),
            recent_word_delta: delta,
            structural_inflection: StructuralInflection::None,
        }
    }

    #[test]
    fn classify_writer_mode_pour_during_burst() {
        // High word delta, short idle — pure pour.
        assert_eq!(classify_writer_mode(&telem(500, 40)), WriterMode::Pour);
    }

    #[test]
    fn classify_writer_mode_reflect_after_pause() {
        // Idle ≥ 4s, low delta — reflect.
        assert_eq!(
            classify_writer_mode(&telem(5000, 3)),
            WriterMode::Reflect
        );
    }

    #[test]
    fn classify_writer_mode_pour_on_boundary() {
        // Idle = 3999 ms (under 4s) with low delta: still pour because
        // idle threshold not met. Safer side of the fence.
        assert_eq!(classify_writer_mode(&telem(3999, 2)), WriterMode::Pour);
    }

    #[test]
    fn compute_reward_pin_dominates() {
        // Pin (pour) > click (pour) > evict (pour); same in reflect.
        assert!(
            compute_reward(PillOutcome::Pin, WriterMode::Pour)
                > compute_reward(PillOutcome::Click, WriterMode::Pour)
        );
        assert!(
            compute_reward(PillOutcome::Click, WriterMode::Pour)
                > compute_reward(PillOutcome::Evict, WriterMode::Pour)
        );
    }

    #[test]
    fn compute_reward_dismiss_softer_in_pour_than_reflect() {
        // The whole point of mode-awareness: pour-mode dismissals
        // are near-zero signal; reflect-mode dismissals are strong.
        let pour = compute_reward(PillOutcome::Dismiss, WriterMode::Pour);
        let reflect = compute_reward(PillOutcome::Dismiss, WriterMode::Reflect);
        assert!(pour > reflect);
        assert!(pour.abs() < 0.2);
        assert!(reflect < -0.5);
    }

    #[test]
    fn r_ema_to_sensitivity_clamps() {
        // Extreme positive EMA clamped to 0.8.
        assert!(r_ema_to_sensitivity(5.0, 100) <= SENSITIVITY_CEILING + 1e-6);
        // Extreme negative EMA clamped to 0.2.
        assert!(r_ema_to_sensitivity(-5.0, 100) >= SENSITIVITY_FLOOR - 1e-6);
    }

    #[test]
    fn r_ema_to_sensitivity_cold_start_blends_toward_default() {
        let raw = r_ema_to_sensitivity(1.0, 100);
        let cold = r_ema_to_sensitivity(1.0, 1);
        // Cold-start result must be between default and raw.
        assert!(cold > SENSITIVITY_DEFAULT);
        assert!(cold < raw);
    }

    #[test]
    fn r_ema_to_sensitivity_zero_obs_is_default() {
        // No observations at all → still default. Avoid NaN paths and
        // make absolutely sure first-tick reads are unaffected.
        let v = r_ema_to_sensitivity(2.0, 0);
        assert!((v - SENSITIVITY_DEFAULT).abs() < 1e-6);
    }

    #[test]
    fn store_record_outcome_creates_row_on_first_observation() {
        let db = Db::open_in_memory().unwrap();
        let store = FeedbackStore::new(&db);
        store
            .record_outcome("topic_drift", PillOutcome::Pin, WriterMode::Reflect)
            .unwrap();
        let all = store.load_all().unwrap();
        assert_eq!(all.len(), 1);
        let row = &all[0];
        assert_eq!(row.trigger_id, "topic_drift");
        assert_eq!(row.n_observations, 1);
        assert_eq!(row.reflect_observations, 1);
        assert_eq!(row.pour_observations, 0);
        assert!(row.r_ema > 0.0, "pin reward must move EMA positive");
    }

    #[test]
    fn store_record_outcome_updates_existing_row() {
        let db = Db::open_in_memory().unwrap();
        let store = FeedbackStore::new(&db);
        for _ in 0..5 {
            store
                .record_outcome("topic_drift", PillOutcome::Pin, WriterMode::Reflect)
                .unwrap();
        }
        let all = store.load_all().unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].n_observations, 5);
    }

    #[test]
    fn store_load_sensitivities_returns_map() {
        let db = Db::open_in_memory().unwrap();
        let store = FeedbackStore::new(&db);
        store
            .record_outcome("topic_drift", PillOutcome::Pin, WriterMode::Reflect)
            .unwrap();
        store
            .record_outcome("pace_floor", PillOutcome::Dismiss, WriterMode::Reflect)
            .unwrap();
        let map = store.load_sensitivities().unwrap();
        assert_eq!(map.len(), 2);
        assert!(map.contains_key("topic_drift"));
        assert!(map.contains_key("pace_floor"));
    }

    #[test]
    fn store_reset_clears_all_rows() {
        let db = Db::open_in_memory().unwrap();
        let store = FeedbackStore::new(&db);
        store
            .record_outcome("topic_drift", PillOutcome::Pin, WriterMode::Reflect)
            .unwrap();
        store.reset().unwrap();
        let all = store.load_all().unwrap();
        assert!(all.is_empty());
    }

    #[test]
    fn store_repeated_dismiss_in_reflect_drives_sensitivity_down() {
        // Twenty reflect-mode dismissals in a row: a steady negative
        // signal. By the end the trigger's sensitivity must be below
        // the default (heading toward the floor). This is the
        // "writer hates this trigger" code path.
        let db = Db::open_in_memory().unwrap();
        let store = FeedbackStore::new(&db);
        for _ in 0..20 {
            store
                .record_outcome("topic_drift", PillOutcome::Dismiss, WriterMode::Reflect)
                .unwrap();
        }
        let map = store.load_sensitivities().unwrap();
        let s = map["topic_drift"];
        assert!(
            s < SENSITIVITY_DEFAULT - 0.05,
            "expected sensitivity well below default after 20 reflect-mode dismissals, got {s}"
        );
        // Floor still respected.
        assert!(s >= SENSITIVITY_FLOOR - 1e-6);
    }

    #[test]
    fn store_repeated_pin_drives_sensitivity_up() {
        let db = Db::open_in_memory().unwrap();
        let store = FeedbackStore::new(&db);
        for _ in 0..20 {
            store
                .record_outcome("topic_drift", PillOutcome::Pin, WriterMode::Reflect)
                .unwrap();
        }
        let map = store.load_sensitivities().unwrap();
        let s = map["topic_drift"];
        assert!(
            s > SENSITIVITY_DEFAULT + 0.05,
            "expected sensitivity well above default after 20 pins, got {s}"
        );
        assert!(s <= SENSITIVITY_CEILING + 1e-6);
    }
}
