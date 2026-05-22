//! `SQLite` connection wrapper for a single Water project.

use crate::{migrations, Error, Result};
use rusqlite::{Connection, OpenFlags};
use std::path::Path;

pub struct Db {
    conn: Connection,
}

impl Db {
    /// Open (and migrate) the project DB at `path`. Creates the file if
    /// it does not exist. WAL mode is enabled for cloud-sync-folder safety.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let flags = OpenFlags::SQLITE_OPEN_READ_WRITE
            | OpenFlags::SQLITE_OPEN_CREATE
            | OpenFlags::SQLITE_OPEN_URI
            | OpenFlags::SQLITE_OPEN_NO_MUTEX;
        let mut conn = Connection::open_with_flags(path, flags)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;

        let migrations = migrations::all();
        migrations.to_latest(&mut conn).map_err(Error::Migration)?;

        Ok(Self { conn })
    }

    /// In-memory DB for tests.
    pub fn open_in_memory() -> Result<Self> {
        let mut conn = Connection::open_in_memory()?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let migrations = migrations::all();
        migrations.to_latest(&mut conn).map_err(Error::Migration)?;
        Ok(Self { conn })
    }

    #[must_use]
    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    #[must_use]
    pub fn conn_mut(&mut self) -> &mut Connection {
        &mut self.conn
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_memory_db_runs_migrations() {
        let db = Db::open_in_memory().unwrap();
        let count: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='project'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "project table should exist after migration");
    }

    #[test]
    fn file_db_persists_across_opens() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let path = tmp.path().to_owned();
        drop(tmp); // we need the path, not the file
        {
            let _db = Db::open(&path).unwrap();
        }
        let _db2 = Db::open(&path).unwrap();
        std::fs::remove_file(&path).ok();
    }

    #[test]
    fn migration_creates_all_spec_tables() {
        let db = Db::open_in_memory().unwrap();
        let expected = [
            "schema_version",
            "project",
            "manuscript",
            "chapter",
            "scene",
            "scene_character_presence",
            "character",
            "world_segment",
            "world_entry",
            "pinned_pill",
            "scene_metrics",
            "block_metrics",
            "snapshot",
            "settings",
            "provider_config",
            "telemetry_event",
        ];
        for name in expected {
            let count: i64 = db
                .conn()
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
                    [name],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(count, 1, "missing table {name}");
        }
    }

    #[test]
    fn schema_version_row_matches_latest_migration() {
        // `Db::open_in_memory` ratchets to the latest migration; the
        // `schema_version` table is human-readable bookkeeping kept in
        // sync by each migration script. As of v10 (editor_pill table
        // for the Phase-5 diagnostic surface), the current version is 10.
        let db = Db::open_in_memory().unwrap();
        let v: i64 = db
            .conn()
            .query_row("SELECT version FROM schema_version", [], |r| r.get(0))
            .unwrap();
        assert_eq!(v, 10);
    }
}
