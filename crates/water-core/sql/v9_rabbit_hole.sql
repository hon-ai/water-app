-- v9: Rabbit-hole deepening tree (UX_SPEC.md §D.3).
--
-- Each "rabbit thought" is a node in a tree rooted at a pill click.
-- The root carries `parent_id = NULL` and `depth = 0`. Children fan
-- out four-at-a-time (closer / wider / opposite / deeper) and inherit
-- the parent's speaker; a writer can mark one as `resonance = 1` to
-- flag the path as voice-preference signal (future pill prompts read
-- recent resonant picks).
--
-- The tree lives on-device per project; an auto-trim policy keeps
-- it bounded to 5000 rows / 25 MB of message text (configurable in
-- settings). The trim policy is implemented in Rust; this migration
-- only provides the index it relies on.
--
-- Foreign keys cascade from scene so the tree dies with its scene.
-- An additional `parent_id` ON DELETE CASCADE collapses subtrees
-- when a parent is trimmed.

CREATE TABLE rabbit_thought (
    id                  TEXT PRIMARY KEY,
    scene_id            TEXT NOT NULL REFERENCES scene(id) ON DELETE CASCADE,
    parent_id           TEXT REFERENCES rabbit_thought(id) ON DELETE CASCADE,
    speaker_kind        TEXT NOT NULL,                  -- "persona" | "character"
    speaker_id          TEXT NOT NULL,
    message             TEXT NOT NULL,
    depth               INTEGER NOT NULL,               -- 0 for root pill
    siblings_at_depth   INTEGER NOT NULL,               -- usually 4
    sibling_index       INTEGER NOT NULL,               -- 0..3
    direction           TEXT NOT NULL DEFAULT '',       -- "closer"|"wider"|"opposite"|"deeper"|"" for root
    resonance           INTEGER NOT NULL DEFAULT 0,
    created_at          TEXT NOT NULL,
    bytes               INTEGER NOT NULL DEFAULT 0      -- length(message); maintained on insert
);

CREATE INDEX rabbit_thought_by_scene ON rabbit_thought(scene_id, parent_id);

-- Trim helper: cheap rank-by-age query for the non-resonant leaves
-- the trim policy targets first. The `(resonance, created_at)`
-- ordering means a single scan walks resonant-protected rows last.
CREATE INDEX rabbit_thought_trim_order
    ON rabbit_thought(resonance, created_at);

UPDATE schema_version SET version = 9;
