-- M6 schema v6: Macro Spatial Canvas support.
-- Forward-only. v5 -> v6.
--
-- Adds three nullable columns to scene:
--
--   canvas_x       — REAL, x-coordinate in canvas-space units.
--                    NULL means "unplaced"; the renderer auto-flows
--                    unplaced scenes into a left-to-right row by
--                    manuscript order on first paint.
--   canvas_y       — REAL, y-coordinate.
--   canvas_group   — TEXT, optional group label (e.g. "Act II").
--                    Pure visual hint; not a separate model entity.
--
-- Values also round-trip to scene-md frontmatter so manual edits
-- + git moves preserve canvas position (see scene_md::SceneFrontmatter).

ALTER TABLE scene ADD COLUMN canvas_x REAL;
ALTER TABLE scene ADD COLUMN canvas_y REAL;
ALTER TABLE scene ADD COLUMN canvas_group TEXT;

UPDATE schema_version SET version = 6;
