//! Voice subsystem: Speaker trait + persona/character registries +
//! deterministic voice router (Task 16).

pub mod character_template;
pub mod registry;
pub mod router;
pub mod speaker;

pub use registry::PersonaRegistry;
pub use router::{route, CooldownState};
pub use speaker::{PersonaSpeaker, Speaker, SpeakerKind};

use crate::Id;

/// Per-dispatch scene context handed to character-voice rendering so the
/// voice prompt can pull world-bible excerpts (e.g. the scene's
/// `location_id`'s sensory detail). Mirrors the subset of [`crate::orchestrator::SceneSnapshot`]
/// fields that the voice subsystem actually needs.
///
/// Construct with [`SceneContext::empty`] for test fixtures or registry-
/// build paths that don't have scene info. The empty variant carries a
/// fresh [`Id`] (no scene is identified) and no location/POV/characters
/// — `CharacterTemplate::render` treats this as "no world context to
/// inject" and drops the `{{world.location_*}}` lines via the same
/// line-based omission policy as the existing M3 fields.
#[derive(Debug, Clone)]
pub struct SceneContext {
    pub scene_id: Id,
    pub location_id: Option<Id>,
    pub pov_character_id: Option<Id>,
    pub characters_present: Vec<Id>,
}

impl SceneContext {
    /// A minimal `SceneContext` with no location, no POV, and no
    /// characters present. Used by `CharacterRegistry::from_db` (which
    /// builds character speakers before any scene is selected) and by
    /// test fixtures that don't exercise world-track rendering.
    #[must_use]
    pub fn empty() -> Self {
        Self {
            scene_id: Id::new(),
            location_id: None,
            pov_character_id: None,
            characters_present: Vec::new(),
        }
    }
}
