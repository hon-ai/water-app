//! External-edit repair: tolerate users editing files outside Water.
//!
//! Writers may edit scene `Markdown` in `Obsidian`, `VS Code`, or any other
//! editor. This pass regenerates missing `^bk-XXXX` markers, refreshes word
//! counts, reconciles `chapters.toml` against `scene.chapter_id` (chapters
//! win), and archives orphaned pinned pills.

use crate::block::ensure_block_ids;
use crate::chapters::ChaptersFile;
use crate::scene_md::SceneFile;
use crate::{Db, Result};
use chrono::Utc;
use std::path::Path;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct RepairReport {
    pub scenes_re_block_idded: usize,
    pub scenes_wordcount_updated: usize,
    pub chapters_reconciled: usize,
    pub pinned_pills_archived: usize,
}

#[allow(clippy::too_many_lines)]
pub fn run(db: &Db, project_root: &Path) -> Result<RepairReport> {
    let mut report = RepairReport::default();

    // 1. Scenes: regenerate missing block ids, refresh word_count + frontmatter.
    let mut stmt = db
        .conn()
        .prepare("SELECT id, file_path, word_count FROM scene")?;
    let rows: Vec<(String, String, i64)> = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    drop(stmt);
    for (id_s, path_s, prev_wc) in rows {
        let path = std::path::PathBuf::from(&path_s);
        if !path.exists() {
            continue;
        }
        let mut file = SceneFile::read(&path)?;
        let (new_body, _blocks) = ensure_block_ids(&file.body);
        let body_changed = new_body != file.body;
        if body_changed {
            file.body = new_body;
            report.scenes_re_block_idded += 1;
        }
        let new_wc = i64::try_from(
            file.body
                .split_whitespace()
                .filter(|w| !w.starts_with("^bk-"))
                .count(),
        )
        .unwrap_or(i64::MAX);
        let wc_changed = new_wc != prev_wc || new_wc != file.frontmatter.word_count;
        if wc_changed {
            file.frontmatter.word_count = new_wc;
            report.scenes_wordcount_updated += 1;
        }
        if body_changed || wc_changed {
            file.frontmatter.updated_at = Utc::now().to_rfc3339();
            file.write(&path)?;
            let hash = crate::scene::hash_file(&path)?;
            db.conn().execute(
                "UPDATE scene SET word_count = ?2, file_hash = ?3, updated_at = ?4 WHERE id = ?1",
                (id_s, new_wc, hash, file.frontmatter.updated_at),
            )?;
        }
    }

    // 2. Chapters: chapters.toml wins over scene.chapter_id where they disagree.
    let chapters_path = project_root.join("manuscript").join("chapters.toml");
    if chapters_path.exists() {
        let chapters = ChaptersFile::read(&chapters_path)?;
        for ch in &chapters.chapter {
            for sid in &ch.scene_ids {
                let n = db.conn().execute(
                    "UPDATE scene SET chapter_id = ?2 WHERE id = ?1 AND (chapter_id IS NULL OR chapter_id != ?2)",
                    (sid.as_str(), ch.id.as_str()),
                )?;
                report.chapters_reconciled += n;
            }
        }
    }

    // 3. Pinned pills with dead block IDs.
    let mut stmt = db.conn().prepare(
        "SELECT p.id, p.scene_id, p.block_id, p.snippet, s.file_path
                 FROM pinned_pill p JOIN scene s ON p.scene_id = s.id",
    )?;
    let rows: Vec<(String, String, String, String, String)> = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;
    drop(stmt);
    for (pid, _sid, bid, snippet, scene_path) in rows {
        let path = std::path::PathBuf::from(&scene_path);
        if !path.exists() {
            continue;
        }
        let file = SceneFile::read(&path)?;
        let block_present = file.body.contains(&format!("^{bid}"));
        let snippet_present = file.body.contains(snippet.as_str());
        if !block_present && !snippet_present {
            // Archive the pin (soft delete by appending a sentinel; v1 uses
            // a separate archived flag — we add one via inline UPDATE).
            db.conn().execute(
                "UPDATE pinned_pill SET rabbit_hole_path = COALESCE(rabbit_hole_path, '') || '|archived' WHERE id = ?1",
                [pid.as_str()],
            )?;
            report.pinned_pills_archived += 1;
        }
    }

    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{rebuild_from_truth, water_toml::WaterToml, Id};

    fn make_project() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let pid = Id::new();
        let mid = Id::new();
        WaterToml {
            schema_version: 1,
            project_id: pid.clone(),
            name: "P".into(),
            default_manuscript_id: Some(mid),
            created_at: "2026-05-16T09:00:00+00:00".into(),
            updated_at: "2026-05-16T09:00:00+00:00".into(),
        }
        .write(dir.path())
        .unwrap();
        std::fs::create_dir_all(dir.path().join("manuscript").join("scenes")).unwrap();

        let scene_id = Id::new();
        let scene_path = dir
            .path()
            .join("manuscript")
            .join("scenes")
            .join(format!("{scene_id}.md"));
        let file = SceneFile {
            frontmatter: crate::scene_md::SceneFrontmatter {
                water_scene: true,
                id: scene_id.clone(),
                name: "S".into(),
                chapter_id: None,
                order: 0,
                pov_character_id: None,
                characters_present: vec![],
                location_id: None,
                scene_goal: None,
                status: "draft".into(),
                created_at: "2026-05-16T09:00:00+00:00".into(),
                updated_at: "2026-05-16T09:00:00+00:00".into(),
                word_count: 0,
                canvas_x: None,
                canvas_y: None,
                canvas_group: None, // intentionally wrong; repair fixes it
            },
            // No block ids; repair will add them.
            body: "First.\n\nSecond.\n".into(),
        };
        file.write(&scene_path).unwrap();
        dir
    }

    #[test]
    fn repair_adds_block_ids_and_fixes_word_count() {
        let dir = make_project();
        let (db, _) = rebuild_from_truth(dir.path()).unwrap();
        let report = run(&db, dir.path()).unwrap();
        assert!(report.scenes_re_block_idded >= 1);
        assert!(report.scenes_wordcount_updated >= 1);
        let scene_file = std::fs::read_to_string(
            std::fs::read_dir(dir.path().join("manuscript").join("scenes"))
                .unwrap()
                .find_map(std::result::Result::ok)
                .unwrap()
                .path(),
        )
        .unwrap();
        // Leading-token convention: each paragraph starts with
        // `^bk-XXXX` followed by a space and the prose.
        assert!(scene_file.contains(" First."), "got: {scene_file}");
        assert!(scene_file.contains(" Second."), "got: {scene_file}");
        assert!(!scene_file.contains("First. ^bk-"));
        assert!(!scene_file.contains("Second. ^bk-"));
    }
}
