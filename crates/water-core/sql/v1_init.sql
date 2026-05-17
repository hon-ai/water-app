PRAGMA foreign_keys = ON;

CREATE TABLE schema_version (
    version INTEGER NOT NULL
);
INSERT INTO schema_version (version) VALUES (1);

CREATE TABLE project (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    default_manuscript_id TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE manuscript (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES project(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    ordering INTEGER NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE chapter (
    id TEXT PRIMARY KEY,
    manuscript_id TEXT NOT NULL REFERENCES manuscript(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    ordering INTEGER NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE character (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES project(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    schema_version TEXT NOT NULL DEFAULT 'lsm-v2.1',
    data_json TEXT NOT NULL,
    file_path TEXT NOT NULL,
    file_hash TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE world_segment (
    id TEXT PRIMARY KEY,
    project_id TEXT NOT NULL REFERENCES project(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    ordering INTEGER NOT NULL,
    is_collection INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE world_entry (
    id TEXT PRIMARY KEY,
    segment_id TEXT NOT NULL REFERENCES world_segment(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    data_json TEXT NOT NULL,
    file_path TEXT NOT NULL,
    file_hash TEXT
);

CREATE TABLE scene (
    id TEXT PRIMARY KEY,
    manuscript_id TEXT NOT NULL REFERENCES manuscript(id) ON DELETE CASCADE,
    chapter_id TEXT REFERENCES chapter(id) ON DELETE SET NULL,
    ordering INTEGER NOT NULL,
    name TEXT NOT NULL,
    pov_character_id TEXT REFERENCES character(id) ON DELETE SET NULL,
    location_id TEXT REFERENCES world_entry(id) ON DELETE SET NULL,
    scene_goal TEXT,
    status TEXT NOT NULL DEFAULT 'draft',
    word_count INTEGER NOT NULL DEFAULT 0,
    file_path TEXT NOT NULL,
    file_hash TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX scene_by_manuscript ON scene(manuscript_id, ordering);

CREATE TABLE scene_character_presence (
    scene_id TEXT NOT NULL REFERENCES scene(id) ON DELETE CASCADE,
    character_id TEXT NOT NULL REFERENCES character(id) ON DELETE CASCADE,
    PRIMARY KEY (scene_id, character_id)
);

CREATE TABLE pinned_pill (
    id TEXT PRIMARY KEY,
    scene_id TEXT NOT NULL REFERENCES scene(id) ON DELETE CASCADE,
    block_id TEXT NOT NULL,
    snippet TEXT NOT NULL,
    speaker_kind TEXT NOT NULL,
    speaker_id TEXT NOT NULL,
    message TEXT NOT NULL,
    hue TEXT NOT NULL,
    rabbit_hole_path TEXT,
    created_at TEXT NOT NULL
);

CREATE TABLE scene_metrics (
    scene_id TEXT PRIMARY KEY REFERENCES scene(id) ON DELETE CASCADE,
    flow REAL, coherence REAL, engagement REAL, divergence REAL,
    pace REAL, intensity REAL, valence REAL,
    lexical_diversity REAL, sentence_complexity REAL,
    ocean_o REAL, ocean_c REAL, ocean_e REAL, ocean_a REAL, ocean_n REAL,
    beat_label TEXT,
    beat_confidence REAL,
    summary TEXT,
    summary_for_hash TEXT,
    summary_model_id TEXT,
    last_analyzed_at TEXT,
    source_file_hash TEXT
);

CREATE TABLE block_metrics (
    scene_id TEXT NOT NULL REFERENCES scene(id) ON DELETE CASCADE,
    block_id TEXT NOT NULL,
    flow REAL, coherence REAL, divergence REAL,
    PRIMARY KEY (scene_id, block_id)
);

CREATE TABLE snapshot (
    id TEXT PRIMARY KEY,
    scene_id TEXT NOT NULL REFERENCES scene(id) ON DELETE CASCADE,
    taken_at TEXT NOT NULL,
    trigger TEXT NOT NULL,
    file_path TEXT NOT NULL,
    byte_size INTEGER NOT NULL
);
CREATE INDEX snapshot_by_scene ON snapshot(scene_id, taken_at DESC);

CREATE TABLE settings (
    key TEXT PRIMARY KEY,
    value_json TEXT NOT NULL
);

CREATE TABLE provider_config (
    id TEXT PRIMARY KEY,
    enabled INTEGER NOT NULL,
    config_json TEXT NOT NULL,
    ordering INTEGER NOT NULL
);

CREATE TABLE telemetry_event (
    id TEXT PRIMARY KEY,
    recorded_at TEXT NOT NULL,
    kind TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    sent INTEGER NOT NULL DEFAULT 0
);
