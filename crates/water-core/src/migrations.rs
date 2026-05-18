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

#[must_use]
pub fn all() -> Migrations<'static> {
    Migrations::new(vec![M::up(V1_INIT), M::up(V2_PILL_ENGINE)])
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
    fn migration_ratchets_from_v1_to_v2() {
        // Open Db::open against a fresh path; it runs both migrations.
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

        // Db::open now sees a v1 DB and ratchets to v2.
        let db = Db::open(&path).unwrap();
        assert_eq!(current_version(db.conn()).unwrap(), 2);
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
        // Db::open already ratcheted to v2; another run_pending must be a no-op.
        run_pending(&mut db).unwrap();
        run_pending(&mut db).unwrap();
        assert_eq!(current_version(db.conn()).unwrap(), 2);
    }
}
