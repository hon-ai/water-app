//! Tone audit harness. See spec § 12 ("Voice + tone discipline") and § 16
//! ("M2 exit gate").
//!
//! This module drives a configured LLM provider against the 200-fixture
//! corpus in `eval/tone_audit/fixtures/` and reports any pill outputs that
//! tripped a tone-blacklist pattern. Two entry points are exposed:
//!
//! * [`run_gate`] — used by the M2 exit gate. Returns the report verbatim;
//!   callers should treat any non-zero `layer3_catches` or
//!   `audit_violations` as a hard failure.
//! * [`run_nightly`] — the nightly scorecard. Same underlying loop, but
//!   the caller is expected to render the report into a tracked artifact
//!   instead of failing the build.

pub mod runner;

pub use runner::{run_gate, run_nightly, AuditFailure, AuditReport, Fixture};
