//! `HeatStore` — CRUD over the `heat_metric` cache table (v5 schema).
//!
//! Per-(scene, paragraph_ix, metric) score storage with text_hash-gated
//! invalidation. The compute path (Phase A Task 4 onward) writes batches
//! via [`HeatStore::write_batch`]; the renderer reads via
//! [`HeatStore::read`] keyed by metric kind. Per-paragraph invalidation
//! ([`HeatStore::invalidate`]) drops all five metrics for one paragraph
//! in a single statement so the compute path can re-emit them.

use crate::{heat::types::HeatRow, heat::HeatMetricKind, Db, Error, Id, Result};

// SQLite errors auto-convert via the `#[from] rusqlite::Error` arm on
// `Error::Sqlite`, but we surface them with an explicit `.map_err(Error::Sqlite)`
// at every call so a future refactor that drops the From impl doesn't
// silently change the behavior here.

pub struct HeatStore<'a> {
    db: &'a Db,
}

impl<'a> HeatStore<'a> {
    #[must_use]
    pub const fn new(db: &'a Db) -> Self {
        Self { db }
    }

    /// Read all cached rows for `(scene_id, kind)`, ordered by ascending
    /// `paragraph_ix`. Empty result is normal for scenes that haven't
    /// had a compute pass yet — the renderer treats that as "no data,
    /// strip is blank."
    ///
    /// # Errors
    /// Returns [`Error::Sql`] on any underlying SQLite failure.
    pub fn read(&self, scene_id: &Id, kind: HeatMetricKind) -> Result<Vec<HeatRow>> {
        let conn = self.db.conn();
        let mut stmt = conn
            .prepare(
                "SELECT paragraph_ix, value, text_hash, updated_at
                 FROM heat_metric
                 WHERE scene_id = ?1 AND metric = ?2
                 ORDER BY paragraph_ix ASC",
            )
            .map_err(Error::Sqlite)?;
        let rows = stmt
            .query_map((scene_id.as_str(), kind.as_str()), |r| {
                let ix: i64 = r.get(0)?;
                let value: f64 = r.get(1)?;
                let text_hash: String = r.get(2)?;
                let updated_at: String = r.get(3)?;
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                Ok(HeatRow {
                    paragraph_ix: ix as u32,
                    #[allow(clippy::cast_possible_truncation)]
                    value: value as f32,
                    text_hash,
                    updated_at,
                })
            })
            .map_err(Error::Sqlite)?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r.map_err(Error::Sqlite)?);
        }
        Ok(out)
    }

    /// Replace (or insert) a batch of rows for `(scene_id, kind)`. The
    /// `(paragraph_ix, value, text_hash)` tuples carry exactly what the
    /// compute path produced. `updated_at` is stamped to "now" for every
    /// row in the batch. Atomic per batch — either every row lands or
    /// none do.
    ///
    /// # Errors
    /// Returns [`Error::Sql`] if the transaction or any INSERT fails.
    pub fn write_batch(
        &self,
        scene_id: &Id,
        kind: HeatMetricKind,
        rows: &[(u32, f32, &str)],
    ) -> Result<()> {
        if rows.is_empty() {
            return Ok(());
        }
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.db.conn();
        let tx = conn.unchecked_transaction().map_err(Error::Sqlite)?;
        {
            let mut stmt = tx
                .prepare(
                    "INSERT OR REPLACE INTO heat_metric
                     (scene_id, paragraph_ix, metric, value, text_hash, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                )
                .map_err(Error::Sqlite)?;
            for (ix, value, text_hash) in rows {
                stmt.execute((
                    scene_id.as_str(),
                    i64::from(*ix),
                    kind.as_str(),
                    f64::from(*value),
                    *text_hash,
                    now.as_str(),
                ))
                .map_err(Error::Sqlite)?;
            }
        }
        tx.commit().map_err(Error::Sqlite)?;
        Ok(())
    }

    /// Drop every metric row for one paragraph. Used by the compute path
    /// when the live paragraph's text_hash changed (and the next compute
    /// pass will refill).
    ///
    /// # Errors
    /// Returns [`Error::Sql`] on any underlying SQLite failure.
    pub fn invalidate(&self, scene_id: &Id, paragraph_ix: u32) -> Result<()> {
        self.db
            .conn()
            .execute(
                "DELETE FROM heat_metric WHERE scene_id = ?1 AND paragraph_ix = ?2",
                (scene_id.as_str(), i64::from(paragraph_ix)),
            )
            .map_err(Error::Sqlite)?;
        Ok(())
    }

    /// Drop every metric row for a scene. Used when the scene body's
    /// paragraph count shrinks (the compute path can't otherwise tell
    /// which orphaned rows to evict) or on a "recompute everything"
    /// command from the renderer.
    ///
    /// # Errors
    /// Returns [`Error::Sql`] on any underlying SQLite failure.
    pub fn invalidate_all(&self, scene_id: &Id) -> Result<()> {
        self.db
            .conn()
            .execute(
                "DELETE FROM heat_metric WHERE scene_id = ?1",
                [scene_id.as_str()],
            )
            .map_err(Error::Sqlite)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ManuscriptStore, NewScene, ProjectStore, SceneStore};
    use tempfile::TempDir;

    fn seed_scene() -> (TempDir, Db, Id) {
        let dir = TempDir::new().unwrap();
        let db = Db::open(dir.path().join("project.db")).unwrap();
        let project = ProjectStore::new(&db).insert("P").unwrap();
        let manuscript = ManuscriptStore::new(&db)
            .insert(&project.id, "M", 0)
            .unwrap();
        let scene = SceneStore::new(&db, dir.path().to_path_buf())
            .create(NewScene {
                manuscript_id: manuscript.id,
                chapter_id: None,
                name: "S".into(),
                ordering: 0,
            })
            .unwrap();
        (dir, db, scene.id)
    }

    #[test]
    fn write_batch_then_read_round_trips() {
        let (_dir, db, scene_id) = seed_scene();
        let store = HeatStore::new(&db);
        store
            .write_batch(
                &scene_id,
                HeatMetricKind::Pacing,
                &[(0, 0.2, "h0"), (1, 0.8, "h1"), (2, 0.5, "h2")],
            )
            .unwrap();
        let rows = store.read(&scene_id, HeatMetricKind::Pacing).unwrap();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].paragraph_ix, 0);
        assert!((rows[0].value - 0.2).abs() < 1e-5);
        assert_eq!(rows[0].text_hash, "h0");
        assert_eq!(rows[2].paragraph_ix, 2);
        assert!((rows[2].value - 0.5).abs() < 1e-5);
    }

    #[test]
    fn write_batch_replaces_existing_rows_for_same_key() {
        let (_dir, db, scene_id) = seed_scene();
        let store = HeatStore::new(&db);
        store
            .write_batch(&scene_id, HeatMetricKind::Pacing, &[(0, 0.1, "h0")])
            .unwrap();
        store
            .write_batch(&scene_id, HeatMetricKind::Pacing, &[(0, 0.9, "h0-new")])
            .unwrap();
        let rows = store.read(&scene_id, HeatMetricKind::Pacing).unwrap();
        assert_eq!(rows.len(), 1, "duplicate (scene, ix, metric) replaces, not appends");
        assert!((rows[0].value - 0.9).abs() < 1e-5);
        assert_eq!(rows[0].text_hash, "h0-new");
    }

    #[test]
    fn read_segregates_metrics_by_kind() {
        let (_dir, db, scene_id) = seed_scene();
        let store = HeatStore::new(&db);
        store
            .write_batch(&scene_id, HeatMetricKind::Pacing, &[(0, 0.1, "h")])
            .unwrap();
        store
            .write_batch(&scene_id, HeatMetricKind::Valence, &[(0, -0.5, "h")])
            .unwrap();
        let pacing = store.read(&scene_id, HeatMetricKind::Pacing).unwrap();
        let valence = store.read(&scene_id, HeatMetricKind::Valence).unwrap();
        assert_eq!(pacing.len(), 1);
        assert_eq!(valence.len(), 1);
        assert!((pacing[0].value - 0.1).abs() < 1e-5);
        assert!((valence[0].value + 0.5).abs() < 1e-5);
    }

    #[test]
    fn invalidate_drops_only_target_paragraph_across_all_metrics() {
        let (_dir, db, scene_id) = seed_scene();
        let store = HeatStore::new(&db);
        store
            .write_batch(
                &scene_id,
                HeatMetricKind::Pacing,
                &[(0, 0.1, "h0"), (1, 0.2, "h1")],
            )
            .unwrap();
        store
            .write_batch(
                &scene_id,
                HeatMetricKind::Valence,
                &[(0, -0.5, "h0"), (1, 0.5, "h1")],
            )
            .unwrap();
        store.invalidate(&scene_id, 0).unwrap();
        let pacing = store.read(&scene_id, HeatMetricKind::Pacing).unwrap();
        let valence = store.read(&scene_id, HeatMetricKind::Valence).unwrap();
        // Paragraph 0 gone in BOTH metrics; paragraph 1 still there in both.
        assert_eq!(pacing.len(), 1);
        assert_eq!(pacing[0].paragraph_ix, 1);
        assert_eq!(valence.len(), 1);
        assert_eq!(valence[0].paragraph_ix, 1);
    }

    #[test]
    fn invalidate_all_drops_every_row_for_scene() {
        let (_dir, db, scene_id) = seed_scene();
        let store = HeatStore::new(&db);
        store
            .write_batch(
                &scene_id,
                HeatMetricKind::Pacing,
                &[(0, 0.1, "h"), (1, 0.2, "h")],
            )
            .unwrap();
        store
            .write_batch(
                &scene_id,
                HeatMetricKind::Valence,
                &[(0, 0.0, "h")],
            )
            .unwrap();
        store.invalidate_all(&scene_id).unwrap();
        assert!(store.read(&scene_id, HeatMetricKind::Pacing).unwrap().is_empty());
        assert!(store.read(&scene_id, HeatMetricKind::Valence).unwrap().is_empty());
    }

    #[test]
    fn empty_batch_is_noop() {
        let (_dir, db, scene_id) = seed_scene();
        let store = HeatStore::new(&db);
        // Should not error; should not start a transaction either.
        store
            .write_batch(&scene_id, HeatMetricKind::Pacing, &[])
            .unwrap();
        assert!(store.read(&scene_id, HeatMetricKind::Pacing).unwrap().is_empty());
    }

    #[test]
    fn read_returns_empty_for_unknown_scene() {
        let (_dir, db, _scene_id) = seed_scene();
        let store = HeatStore::new(&db);
        let bogus = crate::Id::new();
        let rows = store.read(&bogus, HeatMetricKind::Pacing).unwrap();
        assert!(rows.is_empty());
    }
}
