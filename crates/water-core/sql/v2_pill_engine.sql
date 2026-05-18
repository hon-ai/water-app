-- M2 schema v2: pill engine extensions to pinned_pill + ratchet schema_version.
-- Forward-only. v1 -> v2.
--
-- The `schema_version` table is human-readable bookkeeping; rusqlite_migration
-- separately tracks the canonical version via SQLite's `user_version` PRAGMA.
-- We keep both in sync so observability and the runtime agree.

ALTER TABLE pinned_pill ADD COLUMN parent_pill_id TEXT NULL
    REFERENCES pinned_pill(id) ON DELETE SET NULL;
ALTER TABLE pinned_pill ADD COLUMN pinned_at TEXT NOT NULL DEFAULT '';
ALTER TABLE pinned_pill ADD COLUMN trigger_class TEXT NOT NULL DEFAULT '';
ALTER TABLE pinned_pill ADD COLUMN bouquet_position INTEGER NULL;

-- Backfill pinned_at from created_at for any pre-existing rows.
UPDATE pinned_pill SET pinned_at = created_at WHERE pinned_at = '';

UPDATE schema_version SET version = 2;
