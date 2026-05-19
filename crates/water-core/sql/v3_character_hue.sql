-- M3 schema v3: add character hue_token column with round-robin backfill.
-- Forward-only. v2 -> v3.
--
-- New characters created post-M3 will be assigned a hue at insert time
-- (M3 T12 character_create). This migration backfills any existing
-- pre-M3 character rows round-robin against the six --water-hue-character-N
-- CSS variables defined in app/src/styles/tokens.css.

ALTER TABLE character ADD COLUMN hue_token TEXT NOT NULL DEFAULT '';

-- Backfill: round-robin assign hues to existing characters by created_at order.
-- M1 created the character row format; M3 introduces hue tokens. Existing rows
-- get hues 1..6 cycling by oldest-first.
--
-- The correlated-subquery rank avoids the ROW_NUMBER() window function
-- (only available in SQLite 3.25+); this pattern works on the bundled
-- rusqlite SQLite regardless of version. Tie-break on id keeps the
-- assignment stable when two characters share a created_at timestamp.
UPDATE character
SET hue_token = '--water-hue-character-' || (
    ((SELECT COUNT(*) FROM character AS c2
      WHERE c2.created_at < character.created_at
         OR (c2.created_at = character.created_at AND c2.id < character.id)
     ) % 6) + 1
);

UPDATE schema_version SET version = 3;
