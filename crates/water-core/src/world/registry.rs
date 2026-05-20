//! `WorldRegistry` — hot-path read-only snapshot of all world segments
//! and entries for a project. Built once per orchestrator dispatch by
//! `WorldRegistry::from_db`.
//!
//! Name + alias lookups are **case-insensitive** (lowercased on both
//! insertion and query). Character autosuggest (M3) is case-sensitive
//! on word boundaries — this asymmetry is intentional: place names are
//! more case-variable in English prose than character names. See `KNOWN_FRAGILE`
//! note (Task 34 captures this as M4 entry #22).

use crate::{world::WorldStore, Db, Id, Result};
use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct WorldRegistry {
    by_id: HashMap<Id, WorldEntrySnapshot>,
    by_name_or_alias: HashMap<String, Vec<Id>>,
    segments: HashMap<Id, crate::world::WorldSegmentRow>,
    by_segment_slug: HashMap<String, Id>,
}

#[derive(Debug, Clone)]
pub struct WorldEntrySnapshot {
    pub id: Id,
    pub segment_id: Id,
    pub segment_slug: String,
    pub name: String,
    pub aliases: Vec<String>,
    pub data: serde_json::Value,
}

impl WorldRegistry {
    /// Builds a snapshot of all segments + entries for the given project.
    /// Called once per orchestrator dispatch.
    ///
    /// # Errors
    /// Propagates any DB error from [`WorldStore::list_segments`],
    /// [`WorldStore::list_entries`], or [`WorldStore::read_entry`].
    pub fn from_db(db: &Db, project_id: &Id, project_root: std::path::PathBuf) -> Result<Self> {
        let store = WorldStore::new(db, project_root);
        let segments = store.list_segments(project_id)?;
        let mut by_segment_slug = HashMap::new();
        let mut by_id_seg = HashMap::new();
        for s in &segments {
            by_segment_slug.insert(s.slug.clone(), s.id.clone());
            by_id_seg.insert(s.id.clone(), s.clone());
        }

        let mut by_id = HashMap::new();
        let mut by_name_or_alias: HashMap<String, Vec<Id>> = HashMap::new();
        for s in &segments {
            let entries = store.list_entries(&s.id)?;
            for index_row in entries {
                let entry = store.read_entry(&index_row.id)?;
                let snap = WorldEntrySnapshot {
                    id: entry.id.clone(),
                    segment_id: entry.segment_id.clone(),
                    segment_slug: s.slug.clone(),
                    name: entry.name.clone(),
                    aliases: entry.aliases.clone(),
                    data: serde_json::Value::Object(entry.data),
                };
                if !snap.name.trim().is_empty() {
                    by_name_or_alias
                        .entry(snap.name.to_lowercase())
                        .or_default()
                        .push(snap.id.clone());
                }
                for alias in &snap.aliases {
                    if !alias.trim().is_empty() {
                        by_name_or_alias
                            .entry(alias.to_lowercase())
                            .or_default()
                            .push(snap.id.clone());
                    }
                }
                by_id.insert(snap.id.clone(), snap);
            }
        }

        Ok(Self {
            by_id,
            by_name_or_alias,
            segments: by_id_seg,
            by_segment_slug,
        })
    }

    #[must_use]
    pub fn by_id(&self, id: &Id) -> Option<&WorldEntrySnapshot> {
        self.by_id.get(id)
    }

    /// Returns IDs whose `name` or any alias matches `lowercased_token`
    /// (case-insensitive). Caller MUST lowercase before calling.
    #[must_use]
    pub fn find_by_token(&self, lowercased_token: &str) -> &[Id] {
        self.by_name_or_alias
            .get(lowercased_token)
            .map_or(&[], Vec::as_slice)
    }

    #[must_use]
    pub fn entries_by_segment_slug(&self, slug: &str) -> Vec<&WorldEntrySnapshot> {
        let Some(seg_id) = self.by_segment_slug.get(slug) else {
            return vec![];
        };
        self.by_id
            .values()
            .filter(|e| &e.segment_id == seg_id)
            .collect()
    }

    pub fn segments(&self) -> impl Iterator<Item = &crate::world::WorldSegmentRow> {
        self.segments.values()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ProjectStore;

    #[test]
    fn from_db_indexes_seeded_segments() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        let p = ProjectStore::new(&db).insert("P").unwrap();
        WorldStore::new(&db, dir.path().to_path_buf())
            .seed_builtins(&p.id)
            .unwrap();

        let reg = WorldRegistry::from_db(&db, &p.id, dir.path().to_path_buf()).unwrap();
        let slugs: Vec<&str> = reg.segments().map(|s| s.slug.as_str()).collect();
        assert!(slugs.contains(&"concept"));
        assert!(slugs.contains(&"locations"));
        assert_eq!(slugs.len(), 6);
    }

    #[test]
    fn find_by_token_is_case_insensitive() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        let p = ProjectStore::new(&db).insert("P").unwrap();
        let store = WorldStore::new(&db, dir.path().to_path_buf());
        store.seed_builtins(&p.id).unwrap();
        let loc = store
            .find_segment_by_slug(&p.id, "locations")
            .unwrap()
            .unwrap();
        let id = store.create_entry(&loc.id, "The Pell Library").unwrap();
        store
            .update_entry_aliases(&id, &["Pell".to_string(), "the library".to_string()])
            .unwrap();

        let reg = WorldRegistry::from_db(&db, &p.id, dir.path().to_path_buf()).unwrap();

        let matches_name_lower = reg.find_by_token("the pell library");
        let matches_alias_pell = reg.find_by_token("pell");
        let matches_alias_lib = reg.find_by_token("the library");

        assert_eq!(matches_name_lower, &[id.clone()][..]);
        assert_eq!(matches_alias_pell, &[id.clone()][..]);
        assert_eq!(matches_alias_lib, &[id][..]);
    }

    #[test]
    fn find_by_token_returns_empty_for_no_match() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        let p = ProjectStore::new(&db).insert("P").unwrap();
        WorldStore::new(&db, dir.path().to_path_buf())
            .seed_builtins(&p.id)
            .unwrap();
        let reg = WorldRegistry::from_db(&db, &p.id, dir.path().to_path_buf()).unwrap();
        assert!(reg.find_by_token("nonexistent").is_empty());
    }

    #[test]
    fn entries_by_segment_slug_returns_only_matching_segment() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        let p = ProjectStore::new(&db).insert("P").unwrap();
        let store = WorldStore::new(&db, dir.path().to_path_buf());
        store.seed_builtins(&p.id).unwrap();
        let loc = store
            .find_segment_by_slug(&p.id, "locations")
            .unwrap()
            .unwrap();
        store.create_entry(&loc.id, "A").unwrap();
        store.create_entry(&loc.id, "B").unwrap();

        let reg = WorldRegistry::from_db(&db, &p.id, dir.path().to_path_buf()).unwrap();
        let in_locations = reg.entries_by_segment_slug("locations");
        assert_eq!(in_locations.len(), 2);
        let in_concept = reg.entries_by_segment_slug("concept");
        assert_eq!(in_concept.len(), 0);
    }

    #[test]
    fn by_id_returns_some_for_known_entry() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        let p = ProjectStore::new(&db).insert("P").unwrap();
        let store = WorldStore::new(&db, dir.path().to_path_buf());
        store.seed_builtins(&p.id).unwrap();
        let loc = store
            .find_segment_by_slug(&p.id, "locations")
            .unwrap()
            .unwrap();
        let id = store.create_entry(&loc.id, "X").unwrap();
        let reg = WorldRegistry::from_db(&db, &p.id, dir.path().to_path_buf()).unwrap();
        let snap = reg.by_id(&id).expect("entry must be present");
        assert_eq!(snap.name, "X");
        assert_eq!(snap.segment_slug, "locations");
    }

    #[test]
    fn default_registry_is_empty() {
        let reg = WorldRegistry::default();
        assert_eq!(reg.segments().count(), 0);
        assert!(reg.find_by_token("anything").is_empty());
    }

    #[test]
    fn empty_name_and_empty_aliases_are_not_indexed() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        let p = ProjectStore::new(&db).insert("P").unwrap();
        let store = WorldStore::new(&db, dir.path().to_path_buf());
        store.seed_builtins(&p.id).unwrap();
        let loc = store
            .find_segment_by_slug(&p.id, "locations")
            .unwrap()
            .unwrap();
        let id = store.create_entry(&loc.id, "").unwrap();
        // Don't set aliases — leave default empty.
        let reg = WorldRegistry::from_db(&db, &p.id, dir.path().to_path_buf()).unwrap();
        // The unnamed entry is in by_id but not findable by name (since name is empty).
        assert!(reg.by_id(&id).is_some());
        assert!(reg.find_by_token("").is_empty());
    }
}
