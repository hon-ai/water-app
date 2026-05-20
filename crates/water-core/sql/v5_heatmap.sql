-- M5 schema v5: Heatmap audiovisualizer support.
-- Forward-only. v4 -> v5.
--
-- Adds two tables:
--
--   heat_metric           — per-(scene, paragraph_ix, metric) rolling score
--                           cache. The five metrics (pacing/valence/coherence/
--                           presence/world_refs) each get their own row per
--                           paragraph. `text_hash` lets the orchestrator
--                           skip recompute when a paragraph's body hasn't
--                           changed since the cached value landed.
--
--   scene_typing_history  — append-only ring of (ts_ms, word_delta) for the
--                           pacing computation. Older entries roll up into
--                           per-paragraph heat_metric rows and are then
--                           pruned (handled by the heat compute path, not
--                           by this migration).
--
-- Both tables ON DELETE CASCADE against scene(id) so Heatmap state never
-- outlives its scene.

CREATE TABLE heat_metric (
    scene_id      TEXT NOT NULL,
    paragraph_ix  INTEGER NOT NULL,
    metric        TEXT NOT NULL,
    value         REAL NOT NULL,
    text_hash     TEXT NOT NULL,
    updated_at    TEXT NOT NULL,
    PRIMARY KEY (scene_id, paragraph_ix, metric),
    FOREIGN KEY (scene_id) REFERENCES scene(id) ON DELETE CASCADE
);

CREATE INDEX heat_metric_by_scene ON heat_metric(scene_id, metric);

CREATE TABLE scene_typing_history (
    scene_id    TEXT NOT NULL,
    ts_ms       INTEGER NOT NULL,
    word_delta  INTEGER NOT NULL,
    PRIMARY KEY (scene_id, ts_ms),
    FOREIGN KEY (scene_id) REFERENCES scene(id) ON DELETE CASCADE
);

UPDATE schema_version SET version = 5;
