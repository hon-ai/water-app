//! Phase 5 — `EditorPillStore`: persistence layer for diagnostic pills.
//!
//! The store owns the lifecycle of `editor_pill` rows (v10 migration):
//!   - `run_and_upsert`: scan a set of blocks, render messages, upsert
//!     findings. Idempotent — re-running against the same prose
//!     refreshes `updated_at` instead of duplicating rows.
//!   - `list_active`: read-side query for the diagnostics tab.
//!   - `dismiss`: writer flagged the pill away; row stays for
//!     telemetry but never surfaces again.
//!   - `cleanup_orphaned_blocks`: drop rows whose anchor block has
//!     been removed from the manuscript (writer deleted a paragraph
//!     and never re-added it).
//!
//! The anchor model mirrors Phase 3.5's `anchorResolver`: block-id
//! + text_snippet + content_hash. Stale anchors that *partially*
//! drift are recovered by a fuzzy match at hover time on the
//! renderer side; the store stays minimal.

use crate::editor::diagnostics::{run_diagnostics_on_block, DiagnosticFinding};
use crate::editor::phrasebank::render_message;
use crate::prompts::loader::ToneClauses;
use crate::{Db, Id, Result};
use rusqlite::params;
use serde::{Deserialize, Serialize};

/// A persisted editor pill, ready to ship to the renderer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorPillRow {
    pub id: String,
    pub scene_id: String,
    pub rule: String,
    pub severity: String,
    pub message: String,
    pub suggestion: Option<String>,
    pub anchor_block_id: String,
    pub anchor_start: u32,
    pub anchor_end: u32,
    pub text_snippet: String,
    pub content_hash: String,
    pub dismissed: bool,
    pub created_at: String,
    pub updated_at: String,
}

/// One block of prose ready for diagnostic scanning. The renderer
/// extracts these from the PM doc (each top-level block has a
/// `blockId` attr + a `textContent`).
#[derive(Debug, Clone)]
pub struct ScanBlock<'a> {
    pub block_id: &'a str,
    pub text: &'a str,
}

pub struct EditorPillStore<'a> {
    db: &'a Db,
}

impl<'a> EditorPillStore<'a> {
    #[must_use]
    pub fn new(db: &'a Db) -> Self {
        Self { db }
    }

    /// Scan each block in `blocks` for diagnostic findings, render
    /// them through the phrasebank, and upsert into `editor_pill`.
    /// Findings that fail the tone gate are dropped silently. The
    /// returned vec is the set of *live* (non-dismissed) rows after
    /// the upsert, ordered by anchor_block_id + anchor_start.
    pub fn run_and_upsert(
        &self,
        scene_id: &Id,
        blocks: &[ScanBlock<'_>],
        tone: &ToneClauses,
    ) -> Result<Vec<EditorPillRow>> {
        let conn = self.db.conn();
        let now = chrono::Utc::now().to_rfc3339();
        conn.execute("BEGIN", [])?;
        for block in blocks {
            let content_hash = compute_block_hash(block.text);
            let findings = run_diagnostics_on_block(block.text);
            for f in findings {
                let Some(message) = render_message(&f, tone) else {
                    continue;
                };
                if let Err(e) = self.upsert_one(
                    scene_id.as_str(),
                    block.block_id,
                    &content_hash,
                    &f,
                    &message,
                    &now,
                ) {
                    let _ = conn.execute("ROLLBACK", []);
                    return Err(e);
                }
            }
        }
        conn.execute("COMMIT", [])?;
        self.list_active(scene_id)
    }

    /// Upsert one finding. Existing row with the same (scene_id,
    /// rule, anchor_block_id, text_snippet) and not dismissed gets
    /// its `updated_at` refreshed; otherwise we insert a new row.
    /// Dismissed rows are never touched — re-firing won't resurface
    /// what the writer already silenced.
    fn upsert_one(
        &self,
        scene_id: &str,
        block_id: &str,
        content_hash: &str,
        f: &DiagnosticFinding,
        message: &str,
        now: &str,
    ) -> Result<()> {
        let conn = self.db.conn();
        let existing: rusqlite::Result<String> = conn.query_row(
            "SELECT id FROM editor_pill
             WHERE scene_id = ?1 AND rule = ?2 AND anchor_block_id = ?3
               AND text_snippet = ?4 AND dismissed = 0",
            params![scene_id, f.rule.as_str(), block_id, f.snippet],
            |r| r.get(0),
        );
        if let Ok(existing_id) = existing {
            conn.execute(
                "UPDATE editor_pill SET
                    anchor_start = ?2, anchor_end = ?3,
                    content_hash = ?4, message = ?5, suggestion = ?6,
                    updated_at = ?7
                 WHERE id = ?1",
                params![
                    existing_id,
                    f.start as i64,
                    f.end as i64,
                    content_hash,
                    message,
                    f.suggestion,
                    now,
                ],
            )?;
            return Ok(());
        }
        let new_id = Id::new();
        conn.execute(
            "INSERT INTO editor_pill
                (id, scene_id, rule, severity, message, suggestion,
                 anchor_block_id, anchor_start, anchor_end,
                 text_snippet, content_hash, dismissed,
                 created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, 0, ?12, ?12)",
            params![
                new_id.as_str(),
                scene_id,
                f.rule.as_str(),
                f.severity.as_str(),
                message,
                f.suggestion,
                block_id,
                f.start as i64,
                f.end as i64,
                f.snippet,
                content_hash,
                now,
            ],
        )?;
        Ok(())
    }

    /// Active (non-dismissed) pills for a scene, sorted for stable
    /// rendering.
    pub fn list_active(&self, scene_id: &Id) -> Result<Vec<EditorPillRow>> {
        let conn = self.db.conn();
        let mut stmt = conn.prepare(
            "SELECT id, scene_id, rule, severity, message, suggestion,
                    anchor_block_id, anchor_start, anchor_end,
                    text_snippet, content_hash, dismissed,
                    created_at, updated_at
             FROM editor_pill
             WHERE scene_id = ?1 AND dismissed = 0
             ORDER BY anchor_block_id, anchor_start",
        )?;
        let rows = stmt.query_map(params![scene_id.as_str()], row_to_pill)?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    /// Phase 5.8 — persist a polish observation from the LLM as a
    /// regular editor_pill row. The "anchor" covers the whole block
    /// (start = 0, end = block_text.len()); text_snippet is the
    /// first 60 chars of the block so the diagnostics tab has
    /// something to render. Idempotent at the (scene, block,
    /// message) tuple — re-running the same polish doesn't
    /// duplicate.
    pub fn insert_polish(
        &self,
        scene_id: &Id,
        block_id: &str,
        block_text: &str,
        message: &str,
    ) -> Result<()> {
        let conn = self.db.conn();
        // Don't write a duplicate if the same message already exists
        // for this (scene, block) and isn't dismissed.
        let existing: rusqlite::Result<String> = conn.query_row(
            "SELECT id FROM editor_pill
             WHERE scene_id = ?1 AND rule = 'editor_polish'
               AND anchor_block_id = ?2 AND message = ?3 AND dismissed = 0",
            params![scene_id.as_str(), block_id, message],
            |r| r.get(0),
        );
        let now = chrono::Utc::now().to_rfc3339();
        if existing.is_ok() {
            return Ok(());
        }
        let snippet: String = block_text.chars().take(60).collect();
        let new_id = Id::new();
        conn.execute(
            "INSERT INTO editor_pill
                (id, scene_id, rule, severity, message, suggestion,
                 anchor_block_id, anchor_start, anchor_end,
                 text_snippet, content_hash, dismissed,
                 created_at, updated_at)
             VALUES (?1, ?2, 'editor_polish', 'observation', ?3, NULL,
                     ?4, 0, ?5, ?6, ?7, 0, ?8, ?8)",
            params![
                new_id.as_str(),
                scene_id.as_str(),
                message,
                block_id,
                block_text.len() as i64,
                snippet,
                compute_block_hash(block_text),
                now,
            ],
        )?;
        Ok(())
    }

    /// Flag a pill as dismissed.
    pub fn dismiss(&self, id: &Id) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        self.db.conn().execute(
            "UPDATE editor_pill SET dismissed = 1, updated_at = ?2 WHERE id = ?1",
            params![id.as_str(), now],
        )?;
        Ok(())
    }

    /// Delete pills whose anchor block is no longer in the scene.
    /// `live_block_ids` is the set the renderer just scanned.
    /// Returns the number of rows removed.
    pub fn cleanup_orphaned_blocks(
        &self,
        scene_id: &Id,
        live_block_ids: &[String],
    ) -> Result<u32> {
        if live_block_ids.is_empty() {
            // Nothing live → orphan-purge would wipe the whole
            // scene's pills. Bail; the writer is mid-rebuild.
            return Ok(0);
        }
        let conn = self.db.conn();
        // Build "?,?,?,..." placeholder string for IN clause.
        let placeholders = vec!["?"; live_block_ids.len()].join(",");
        let sql = format!(
            "DELETE FROM editor_pill
             WHERE scene_id = ? AND anchor_block_id NOT IN ({placeholders})"
        );
        let mut params_vec: Vec<&str> = Vec::with_capacity(live_block_ids.len() + 1);
        params_vec.push(scene_id.as_str());
        for b in live_block_ids {
            params_vec.push(b.as_str());
        }
        let affected = conn.execute(&sql, rusqlite::params_from_iter(params_vec.iter()))?;
        Ok(affected.try_into().unwrap_or(0))
    }
}

/// First 50 chars of the block's text, normalized (lowercase,
/// whitespace collapsed). Shorter than the Phase-3.5 resolver's
/// 80-char hash because editor pills anchor more narrowly — the
/// snippet is the load-bearing piece; hash is the fallback.
#[must_use]
pub fn compute_block_hash(text: &str) -> String {
    text.to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(50)
        .collect()
}

fn row_to_pill(r: &rusqlite::Row<'_>) -> rusqlite::Result<EditorPillRow> {
    Ok(EditorPillRow {
        id: r.get(0)?,
        scene_id: r.get(1)?,
        rule: r.get(2)?,
        severity: r.get(3)?,
        message: r.get(4)?,
        suggestion: r.get(5)?,
        anchor_block_id: r.get(6)?,
        anchor_start: r.get::<_, i64>(7)? as u32,
        anchor_end: r.get::<_, i64>(8)? as u32,
        text_snippet: r.get(9)?,
        content_hash: r.get(10)?,
        dismissed: r.get::<_, i64>(11)? != 0,
        created_at: r.get(12)?,
        updated_at: r.get(13)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prompts::loader::PromptLibrary;
    use std::str::FromStr;

    fn seed_scene(db: &Db) -> Id {
        let scene_id = Id::new();
        let conn = db.conn();
        conn.execute(
            "INSERT INTO project (id, name, created_at, updated_at)
             VALUES ('p1', 'P', '0', '0')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO manuscript (id, project_id, name, ordering, created_at, updated_at)
             VALUES ('m1', 'p1', 'M', 0, '0', '0')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO scene
                (id, manuscript_id, ordering, name, file_path, created_at, updated_at)
             VALUES (?1, 'm1', 0, 'scene', 'manuscript/scenes/s1.md', '0', '0')",
            params![scene_id.as_str()],
        )
        .unwrap();
        scene_id
    }

    fn tone() -> ToneClauses {
        PromptLibrary::load_builtin().unwrap().tone
    }

    #[test]
    fn run_and_upsert_writes_findings_from_scan_blocks() {
        let db = Db::open_in_memory().unwrap();
        let scene = seed_scene(&db);
        let store = EditorPillStore::new(&db);
        let blocks = &[ScanBlock {
            block_id: "^bk-0001",
            text: "She could of stayed. The lamp was finished by midnight.",
        }];
        let live = store.run_and_upsert(&scene, blocks, &tone()).unwrap();
        // Two findings: common_mistake ("could of") + passive_voice
        // ("was finished").
        let rules: Vec<&str> = live.iter().map(|r| r.rule.as_str()).collect();
        assert!(rules.contains(&"common_mistake"));
        assert!(rules.contains(&"passive_voice"));
    }

    #[test]
    fn run_and_upsert_is_idempotent() {
        let db = Db::open_in_memory().unwrap();
        let scene = seed_scene(&db);
        let store = EditorPillStore::new(&db);
        let blocks = &[ScanBlock {
            block_id: "^bk-0001",
            text: "She could of stayed.",
        }];
        let first = store.run_and_upsert(&scene, blocks, &tone()).unwrap();
        let second = store.run_and_upsert(&scene, blocks, &tone()).unwrap();
        assert_eq!(first.len(), second.len(), "re-run must not duplicate rows");
        let first_id = first[0].id.clone();
        let second_id = second[0].id.clone();
        assert_eq!(first_id, second_id, "row id must be stable across re-runs");
    }

    #[test]
    fn dismiss_removes_from_active_set() {
        let db = Db::open_in_memory().unwrap();
        let scene = seed_scene(&db);
        let store = EditorPillStore::new(&db);
        let live = store
            .run_and_upsert(
                &scene,
                &[ScanBlock {
                    block_id: "^bk-0001",
                    text: "She could of stayed.",
                }],
                &tone(),
            )
            .unwrap();
        assert!(!live.is_empty());
        let id = Id::from_str(&live[0].id).unwrap();
        store.dismiss(&id).unwrap();
        let still_live = store.list_active(&scene).unwrap();
        assert!(still_live.iter().all(|p| p.id != id.as_str()));
    }

    #[test]
    fn dismissed_finding_does_not_resurface_on_rerun() {
        let db = Db::open_in_memory().unwrap();
        let scene = seed_scene(&db);
        let store = EditorPillStore::new(&db);
        let blocks = &[ScanBlock {
            block_id: "^bk-0001",
            text: "She could of stayed.",
        }];
        let live = store.run_and_upsert(&scene, blocks, &tone()).unwrap();
        let id = Id::from_str(&live[0].id).unwrap();
        store.dismiss(&id).unwrap();
        // Re-running on the same prose must NOT bring it back.
        let after = store.run_and_upsert(&scene, blocks, &tone()).unwrap();
        assert!(
            after.iter().all(|p| p.id != id.as_str()),
            "dismissed row resurfaced after re-run"
        );
    }

    #[test]
    fn cleanup_orphaned_blocks_drops_rows_for_missing_blocks() {
        let db = Db::open_in_memory().unwrap();
        let scene = seed_scene(&db);
        let store = EditorPillStore::new(&db);
        store
            .run_and_upsert(
                &scene,
                &[ScanBlock {
                    block_id: "^bk-0001",
                    text: "She could of stayed.",
                }],
                &tone(),
            )
            .unwrap();
        let n = store
            .cleanup_orphaned_blocks(&scene, &["^bk-9999".to_string()])
            .unwrap();
        assert!(n >= 1);
        let live = store.list_active(&scene).unwrap();
        assert!(live.is_empty());
    }

    #[test]
    fn cleanup_with_empty_live_set_is_safe_no_op() {
        let db = Db::open_in_memory().unwrap();
        let scene = seed_scene(&db);
        let store = EditorPillStore::new(&db);
        store
            .run_and_upsert(
                &scene,
                &[ScanBlock {
                    block_id: "^bk-0001",
                    text: "She could of stayed.",
                }],
                &tone(),
            )
            .unwrap();
        let n = store.cleanup_orphaned_blocks(&scene, &[]).unwrap();
        assert_eq!(n, 0, "empty live-set must NOT wipe the scene");
    }
}
