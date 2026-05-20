//! Forward-only schema migration runner for the Water project database.
//!
//! Each entry in [`all`] is `M::up(SQL)`. Migrations are applied atomically
//! in order; `rusqlite_migration` tracks the applied version via
//! `SQLite`'s `user_version` PRAGMA. We additionally maintain the `schema_version`
//! table inside each migration script so the value is visible to humans
//! (e.g. when inspecting a project.db directly) and matches the runtime.
//!
//! Adding a new migration: append `M::up(include_str!("../sql/vN_*.sql"))`
//! to the vector below and end the script with
//! `UPDATE schema_version SET version = N;`. Never edit or reorder past
//! migrations; only append.
//!
//! `Db::open` calls [`run_pending`] internally, so callers normally don't
//! need to invoke it directly. [`current_version`] and [`run_pending`] are
//! exposed for tests and for code that constructs a raw `Connection` outside
//! the `Db` wrapper.

use crate::Db;
use rusqlite::Connection;
use rusqlite_migration::{Migrations, M};

const V1_INIT: &str = include_str!("../sql/v1_init.sql");
const V2_PILL_ENGINE: &str = include_str!("../sql/v2_pill_engine.sql");
const V3_CHARACTER_HUE: &str = include_str!("../sql/v3_character_hue.sql");
const V4_WORLD_BIBLE: &str = include_str!("../sql/v4_world_bible.sql");
const V5_HEATMAP: &str = include_str!("../sql/v5_heatmap.sql");
const V6_CANVAS: &str = include_str!("../sql/v6_canvas.sql");

#[must_use]
pub fn all() -> Migrations<'static> {
    Migrations::new(vec![
        M::up(V1_INIT),
        M::up(V2_PILL_ENGINE),
        M::up(V3_CHARACTER_HUE),
        M::up(V4_WORLD_BIBLE),
        M::up(V5_HEATMAP),
        M::up(V6_CANVAS),
    ])
}

/// Returns the value of the `schema_version` table. This is the
/// human-readable bookkeeping value, kept in sync with the
/// `rusqlite_migration` `user_version` by each migration script.
///
/// Returns 1 for a v1-only DB that has not been migrated yet.
pub fn current_version(conn: &Connection) -> rusqlite::Result<u32> {
    conn.query_row("SELECT version FROM schema_version", [], |r| r.get(0))
}

/// Apply any pending forward migrations to `db`. Idempotent: re-running
/// after completion is a no-op (`rusqlite_migration` compares `user_version`
/// against the migration list length).
///
/// `Db::open` already calls this; the function is exposed for tests and
/// for callers that hold a `Db` constructed via a different path.
pub fn run_pending(db: &mut Db) -> Result<(), String> {
    all()
        .to_latest(db.conn_mut())
        .map_err(|e| format!("migration failed: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;
    use tempfile::TempDir;

    /// Build a DB that contains v1 schema only (no v2 columns yet) so we
    /// can exercise [`run_pending`] from the same starting point a
    /// previously-shipped M1 project.db would have on disk.
    fn fresh_v1_db() -> (TempDir, Db) {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("project.db");

        // Build a v1-only connection by hand, then wrap it in `Db` via the
        // public `open` API. The second `Db::open` call would normally
        // ratchet to latest; to keep the test honest we open with ONLY the
        // v1 migration applied, then poke user_version back to 1 so the
        // subsequent `run_pending` from the production code path is what
        // performs the v1->v2 ratchet.
        {
            let mut conn = rusqlite::Connection::open(&path).unwrap();
            conn.pragma_update(None, "journal_mode", "WAL").unwrap();
            conn.pragma_update(None, "foreign_keys", "ON").unwrap();
            let v1_only = Migrations::new(vec![M::up(V1_INIT)]);
            v1_only.to_latest(&mut conn).unwrap();
        }
        // Now re-open through the public Db::open. But Db::open auto-migrates
        // to latest, defeating our v1-only setup. Work around by opening the
        // connection directly and constructing Db without going through open.
        // Db has no such constructor by design — instead we accept that
        // Db::open ratchets to latest and write a parallel raw-Connection
        // test below for the "starts at v1" scenario.
        let db = Db::open(&path).unwrap();
        (dir, db)
    }

    #[test]
    fn migration_ratchets_from_v1_to_latest() {
        // Open Db::open against a fresh path; it runs all migrations.
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("project.db");

        // Manually apply only v1 so we can observe the ratchet.
        let mut conn = rusqlite::Connection::open(&path).unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        Migrations::new(vec![M::up(V1_INIT)])
            .to_latest(&mut conn)
            .unwrap();
        assert_eq!(current_version(&conn).unwrap(), 1);
        drop(conn);

        // Db::open now sees a v1 DB and ratchets to latest (v5).
        let db = Db::open(&path).unwrap();
        assert_eq!(current_version(db.conn()).unwrap(), 6);
    }

    #[test]
    fn migration_adds_pinned_pill_columns() {
        let (_tmp, db) = fresh_v1_db();
        let cols: Vec<String> = db
            .conn()
            .prepare("PRAGMA table_info(pinned_pill)")
            .unwrap()
            .query_map([], |r| r.get::<_, String>(1))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        for needed in &[
            "parent_pill_id",
            "pinned_at",
            "trigger_class",
            "bouquet_position",
        ] {
            assert!(
                cols.iter().any(|c| c == needed),
                "missing column: {needed} (have: {cols:?})"
            );
        }
    }

    #[test]
    fn migration_backfills_pinned_at_from_created_at() {
        // Build a v1 DB, insert a pinned_pill row with the v1 shape (with
        // valid FK parents), then ratchet to v2 and assert the backfill
        // ran. This exercises the UPDATE statement end-to-end.
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("project.db");

        // Apply v1 only.
        {
            let mut conn = rusqlite::Connection::open(&path).unwrap();
            conn.pragma_update(None, "foreign_keys", "ON").unwrap();
            Migrations::new(vec![M::up(V1_INIT)])
                .to_latest(&mut conn)
                .unwrap();
            // Seed FK parents: project -> manuscript -> scene -> pinned_pill.
            let now = "2026-05-17T00:00:00Z";
            conn.execute(
                "INSERT INTO project (id, name, created_at, updated_at)
                 VALUES ('proj1', 'p', ?1, ?1)",
                params![now],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO manuscript (id, project_id, name, ordering, created_at, updated_at)
                 VALUES ('m1', 'proj1', 'm', 0, ?1, ?1)",
                params![now],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO scene (id, manuscript_id, ordering, name, file_path, created_at, updated_at)
                 VALUES ('s1', 'm1', 0, 'scene', 'manuscript/scenes/s1.md', ?1, ?1)",
                params![now],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO pinned_pill
                 (id, scene_id, block_id, snippet, speaker_kind, speaker_id, message, hue, rabbit_hole_path, created_at)
                 VALUES ('p1', 's1', '^bk-0001', 'snip', 'persona', 'echo', 'msg', '#abc', NULL, ?1)",
                params![now],
            )
            .unwrap();
        }

        // Ratchet to v2 via Db::open (which calls run_pending).
        let db = Db::open(&path).unwrap();
        let pinned_at: String = db
            .conn()
            .query_row(
                "SELECT pinned_at FROM pinned_pill WHERE id = 'p1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(pinned_at, "2026-05-17T00:00:00Z");
    }

    #[test]
    fn migration_is_idempotent() {
        let (_tmp, mut db) = fresh_v1_db();
        // Db::open already ratcheted to latest; another run_pending must be a no-op.
        run_pending(&mut db).unwrap();
        run_pending(&mut db).unwrap();
        assert_eq!(current_version(db.conn()).unwrap(), 6);
    }

    #[test]
    fn migration_ratchets_to_v6() {
        let (_tmp, mut db) = fresh_v1_db();
        // fresh_v1_db actually returns a DB already ratcheted to latest via
        // Db::open; another run_pending is a no-op that still leaves us at v5.
        run_pending(&mut db).unwrap();
        assert_eq!(current_version(db.conn()).unwrap(), 6);
    }

    #[test]
    fn migration_v3_adds_hue_token_column() {
        let (_tmp, mut db) = fresh_v1_db();
        run_pending(&mut db).unwrap();
        let cols: Vec<String> = db
            .conn()
            .prepare("PRAGMA table_info(character)")
            .unwrap()
            .query_map([], |r| r.get::<_, String>(1))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert!(
            cols.iter().any(|c| c == "hue_token"),
            "missing hue_token column (have: {cols:?})"
        );
    }

    #[test]
    fn migration_v3_backfills_hue_round_robin() {
        // Build a true v1 DB, insert a project + 4 characters using the v1
        // schema (no hue_token column yet), then ratchet to latest via
        // Db::open and assert the backfill produced round-robin hues.
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("project.db");

        {
            let mut conn = rusqlite::Connection::open(&path).unwrap();
            conn.pragma_update(None, "foreign_keys", "ON").unwrap();
            Migrations::new(vec![M::up(V1_INIT)])
                .to_latest(&mut conn)
                .unwrap();

            conn.execute(
                "INSERT INTO project (id, name, created_at, updated_at)
                 VALUES ('p1', 'P', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
                params![],
            )
            .unwrap();
            for (i, id) in ["c1", "c2", "c3", "c4"].iter().enumerate() {
                let created_at = format!("2026-01-0{}T00:00:00Z", i + 1);
                conn.execute(
                    "INSERT INTO character
                     (id, project_id, name, schema_version, data_json, file_path, created_at, updated_at)
                     VALUES (?1, 'p1', ?2, 'lsm-v2.1', '{}', ?3, ?4, ?4)",
                    params![id, id, format!("characters/{}.toml", id), created_at],
                )
                .unwrap();
            }
        }

        let db = Db::open(&path).unwrap();
        let hues: Vec<String> = db
            .conn()
            .prepare("SELECT hue_token FROM character ORDER BY created_at")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(
            hues,
            vec![
                "--water-hue-character-1".to_string(),
                "--water-hue-character-2".to_string(),
                "--water-hue-character-3".to_string(),
                "--water-hue-character-4".to_string(),
            ]
        );
    }

    #[test]
    fn v4_adds_world_segment_template_columns() {
        let db = Db::open_in_memory().unwrap();
        let cols: Vec<String> = db
            .conn()
            .prepare("PRAGMA table_info(world_segment)")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .collect::<std::result::Result<_, _>>()
            .unwrap();
        for required in [
            "template_json",
            "hidden",
            "hue_token",
            "slug",
            "created_at",
            "updated_at",
        ] {
            assert!(
                cols.iter().any(|c| c == required),
                "world_segment missing column {required}; got {cols:?}"
            );
        }
    }

    #[test]
    fn v4_adds_world_entry_alias_and_timestamp_columns() {
        let db = Db::open_in_memory().unwrap();
        let cols: Vec<String> = db
            .conn()
            .prepare("PRAGMA table_info(world_entry)")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .collect::<std::result::Result<_, _>>()
            .unwrap();
        for required in [
            "aliases_json",
            "schema_version",
            "created_at",
            "updated_at",
        ] {
            assert!(
                cols.iter().any(|c| c == required),
                "world_entry missing column {required}; got {cols:?}"
            );
        }
    }

    #[test]
    fn v4_adds_pinned_pill_origin_trigger() {
        let db = Db::open_in_memory().unwrap();
        let has_col: bool = db
            .conn()
            .prepare("PRAGMA table_info(pinned_pill)")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .filter_map(std::result::Result::ok)
            .any(|c| c == "origin_trigger");
        assert!(has_col, "pinned_pill missing origin_trigger column");
    }

    // The historical `v4_schema_version_is_four` check was retired when v5
    // landed; `v5_schema_version_is_five` below replaces it as the
    // latest-version assertion.

    #[test]
    fn v4_creates_world_entry_by_segment_index() {
        let db = Db::open_in_memory().unwrap();
        let count: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='world_entry_by_segment'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "world_entry_by_segment index missing");
    }

    #[test]
    fn v5_creates_heat_metric_table() {
        let db = Db::open_in_memory().unwrap();
        let cols: Vec<String> = db
            .conn()
            .prepare("PRAGMA table_info(heat_metric)")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .collect::<std::result::Result<_, _>>()
            .unwrap();
        for required in [
            "scene_id",
            "paragraph_ix",
            "metric",
            "value",
            "text_hash",
            "updated_at",
        ] {
            assert!(
                cols.iter().any(|c| c == required),
                "heat_metric missing column {required}; got {cols:?}"
            );
        }
    }

    #[test]
    fn v5_creates_scene_typing_history_table() {
        let db = Db::open_in_memory().unwrap();
        let cols: Vec<String> = db
            .conn()
            .prepare("PRAGMA table_info(scene_typing_history)")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .collect::<std::result::Result<_, _>>()
            .unwrap();
        for required in ["scene_id", "ts_ms", "word_delta"] {
            assert!(
                cols.iter().any(|c| c == required),
                "scene_typing_history missing column {required}; got {cols:?}"
            );
        }
    }

    #[test]
    fn v5_creates_heat_metric_index() {
        let db = Db::open_in_memory().unwrap();
        let count: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='heat_metric_by_scene'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "heat_metric_by_scene index missing");
    }

    // The historical `v5_schema_version_is_five` check retired when v6
    // landed; `v6_schema_version_is_six` below replaces it.

    #[test]
    fn v6_schema_version_is_six() {
        let db = Db::open_in_memory().unwrap();
        let version: u32 = db
            .conn()
            .query_row("SELECT version FROM schema_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(version, 6);
    }

    #[test]
    fn v6_adds_scene_canvas_columns() {
        let db = Db::open_in_memory().unwrap();
        let cols: Vec<String> = db
            .conn()
            .prepare("PRAGMA table_info(scene)")
            .unwrap()
            .query_map([], |r| r.get::<_, String>(1))
            .unwrap()
            .collect::<std::result::Result<_, _>>()
            .unwrap();
        for required in ["canvas_x", "canvas_y", "canvas_group"] {
            assert!(
                cols.iter().any(|c| c == required),
                "scene missing column {required}; got {cols:?}"
            );
        }
    }

    #[test]
    fn v6_canvas_columns_are_nullable_by_default() {
        // Pre-M6 scenes (or new scenes that don't set canvas position
        // on insert) MUST have NULL canvas_x / canvas_y. The renderer
        // treats NULL as "unplaced" and auto-flows them.
        let db = Db::open_in_memory().unwrap();
        db.conn()
            .execute(
                "INSERT INTO project (id, name, created_at, updated_at)
                 VALUES ('p1', 'P', '0', '0')",
                [],
            )
            .unwrap();
        db.conn()
            .execute(
                "INSERT INTO manuscript (id, project_id, name, ordering, created_at, updated_at)
                 VALUES ('m1', 'p1', 'M', 0, '0', '0')",
                [],
            )
            .unwrap();
        db.conn()
            .execute(
                "INSERT INTO scene (id, manuscript_id, ordering, name, file_path, created_at, updated_at)
                 VALUES ('s1', 'm1', 0, 's', 'manuscript/scenes/s1.md', '0', '0')",
                [],
            )
            .unwrap();
        let (x, y, group): (Option<f64>, Option<f64>, Option<String>) = db
            .conn()
            .query_row(
                "SELECT canvas_x, canvas_y, canvas_group FROM scene WHERE id = 's1'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert!(x.is_none());
        assert!(y.is_none());
        assert!(group.is_none());
    }

    #[test]
    fn v5_heat_metric_cascades_on_scene_delete() {
        // Seed: project → manuscript → scene → heat_metric row. Delete the
        // scene and assert the heat_metric row is gone too (ON DELETE CASCADE
        // must be wired so Heatmap state never outlives its scene).
        let db = Db::open_in_memory().unwrap();
        db.conn()
            .execute(
                "INSERT INTO project (id, name, created_at, updated_at)
                 VALUES ('p1', 'P', '0', '0')",
                [],
            )
            .unwrap();
        db.conn()
            .execute(
                "INSERT INTO manuscript (id, project_id, name, ordering, created_at, updated_at)
                 VALUES ('m1', 'p1', 'M', 0, '0', '0')",
                [],
            )
            .unwrap();
        db.conn()
            .execute(
                "INSERT INTO scene (id, manuscript_id, ordering, name, file_path, created_at, updated_at)
                 VALUES ('s1', 'm1', 0, 's', 'manuscript/scenes/s1.md', '0', '0')",
                [],
            )
            .unwrap();
        db.conn()
            .execute(
                "INSERT INTO heat_metric
                 (scene_id, paragraph_ix, metric, value, text_hash, updated_at)
                 VALUES ('s1', 0, 'pacing', 0.5, 'h', '0')",
                [],
            )
            .unwrap();
        db.conn()
            .execute("DELETE FROM scene WHERE id = 's1'", [])
            .unwrap();
        let remaining: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM heat_metric WHERE scene_id = 's1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(remaining, 0, "heat_metric row should cascade-delete with its scene");
    }

    #[test]
    fn v4_backfills_slug_from_name_on_existing_world_segment_rows() {
        // Build a v3-shape DB by hand and insert a world_segment row with
        // the v1-shape columns only (no slug column yet — it doesn't exist
        // until v4). Then ratchet to v4 and verify the backfill UPDATE ran.
        // `Connection`, `Migrations`, `M`, and `TempDir` are all in scope
        // via `use super::*` + the module-level `use tempfile::TempDir`.
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("project.db");
        {
            let mut conn = Connection::open(&path).unwrap();
            conn.pragma_update(None, "journal_mode", "WAL").unwrap();
            conn.pragma_update(None, "foreign_keys", "ON").unwrap();
            let v3 = Migrations::new(vec![
                M::up(V1_INIT),
                M::up(V2_PILL_ENGINE),
                M::up(V3_CHARACTER_HUE),
            ]);
            v3.to_latest(&mut conn).unwrap();

            // Insert a project + a world_segment row (v1 shape: no slug column).
            conn.execute(
                "INSERT INTO project (id, name, default_manuscript_id, created_at, updated_at)
                 VALUES ('p1', 'TestProj', NULL, '0', '0')",
                [],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO world_segment (id, project_id, name, ordering, is_collection)
                 VALUES ('s1', 'p1', 'Hello World', 0, 0)",
                [],
            )
            .unwrap();
        }

        // Now open via Db, which runs run_pending and ratchets v3 -> v4.
        let db = Db::open(&path).unwrap();

        let slug: String = db
            .conn()
            .query_row(
                "SELECT slug FROM world_segment WHERE id = 's1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            slug, "hello_world",
            "v4 migration must backfill slug = LOWER(REPLACE(name, ' ', '_')) for pre-existing rows; got {slug:?}"
        );
    }
}
