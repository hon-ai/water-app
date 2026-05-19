-- M4 schema v4: World/Setting Bible support.
-- Forward-only. v3 -> v4.
--
-- Adds template-override and visibility/timestamp columns to world_segment
-- (so user-customized templates can be persisted in the DB while built-ins
-- live as Rust consts in crates/water-core/src/world/templates.rs); adds
-- aliases + schema_version + timestamps to world_entry (parity with
-- character + Stage 1 of the world_drift trigger needs the alias index);
-- and adds pinned_pill.origin_trigger so the M4 Chorus-pin -> world_entry
-- stub handler can detect the no_universe_yet origin.

ALTER TABLE world_segment ADD COLUMN template_json TEXT;
ALTER TABLE world_segment ADD COLUMN hidden INTEGER NOT NULL DEFAULT 0;
ALTER TABLE world_segment ADD COLUMN hue_token TEXT NOT NULL DEFAULT '';
ALTER TABLE world_segment ADD COLUMN slug TEXT NOT NULL DEFAULT '';
ALTER TABLE world_segment ADD COLUMN created_at TEXT NOT NULL DEFAULT '';
ALTER TABLE world_segment ADD COLUMN updated_at TEXT NOT NULL DEFAULT '';

ALTER TABLE world_entry ADD COLUMN aliases_json TEXT NOT NULL DEFAULT '[]';
ALTER TABLE world_entry ADD COLUMN schema_version TEXT NOT NULL DEFAULT '';
ALTER TABLE world_entry ADD COLUMN created_at TEXT NOT NULL DEFAULT '';
ALTER TABLE world_entry ADD COLUMN updated_at TEXT NOT NULL DEFAULT '';

ALTER TABLE pinned_pill ADD COLUMN origin_trigger TEXT;

CREATE INDEX IF NOT EXISTS world_entry_by_segment ON world_entry(segment_id);

-- Backfill: any pre-existing world_segment rows (unlikely since the world
-- surface lands in M4) get their slug seeded from the existing name in
-- lowercase. Built-in seeding in Rust (Task 3) will overwrite to the
-- canonical slug ("concept", "locations", ...) when seed_builtins runs.
UPDATE world_segment SET slug = LOWER(REPLACE(name, ' ', '_')) WHERE slug = '';

-- TODO(m5 polish): consider adding
--   CHECK (hue_token = '' OR hue_token LIKE '--water-hue-world-%')
-- once the round-robin assignment in Rust covers all entry points.

UPDATE schema_version SET version = 4;
