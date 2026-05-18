//! M2 exit gate (spec § 16): run the full 200-fixture tone audit against a
//! canned provider whose output contains no blacklisted phrase. Both
//! `layer3_catches` and `audit_violations` must be zero.
//!
//! This test does not exercise a real model — that runs in the nightly CI
//! workflow (`.github/workflows/tone-audit.yml`). It verifies the harness
//! itself: that 200 fixtures load and round-trip through the
//! provider/filter pipeline cleanly.

use tempfile::TempDir;
use water_core::llm::CannedProvider;
use water_core::tone_audit::run_gate;
use water_core::Db;

#[tokio::test]
async fn full_200_pill_gate_against_canned_provider() {
    let dir = TempDir::new().unwrap();
    let db = Db::open(dir.path().join("p.db")).unwrap();
    let provider =
        CannedProvider::with_response("Something held at the threshold, not yet curiosity.");
    // `CARGO_MANIFEST_DIR` is `<repo>/crates/water-core`; pop twice to reach
    // the repo root, then descend into the committed fixture set.
    let fixtures_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("eval/tone_audit/fixtures");
    let report = run_gate(&fixtures_dir, &provider, &db).await.unwrap();
    assert_eq!(
        report.total, 200,
        "expected 200 fixtures, got {}",
        report.total
    );
    assert_eq!(
        report.layer3_catches, 0,
        "tone leak in production filter: {:?}",
        report.failures
    );
    assert_eq!(
        report.audit_violations, 0,
        "tone leak in audit filter: {:?}",
        report.failures
    );
    assert_eq!(report.passed, 200);
}
