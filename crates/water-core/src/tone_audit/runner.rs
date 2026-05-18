//! Tone-audit runner. Drives a provider against fixture prompts and reports
//! any blacklist trips. See `mod.rs` for the two entry points.
//!
//! ## Contract
//!
//! The runner distinguishes two failure modes so the M2 exit gate can
//! reason about each independently:
//!
//! * **`layer3_catches`** — the production post-filter chain
//!   (`builtin_post_filters`) dropped the pill. This is what users would
//!   have seen rejected at run time.
//! * **`audit_violations`** — an independently-compiled
//!   [`ToneBlacklistFilter`] saw the same drop. In M2 the two paths are the
//!   same set of patterns, so this is structurally redundant; the field is
//!   retained so future post-filter layers cannot mask a tone leak.
//!
//! For the M2 exit gate, **both** counters must be zero across the full
//! fixture corpus.

use crate::llm::{GenerateRequest, LlmProvider};
use crate::post_filter::tone_blacklist::ToneBlacklistFilter;
use crate::post_filter::{builtin_post_filters, FilterDecision, PostFilter};
use crate::prompts::{assemble_level_0, PromptLibrary};
use crate::voice::registry::PersonaRegistry;
use crate::Db;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// One fixture entry from `eval/tone_audit/fixtures/*.json`.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Fixture {
    pub id: String,
    pub trigger: String,
    pub speaker: String,
    pub scene_excerpt: String,
    pub expected_pass: bool,
}

/// Aggregate result of one audit run. `total` counts fixtures actually
/// processed (skipping non-JSON entries). For clean runs,
/// `passed == total - layer3_catches - audit_violations`.
#[derive(Debug, Default, Serialize)]
pub struct AuditReport {
    pub total: u32,
    pub layer3_catches: u32,
    pub audit_violations: u32,
    pub passed: u32,
    pub failures: Vec<AuditFailure>,
}

/// One per failed fixture. `raw` is the model output so failures are
/// reproducible from the report alone.
#[derive(Debug, Clone, Serialize)]
pub struct AuditFailure {
    pub fixture_id: String,
    pub reason: String,
    pub raw: String,
}

/// One-time gate. The M2 exit criterion is that the returned report has
/// `layer3_catches == 0` AND `audit_violations == 0`; this function does
/// **not** panic when that fails — it surfaces the report and lets the
/// caller (test, CLI, CI) render its own assertion.
pub async fn run_gate<P: LlmProvider + ?Sized>(
    fixtures_dir: &Path,
    provider: &P,
    db: &Db,
) -> Result<AuditReport, String> {
    audit_loop(fixtures_dir, provider, db).await
}

/// Nightly variant. Identical implementation; kept as a distinct function
/// to give the nightly workflow a stable name even when the gate semantics
/// evolve.
pub async fn run_nightly<P: LlmProvider + ?Sized>(
    fixtures_dir: &Path,
    provider: &P,
    db: &Db,
) -> Result<AuditReport, String> {
    audit_loop(fixtures_dir, provider, db).await
}

async fn audit_loop<P: LlmProvider + ?Sized>(
    fixtures_dir: &Path,
    provider: &P,
    db: &Db,
) -> Result<AuditReport, String> {
    let prompts = PromptLibrary::load_builtin()?;
    let personas = PersonaRegistry::from_db(db)?;
    let filters = builtin_post_filters(&prompts.tone.blacklist_regex.patterns);
    // Independent audit filter: same patterns today, but compiled from the
    // raw TOML so that any future divergence between `builtin_post_filters`
    // and the tone source surfaces as `audit_violations`.
    let audit_filter = ToneBlacklistFilter::compile(&prompts.tone.blacklist_regex.patterns)
        .map_err(|e| format!("audit filter compile failed: {e}"))?;

    let mut report = AuditReport::default();

    // Collect entries up-front and sort so reports are deterministic across
    // platforms (Windows readdir order differs from Unix).
    let mut entries: Vec<_> = std::fs::read_dir(fixtures_dir)
        .map_err(|e| format!("read_dir {}: {e}", fixtures_dir.display()))?
        .filter_map(std::result::Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("json"))
        .collect();
    entries.sort();

    for path in entries {
        let raw = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let fx: Fixture =
            serde_json::from_str(&raw).map_err(|e| format!("parse {}: {e}", path.display()))?;
        let speaker = personas
            .by_id(&fx.speaker)
            .ok_or_else(|| format!("unknown persona: {}", fx.speaker))?;
        let req_prompt = assemble_level_0(&prompts, &*speaker, &fx.trigger, &fx.scene_excerpt)?;
        let req = GenerateRequest {
            system: req_prompt.system,
            user: req_prompt.user,
            ..GenerateRequest::default()
        };
        let out = provider
            .generate_raw(req)
            .await
            .map_err(|e| format!("{e:?}"))?;
        report.total += 1;

        let mut layer3_caught = false;
        for f in &filters {
            if let FilterDecision::Drop { reason } = f.evaluate(&out) {
                report.layer3_catches += 1;
                layer3_caught = true;
                report.failures.push(AuditFailure {
                    fixture_id: fx.id.clone(),
                    reason,
                    raw: out.clone(),
                });
                break;
            }
        }
        if layer3_caught {
            continue;
        }
        // Independent audit pass; today identical, but logically distinct.
        if let FilterDecision::Drop { reason } = audit_filter.evaluate(&out) {
            report.audit_violations += 1;
            report.failures.push(AuditFailure {
                fixture_id: fx.id.clone(),
                reason,
                raw: out.clone(),
            });
        } else {
            report.passed += 1;
        }
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::CannedProvider;
    use crate::Db;
    use tempfile::TempDir;

    fn write_fixture(dir: &Path, idx: usize, speaker: &str, trigger: &str) {
        let id = format!("tone-{idx:03}");
        let fx = serde_json::json!({
            "id": id,
            "trigger": trigger,
            "speaker": speaker,
            "scene_excerpt": "She walked across the square.",
            "expected_pass": true,
        });
        std::fs::write(dir.join(format!("{id}.json")), fx.to_string()).unwrap();
    }

    #[tokio::test]
    async fn gate_passes_with_clean_canned_output() {
        let dir = TempDir::new().unwrap();
        let db = Db::open(dir.path().join("p.db")).unwrap();
        let provider =
            CannedProvider::with_response("Something held at the threshold, not yet curiosity.");
        let fxd = dir.path().join("fixtures");
        std::fs::create_dir_all(&fxd).unwrap();
        for i in 0..5 {
            write_fixture(&fxd, i, "echo", "block_anchored_drift");
        }
        let r = run_gate(&fxd, &provider, &db).await.unwrap();
        assert_eq!(r.total, 5);
        assert_eq!(r.layer3_catches, 0);
        assert_eq!(r.audit_violations, 0);
        assert_eq!(r.passed, 5);
        assert!(r.failures.is_empty());
    }

    #[tokio::test]
    async fn gate_catches_blacklisted_output() {
        let dir = TempDir::new().unwrap();
        let db = Db::open(dir.path().join("p.db")).unwrap();
        let provider =
            CannedProvider::with_response("You should consider rewriting this paragraph.");
        let fxd = dir.path().join("fixtures");
        std::fs::create_dir_all(&fxd).unwrap();
        write_fixture(&fxd, 1, "echo", "block_anchored_drift");
        let r = run_gate(&fxd, &provider, &db).await.unwrap();
        assert_eq!(r.total, 1);
        assert!(r.layer3_catches >= 1, "expected layer3 catch, got {r:?}");
        assert_eq!(r.passed, 0);
        assert!(!r.failures.is_empty());
        assert_eq!(r.failures[0].fixture_id, "tone-001");
    }

    #[tokio::test]
    async fn run_nightly_mirrors_gate_loop() {
        let dir = TempDir::new().unwrap();
        let db = Db::open(dir.path().join("p.db")).unwrap();
        let provider = CannedProvider::with_response("Light pooled on the threshold.");
        let fxd = dir.path().join("fixtures");
        std::fs::create_dir_all(&fxd).unwrap();
        write_fixture(&fxd, 1, "architect", "scene_flow_dip");
        write_fixture(&fxd, 2, "chorus", "topic_drift");
        let r = run_nightly(&fxd, &provider, &db).await.unwrap();
        assert_eq!(r.total, 2);
        assert_eq!(r.layer3_catches, 0);
        assert_eq!(r.audit_violations, 0);
        assert_eq!(r.passed, 2);
    }
}
