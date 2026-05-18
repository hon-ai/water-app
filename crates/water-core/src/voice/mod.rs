//! Voice subsystem: Speaker trait + persona/character registries +
//! deterministic voice router (Task 16).

pub mod registry;
pub mod router;
pub mod speaker;

pub use registry::PersonaRegistry;
pub use speaker::{PersonaSpeaker, Speaker, SpeakerKind};
