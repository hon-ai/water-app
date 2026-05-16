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
