//! water-core — Rust core for the Water writing app.
//!
//! All disk, secrets, processes, and policy live here. The renderer is dumb
//! about timing; this crate decides when things happen.

#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions, clippy::missing_errors_doc)]

pub mod error;

pub use error::{Error, Result};

/// Crate version, exposed for diagnostics surfaces.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub mod id;
pub use id::Id;

pub mod db;
pub mod migrations;
pub use db::Db;

pub mod project;
pub use project::{Manuscript, ManuscriptStore, Project, ProjectStore};

pub mod water_toml;
pub use water_toml::WaterToml;

pub mod scene_md;
pub use scene_md::{SceneFile, SceneFrontmatter};

pub mod block;
pub use block::Block;

pub mod scene;
pub use scene::{NewScene, SceneRow, SceneStore};

pub mod scene_locks;
pub use scene_locks::SceneWriteLocks;

pub mod chapters;
pub use chapters::{Chapter, ChaptersFile};

pub mod character;
pub use character::{CharacterFile, CharacterRow, CharacterStore};

pub mod world;
pub use world::{WorldSegmentRow, WorldStore};

pub mod snapshot;
pub use snapshot::{SnapshotRow, SnapshotStore, SnapshotTrigger};

pub mod snapshot_scheduler;
pub use snapshot_scheduler::{ActiveScene, SnapshotScheduler};

pub mod rebuild;
pub use rebuild::{rebuild_from_truth, RebuildStats};

pub mod repair;
pub use repair::{run as repair, RepairReport};

pub mod ipc;

pub mod sidecar;
pub use sidecar::{Sidecar, SidecarSpec};

pub mod sidecar_supervisor;
pub use sidecar_supervisor::{SidecarStatus, SidecarStatusEvent, SidecarSupervisor};

pub mod llm;
pub use llm::{BouquetRequest, BouquetVariant, CannedProvider, LlmProvider, ProviderId};

pub mod orchestrator;

pub mod voice;

pub mod prompts;

pub mod post_filter;

pub mod replay_log;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_non_empty() {
        assert!(
            !VERSION.is_empty(),
            "VERSION must be exposed for diagnostics"
        );
    }
}
