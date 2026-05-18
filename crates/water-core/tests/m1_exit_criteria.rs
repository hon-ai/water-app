//! M1 exit-criteria integration tests.
//!
//! These exercise the public API of water-core to assert the behaviours
//! the milestone gate requires.

use std::sync::Arc;
use water_core::llm::{BouquetRequest, CannedProvider, LlmRouter};
use water_core::{
    chapters::ChaptersFile, rebuild_from_truth, water_toml::WaterToml, Db, Id, ManuscriptStore,
    NewScene, ProjectStore, SceneStore,
};

fn scaffold(root: &std::path::Path, name: &str) -> Id {
    std::fs::create_dir_all(root.join("manuscript").join("scenes")).unwrap();
    std::fs::create_dir_all(root.join("characters")).unwrap();
    std::fs::create_dir_all(root.join("world")).unwrap();
    std::fs::create_dir_all(root.join("snapshots")).unwrap();
    let db = Db::open(root.join("project.db")).unwrap();
    let p = ProjectStore::new(&db).insert(name).unwrap();
    let m = ManuscriptStore::new(&db)
        .insert(&p.id, "Manuscript", 0)
        .unwrap();
    ProjectStore::new(&db)
        .set_default_manuscript(&p.id, &m.id)
        .unwrap();
    WaterToml {
        schema_version: 1,
        project_id: p.id.clone(),
        name: name.into(),
        default_manuscript_id: Some(m.id.clone()),
        created_at: chrono::Utc::now().to_rfc3339(),
        updated_at: chrono::Utc::now().to_rfc3339(),
    }
    .write(root)
    .unwrap();
    ChaptersFile::empty()
        .write(root.join("manuscript").join("chapters.toml"))
        .unwrap();
    m.id
}

#[test]
fn exit_create_type_close_reopen_persists() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let manuscript_id = scaffold(root, "TestProj");

    // Open #1: write into a scene.
    {
        let db = Db::open(root.join("project.db")).unwrap();
        let store = SceneStore::new(&db, root.to_path_buf());
        let scene = store
            .create(NewScene {
                manuscript_id: manuscript_id.clone(),
                chapter_id: None,
                name: "S1".into(),
                ordering: 0,
            })
            .unwrap();
        store
            .write_body(&scene.id, "Maren watched the harbour lanterns.")
            .unwrap();
    }

    // Open #2: read it back.
    {
        let db = Db::open(root.join("project.db")).unwrap();
        let store = SceneStore::new(&db, root.to_path_buf());
        let scenes = store.list(&manuscript_id).unwrap();
        assert_eq!(scenes.len(), 1);
        let file = store.read(&scenes[0].id).unwrap();
        assert!(file.body.contains("Maren watched the harbour lanterns."));
    }
}

#[test]
fn exit_rebuild_from_truth_when_db_deleted() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let manuscript_id = scaffold(root, "TestProj");
    // Author a scene.
    {
        let db = Db::open(root.join("project.db")).unwrap();
        let store = SceneStore::new(&db, root.to_path_buf());
        let scene = store
            .create(NewScene {
                manuscript_id: manuscript_id.clone(),
                chapter_id: None,
                name: "Opening".into(),
                ordering: 0,
            })
            .unwrap();
        store.write_body(&scene.id, "First.").unwrap();
    }
    // Delete the DB.
    std::fs::remove_file(root.join("project.db")).unwrap();

    // Rebuild.
    let (db, stats) = rebuild_from_truth(root).unwrap();
    assert_eq!(stats.scenes, 1);
    let count: i64 = db
        .conn()
        .query_row("SELECT COUNT(*) FROM scene", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn exit_provider_test_canned_round_trip_returns_three_variants() {
    let provider = Arc::new(CannedProvider);
    let router = LlmRouter::new(vec![provider]);
    let req = BouquetRequest {
        system: "tone".into(),
        user: "test".into(),
        n_variants: 3,
        previous_variants_first_words: vec![],
        model: "canned".into(),
        temperature: 0.7,
        max_output_tokens: 100,
    };
    let (id, variants) = router.generate_bouquet(&req).await.unwrap();
    assert_eq!(id.as_str(), "canned");
    assert_eq!(variants.len(), 3);
    for v in &variants {
        assert!(!v.text.is_empty());
    }
}

#[test]
fn exit_snapshot_hourly_entries_and_restore_works() {
    use water_core::{SnapshotStore, SnapshotTrigger};
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let manuscript_id = scaffold(root, "TestProj");

    let db = Db::open(root.join("project.db")).unwrap();
    let scene_store = SceneStore::new(&db, root.to_path_buf());
    let scene = scene_store
        .create(NewScene {
            manuscript_id,
            chapter_id: None,
            name: "S".into(),
            ordering: 0,
        })
        .unwrap();
    scene_store.write_body(&scene.id, "first").unwrap();
    let scene_path = root
        .join("manuscript")
        .join("scenes")
        .join(format!("{}.md", scene.id));

    let snap_store = SnapshotStore::new(&db, root.to_path_buf());
    let s1 = snap_store
        .take(&scene.id, &scene_path, SnapshotTrigger::Hourly)
        .unwrap();
    scene_store.write_body(&scene.id, "second").unwrap();
    snap_store
        .take(&scene.id, &scene_path, SnapshotTrigger::Manual)
        .unwrap();

    let list = snap_store.list(&scene.id).unwrap();
    assert!(list.iter().any(|r| r.trigger == SnapshotTrigger::Hourly));
    assert!(list.len() >= 2);

    // Restore to first state.
    snap_store.restore(&scene.id, &s1.id, &scene_path).unwrap();
    let body = scene_store.read(&scene.id).unwrap().body;
    assert!(body.contains("first"));
    assert!(!body.contains("second"));
    // Pre-restore snapshot added.
    let list2 = snap_store.list(&scene.id).unwrap();
    assert!(list2
        .iter()
        .any(|r| r.trigger == SnapshotTrigger::PreRestore));
}

#[tokio::test]
#[ignore = "requires uv and the sidecar workspace; run with --ignored"]
async fn exit_sidecar_boots_under_8s() {
    use std::time::Duration;
    use water_core::{Sidecar, SidecarSpec};
    let uv = which::which("uv").expect("uv not found on PATH");
    let workspace = std::path::PathBuf::from("../../sidecar");
    let port = 18766;
    let start = std::time::Instant::now();
    let sc = Sidecar::spawn(SidecarSpec {
        working_dir: workspace,
        uv_bin: uv,
        port,
        host: "127.0.0.1".into(),
        boot_timeout: Duration::from_secs(12),
    })
    .await
    .unwrap();
    let elapsed = start.elapsed();
    assert!(
        elapsed < Duration::from_secs(8),
        "sidecar boot took {elapsed:?}"
    );
    sc.shutdown().await.unwrap();
}

#[test]
fn exit_rebuild_with_character_reference_round_trips() {
    use water_core::scene_md::SceneFrontmatter;
    let dir = tempfile::tempdir().unwrap();
    let project_id = Id::new();
    let manuscript_id = Id::new();
    let character_id = Id::new();
    let scene_id = Id::new();

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

    std::fs::create_dir_all(dir.path().join("manuscript").join("scenes")).unwrap();
    let scene_path = dir
        .path()
        .join("manuscript")
        .join("scenes")
        .join(format!("{scene_id}.md"));
    water_core::scene_md::SceneFile {
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

    let (db, stats) = rebuild_from_truth(dir.path()).unwrap();
    assert_eq!(stats.characters, 1);
    assert_eq!(stats.scenes, 1);

    let pov: Option<String> = db
        .conn()
        .query_row(
            "SELECT pov_character_id FROM scene WHERE id = ?1",
            [scene_id.as_str()],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(pov.as_deref(), Some(character_id.as_str()));
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn exit_snapshot_scheduler_on_close_fires_for_registered_scenes() {
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::Mutex;
    use water_core::{
        ActiveScene, ManuscriptStore, NewScene, ProjectStore, SceneStore, SnapshotScheduler,
    };

    let dir = tempfile::tempdir().unwrap();
    let db = Db::open_in_memory().unwrap();
    let p = ProjectStore::new(&db).insert("P").unwrap();
    let m = ManuscriptStore::new(&db).insert(&p.id, "M", 0).unwrap();
    let ss = SceneStore::new(&db, dir.path().to_path_buf());
    let scene = ss
        .create(NewScene {
            manuscript_id: m.id.clone(),
            chapter_id: None,
            name: "S".into(),
            ordering: 0,
        })
        .unwrap();
    ss.write_body(&scene.id, "hello world").unwrap();
    let scene_path = dir
        .path()
        .join("manuscript")
        .join("scenes")
        .join(format!("{}.md", scene.id));

    let db_arc = Arc::new(Mutex::new(db));
    let scheduler = SnapshotScheduler::spawn(db_arc.clone(), dir.path().to_path_buf());
    scheduler
        .register(ActiveScene {
            scene_id: scene.id.clone(),
            file_path: scene_path,
        })
        .await;

    // Fire the OnClose snapshot path; give the spawned task a virtual moment.
    scheduler.on_close().await.unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;

    let count: i64 = {
        let db_guard = db_arc.lock().await;
        db_guard
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM snapshot WHERE scene_id = ?1 AND trigger = 'on-close'",
                [scene.id.as_str()],
                |r| r.get(0),
            )
            .unwrap()
    };
    assert!(
        count >= 1,
        "expected at least one OnClose snapshot row, got {count}"
    );
    scheduler.stop().await.unwrap();
}
