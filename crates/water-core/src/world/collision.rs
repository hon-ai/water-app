//! Character-vs-world name-collision resolution policy (M4 spec § 6.2).
//!
//! Policy:
//! - If a token matches both a character and a world entry AND the
//!   character is in `scene.characters_present`, suppress the world match
//!   (return [`TokenKind::CharacterOnly`]).
//! - Otherwise both fire ([`TokenKind::BothFire`]); downstream surfaces
//!   decide how to disambiguate.
//! - Character-only and world-only matches pass through.
//!
//! Case-sensitivity asymmetry (intentional — see
//! [`crate::world::WorldRegistry`] module docs):
//! * `WorldRegistry::find_by_token` is **case-insensitive**; the caller
//!   MUST lowercase before calling. This function does that lowercasing
//!   on behalf of its caller.
//! * `CharacterRegistry::find_by_name` is **case-sensitive** (M3 word-
//!   boundary convention). The raw `token` is passed through unchanged.
//!
//! Consumers: world-drift Stage 1 (Task 16) and `SceneAutosuggestChips`
//! (Task 28).

use crate::{character::CharacterRegistry, world::WorldRegistry, Id};

/// Outcome of resolving a single token against the character + world
/// registries given the current scene's presence list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    /// Only a character matches (either no world hit, or the character is
    /// present in the scene and suppresses the world hit).
    CharacterOnly(Id),
    /// No character matches; one or more world entries do.
    WorldOnly(Vec<Id>),
    /// Both a character and at least one world entry match, and the
    /// character is NOT in `characters_present` — downstream surfaces
    /// must decide.
    BothFire {
        character_id: Id,
        world_ids: Vec<Id>,
    },
    /// No match in either registry.
    Neither,
}

/// Apply the M4 § 6.2 collision policy to `token`.
///
/// `characters_present` is the scene's `characters_present` list (id-only).
/// When the matched character's id is in that list, world matches for the
/// same token are suppressed.
#[must_use]
pub fn resolve_token_kind(
    token: &str,
    char_registry: &CharacterRegistry,
    world_registry: &WorldRegistry,
    characters_present: &[Id],
) -> TokenKind {
    let char_match = char_registry.find_by_name(token).map(|r| r.id.clone());
    let world_matches: Vec<Id> = world_registry
        .find_by_token(&token.to_lowercase())
        .to_vec();

    match (char_match, world_matches.is_empty()) {
        (Some(c), false) if characters_present.contains(&c) => TokenKind::CharacterOnly(c),
        (Some(c), false) => TokenKind::BothFire {
            character_id: c,
            world_ids: world_matches,
        },
        (Some(c), true) => TokenKind::CharacterOnly(c),
        (None, false) => TokenKind::WorldOnly(world_matches),
        (None, true) => TokenKind::Neither,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        character::{CharacterStore, NewCharacter},
        world::WorldStore,
        Db, ProjectStore,
    };
    use serde_json::json;

    /// Builds a project with one world entry and one character, both named
    /// "Aren". Returns the temp dir (kept alive), db, project id, character
    /// id, and world entry id.
    fn setup() -> (
        tempfile::TempDir,
        Db,
        Id, // project id
        Id, // character id
        Id, // world entry id (also named "Aren")
    ) {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        let p = ProjectStore::new(&db).insert("P").unwrap();
        let store = WorldStore::new(&db, dir.path().to_path_buf());
        store.seed_builtins(&p.id).unwrap();
        let loc = store
            .find_segment_by_slug(&p.id, "locations")
            .unwrap()
            .unwrap();
        let world_id = store.create_entry(&loc.id, "Aren").unwrap();

        // Create a character and rename it to "Aren" so the SQL `name`
        // column (which `CharacterRegistry::find_by_name` reads) matches.
        let char_store = CharacterStore::new(&db, dir.path().to_path_buf());
        let row = char_store
            .create(NewCharacter {
                project_id: p.id.clone(),
                hue_token: "--water-hue-character-1".into(),
            })
            .unwrap();
        char_store
            .update_field(&row.id, "main.full_name", &json!("Aren"))
            .unwrap();

        (dir, db, p.id, row.id, world_id)
    }

    #[test]
    fn character_in_scene_wins() {
        let (dir, db, p, c, _w) = setup();
        let char_reg = CharacterRegistry::from_db(&db).unwrap();
        let world_reg = WorldRegistry::from_db(&db, &p, dir.path().to_path_buf()).unwrap();
        let result = resolve_token_kind("Aren", &char_reg, &world_reg, std::slice::from_ref(&c));
        assert_eq!(result, TokenKind::CharacterOnly(c));
    }

    #[test]
    fn both_fire_when_character_not_present() {
        let (dir, db, p, c, w) = setup();
        let char_reg = CharacterRegistry::from_db(&db).unwrap();
        let world_reg = WorldRegistry::from_db(&db, &p, dir.path().to_path_buf()).unwrap();
        let result = resolve_token_kind("Aren", &char_reg, &world_reg, &[]);
        assert_eq!(
            result,
            TokenKind::BothFire {
                character_id: c,
                world_ids: vec![w],
            }
        );
    }

    #[test]
    fn world_only_when_no_character_match() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        let p = ProjectStore::new(&db).insert("P").unwrap();
        let store = WorldStore::new(&db, dir.path().to_path_buf());
        store.seed_builtins(&p.id).unwrap();
        let loc = store
            .find_segment_by_slug(&p.id, "locations")
            .unwrap()
            .unwrap();
        let w = store.create_entry(&loc.id, "Pell").unwrap();
        let char_reg = CharacterRegistry::default();
        let world_reg = WorldRegistry::from_db(&db, &p.id, dir.path().to_path_buf()).unwrap();
        // Lowercase input exercises the lowercasing the function applies
        // before hitting the case-insensitive world index.
        let result = resolve_token_kind("pell", &char_reg, &world_reg, &[]);
        assert_eq!(result, TokenKind::WorldOnly(vec![w]));
    }

    #[test]
    fn neither_when_no_match_anywhere() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        let p = ProjectStore::new(&db).insert("P").unwrap();
        let char_reg = CharacterRegistry::default();
        let world_reg = WorldRegistry::from_db(&db, &p.id, dir.path().to_path_buf()).unwrap();
        let result = resolve_token_kind("Nonexistent", &char_reg, &world_reg, &[]);
        assert_eq!(result, TokenKind::Neither);
    }
}
