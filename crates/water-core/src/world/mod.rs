//! `WorldStore` — world segments + entries on disk and in the index.
//!
//! Split into submodules so each stays a reasonable size:
//! * [`store`] — `WorldStore` and its segment + single-doc CRUD.
//! * [`templates`] — the canonical six built-in segment templates and the
//!   `WorldTemplateSchema` shape used for user overrides.
//!
//! Task 5 added collection-entry CRUD inside [`store`]; Task 6 added
//! [`registry::WorldRegistry`] / [`registry::WorldEntrySnapshot`] — the
//! read-only hot-path snapshot built once per orchestrator dispatch.

mod store;
pub mod registry;
pub mod templates;

#[cfg(test)]
mod tests;

pub use registry::{WorldEntrySnapshot, WorldRegistry};
pub use store::{
    WorldEntryFile, WorldEntryIndexRow, WorldSegmentRow, WorldSingleDocFile, WorldStore,
};
