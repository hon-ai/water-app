//! Tests for the world submodule. Lives at the parent module level
//! (rather than inside `store.rs`) so `store.rs` stays under ~520 lines
//! ahead of Task 5's collection-CRUD additions.

use super::store::WorldStore;
use crate::{Db, Id, ProjectStore};

/// Helper: spin up an in-memory DB + tempdir project root + `WorldStore`
/// with built-ins seeded, and return the resolved `locations` segment
/// (the only built-in collection). Used by all entry-CRUD tests.
#[cfg(test)]
fn loc_setup() -> (
    tempfile::TempDir,
    Db,
    crate::Project,
    crate::world::WorldSegmentRow,
) {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open_in_memory().unwrap();
    let p = ProjectStore::new(&db).insert("P").unwrap();
    let store = WorldStore::new(&db, dir.path().to_path_buf());
    store.seed_builtins(&p.id).unwrap();
    let seg = store
        .find_segment_by_slug(&p.id, "locations")
        .unwrap()
        .unwrap();
    (dir, db, p, seg)
}

    #[test]
    fn upsert_and_list_segments() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        let p = ProjectStore::new(&db).insert("P").unwrap();
        let store = WorldStore::new(&db, dir.path().to_path_buf());
        store
            .upsert_segment(&p.id, "concept", "Concept", 0, false)
            .unwrap();
        store
            .upsert_segment(&p.id, "locations", "Locations", 1, true)
            .unwrap();
        let list = store.list_segments(&p.id).unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].name, "Concept");
        assert!(!list[0].is_collection);
        assert!(list[1].is_collection);
    }

    #[test]
    fn upsert_segment_with_ulid_slug_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        let p = ProjectStore::new(&db).insert("P").unwrap();
        let store = WorldStore::new(&db, dir.path().to_path_buf());
        // Use a fresh ULID as the slug so the ON CONFLICT(id) path fires.
        let ulid_slug = Id::new();
        let id1 = store
            .upsert_segment(&p.id, ulid_slug.as_str(), "First", 0, false)
            .unwrap();
        let id2 = store
            .upsert_segment(&p.id, ulid_slug.as_str(), "Renamed", 5, true)
            .unwrap();
        assert_eq!(id1, id2, "same ULID slug must yield same id");
        let list = store.list_segments(&p.id).unwrap();
        assert_eq!(list.len(), 1, "second upsert must update, not insert");
        assert_eq!(list[0].name, "Renamed");
        assert_eq!(list[0].ordering, 5);
        assert!(list[0].is_collection);
    }

    #[test]
    fn seed_builtins_inserts_six_segments_idempotently() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        let p = ProjectStore::new(&db).insert("P").unwrap();
        let store = WorldStore::new(&db, dir.path().to_path_buf());

        store.seed_builtins(&p.id).unwrap();
        let segs = store.list_segments(&p.id).unwrap();
        assert_eq!(segs.len(), 6, "expected 6 built-in segments; got {}", segs.len());

        // Second call must be idempotent.
        store.seed_builtins(&p.id).unwrap();
        let segs2 = store.list_segments(&p.id).unwrap();
        assert_eq!(segs2.len(), 6);
    }

    #[test]
    fn seed_builtins_assigns_unique_hue_tokens_round_robin() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        let p = ProjectStore::new(&db).insert("P").unwrap();
        let store = WorldStore::new(&db, dir.path().to_path_buf());
        store.seed_builtins(&p.id).unwrap();

        let hues: Vec<String> = db
            .conn()
            .prepare("SELECT hue_token FROM world_segment WHERE project_id = ?1 ORDER BY ordering")
            .unwrap()
            .query_map([p.id.as_str()], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<std::result::Result<_, _>>()
            .unwrap();

        assert_eq!(
            hues,
            vec![
                "--water-hue-world-1",
                "--water-hue-world-2",
                "--water-hue-world-3",
                "--water-hue-world-4",
                "--water-hue-world-5",
                "--water-hue-world-6",
            ]
        );
    }

    #[test]
    fn seed_builtins_sets_correct_slugs_and_is_collection_flags() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        let p = ProjectStore::new(&db).insert("P").unwrap();
        let store = WorldStore::new(&db, dir.path().to_path_buf());
        store.seed_builtins(&p.id).unwrap();

        let mut stmt = db
            .conn()
            .prepare("SELECT slug, is_collection FROM world_segment WHERE project_id = ?1 ORDER BY ordering")
            .unwrap();
        let rows: Vec<(String, bool)> = stmt
            .query_map([p.id.as_str()], |row| {
                let s: String = row.get(0)?;
                let c: i64 = row.get(1)?;
                Ok((s, c != 0))
            })
            .unwrap()
            .collect::<std::result::Result<_, _>>()
            .unwrap();

        assert_eq!(
            rows,
            vec![
                ("concept".to_string(), false),
                ("locations".to_string(), true),
                ("politics_and_social".to_string(), false),
                ("culture".to_string(), false),
                ("world".to_string(), false),
                ("history".to_string(), false),
            ]
        );
    }

    #[test]
    fn find_segment_by_slug_returns_some_for_builtin() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        let p = ProjectStore::new(&db).insert("P").unwrap();
        let store = WorldStore::new(&db, dir.path().to_path_buf());
        store.seed_builtins(&p.id).unwrap();

        let found = store.find_segment_by_slug(&p.id, "locations").unwrap();
        assert!(found.is_some());
        let s = found.unwrap();
        assert!(s.is_collection);
    }

    #[test]
    fn find_segment_by_slug_returns_none_for_unknown() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        let p = ProjectStore::new(&db).insert("P").unwrap();
        let store = WorldStore::new(&db, dir.path().to_path_buf());
        store.seed_builtins(&p.id).unwrap();

        assert!(store.find_segment_by_slug(&p.id, "nonexistent").unwrap().is_none());
    }

    #[test]
    fn create_user_segment_persists_template_json() {
        use crate::world::templates::{WorldTemplateField, WorldTemplateFieldKind, WorldTemplateSchema};
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        let p = ProjectStore::new(&db).insert("P").unwrap();
        let store = WorldStore::new(&db, dir.path().to_path_buf());

        let custom = WorldTemplateSchema {
            id: "magic_systems".to_string(),
            label: "Magic Systems".to_string(),
            fields: vec![WorldTemplateField {
                id: "main.name".to_string(),
                label: "System Name".to_string(),
                prompt_question: "What's this system called?".to_string(),
                kind: WorldTemplateFieldKind::ShortText,
                optional_skip: false,
            }],
        };
        let id = store
            .create_user_segment(&p.id, "Magic Systems", true, &custom)
            .unwrap();

        let json: String = db
            .conn()
            .query_row(
                "SELECT template_json FROM world_segment WHERE id = ?1",
                [id.as_str()],
                |r| r.get(0),
            )
            .unwrap();
        let parsed: WorldTemplateSchema = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.label, "Magic Systems");
    }

    #[test]
    fn set_segment_hidden_toggles_flag() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        let p = ProjectStore::new(&db).insert("P").unwrap();
        let store = WorldStore::new(&db, dir.path().to_path_buf());
        store.seed_builtins(&p.id).unwrap();
        let s = store.find_segment_by_slug(&p.id, "history").unwrap().unwrap();
        store.set_segment_hidden(&s.id, true).unwrap();
        let again = store.find_segment_by_slug(&p.id, "history").unwrap().unwrap();
        assert!(again.hidden);
        store.set_segment_hidden(&s.id, false).unwrap();
        let third = store.find_segment_by_slug(&p.id, "history").unwrap().unwrap();
        assert!(!third.hidden);
    }

    #[test]
    fn delete_user_segment_refuses_builtin() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        let p = ProjectStore::new(&db).insert("P").unwrap();
        let store = WorldStore::new(&db, dir.path().to_path_buf());
        store.seed_builtins(&p.id).unwrap();
        let s = store.find_segment_by_slug(&p.id, "concept").unwrap().unwrap();
        let err = store.delete_user_segment(&s.id).unwrap_err();
        assert!(err.to_string().contains("built-in"));
    }

    #[test]
    fn delete_user_segment_removes_user_added() {
        use crate::world::templates::{WorldTemplateField, WorldTemplateFieldKind, WorldTemplateSchema};
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        let p = ProjectStore::new(&db).insert("P").unwrap();
        let store = WorldStore::new(&db, dir.path().to_path_buf());
        let custom = WorldTemplateSchema {
            id: "test".to_string(),
            label: "Test".to_string(),
            fields: vec![WorldTemplateField {
                id: "main.thing".to_string(),
                label: "Thing".to_string(),
                prompt_question: "?".to_string(),
                kind: WorldTemplateFieldKind::ShortText,
                optional_skip: false,
            }],
        };
        let id = store.create_user_segment(&p.id, "Test", false, &custom).unwrap();
        store.delete_user_segment(&id).unwrap();
        let segs = store.list_segments(&p.id).unwrap();
        assert!(segs.iter().all(|s| s.id != id));
    }

    #[test]
    fn read_segment_returns_not_found_for_unknown_id() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        let _p = ProjectStore::new(&db).insert("P").unwrap();
        let store = WorldStore::new(&db, dir.path().to_path_buf());
        let unknown = Id::new();
        let err = store.read_segment(&unknown).unwrap_err();
        assert!(
            matches!(err, crate::Error::NotFound(_)),
            "expected NotFound, got {err:?}"
        );
    }

    #[test]
    fn delete_user_segment_returns_not_found_for_unknown_id() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        let _p = ProjectStore::new(&db).insert("P").unwrap();
        let store = WorldStore::new(&db, dir.path().to_path_buf());
        let unknown = Id::new();
        let err = store.delete_user_segment(&unknown).unwrap_err();
        assert!(
            matches!(err, crate::Error::NotFound(_)),
            "expected NotFound, got {err:?}"
        );
    }

    #[test]
    fn read_single_doc_returns_empty_data_for_freshly_seeded_segment() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        let p = ProjectStore::new(&db).insert("P").unwrap();
        let store = WorldStore::new(&db, dir.path().to_path_buf());
        store.seed_builtins(&p.id).unwrap();
        let seg = store.find_segment_by_slug(&p.id, "concept").unwrap().unwrap();

        let file = store.read_single_doc(&seg.id).unwrap();
        assert_eq!(file.name, "Concept");
        // Pre-edit, no [main] or [lists] sections yet.
        assert!(file.data.get("main").is_none_or(|v| {
            v.as_object().is_none_or(serde_json::Map::is_empty)
        }));
    }

    #[test]
    fn update_single_doc_field_persists_to_disk_and_db() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        let p = ProjectStore::new(&db).insert("P").unwrap();
        let store = WorldStore::new(&db, dir.path().to_path_buf());
        store.seed_builtins(&p.id).unwrap();
        let seg = store.find_segment_by_slug(&p.id, "concept").unwrap().unwrap();

        store
            .update_single_doc_field(
                &seg.id,
                "main.core_premise",
                &serde_json::Value::String("A library that remembers".to_string()),
            )
            .unwrap();

        // Re-read from disk via store.
        let file = store.read_single_doc(&seg.id).unwrap();
        let main = file.data.get("main").unwrap().as_object().unwrap();
        assert_eq!(
            main.get("core_premise").unwrap().as_str().unwrap(),
            "A library that remembers"
        );

        // Confirm a TOML file actually landed on disk at world/concept.toml.
        let path = dir.path().join("world").join("concept.toml");
        assert!(path.exists(), "world/concept.toml should exist");
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(
            text.contains("A library that remembers"),
            "TOML body should contain the value"
        );
    }

    #[test]
    fn update_single_doc_field_supports_string_list_kind() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        let p = ProjectStore::new(&db).insert("P").unwrap();
        let store = WorldStore::new(&db, dir.path().to_path_buf());
        store.seed_builtins(&p.id).unwrap();
        let seg = store.find_segment_by_slug(&p.id, "concept").unwrap().unwrap();

        let v = serde_json::json!(["memory", "loss", "obligation"]);
        store
            .update_single_doc_field(&seg.id, "lists.themes", &v)
            .unwrap();

        let file = store.read_single_doc(&seg.id).unwrap();
        let lists = file.data.get("lists").unwrap().as_object().unwrap();
        let arr = lists.get("themes").unwrap().as_array().unwrap();
        assert_eq!(arr.len(), 3);
        assert_eq!(arr[0].as_str().unwrap(), "memory");
    }

    #[test]
    fn update_single_doc_field_updates_file_hash_in_db() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        let p = ProjectStore::new(&db).insert("P").unwrap();
        let store = WorldStore::new(&db, dir.path().to_path_buf());
        store.seed_builtins(&p.id).unwrap();
        let seg = store.find_segment_by_slug(&p.id, "concept").unwrap().unwrap();

        store
            .update_single_doc_field(
                &seg.id,
                "main.genre",
                &serde_json::Value::String("literary".to_string()),
            )
            .unwrap();

        // The single-doc row lives in world_entry with segment_id = seg.id.
        let hash: String = db
            .conn()
            .query_row(
                "SELECT file_hash FROM world_entry WHERE segment_id = ?1",
                [seg.id.as_str()],
                |r| r.get(0),
            )
            .unwrap();
        assert!(!hash.is_empty(), "file_hash should be populated");
    }

    #[test]
    fn read_single_doc_errors_on_collection_segment() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        let p = ProjectStore::new(&db).insert("P").unwrap();
        let store = WorldStore::new(&db, dir.path().to_path_buf());
        store.seed_builtins(&p.id).unwrap();
        let seg = store.find_segment_by_slug(&p.id, "locations").unwrap().unwrap();
        let err = store.read_single_doc(&seg.id).unwrap_err();
        assert!(
            err.to_string().contains("collection"),
            "expected 'collection' in error; got {err}"
        );
    }

    #[test]
    fn update_single_doc_field_rejects_invalid_field_ids() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        let p = ProjectStore::new(&db).insert("P").unwrap();
        let store = WorldStore::new(&db, dir.path().to_path_buf());
        store.seed_builtins(&p.id).unwrap();
        let seg = store.find_segment_by_slug(&p.id, "concept").unwrap().unwrap();
        for bad in ["main", "main.", ".leaf", "main.a.b", ""] {
            let err = store
                .update_single_doc_field(&seg.id, bad, &serde_json::json!("x"))
                .unwrap_err();
            assert!(
                err.to_string().contains("field_id"),
                "field_id '{bad}': expected error, got {err}"
            );
        }
    }

    #[test]
    fn update_single_doc_field_hash_changes_on_edit() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        let p = ProjectStore::new(&db).insert("P").unwrap();
        let store = WorldStore::new(&db, dir.path().to_path_buf());
        store.seed_builtins(&p.id).unwrap();
        let seg = store.find_segment_by_slug(&p.id, "concept").unwrap().unwrap();

        store
            .update_single_doc_field(&seg.id, "main.genre", &serde_json::json!("literary"))
            .unwrap();
        let hash1: String = db
            .conn()
            .query_row(
                "SELECT file_hash FROM world_entry WHERE segment_id = ?1",
                [seg.id.as_str()],
                |r| r.get(0),
            )
            .unwrap();

        store
            .update_single_doc_field(&seg.id, "main.genre", &serde_json::json!("speculative"))
            .unwrap();
        let hash2: String = db
            .conn()
            .query_row(
                "SELECT file_hash FROM world_entry WHERE segment_id = ?1",
                [seg.id.as_str()],
                |r| r.get(0),
            )
            .unwrap();

        assert_ne!(hash1, hash2, "file_hash should change after data update");
        assert!(!hash2.is_empty());
    }

    // ----- Task 5: collection-entry CRUD -----

    #[test]
    fn create_entry_inserts_row_and_returns_id() {
        let (dir, db, _p, seg) = loc_setup();
        let store = WorldStore::new(&db, dir.path().to_path_buf());
        let id = store.create_entry(&seg.id, "Old Library").unwrap();

        let entries = store.list_entries(&seg.id).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].id, id);
        assert_eq!(entries[0].name, "Old Library");
    }

    #[test]
    fn create_entry_with_empty_name_is_allowed_for_chorus_stub() {
        // Task 29's pin-handler calls `create_entry_seeded(&loc.id, "", ...)`,
        // so the empty-name path MUST succeed.
        let (dir, db, _p, seg) = loc_setup();
        let store = WorldStore::new(&db, dir.path().to_path_buf());
        let id = store.create_entry(&seg.id, "").unwrap();
        let file = store.read_entry(&id).unwrap();
        assert_eq!(file.name, "");
    }

    #[test]
    fn create_entry_refuses_non_collection_segment() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        let p = ProjectStore::new(&db).insert("P").unwrap();
        let store = WorldStore::new(&db, dir.path().to_path_buf());
        store.seed_builtins(&p.id).unwrap();
        let concept = store.find_segment_by_slug(&p.id, "concept").unwrap().unwrap();
        let err = store.create_entry(&concept.id, "Nope").unwrap_err();
        assert!(
            err.to_string().contains("not a collection"),
            "expected 'not a collection' in error; got {err}"
        );
    }

    #[test]
    fn create_entry_seeded_writes_initial_field_value() {
        let (dir, db, _p, seg) = loc_setup();
        let store = WorldStore::new(&db, dir.path().to_path_buf());
        let id = store
            .create_entry_seeded(&seg.id, "Atrium", "main.sensory_detail", "cold marble")
            .unwrap();

        let file = store.read_entry(&id).unwrap();
        let main = file.data.get("main").unwrap().as_object().unwrap();
        assert_eq!(
            main.get("sensory_detail").unwrap().as_str().unwrap(),
            "cold marble"
        );

        // Disk file must exist at world/locations/<id>.toml.
        let on_disk = dir
            .path()
            .join("world")
            .join("locations")
            .join(format!("{}.toml", id.as_str()));
        assert!(on_disk.exists(), "expected {} on disk", on_disk.display());
    }

    #[test]
    fn update_entry_field_persists_to_disk_with_correct_path() {
        let (dir, db, _p, seg) = loc_setup();
        let store = WorldStore::new(&db, dir.path().to_path_buf());
        let id = store.create_entry(&seg.id, "Crypt").unwrap();

        store
            .update_entry_field(
                &id,
                "main.sensory_detail",
                &serde_json::Value::String("dust and copper".to_string()),
            )
            .unwrap();

        let on_disk = dir
            .path()
            .join("world")
            .join("locations")
            .join(format!("{}.toml", id.as_str()));
        assert!(on_disk.exists(), "TOML file should exist at {}", on_disk.display());
        let body = std::fs::read_to_string(&on_disk).unwrap();
        assert!(body.contains("dust and copper"));
    }

    #[test]
    fn update_entry_field_rename_cascade_guard_rejects_non_string_main_name() {
        let (dir, db, _p, seg) = loc_setup();
        let store = WorldStore::new(&db, dir.path().to_path_buf());
        let id = store.create_entry(&seg.id, "Original").unwrap();

        let err = store
            .update_entry_field(&id, "main.name", &serde_json::json!(42))
            .unwrap_err();
        assert!(
            err.to_string().contains("main.name"),
            "expected 'main.name' guard error; got {err}"
        );

        // DB row name must be untouched.
        let still: String = db
            .conn()
            .query_row(
                "SELECT name FROM world_entry WHERE id = ?1",
                [id.as_str()],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(still, "Original");
    }

    #[test]
    fn update_entry_field_main_name_renames_world_entry_row() {
        let (dir, db, _p, seg) = loc_setup();
        let store = WorldStore::new(&db, dir.path().to_path_buf());
        let id = store.create_entry(&seg.id, "Old Name").unwrap();

        store
            .update_entry_field(
                &id,
                "main.name",
                &serde_json::Value::String("New Name".to_string()),
            )
            .unwrap();

        let row_name: String = db
            .conn()
            .query_row(
                "SELECT name FROM world_entry WHERE id = ?1",
                [id.as_str()],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(row_name, "New Name", "main.name should rename the row");

        // read_entry must also reflect the cascade.
        let file = store.read_entry(&id).unwrap();
        assert_eq!(file.name, "New Name");
    }

    #[test]
    fn update_entry_aliases_persists_to_db_and_disk() {
        let (dir, db, _p, seg) = loc_setup();
        let store = WorldStore::new(&db, dir.path().to_path_buf());
        let id = store.create_entry(&seg.id, "The Spire").unwrap();

        store
            .update_entry_aliases(&id, &["Spire".to_string(), "Tower".to_string()])
            .unwrap();

        // DB column.
        let aliases_json: String = db
            .conn()
            .query_row(
                "SELECT aliases_json FROM world_entry WHERE id = ?1",
                [id.as_str()],
                |r| r.get(0),
            )
            .unwrap();
        let parsed: Vec<String> = serde_json::from_str(&aliases_json).unwrap();
        assert_eq!(parsed, vec!["Spire", "Tower"]);

        // read_entry round-trip.
        let file = store.read_entry(&id).unwrap();
        assert_eq!(file.aliases, vec!["Spire", "Tower"]);

        // On-disk TOML body should mention the aliases too.
        let on_disk = dir
            .path()
            .join("world")
            .join("locations")
            .join(format!("{}.toml", id.as_str()));
        let body = std::fs::read_to_string(&on_disk).unwrap();
        assert!(body.contains("Spire"));
        assert!(body.contains("Tower"));
    }

    #[test]
    fn delete_entry_removes_row_and_file() {
        let (dir, db, _p, seg) = loc_setup();
        let store = WorldStore::new(&db, dir.path().to_path_buf());
        let id = store
            .create_entry_seeded(&seg.id, "Mausoleum", "main.sensory_detail", "shadows")
            .unwrap();

        let on_disk = dir
            .path()
            .join("world")
            .join("locations")
            .join(format!("{}.toml", id.as_str()));
        assert!(on_disk.exists(), "precondition: file written");

        store.delete_entry(&id).unwrap();

        assert!(!on_disk.exists(), "file should be removed");
        let count: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM world_entry WHERE id = ?1",
                [id.as_str()],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 0, "DB row should be removed");
    }

    #[test]
    fn delete_entry_if_empty_returns_true_for_blank_entry() {
        let (dir, db, _p, seg) = loc_setup();
        let store = WorldStore::new(&db, dir.path().to_path_buf());
        // Empty name, no aliases, no data — Chorus-style draft stub.
        let id = store.create_entry(&seg.id, "").unwrap();
        let reaped = store.delete_entry_if_empty(&id).unwrap();
        assert!(reaped, "blank entry should be reaped");
        let count: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM world_entry WHERE id = ?1",
                [id.as_str()],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn delete_entry_if_empty_returns_false_for_populated_entry() {
        let (dir, db, _p, seg) = loc_setup();
        let store = WorldStore::new(&db, dir.path().to_path_buf());
        let id = store
            .create_entry_seeded(&seg.id, "", "main.sensory_detail", "wind in pines")
            .unwrap();
        let reaped = store.delete_entry_if_empty(&id).unwrap();
        assert!(!reaped, "populated entry must NOT be reaped");
        // And the row is still there.
        let still = store.read_entry(&id).unwrap();
        assert_eq!(
            still
                .data
                .get("main")
                .unwrap()
                .as_object()
                .unwrap()
                .get("sensory_detail")
                .unwrap()
                .as_str()
                .unwrap(),
            "wind in pines"
        );
    }

    #[test]
    fn read_entry_returns_not_found_for_unknown_id() {
        let (dir, db, _p, _seg) = loc_setup();
        let store = WorldStore::new(&db, dir.path().to_path_buf());
        let unknown = Id::new();
        let err = store.read_entry(&unknown).unwrap_err();
        assert!(
            matches!(err, crate::Error::NotFound(_)),
            "expected NotFound, got {err:?}"
        );
    }

    #[test]
    fn create_entry_seeded_rolls_back_on_invalid_seed_field_id() {
        let (dir, db, _p, seg) = loc_setup();
        let store = WorldStore::new(&db, dir.path().to_path_buf());

        let result = store.create_entry_seeded(
            &seg.id,
            "Atrium",
            "no_dot", // invalid — apply_dotted_mutation rejects
            "value",
        );
        assert!(result.is_err(), "expected error from invalid seed_field_id");

        // No orphan row should remain.
        let entries = store.list_entries(&seg.id).unwrap();
        assert!(
            entries.is_empty(),
            "expected zero entries after rollback; got {entries:?}"
        );
    }

    #[test]
    fn delete_entry_if_empty_reaps_entry_with_only_empty_string_fields() {
        let (dir, db, _p, seg) = loc_setup();
        let store = WorldStore::new(&db, dir.path().to_path_buf());

        // Seed a field with an empty string so [main] becomes
        // {"type": ""} — exercises is_value_empty's recursive
        // object-with-empty-string-values branch.
        let id = store
            .create_entry_seeded(&seg.id, "", "main.type", "")
            .unwrap();

        let reaped = store.delete_entry_if_empty(&id).unwrap();
        assert!(
            reaped,
            "entry with only empty-string fields should be reapable"
        );

        let entries = store.list_entries(&seg.id).unwrap();
        assert!(entries.is_empty());
    }
