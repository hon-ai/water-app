//! `WorldStore` — world segments + entries on disk and in the index.
//!
//! Split into submodules so each stays a reasonable size:
//! * [`store`] — `WorldStore` and its segment + single-doc CRUD.
//! * [`templates`] — the canonical six built-in segment templates and the
//!   `WorldTemplateSchema` shape used for user overrides.
//!
//! Task 5 will add collection-entry CRUD inside [`store`]; Task 6 will add
//! `WorldRegistry` / `WorldEntrySnapshot` in a future submodule.

mod store;
pub mod templates;

#[cfg(test)]
mod tests;

pub use store::{WorldSegmentRow, WorldSingleDocFile, WorldStore};
