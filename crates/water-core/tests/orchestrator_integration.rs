//! End-to-end orchestrator smoke: synthetic telemetry + analysis →
//! trigger evaluation → voice routing → prompt assembly → fake LLM.
//!
//! This intentionally does NOT run the Tauri-side `OrchestratorService`
//! (which depends on `AppHandle`). Instead it exercises the pure pipeline
//! against `water-core` types so a single test verifies that the wiring
//! the M2 Phase F orchestrator depends on still composes after future
//! refactors.

use std::sync::Arc;
use std::time::Instant;
use water_core::llm::{CannedProvider, LlmProvider, LlmRouter};
use water_core::orchestrator::triggers::builtin_triggers;
use water_core::orchestrator::{
    AnalysisSnapshot, BlockMetrics, CursorClassification, ProjectSnapshot, SceneSnapshot,
    StructuralInflection, TriggerContext, TypingTelemetry,
};
use water_core::prompts::{assemble_level_0, PromptLibrary};
use water_core::voice::registry::PersonaRegistry;
use water_core::voice::router::{route, CooldownState};
use water_core::{Db, Id};

#[tokio::test]
async fn end_to_end_trigger_evaluation_picks_speaker_and_assembles_prompt() {
    // --- setup: fresh project DB + registries ---
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open(dir.path().join("p.db")).unwrap();
    let personas = PersonaRegistry::from_db(&db).unwrap();
    let prompts = PromptLibrary::load_builtin().unwrap();

    // --- synthetic context: block_anchored_drift conditions ---
    // High divergence (0.7 > 0.6 threshold) on an anchored block while the
    // cursor sits at a paragraph break — fires `block_anchored_drift` per
    // its evaluate() rule.
    let block_id = "^bk-0001".to_string();
    let telem = TypingTelemetry {
        idle_for_ms: 3000,
        cursor_classification: CursorClassification::AtParagraphEnd,
        block_id: block_id.clone(),
        recent_word_delta: 0,
        structural_inflection: StructuralInflection::None,
    };
    let mut analysis = AnalysisSnapshot::default();
    analysis.block_metrics.insert(
        block_id.clone(),
        BlockMetrics {
            flow: Some(0.5),
            coherence: Some(0.2),
            divergence: Some(0.7),
        },
    );
    let scene = SceneSnapshot {
        id: Id::new(),
        pov_character_id: None,
        location_id: None,
        characters_present: vec![],
        word_count: 300,
        seconds_since_last_pill: 60,
    };
    let project = ProjectSnapshot::default();
    let ctx = TriggerContext {
        telemetry: &telem,
        analysis: &analysis,
        scene: &scene,
        project: &project,
    };

    // --- trigger evaluation: highest-priority candidate wins ---
    let triggers = builtin_triggers();
    let cand = triggers
        .iter()
        .filter_map(|t| t.evaluate(&ctx))
        .max_by(|a, b| a.priority.partial_cmp(&b.priority).unwrap())
        .expect("at least one trigger should fire on this synthetic context");
    assert_eq!(
        cand.trigger_id, "block_anchored_drift",
        "expected block_anchored_drift to win on high-divergence anchored block"
    );

    // --- voice routing: block_anchored_drift -> editor persona ---
    let speaker = route(&cand, &personas, &CooldownState::default(), Instant::now())
        .expect("editor persona should be available with empty cooldowns");
    assert_eq!(speaker.id(), "editor");

    // --- prompt assembly: system carries speaker+trigger; user carries excerpt ---
    let req = assemble_level_0(
        &prompts,
        &*speaker,
        cand.trigger_id,
        "She walked across the square.",
    )
    .unwrap();
    assert!(
        req.system.contains("Editor"),
        "system block must name the Editor speaker; got: {}",
        req.system
    );
    assert!(
        req.system.contains("block_anchored_drift"),
        "system block must name the trigger; got: {}",
        req.system
    );
    assert!(
        req.user.contains("square"),
        "user block must carry the scene excerpt; got: {}",
        req.user
    );
    assert!(!req.expect_json, "level-0 is a plain-text task, not JSON");

    // --- LLM dispatch: prove the router → primary → response path works.
    // Uses CannedProvider::with_response so the test is deterministic.
    let canned: Arc<dyn LlmProvider> =
        Arc::new(CannedProvider::with_response("a small bell rings nearby."));
    let router = LlmRouter::new(vec![canned]);
    let raw = router
        .generate_raw_with_default(req.system.clone(), req.user.clone())
        .await
        .unwrap();
    assert_eq!(raw, "a small bell rings nearby.");
}
