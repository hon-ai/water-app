//! Rebuild-from-truth: if `project.db` is missing or stale, regenerate the
//! index by scanning the project folder.

use crate::{chapters::ChaptersFile, scene_md::SceneFile, water_toml::WaterToml, Db, Id, Result};
use chrono::Utc;
use std::path::{Path, PathBuf};

#[derive(Debug, Default)]
pub struct RebuildStats {
    pub projects: usize,
    pub manuscripts: usize,
    pub chapters: usize,
    pub scenes: usize,
    pub characters: usize,
    pub world_entries: usize,
}

/// Rebuild the `SQLite` index from on-disk truth.
pub fn rebuild_from_truth(project_root: &Path) -> Result<(Db, RebuildStats)> {
    let db_path = project_root.join("project.db");
    // Remove any existing DB; we are about to recreate it from truth.
    if db_path.exists() {
        std::fs::remove_file(&db_path)?;
    }
    let mut db = Db::open(&db_path)?;
    let stats = repopulate(&mut db, project_root)?;
    Ok((db, stats))
}

#[allow(clippy::too_many_lines)]
fn repopulate(db: &mut Db, project_root: &Path) -> Result<RebuildStats> {
    let mut stats = RebuildStats::default();
    let water = WaterToml::read(project_root)?;

    // 1. project
    let now = Utc::now().to_rfc3339();
    db.conn().execute(
        "INSERT INTO project (id, name, default_manuscript_id, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        (
            water.project_id.as_str(),
            &water.name,
            water.default_manuscript_id.as_ref().map(Id::as_str),
            &water.created_at,
            &water.updated_at,
        ),
    )?;
    stats.projects = 1;

    // 2. manuscript
    let manuscript_id = water.default_manuscript_id.clone().unwrap_or_default();
    db.conn().execute(
        "INSERT INTO manuscript (id, project_id, name, ordering, created_at, updated_at)
         VALUES (?1, ?2, 'Manuscript', 0, ?3, ?3)",
        (manuscript_id.as_str(), water.project_id.as_str(), &now),
    )?;
    stats.manuscripts = 1;

    // 3. characters (must precede scenes because scene.pov_character_id is a FK)
    let chars_dir = project_root.join("characters");
    if chars_dir.exists() {
        let entries: Vec<PathBuf> = std::fs::read_dir(&chars_dir)?
            .filter_map(|e| e.ok().map(|d| d.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "toml"))
            .collect();
        for path in entries {
            let text = std::fs::read_to_string(&path)?;
            let parsed: toml::Table = toml::from_str(&text)?;
            let id: Id = parsed
                .get("id")
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse().ok())
                .unwrap_or_default();
            let name = parsed
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("Unnamed");
            let schema_version = parsed
                .get("schema_version")
                .and_then(|v| v.as_str())
                .unwrap_or("lsm-v2.1");
            let data_json = serde_json::to_string(&parsed)?;
            let hash = crate::scene::hash_file(&path)?;
            db.conn().execute(
                "INSERT INTO character (id, project_id, name, schema_version, data_json, file_path, file_hash, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8)",
                (
                    id.as_str(),
                    water.project_id.as_str(),
                    name,
                    schema_version,
                    &data_json,
                    path.to_string_lossy(),
                    &hash,
                    &now,
                ),
            )?;
            stats.characters += 1;
        }
    }

    // 4. world segments + entries (segment FK -> project; entry FK -> segment)
    let world_dir = project_root.join("world");
    if world_dir.exists() {
        let entries: Vec<PathBuf> = std::fs::read_dir(&world_dir)?
            .filter_map(|e| e.ok().map(|d| d.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "toml"))
            .collect();
        for path in entries {
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("segment");
            let seg_id = Id::new();
            db.conn().execute(
                "INSERT INTO world_segment (id, project_id, name, ordering, is_collection) VALUES (?1, ?2, ?3, 0, 0)",
                (seg_id.as_str(), water.project_id.as_str(), stem),
            )?;
            let text = std::fs::read_to_string(&path)?;
            let parsed: toml::Table = toml::from_str(&text)?;
            let data_json = serde_json::to_string(&parsed)?;
            let hash = crate::scene::hash_file(&path)?;
            db.conn().execute(
                "INSERT INTO world_entry (id, segment_id, name, data_json, file_path, file_hash) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                (
                    Id::new().as_str(),
                    seg_id.as_str(),
                    stem,
                    &data_json,
                    path.to_string_lossy(),
                    &hash,
                ),
            )?;
            stats.world_entries += 1;
        }
    }

    // 5. chapters (manuscript FK)
    let chapters_path = project_root.join("manuscript").join("chapters.toml");
    let mut chapters_file = if chapters_path.exists() {
        ChaptersFile::read(&chapters_path)?
    } else {
        ChaptersFile::empty()
    };
    chapters_file.chapter.sort_by_key(|c| c.ordering);
    for ch in &chapters_file.chapter {
        db.conn().execute(
            "INSERT INTO chapter (id, manuscript_id, name, ordering, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
            (
                ch.id.as_str(),
                manuscript_id.as_str(),
                &ch.name,
                ch.ordering,
                &now,
            ),
        )?;
        stats.chapters += 1;
    }

    // 6. scenes (last — references manuscript, chapter, character, world_entry)
    let scenes_dir = project_root.join("manuscript").join("scenes");
    if scenes_dir.exists() {
        let mut entries: Vec<PathBuf> = std::fs::read_dir(&scenes_dir)?
            .filter_map(|e| e.ok().map(|d| d.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "md"))
            .collect();
        entries.sort();
        for path in entries {
            let file = SceneFile::read(&path)?;
            let fm = file.frontmatter;
            let hash = crate::scene::hash_file(&path)?;
            db.conn().execute(
                "INSERT INTO scene (id, manuscript_id, chapter_id, ordering, name, pov_character_id,
                                    location_id, scene_goal, status, word_count, file_path,
                                    file_hash, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
                (
                    fm.id.as_str(),
                    manuscript_id.as_str(),
                    fm.chapter_id.as_ref().map(Id::as_str),
                    fm.order,
                    &fm.name,
                    fm.pov_character_id.as_ref().map(Id::as_str),
                    fm.location_id.as_ref().map(Id::as_str),
                    &fm.scene_goal,
                    &fm.status,
                    fm.word_count,
                    path.to_string_lossy(),
                    &hash,
                    &fm.created_at,
                    &fm.updated_at,
                ),
            )?;
            // characters_present rows — characters now exist so a direct INSERT
            // succeeds; FK violations would indicate a malformed frontmatter
            // pointing at a character that simply isn't on disk.
            for cid in &fm.characters_present {
                let _ = db.conn().execute(
                    "INSERT OR IGNORE INTO scene_character_presence (scene_id, character_id) VALUES (?1, ?2)",
                    (fm.id.as_str(), cid.as_str()),
                );
            }
            stats.scenes += 1;
        }
    }

    Ok(stats)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{chapters::Chapter, scene_md::SceneFrontmatter, water_toml::WaterToml};

    fn make_project() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        // Compose the truth files manually so this test exercises *only*
        // the rebuild path, not the writer paths.
        let project_id = Id::new();
        let manuscript_id = Id::new();
        WaterToml {
            schema_version: 1,
            project_id: project_id.clone(),
            name: "TestProj".into(),
            default_manuscript_id: Some(manuscript_id.clone()),
            created_at: "2026-05-16T09:00:00+00:00".into(),
            updated_at: "2026-05-16T09:00:00+00:00".into(),
        }
        .write(dir.path())
        .unwrap();

        std::fs::create_dir_all(dir.path().join("manuscript").join("scenes")).unwrap();
        ChaptersFile {
            schema_version: 1,
            chapter: vec![Chapter {
                id: Id::new(),
                name: "Part One".into(),
                ordering: 0,
                scene_ids: vec![],
            }],
        }
        .write(dir.path().join("manuscript").join("chapters.toml"))
        .unwrap();

        let scene_id = Id::new();
        let scene_path = dir
            .path()
            .join("manuscript")
            .join("scenes")
            .join(format!("{scene_id}.md"));
        crate::scene_md::SceneFile {
            frontmatter: SceneFrontmatter {
                id: scene_id,
                name: "Opening".into(),
                chapter_id: None,
                order: 0,
                pov_character_id: None,
                characters_present: vec![],
                location_id: None,
                scene_goal: None,
                status: "draft".into(),
                created_at: "2026-05-16T09:00:00+00:00".into(),
                updated_at: "2026-05-16T09:00:00+00:00".into(),
                word_count: 2,
            },
            body: "Hello world.\n".into(),
        }
        .write(&scene_path)
        .unwrap();

        dir
    }

    #[test]
    fn rebuild_creates_project_manuscript_chapter_and_scene() {
        let dir = make_project();
        let (db, stats) = rebuild_from_truth(dir.path()).unwrap();
        assert_eq!(stats.projects, 1);
        assert_eq!(stats.manuscripts, 1);
        assert_eq!(stats.chapters, 1);
        assert_eq!(stats.scenes, 1);
        let n: i64 = db
            .conn()
            .query_row("SELECT COUNT(*) FROM scene", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1);
    }

    #[test]
    fn rebuild_is_idempotent_against_pre_existing_db() {
        let dir = make_project();
        let _ = rebuild_from_truth(dir.path()).unwrap();
        // Run again — should drop the old DB and rebuild.
        let (db2, _stats) = rebuild_from_truth(dir.path()).unwrap();
        let n: i64 = db2
            .conn()
            .query_row("SELECT COUNT(*) FROM scene", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1);
    }

    #[test]
    fn rebuild_succeeds_when_scene_references_character() {
        let dir = tempfile::tempdir().unwrap();
        let project_id = Id::new();
        let manuscript_id = Id::new();
        let character_id = Id::new();
        let scene_id = Id::new();

        // 1. water.toml
        WaterToml {
            schema_version: 1,
            project_id: project_id.clone(),
            name: "RefProj".into(),
            default_manuscript_id: Some(manuscript_id.clone()),
            created_at: "2026-05-17T09:00:00+00:00".into(),
            updated_at: "2026-05-17T09:00:00+00:00".into(),
        }
        .write(dir.path())
        .unwrap();

        // 2. characters/<id>.toml — the scene references this
        std::fs::create_dir_all(dir.path().join("characters")).unwrap();
        let char_toml =
            format!("id = \"{character_id}\"\nname = \"Maren\"\nschema_version = \"lsm-v2.1\"\n");
        std::fs::write(
            dir.path()
                .join("characters")
                .join(format!("{character_id}.toml")),
            char_toml,
        )
        .unwrap();

        // 3. manuscript/scenes/<id>.md — pov_character_id points at the character
        std::fs::create_dir_all(dir.path().join("manuscript").join("scenes")).unwrap();
        let scene_path = dir
            .path()
            .join("manuscript")
            .join("scenes")
            .join(format!("{scene_id}.md"));
        crate::scene_md::SceneFile {
            frontmatter: SceneFrontmatter {
                id: scene_id.clone(),
                name: "Opening".into(),
                chapter_id: None,
                order: 0,
                pov_character_id: Some(character_id.clone()),
                characters_present: vec![character_id.clone()],
                location_id: None,
                scene_goal: None,
                status: "draft".into(),
                created_at: "2026-05-17T09:00:00+00:00".into(),
                updated_at: "2026-05-17T09:00:00+00:00".into(),
                word_count: 1,
            },
            body: "Hello.\n".into(),
        }
        .write(&scene_path)
        .unwrap();

        // Rebuild must NOT fail on the scene's pov_character_id FK.
        let (db, stats) = rebuild_from_truth(dir.path()).unwrap();
        assert_eq!(stats.scenes, 1);
        assert_eq!(stats.characters, 1);

        // Verify the FK actually landed.
        let pov: Option<String> = db
            .conn()
            .query_row(
                "SELECT pov_character_id FROM scene WHERE id = ?1",
                [scene_id.as_str()],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(pov.as_deref(), Some(character_id.as_str()));

        // Presence row also landed.
        let presence: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM scene_character_presence WHERE scene_id = ?1 AND character_id = ?2",
                [scene_id.as_str(), character_id.as_str()],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(presence, 1);
    }
}
