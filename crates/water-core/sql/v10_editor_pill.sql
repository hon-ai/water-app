-- v10: Editor pills (UX_SPEC.md §E.5).
--
-- Diagnostic pills — distinct from the generative pill engine. Each row
-- is a sticky observation about a span of prose: rule kind, severity,
-- the Editor-voiced message, an optional concrete suggestion (e.g. a
-- spelling correction), and the anchor fields needed to keep the
-- highlight on the right text after edits.
--
-- Lifecycle:
--   - The diagnostic engine writes / upserts on save + on the 1.5s
--     edit debounce.
--   - The writer dismisses via the diagnostics tab or inline action;
--     `dismissed = 1` keeps the row for telemetry without re-surfacing.
--   - A re-run that no longer fires for the same span will simply not
--     refresh the row; an explicit cleanup pass deletes editor_pills
--     whose anchor span has been removed.
--
-- The two-layer anchor (block_id + content_hash + text_snippet) mirrors
-- the generative pill's anchor resolver — same Phase-3.5 algorithm
-- shifted to the diagnostic side.

CREATE TABLE editor_pill (
    id                  TEXT PRIMARY KEY,
    scene_id            TEXT NOT NULL REFERENCES scene(id) ON DELETE CASCADE,
    rule                TEXT NOT NULL,
    severity            TEXT NOT NULL,         -- "observation"|"suggestion"|"warning"
    message             TEXT NOT NULL,
    suggestion          TEXT,                  -- nullable; only e.g. spelling fixes
    anchor_block_id     TEXT NOT NULL,
    anchor_start        INTEGER NOT NULL,
    anchor_end          INTEGER NOT NULL,
    text_snippet        TEXT NOT NULL,         -- 3-10 word excerpt
    content_hash        TEXT NOT NULL,         -- first 50 chars of block, normalized
    dismissed           INTEGER NOT NULL DEFAULT 0,
    created_at          TEXT NOT NULL,
    updated_at          TEXT NOT NULL
);

CREATE INDEX editor_pill_by_scene ON editor_pill(scene_id, dismissed);
CREATE INDEX editor_pill_by_block ON editor_pill(scene_id, anchor_block_id);

UPDATE schema_version SET version = 10;
