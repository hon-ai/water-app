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
