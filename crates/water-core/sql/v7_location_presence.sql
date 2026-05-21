-- v7: multi-location presence on scenes.
--
-- scene.location_id stays the "primary" location (the one the scene is
-- set in by default). This junction table records every world_entry a
-- scene also touches — for the canvas multi-lane view and for future
-- world-drift / cross-reference checks. Mirrors scene_character_presence
-- from v1 exactly.

CREATE TABLE scene_location_presence (
    scene_id TEXT NOT NULL REFERENCES scene(id) ON DELETE CASCADE,
    location_id TEXT NOT NULL REFERENCES world_entry(id) ON DELETE CASCADE,
    PRIMARY KEY (scene_id, location_id)
);

UPDATE schema_version SET version = 7;
