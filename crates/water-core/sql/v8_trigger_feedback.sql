-- v8: per-project trigger-feedback learning.
--
-- Each Stage-1 trigger has one row carrying:
--   r_ema             — exponential moving average of pill-outcome reward
--                       (pin=+1.5/+1.0, click=+0.6/+0.4, dismiss=-0.1/-0.7,
--                       evict=0.0/-0.3 — first value = pour mode, second =
--                       reflect mode; see orchestrator_service.rs).
--   sensitivity       — derived from r_ema, clamped to [0.2, 0.8]. The
--                       triggers map sensitivity to their numeric
--                       thresholds (sensitivity 0.5 = current defaults).
--   n_observations    — how many outcome events have updated this row.
--                       Used for cold-start blending: under 10, sensitivity
--                       blends with default 0.5 to avoid swinging on a
--                       handful of samples.
--   pour_observations
--   reflect_observations
--                     — split count for the writer-mode telemetry view.
--                       Not load-bearing for learning; useful for audit.
--
-- Rows are upserted on demand; the migration does not pre-seed all
-- known trigger ids so that adding new triggers later doesn't require a
-- backfill. Reset-trigger-learning (settings) is a single TRUNCATE.

CREATE TABLE trigger_feedback (
    trigger_id            TEXT PRIMARY KEY,
    r_ema                 REAL NOT NULL DEFAULT 0.0,
    sensitivity           REAL NOT NULL DEFAULT 0.5,
    n_observations        INTEGER NOT NULL DEFAULT 0,
    pour_observations     INTEGER NOT NULL DEFAULT 0,
    reflect_observations  INTEGER NOT NULL DEFAULT 0,
    last_updated          TEXT NOT NULL
);

UPDATE schema_version SET version = 8;
