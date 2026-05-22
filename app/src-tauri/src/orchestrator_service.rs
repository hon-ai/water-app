//! Process-side orchestrator that owns the live M2 pill pipeline.
//!
//! The service holds:
//! - the in-memory `Pill` list (max 2 on-screen + pinned in the DB)
//! - per-speaker cooldown state
//! - per-bouquet history for anti-loop
//! - the scene + project + analysis snapshots
//! - a snapshot of the current scene body for prompt excerpting
//! - shared handles to the `LlmRouter`, `PersonaRegistry`, `PromptLibrary`,
//!   and the built-in `PostFilter` chain
//!
//! It is driven by `OrchestratorRequest`s on a single mpsc channel and emits
//! Tauri events (`pill:emerged`, `bouquet:ready`, `pill:dismissed`,
//! `pill:evicted`) through the `AppHandle` it was constructed with.
//!
//! Concurrency contract:
//! - The channel-processing loop holds the service `Mutex` for the duration
//!   of each request. Telemetry and expand handlers may `tokio::spawn` an
//!   LLM call that outlives the lock; those spawned tasks NEVER touch
//!   `self.pills` — they only emit events, identifying pills by `pill_id`.
//! - `bouquet_history` is therefore stored as `Arc<Mutex<HashMap>>` so the
//!   expand spawn can read prior variants and append new ones without
//!   re-acquiring the service lock.
//! - The `LlmRouter` is looked up via `Arc<Mutex<Option<Arc<LlmRouter>>>>`
//!   (a clone of `AppState.router`) so `provider_test` reconfiguration
//!   takes effect immediately without restarting the service.

use crate::events::emit;
use std::collections::HashMap;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use tauri::AppHandle;
use tokio::sync::{mpsc, Mutex};
use water_core::character::registry::CharacterRegistry;
use water_core::llm::LlmRouter;
use water_core::orchestrator::{
    anti_loop::max_overlap,
    eviction::pick_evictee,
    feedback::{
        classify_writer_mode, FeedbackStore, PillOutcome, TriggerTuning, WriterMode,
    },
    state::{Pill, PillEvent},
    triggers::builtin_triggers,
    AnalysisSnapshot, ConfirmationRequest, CursorClassification, ProjectSnapshot, SceneSnapshot,
    TriggerCandidate, TriggerContext, TypingTelemetry,
};
use water_core::post_filter::{builtin_post_filters, FilterDecision, PostFilter};
use water_core::prompts::{
    assemble_editor_polish, assemble_level_0, assemble_pill_expand, assemble_pill_regenerate,
    assemble_rabbit_deepen_inherit, assemble_rabbit_fan_4, PromptContext, PromptLibrary,
};
use water_core::rabbit::{
    ChildInsert as RabbitChildInsert, Direction as RabbitDirection, RabbitStore,
    RootInsert as RabbitRootInsert, SpeakerKind as RabbitSpeakerKind,
};
use water_core::replay_log::{ReplayEntry, ReplayLog};
use water_core::voice::registry::PersonaRegistry;
use water_core::voice::router::{route_with_chars, CooldownState};
use water_core::voice::speaker::SpeakerArc;
use water_core::world::WorldRegistry;
use water_core::Id;

/// Sidecar bridge — pure patcher. Applies a fresh `AnalyzeResponse`
/// to `analysis` in place. Extracted from `on_block_analysis` so the
/// patching logic is testable without an `AppHandle`.
///
/// Semantics:
///   * Scene-level fields (`flow`/`coherence`/etc.) take the latest
///     paragraph as a proxy for "current state" — a recent paragraph
///     is a better signal than an arithmetic mean over the whole
///     scene.
///   * `block_metrics[block_id]` is upserted with flow / coherence /
///     divergence so `block_anchored_drift` can read paragraph-
///     specific values keyed by the just-finished block.
///   * `valence_history` appends, capped at `VALENCE_HISTORY_CAP` so
///     a long writing session can't grow it unboundedly.
fn apply_block_analysis(
    analysis: &mut AnalysisSnapshot,
    block_id: String,
    response: &water_core::ipc::AnalyzeResponse,
) {
    // f64 -> f32 narrows are safe: sidecar responses are in [0.0, 1.0].
    analysis.flow = Some(response.flow as f32);
    analysis.coherence = Some(response.coherence as f32);
    analysis.engagement = Some(response.engagement as f32);
    analysis.divergence = Some(response.divergence as f32);
    analysis.pace = Some(response.pace as f32);
    analysis.intensity = Some(response.intensity as f32);
    analysis.valence = Some(response.valence as f32);
    analysis.block_metrics.insert(
        block_id,
        water_core::orchestrator::BlockMetrics {
            flow: Some(response.flow as f32),
            coherence: Some(response.coherence as f32),
            divergence: Some(response.divergence as f32),
        },
    );
    analysis.valence_history.push(response.valence as f32);
    if analysis.valence_history.len() > VALENCE_HISTORY_CAP {
        let drop_n = analysis.valence_history.len() - VALENCE_HISTORY_CAP;
        analysis.valence_history.drain(0..drop_n);
    }
}

/// Average of the last `n` values in a slice. Returns `None` for
/// empty input. Used to flatten the heat metric tracks into the
/// trigger-facing `heat_*_tail` fields.
fn tail_average(scores: &[f32], n: usize) -> Option<f32> {
    if scores.is_empty() {
        return None;
    }
    let take = n.min(scores.len());
    let sum: f32 = scores.iter().rev().take(take).sum();
    Some(sum / take as f32)
}

/// Best-effort read of the scene's typing-history rows. SQLite errors
/// (e.g., table-not-found on legacy DBs) are swallowed and an empty
/// vec is returned — local-metric compute falls back to the empty
/// case gracefully.
fn read_typing_history(
    db: &Arc<Mutex<water_core::Db>>,
    scene_id: &Id,
) -> Vec<water_core::heat::TypingEvent> {
    // Acquire the lock synchronously; this fn is called from inside
    // the orchestrator's `handle` loop, which is already on a tokio
    // task, so `blocking_lock` is safe here.
    let g = match db.try_lock() {
        Ok(g) => g,
        Err(_) => {
            // Contention: skip this tick. The next SceneState arrival
            // will retry.
            return Vec::new();
        }
    };
    let conn = g.conn();
    let stmt = conn.prepare(
        "SELECT ts_ms, word_delta FROM scene_typing_history
         WHERE scene_id = ?1 ORDER BY ts_ms ASC",
    );
    let mut stmt = match stmt {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };
    let rows = stmt.query_map([scene_id.as_str()], |r| {
        Ok(water_core::heat::TypingEvent {
            ts_ms: r.get::<_, i64>(0)?,
            #[allow(clippy::cast_possible_truncation)]
            word_delta: r.get::<_, i64>(1)? as i32,
        })
    });
    match rows {
        Ok(rs) => rs.filter_map(std::result::Result::ok).collect(),
        Err(_) => Vec::new(),
    }
}

/// Minimum milliseconds between consecutive pill emissions, across all
/// speakers and triggers. Tuned during the M4 smoke walk: with 5
/// speakers and idle pulses every 3s, the unrestricted dispatch rate
/// produced visibly cycling pills (FIFO-evicting each other before the
/// writer could read them). 15s leaves room for each pill to be read
/// while still feeling responsive on long writing sessions.
const MIN_PILL_INTERVAL_MS: u64 = 15_000;

/// Shared slot holding the optional `LlmRouter`. Mirrors
/// `AppState.router` so the service sees provider reconfiguration without
/// being torn down and rebuilt.
pub type SharedRouter = Arc<Mutex<Option<Arc<LlmRouter>>>>;

#[derive(Debug)]
// `Analysis` and `Dismiss` are dispatched by M3 (sidecar analysis feed) and
// by service-side pill lifecycle tracking (currently the renderer talks to
// `pill_dismiss` directly). Allowed dead-code so the public surface stays
// stable as those land.
#[allow(dead_code)]
pub enum OrchestratorRequest {
    /// Live typing-telemetry tick. `last_block_text` is populated only on
    /// idle pulses (>=3 s) so the orchestrator can update
    /// `AnalysisSnapshot.last_block_text` for `character_dissonance`;
    /// during typing bursts (5 Hz cap) the renderer sends `None` to keep
    /// the wire small.
    Telemetry {
        telemetry: TypingTelemetry,
        last_block_text: Option<String>,
    },
    Analysis(AnalysisSnapshot),
    /// Sidecar-bridge delta: a fresh per-paragraph metric from
    /// `Sidecar::analyze` came back. The handler patches the live
    /// `AnalysisSnapshot` in place — scene-level fields take the
    /// latest values, `block_metrics[block_id]` is upserted, and
    /// `valence_history` appends (capped at
    /// `VALENCE_HISTORY_CAP`). `scene_id` lets a late-arriving
    /// response for a previously-active scene get dropped on the
    /// floor instead of poisoning the current scene's analysis.
    BlockAnalysis {
        scene_id: Id,
        block_id: String,
        response: water_core::ipc::AnalyzeResponse,
    },
    /// Scene + project snapshot AND the full scene body text. The
    /// orchestrator caches the body so each telemetry tick can build a
    /// prompt excerpt without re-reading from disk.
    SceneState(SceneSnapshot, ProjectSnapshot, String),
    Expand {
        parent_pill_id: Id,
    },
    Regenerate {
        parent_pill_id: Id,
    },
    Dismiss {
        pill_id: Id,
    },
    /// v8: pill lifecycle terminal for adaptive learning. Renderer
    /// fires these from the IPC layer on pin / dismiss / evict / click.
    /// The orchestrator looks up the attribution it stored at emerge
    /// time, applies the reward to `trigger_feedback`, and refreshes
    /// the in-memory `tuning` snapshot.
    RecordOutcome {
        pill_id: Id,
        outcome: OutcomeSignal,
    },
    /// v8: wipe `trigger_feedback` and reset the in-memory tuning
    /// snapshot. Fired from the Settings "Reset trigger learning"
    /// button. Attribution map is also cleared so any in-flight
    /// pills don't write a final reward against the just-cleared
    /// state.
    ResetLearning,
    /// Phase 4: open the rabbit hole on a freshly-clicked pill.
    /// Creates a root thought from the pill in the DB, then fans
    /// four children from it. Emits `deepen:ready` when the LLM
    /// returns.
    ///
    /// `parent_text` / `speaker_id` / `block_target_id` come from
    /// the renderer's `Pill` record, not the service's own list.
    /// The service-side `Pill.text` is never written back after
    /// the LLM call lands (legacy: the LLM response goes straight
    /// to the renderer via `pill:emerged` and the server record
    /// stays at `text=None`), so re-looking-up here would always
    /// see empty text.
    DeepenPill {
        pill_id: Id,
        parent_text: String,
        speaker_id: String,
        block_target_id: Option<String>,
    },
    /// Phase 4: fan four children from an *existing* rabbit thought
    /// (the writer clicked a child in the panel). Emits `deepen:ready`.
    DeepenThought {
        thought_id: Id,
    },
    /// Phase 4: toggle the resonance flag on a rabbit thought.
    /// Resonant thoughts (and their ancestors) are protected from
    /// auto-trim; future Phase-6 prompts read recent resonant picks
    /// as a voice-preference signal.
    SetRabbitResonance {
        thought_id: Id,
        resonant: bool,
    },
    /// Phase 5.8 — dispatch the LLM polish prompt against a single
    /// paragraph. Renderer fires this on the post-save path. The
    /// handler enforces the per-scene cap + per-block cooldown
    /// before spending the LLM call.
    EditorPolish {
        scene_id: Id,
        block_id: String,
        block_text: String,
    },
    Shutdown,
}

/// Renderer-side lifecycle events that map to learning rewards.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutcomeSignal {
    /// Writer clicked the pill (opened the bouquet). Not terminal —
    /// the orchestrator marks the attribution as `clicked` and waits
    /// for the actual terminal (pin / dismiss / evict).
    Click,
    /// Writer pinned the pill. Terminal positive.
    Pin,
    /// Writer dismissed via the × button. Terminal negative.
    Dismiss,
    /// FIFO eviction; renderer drops the oldest on overflow.
    /// Reward depends on whether the pill was clicked first.
    Evict,
}

/// Handle exposed to Tauri commands. Cloneable — clones share the same
/// underlying `mpsc::Sender`, so any clone can dispatch a request and they
/// all close together when the last one drops.
#[derive(Clone)]
pub struct OrchestratorHandle {
    tx: mpsc::Sender<OrchestratorRequest>,
}

impl OrchestratorHandle {
    /// Send a request to the orchestrator. Best-effort: a closed channel
    /// (service shut down) silently drops the request rather than
    /// propagating an error, because every caller currently treats the
    /// orchestrator as fire-and-forget.
    pub async fn send(&self, req: OrchestratorRequest) {
        let _ = self.tx.send(req).await;
    }
}

pub struct OrchestratorService {
    app: AppHandle,
    router: SharedRouter,
    personas: PersonaRegistry,
    /// Loaded once at `start` from the project DB. Re-loading on character
    /// upsert is an M3+ concern; the current registry is a snapshot of the
    /// project's characters at open time. Used by `route_with_chars` for
    /// POV-prefer routing and by `pick_best_trigger` for the
    /// `character_dissonance` gate.
    characters: CharacterRegistry,
    /// World-bible snapshot loaded once at `start` from the project DB
    /// (M4 Task 13). Threaded into every `TriggerContext` so world-track
    /// triggers (Task 17: `world_drift`) can read segments + entries
    /// without round-tripping to disk. Like `characters`, re-loading on
    /// world upsert is an M4+ concern; this is an open-time snapshot.
    world_registry: WorldRegistry,
    prompts: Arc<PromptLibrary>,
    pills: Vec<Pill>,
    cooldowns: CooldownState,
    /// Global pill-emission interval gate. Independent of per-speaker
    /// cooldowns (which range 45-90s each). With 5 speakers and idle
    /// pulses every 3s, the writer could otherwise see a new pill from
    /// a different speaker every tick — 5 pills in 15s — which the
    /// M4 smoke walk surfaced as too fast to read. This is a coarse
    /// rate limit: at least `MIN_PILL_INTERVAL_MS` between any two
    /// successful pill dispatches, regardless of which speaker.
    last_pill_emit_at: Option<std::time::Instant>,
    /// M5: per-project DB handle used by the heat-compute path to read
    /// scene_typing_history + write heat_metric rows after each
    /// SceneState arrival. Wrapped in `Arc<Mutex<…>>` so the heat
    /// compute can grab it briefly without blocking the orchestrator
    /// loop's other handlers.
    db: Arc<Mutex<water_core::Db>>,
    /// M5: per-session token budget for LLM-backed heat metrics
    /// (valence + coherence). Bites after sustained heavy editing;
    /// see `heat::LlmBudget`. Reset on project close (orchestrator
    /// service drops with the project; the budget drops with it).
    #[allow(dead_code)]
    heat_budget: Arc<water_core::heat::LlmBudget>,
    /// parent_pill_id (string form) -> list of prior bouquet variant texts.
    /// Shared via `Arc` so expand-spawn tasks can append accepted variants
    /// without re-entering the service mutex.
    bouquet_history: Arc<Mutex<HashMap<String, Vec<String>>>>,
    scene: Option<SceneSnapshot>,
    project: ProjectSnapshot,
    analysis: AnalysisSnapshot,
    /// Snapshot of the current scene's full body. Updated whenever the
    /// renderer sends `SceneState`. Used to compute prompt excerpts on
    /// each telemetry tick without round-tripping to `SceneStore`.
    scene_text: String,
    /// Optional opt-in replay log. `Some` when `WATER_REPLAY_LOG=1` was
    /// set at service start; cloned into spawned LLM tasks so they can
    /// append request/response pairs without re-acquiring the service
    /// mutex. `None` is the production default (no IO overhead).
    replay_log: Option<Arc<ReplayLog>>,
    /// v8: per-trigger learned sensitivity, reloaded from
    /// `trigger_feedback` on project open + after every reward
    /// observation. Passed into `TriggerContext` on each
    /// `pick_best_trigger` call.
    tuning: TriggerTuning,
    /// v8: in-flight attribution per emerged pill. Built on
    /// `pill:emerged`, drained on the terminal lifecycle event
    /// (pin / dismiss / evict). Cleared on project close (the
    /// service drops with the project).
    attribution: HashMap<String, PillAttribution>,
    /// Phase 5.8: per-scene count of LLM polish passes spent this
    /// session. Capped at `POLISH_PASS_CAP_PER_SCENE`; manual
    /// re-request will land in a follow-up. Cleared with the
    /// service on project close.
    polish_pass_counts: HashMap<String, u32>,
    /// Phase 5.8: when we last polished a given (scene_id, block_id).
    /// Used to throttle the post-save fire so consecutive saves
    /// against the same block don't burn LLM calls.
    polish_last_at: HashMap<(String, String), std::time::Instant>,
    /// Sidecar bridge — optional handle to the per-project FastAPI
    /// analysis sidecar. Cloned `Arc` so the underlying child
    /// process lives until both this service and `OpenProject` drop
    /// it. `None` when the sidecar failed to boot (project still
    /// opens; sidecar-dependent triggers stay dark).
    sidecar: Option<Arc<water_core::Sidecar>>,
    /// Self-channel sender — clone of the original `mpsc` `tx` so
    /// spawned tasks (notably the sidecar analyze fan-out) can
    /// re-enter the loop with `OrchestratorRequest::BlockAnalysis`.
    self_tx: mpsc::Sender<OrchestratorRequest>,
    /// Per-block debounce for sidecar analyze fan-out. Without it
    /// every idle pulse (>=3 s) would fire an HTTP round-trip for
    /// the same paragraph the writer hasn't touched, burning the
    /// sidecar's CPU on no new signal. Last-analyzed instant per
    /// block id; gated against `BLOCK_ANALYZE_DEBOUNCE` below.
    block_analyze_throttle: HashMap<String, std::time::Instant>,
}

/// Phase 5.8 — most polish passes per scene per session. The cap
/// prevents the LLM from grinding through a long writing day
/// uncapped. UX_SPEC §E.3 calls for 5; the writer can manually
/// re-request once the cap is hit (UI for the request lands later).
const POLISH_PASS_CAP_PER_SCENE: u32 = 5;
/// Minimum seconds between polish dispatches for the same
/// (scene, block). Prevents the autosave path from re-polishing a
/// block the writer is rapidly editing in successive 2-second
/// debounces.
const POLISH_PER_BLOCK_COOLDOWN_SECS: u64 = 30;

/// Sidecar bridge — per-block debounce for analyze fan-out. The
/// orchestrator runs analyze only once per `BLOCK_ANALYZE_DEBOUNCE`
/// for a given block id; a typical writing burst yields idle pulses
/// every ~3 s, but the paragraph the writer just left often doesn't
/// change again for many seconds. Without this gate, we'd burn the
/// sidecar's CPU on signal-free re-analysis.
const BLOCK_ANALYZE_DEBOUNCE: std::time::Duration = std::time::Duration::from_secs(4);

/// Sidecar bridge — keep at most the last N valence readings so
/// `valence_spike` has a trailing baseline without `valence_history`
/// growing unboundedly across a long session.
const VALENCE_HISTORY_CAP: usize = 8;

/// v8: per-pill bookkeeping for the learning loop. Captures the
/// writer-mode classification at emerge time so terminal-event
/// rewards stay attributable to the *moment* the pill appeared,
/// not the moment the writer happened to interact.
#[derive(Debug, Clone)]
struct PillAttribution {
    trigger_id: String,
    mode: WriterMode,
    clicked: bool,
}

impl OrchestratorService {
    /// ~400-char excerpt anchored to the target block. Starts AT the
    /// block-id marker (just before its content) and runs forward
    /// 400 chars, plus a small 80-char prefix for grounding context
    /// from whatever block precedes it.
    ///
    /// Why not center on the marker like the previous implementation
    /// did: when the writer's cursor sits in block N, centering on
    /// block N's marker would capture ~200 chars of block N-1's
    /// content. The LLM, seeing more of N-1 than of N, would
    /// naturally observe N-1's subject — while `block_target_id`
    /// still pointed at N. The hover highlight then resolves to a
    /// paragraph the pill never actually talked about. Anchoring
    /// forward keeps the model's subject and the highlight subject
    /// aligned.
    ///
    /// `String::find` operates on byte indices but we slice the resulting
    /// range directly. For block-id anchors (`^bk-####`) the matched
    /// substring is pure ASCII so the byte offset lands on a char boundary;
    /// the fallback `..end` path uses `floor_char_boundary`-style clamping
    /// via `min(len)` and may slice in the middle of a multibyte char, so
    /// we walk backwards to the previous char boundary before slicing.
    fn scene_excerpt_for(&self, block_id: &str) -> String {
        if !block_id.is_empty() {
            if let Some(pos) = self.scene_text.find(block_id) {
                // Small prefix for grounding (writer's voice / arc),
                // but not enough to outweigh the focus block.
                let start = back_to_char_boundary(&self.scene_text, pos.saturating_sub(80));
                let end = forward_to_char_boundary(
                    &self.scene_text,
                    (pos + 400).min(self.scene_text.len()),
                );
                return self.scene_text[start..end].to_string();
            }
        }
        let end = forward_to_char_boundary(&self.scene_text, 400.min(self.scene_text.len()));
        self.scene_text[..end].to_string()
    }
}

/// Phase 6 — owned mirror of `PromptContext` so the orchestrator can
/// build it without lifetime gymnastics across await points. Each
/// `OwnedPromptContext::as_borrowed()` call materializes a fresh
/// `PromptContext<'_>` that borrows from this owner.
#[derive(Debug, Default)]
struct OwnedPromptContext {
    scene_name: Option<String>,
    arc_position: Option<water_core::orchestrator::arc::ArcPosition>,
    scene_ordering: Option<u32>,
    manuscript_scene_count: Option<u32>,
    pov_character_name: Option<String>,
    location_name: Option<String>,
    location_brief: Option<String>,
    character_compact: Option<String>,
    recent_resonance: Vec<String>,
}

impl OwnedPromptContext {
    fn as_borrowed(&self) -> PromptContext<'_> {
        PromptContext {
            scene_name: self.scene_name.as_deref(),
            arc_position: self.arc_position,
            scene_ordering: self.scene_ordering,
            manuscript_scene_count: self.manuscript_scene_count,
            pov_character_name: self.pov_character_name.as_deref(),
            location_name: self.location_name.as_deref(),
            location_brief: self.location_brief.as_deref(),
            character_compact: self.character_compact.as_deref(),
            recent_resonance: &self.recent_resonance,
        }
    }
}

/// Walk `idx` backwards until it lands on a UTF-8 char boundary. `idx` is
/// already in `0..=s.len()` (we always pass `min(len)` or `saturating_sub`).
fn back_to_char_boundary(s: &str, mut idx: usize) -> usize {
    while idx > 0 && !s.is_char_boundary(idx) {
        idx -= 1;
    }
    idx
}

fn forward_to_char_boundary(s: &str, mut idx: usize) -> usize {
    while idx < s.len() && !s.is_char_boundary(idx) {
        idx += 1;
    }
    idx
}

impl OrchestratorService {
    /// Spawn the service loop and return a handle. The loop terminates when
    /// either `OrchestratorRequest::Shutdown` is received or every handle
    /// clone is dropped.
    ///
    /// `project_root` is the open `.water` directory. It is only used to
    /// place the replay log when `WATER_REPLAY_LOG=1` is set in the
    /// environment; the rest of the service does not consult it.
    pub fn start(
        app: AppHandle,
        router: SharedRouter,
        personas: PersonaRegistry,
        characters: CharacterRegistry,
        world_registry: WorldRegistry,
        project_root: PathBuf,
        db: Arc<Mutex<water_core::Db>>,
        sidecar: Option<Arc<water_core::Sidecar>>,
    ) -> OrchestratorHandle {
        // Channel depth 64 was picked to comfortably absorb a burst of
        // typing-telemetry ticks (renderer fires ~one per keystroke). If
        // the queue ever fills, `send` waits instead of dropping — that's
        // fine because telemetry isn't latency-critical and back-pressure
        // naturally rate-limits the renderer.
        let (tx, mut rx) = mpsc::channel::<OrchestratorRequest>(64);
        // Sidecar bridge: the spawned analyze tasks need to re-enter
        // the loop with `BlockAnalysis`. Clone the tx up front so we
        // can stash one in the service without racing the per-tick
        // handler against its own channel close.
        let self_tx = tx.clone();
        let prompts =
            Arc::new(PromptLibrary::load_builtin().expect("built-in prompts must load at startup"));

        // Replay log: opt-in via env var. Settings-DB opt-in is a no-op
        // stub here — wired when Settings UI lands in M7. We open the
        // file once per service spawn; each open_project mints a fresh
        // session ULID so successive sessions don't collide.
        let replay_log = if std::env::var("WATER_REPLAY_LOG").as_deref() == Ok("1") {
            let session_id = Id::new().as_str().to_string();
            match ReplayLog::open(&project_root, &session_id) {
                Ok(log) => {
                    tracing::info!(
                        session = %session_id,
                        "replay log enabled (.water/log/llm/{session_id}.jsonl)"
                    );
                    Some(Arc::new(log))
                }
                Err(e) => {
                    tracing::warn!(error = %e, "failed to open replay log; continuing without it");
                    None
                }
            }
        } else {
            None
        };

        // v8: prime the tuning snapshot from the DB so cold-boot
        // sensitivities reflect last session's learning. Failure
        // (e.g. legacy DB without the v8 table) falls back to the
        // default tuning — every trigger fires with its original
        // M2 threshold.
        //
        // Phase 4: auto-trim the rabbit-hole tree on project open.
        // The trim is a single-write-transaction pass enforcing the
        // 5000-row / 25-MB caps (spec §D.5.a). Both happen inside
        // the same db-lock acquisition so we touch the lock once at
        // startup rather than twice.
        let tuning = {
            let g = db.try_lock();
            match g {
                Ok(db_guard) => {
                    // Trim first — cheap when the tree is under cap;
                    // bounded by the leaf-then-interior passes when
                    // it's over. Errors logged but never block boot.
                    let trim_store = water_core::RabbitStore::new(&db_guard);
                    match trim_store.auto_trim(water_core::RabbitCaps::default()) {
                        Ok(report) if report.rows_removed > 0 => {
                            tracing::info!(
                                rows = report.rows_removed,
                                bytes = report.bytes_freed,
                                leaves = report.leaves_trimmed,
                                interior = report.interior_trimmed,
                                "rabbit_thought auto-trim ran at project open"
                            );
                        }
                        Ok(_) => {}
                        Err(e) => {
                            tracing::warn!(error = %e, "rabbit auto-trim failed");
                        }
                    }
                    water_core::orchestrator::feedback::FeedbackStore::new(&db_guard)
                        .load_sensitivities()
                        .map(TriggerTuning::new)
                        .unwrap_or_default()
                }
                Err(_) => TriggerTuning::default(),
            }
        };

        let svc = Arc::new(Mutex::new(OrchestratorService {
            app,
            router,
            personas,
            characters,
            world_registry,
            prompts,
            pills: Vec::new(),
            cooldowns: CooldownState::default(),
            last_pill_emit_at: None,
            db,
            heat_budget: Arc::new(water_core::heat::LlmBudget::default()),
            bouquet_history: Arc::new(Mutex::new(HashMap::new())),
            scene: None,
            project: ProjectSnapshot::default(),
            analysis: AnalysisSnapshot::default(),
            scene_text: String::new(),
            replay_log,
            tuning,
            attribution: HashMap::new(),
            polish_pass_counts: HashMap::new(),
            polish_last_at: HashMap::new(),
            sidecar,
            self_tx,
            block_analyze_throttle: HashMap::new(),
        }));

        tokio::spawn({
            let svc = svc.clone();
            async move {
                while let Some(req) = rx.recv().await {
                    if matches!(req, OrchestratorRequest::Shutdown) {
                        break;
                    }
                    let mut s = svc.lock().await;
                    s.handle(req).await;
                }
            }
        });

        OrchestratorHandle { tx }
    }

    async fn handle(&mut self, req: OrchestratorRequest) {
        match req {
            OrchestratorRequest::Telemetry {
                telemetry,
                last_block_text,
            } => self.on_telemetry(telemetry, last_block_text).await,
            OrchestratorRequest::Analysis(a) => {
                self.analysis = a;
            }
            OrchestratorRequest::BlockAnalysis {
                scene_id,
                block_id,
                response,
            } => {
                self.on_block_analysis(scene_id, block_id, response);
            }
            OrchestratorRequest::SceneState(s, p, text) => {
                // Scene-switch concern: cooldowns + bouquet_history carry
                // over for now. Per spec § 11.3 session-only state never
                // persists across project close; scene-switch is finer-
                // grained and intentionally NOT cleared here so a writer
                // bouncing between scenes doesn't reset the anti-loop
                // history mid-thought. Revisit if cross-scene priors
                // cause undesirable suppression.
                let scene_id = s.id.clone();
                let characters_present = s.characters_present.clone();
                self.scene = Some(s);
                self.project = p;
                self.scene_text = text.clone();
                // M5: drive heat recompute. Local metrics only for v1
                // (pacing / presence / world_refs). Valence + coherence
                // are LLM-backed and require the budget-gated path
                // (TODO follow-up). Emits `heat:updated` so the
                // renderer's HeatmapStrip refetches.
                self.recompute_heat_local(scene_id, characters_present, text);
            }
            OrchestratorRequest::Expand { parent_pill_id } => {
                self.on_expand(parent_pill_id, false).await;
            }
            OrchestratorRequest::Regenerate { parent_pill_id } => {
                self.on_expand(parent_pill_id, true).await;
            }
            OrchestratorRequest::Dismiss { pill_id } => {
                if let Some(p) = self.pills.iter_mut().find(|p| p.id == pill_id) {
                    p.state =
                        water_core::orchestrator::state::transition(p, &PillEvent::UserDismiss);
                }
            }
            OrchestratorRequest::RecordOutcome { pill_id, outcome } => {
                self.on_outcome(pill_id, outcome);
            }
            OrchestratorRequest::ResetLearning => {
                self.on_reset_learning();
            }
            OrchestratorRequest::DeepenPill {
                pill_id,
                parent_text,
                speaker_id,
                block_target_id,
            } => {
                self.on_deepen_pill(pill_id, parent_text, speaker_id, block_target_id)
                    .await;
            }
            OrchestratorRequest::DeepenThought { thought_id } => {
                self.on_deepen_thought(thought_id).await;
            }
            OrchestratorRequest::SetRabbitResonance {
                thought_id,
                resonant,
            } => {
                self.on_set_rabbit_resonance(thought_id, resonant).await;
            }
            OrchestratorRequest::EditorPolish {
                scene_id,
                block_id,
                block_text,
            } => {
                self.on_editor_polish(scene_id, block_id, block_text).await;
            }
            OrchestratorRequest::Shutdown => {}
        }
    }

    /// v8: apply a renderer-reported lifecycle event to the learning
    /// store. Click is non-terminal and only marks the attribution;
    /// Pin / Dismiss / Evict are terminal — they write a reward and
    /// drop the row. After every terminal we refresh the in-memory
    /// tuning so the *next* tick sees the updated sensitivity.
    fn on_outcome(&mut self, pill_id: Id, outcome: OutcomeSignal) {
        let key = pill_id.as_str().to_string();
        let Some(attr) = self.attribution.get_mut(&key) else {
            // No attribution row — either the renderer is firing
            // outcome events for a pill that emerged before v8 was
            // live, or attribution was already drained. Either way:
            // silently ignore so we don't double-count or panic.
            return;
        };
        match outcome {
            OutcomeSignal::Click => {
                attr.clicked = true;
                return;
            }
            OutcomeSignal::Pin => {
                let trigger_id = attr.trigger_id.clone();
                let mode = attr.mode;
                self.attribution.remove(&key);
                self.record_reward(&trigger_id, PillOutcome::Pin, mode);
            }
            OutcomeSignal::Dismiss => {
                let trigger_id = attr.trigger_id.clone();
                let mode = attr.mode;
                self.attribution.remove(&key);
                self.record_reward(&trigger_id, PillOutcome::Dismiss, mode);
            }
            OutcomeSignal::Evict => {
                let trigger_id = attr.trigger_id.clone();
                let mode = attr.mode;
                let clicked = attr.clicked;
                self.attribution.remove(&key);
                // A clicked-but-not-pinned eviction counts as
                // engagement (writer opened it, didn't dismiss). A
                // never-touched eviction is the soft negative.
                let resolved = if clicked {
                    PillOutcome::Click
                } else {
                    PillOutcome::Evict
                };
                self.record_reward(&trigger_id, resolved, mode);
            }
        }
    }

    /// Phase 6 — build the rich prompt context the assemblers
    /// consume. Returns owned strings + an `as_borrowed()` method
    /// so the assembler can borrow `&str` slices without us
    /// fighting the lifetimes through async-await boundaries.
    ///
    /// `speaker_id` lets us look up a character-track speaker's
    /// compact sheet. Persona ids resolve to no compact (the
    /// persona's own prompt already says everything).
    async fn build_owned_prompt_context(&self, speaker_id: &str) -> OwnedPromptContext {
        let scene = self.scene.clone();
        let mut owned = OwnedPromptContext::default();
        if let Some(s) = scene.as_ref() {
            owned.scene_ordering = s.scene_ordering;
            owned.manuscript_scene_count = s.manuscript_scene_count;
            if let (Some(o), Some(t)) = (s.scene_ordering, s.manuscript_scene_count) {
                owned.arc_position = Some(
                    water_core::orchestrator::arc::arc_position(o, t),
                );
            }
            if let Some(pov_id) = s.pov_character_id.as_ref() {
                if let Some(row) = self
                    .characters
                    .list()
                    .iter()
                    .find(|r| r.id == *pov_id)
                {
                    owned.pov_character_name = Some(row.name.clone());
                }
            }
            if let Some(loc_id) = s.location_id.as_ref() {
                if let Some(entry) = self.world_registry.by_id(loc_id) {
                    owned.location_name = Some(entry.name.clone());
                }
            }
            // Recent resonance — newest 3 picks in this scene. Skip the
            // db-lock acquisition entirely when contended; the prompt
            // simply omits the line for this tick.
            if let Ok(db_guard) = self.db.try_lock() {
                let store = water_core::RabbitStore::new(&db_guard);
                if let Ok(rows) = store.recent_resonant(&s.id, 3) {
                    owned.recent_resonance =
                        rows.into_iter().map(|t| t.message).collect();
                }
            }
        }
        // Character compact — only when the speaker is a known
        // character. Personas pass through.
        if let Some(row) = self.characters.list().iter().find(|r| r.id.as_str() == speaker_id)
        {
            let compact = water_core::character::character_compact(&row.data);
            if !compact.is_empty() {
                owned.character_compact = Some(compact);
            }
        }
        owned
    }

    /// Phase 4 — open the rabbit hole on a clicked pill. Creates a
    /// root thought in `rabbit_thought`, then dispatches a fan_4
    /// LLM call. Subsequent fans (writer clicks a child) route
    /// through `on_deepen_thought` instead.
    ///
    /// Every early-return path emits `deepen:failed` so the
    /// renderer-side DeepenPanel never spins forever — the panel
    /// shows the "model declined" empty state and the writer can
    /// close it cleanly. Common cases: pill not in service-side
    /// list (cold-start restart), no scene context, or no LLM
    /// provider configured.
    async fn on_deepen_pill(
        &mut self,
        pill_id: Id,
        parent_text: String,
        speaker_id: String,
        block_target_id: Option<String>,
    ) {
        if parent_text.trim().is_empty() {
            // Renderer should never send an empty-text deepen, but
            // defend so we surface the right reason if it does.
            self.emit_deepen_failed(pill_id.as_str(), "pill has no text yet");
            return;
        }
        let Some(scene) = self.scene.clone() else {
            self.emit_deepen_failed(pill_id.as_str(), "no scene loaded");
            return;
        };
        // Persist the root inside a short db lock; release before
        // we hit the LLM await.
        let root_id = {
            let db_guard = self.db.lock().await;
            let store = RabbitStore::new(&db_guard);
            match store.insert_root(RabbitRootInsert {
                scene_id: scene.id.clone(),
                speaker_kind: RabbitSpeakerKind::Persona,
                speaker_id: speaker_id.clone(),
                message: parent_text.clone(),
            }) {
                Ok(id) => id,
                Err(e) => {
                    tracing::warn!(error = %e, "deepen_pill: insert_root failed");
                    self.emit_deepen_failed(
                        pill_id.as_str(),
                        &format!("persist failed: {e}"),
                    );
                    return;
                }
            }
        };
        // From this point on, `root_id` is the deepen-panel's
        // parent_id. spawn_fan_4 will emit deepen:ready /
        // deepen:failed against it.
        self.spawn_fan_4(
            root_id,
            speaker_id,
            parent_text,
            block_target_id.unwrap_or_default(),
            /* inherit */ false,
        )
        .await;
    }

    /// Emit a `deepen:failed` event keyed by parent id (either a
    /// pill id at level-0 or a rabbit_thought id at deeper levels)
    /// so the renderer can stop spinning + surface a reason.
    fn emit_deepen_failed(&self, parent_id: &str, reason: &str) {
        let _ = emit(
            &self.app,
            "deepen:failed",
            serde_json::json!({
                "parent_id": parent_id,
                "reason": reason,
            }),
        );
    }

    /// Phase 4 — fan four further children from an existing rabbit
    /// thought (writer descended via the deepen panel). Uses
    /// `rabbit_deepen_inherit` so the children stay in the
    /// parent's stance instead of re-fanning the original premise.
    async fn on_deepen_thought(&mut self, thought_id: Id) {
        let (parent_text, speaker_id) = {
            let db_guard = self.db.lock().await;
            let conn = db_guard.conn();
            let r: rusqlite::Result<(String, String)> = conn.query_row(
                "SELECT message, speaker_id FROM rabbit_thought WHERE id = ?1",
                rusqlite::params![thought_id.as_str()],
                |r| Ok((r.get(0)?, r.get(1)?)),
            );
            match r {
                Ok(v) => v,
                Err(_) => {
                    tracing::debug!(
                        thought = thought_id.as_str(),
                        "deepen_thought: thought not found"
                    );
                    self.emit_deepen_failed(thought_id.as_str(), "thought not found");
                    return;
                }
            }
        };
        self.spawn_fan_4(
            thought_id,
            speaker_id,
            parent_text,
            String::new(),
            /* inherit */ true,
        )
        .await;
    }

    /// Phase 5.8 — dispatch a one-paragraph LLM polish call.
    /// Throttles via the per-scene cap + per-block cooldown so
    /// rapid autosaves don't burn the LLM budget. On success,
    /// persists the response as an editor_pill row with
    /// `rule = editor_polish` and emits `editor_pills:updated` so
    /// the diagnostics tab + inline-underline pipeline pick it up.
    async fn on_editor_polish(
        &mut self,
        scene_id: Id,
        block_id: String,
        block_text: String,
    ) {
        if block_text.trim().is_empty() {
            return;
        }
        // Per-scene cap.
        let scene_key = scene_id.as_str().to_string();
        let used = *self.polish_pass_counts.get(&scene_key).unwrap_or(&0);
        if used >= POLISH_PASS_CAP_PER_SCENE {
            tracing::debug!(scene = %scene_key, used, "polish cap reached; skipping");
            return;
        }
        // Per-block cooldown.
        let now = std::time::Instant::now();
        let block_key = (scene_key.clone(), block_id.clone());
        if let Some(prev) = self.polish_last_at.get(&block_key) {
            if now.duration_since(*prev).as_secs() < POLISH_PER_BLOCK_COOLDOWN_SECS {
                return;
            }
        }
        // Editor persona is the speaker for every polish call.
        let Some(speaker): Option<SpeakerArc> = self.personas.by_id("editor") else {
            tracing::warn!("editor persona missing; polish skipped");
            return;
        };
        let owned_ctx = self.build_owned_prompt_context("editor").await;
        let ctx = owned_ctx.as_borrowed();
        let Ok(prompt) =
            assemble_editor_polish(&self.prompts, &*speaker, &block_text, &ctx)
        else {
            tracing::warn!("editor_polish prompt assembly failed");
            return;
        };
        let router_arc = {
            let g = self.router.lock().await;
            g.clone()
        };
        let Some(router_arc) = router_arc else {
            tracing::debug!("no LlmRouter configured; skipping polish dispatch");
            return;
        };

        // Reserve a slot now — incrementing pessimistically. If the
        // LLM call fails downstream we don't refund (the cap is a
        // session-cost guard, not a strict accounting).
        self.polish_pass_counts
            .entry(scene_key.clone())
            .and_modify(|n| *n += 1)
            .or_insert(1);
        self.polish_last_at.insert(block_key, now);

        let app = self.app.clone();
        let db = self.db.clone();
        let prompt_system = prompt.system.clone();
        let prompt_user = prompt.user.clone();
        let scene_id_str = scene_key.clone();
        let block_id_for_persist = block_id.clone();
        let block_text_for_persist = block_text.clone();
        tokio::spawn(async move {
            let raw = match router_arc
                .generate_raw_with_default(prompt_system.clone(), prompt_user.clone())
                .await
            {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!(error = %e, "editor_polish LLM call failed");
                    return;
                }
            };
            let trimmed = raw.trim();
            if trimmed == "PASS" || trimmed.is_empty() {
                // Model declined; the spec'd silence path.
                return;
            }
            // Persist. The store is idempotent on (scene, block,
            // message) so a duplicate response is a no-op.
            let db_guard = db.lock().await;
            let store = water_core::editor::EditorPillStore::new(&db_guard);
            let parsed_scene_id =
                match water_core::Id::from_str(&scene_id_str) {
                    Ok(id) => id,
                    Err(_) => return,
                };
            if let Err(e) = store.insert_polish(
                &parsed_scene_id,
                &block_id_for_persist,
                &block_text_for_persist,
                trimmed,
            ) {
                tracing::warn!(error = %e, "editor_polish insert failed");
                return;
            }
            drop(db_guard);
            let _ = emit(
                &app,
                "editor_pills:updated",
                serde_json::json!({
                    "scene_id": scene_id_str,
                    "count": 0_u32,  // renderer refetches by id; count is informational
                }),
            );
        });
    }

    /// Phase 4 — toggle resonance. Fire-and-forget DB write; any
    /// failure logs but does not surface to the renderer, since
    /// the renderer can re-issue if the next read of the tree
    /// doesn't show the flag flipped.
    async fn on_set_rabbit_resonance(&mut self, thought_id: Id, resonant: bool) {
        let db_guard = self.db.lock().await;
        let store = RabbitStore::new(&db_guard);
        if let Err(e) = store.set_resonance(&thought_id, resonant) {
            tracing::warn!(error = %e, "set_rabbit_resonance failed");
        }
    }

    /// Shared LLM dispatch for both the initial fan from a pill
    /// (inherit=false → `rabbit_fan_4` prompt) and subsequent fans
    /// from an existing thought (inherit=true → `rabbit_deepen_inherit`).
    /// Spawns a task; emits `deepen:ready` on success or
    /// `deepen:failed` on parse / LLM error.
    async fn spawn_fan_4(
        &mut self,
        parent_id: Id,
        speaker_id: String,
        parent_text: String,
        block_target_id: String,
        inherit: bool,
    ) {
        let Some(speaker): Option<SpeakerArc> = self.personas.by_id(&speaker_id) else {
            tracing::warn!(speaker = %speaker_id, "deepen: persona not found");
            self.emit_deepen_failed(parent_id.as_str(), "persona not found");
            return;
        };
        let scene_excerpt = self.scene_excerpt_for(&block_target_id);
        let owned_ctx = self.build_owned_prompt_context(&speaker_id).await;
        let ctx = owned_ctx.as_borrowed();
        let prompt = if inherit {
            assemble_rabbit_deepen_inherit(
                &self.prompts,
                &*speaker,
                &parent_text,
                &scene_excerpt,
                &ctx,
            )
        } else {
            assemble_rabbit_fan_4(&self.prompts, &*speaker, &parent_text, &scene_excerpt, &ctx)
        };
        let Ok(prompt) = prompt else {
            tracing::warn!("deepen prompt assembly failed");
            self.emit_deepen_failed(parent_id.as_str(), "prompt assembly failed");
            return;
        };
        let router_arc = {
            let g = self.router.lock().await;
            g.clone()
        };
        let Some(router_arc) = router_arc else {
            tracing::debug!("no LlmRouter configured; skipping deepen dispatch");
            self.emit_deepen_failed(
                parent_id.as_str(),
                "no LLM provider configured — open Settings → Providers and Test one",
            );
            return;
        };

        let app = self.app.clone();
        let db = self.db.clone();
        let prompt_system = prompt.system.clone();
        let prompt_user = prompt.user.clone();
        let parent_id_clone = parent_id.clone();
        tokio::spawn(async move {
            #[derive(serde::Deserialize, Clone)]
            struct Item {
                direction: String,
                text: String,
            }

            let items: Vec<Item> = match router_arc
                .generate_structured_with_default(prompt_system.clone(), prompt_user.clone())
                .await
            {
                Ok(v) => v,
                Err(e) => {
                    tracing::warn!(error = %e, "deepen LLM call failed");
                    let _ = emit(
                        &app,
                        "deepen:failed",
                        serde_json::json!({
                            "parent_id": parent_id_clone.as_str(),
                            "reason": e.to_string(),
                        }),
                    );
                    return;
                }
            };

            // Map response → ChildInsert in fan order. We don't
            // require all four directions to be present (model may
            // drop one); we trust the model's order otherwise.
            let mut children_in = Vec::with_capacity(items.len());
            for it in &items {
                children_in.push(RabbitChildInsert {
                    speaker_kind: RabbitSpeakerKind::Persona,
                    speaker_id: speaker_id.clone(),
                    message: it.text.clone(),
                    direction: RabbitDirection::from_str(&it.direction),
                });
            }

            let new_ids = {
                let db_guard = db.lock().await;
                let store = RabbitStore::new(&db_guard);
                match store.insert_children(&parent_id_clone, &children_in) {
                    Ok(ids) => ids,
                    Err(e) => {
                        tracing::warn!(error = %e, "deepen: insert_children failed");
                        let _ = emit(
                            &app,
                            "deepen:failed",
                            serde_json::json!({
                                "parent_id": parent_id_clone.as_str(),
                                "reason": format!("persist: {e}"),
                            }),
                        );
                        return;
                    }
                }
            };

            let children_out: Vec<serde_json::Value> = new_ids
                .iter()
                .zip(items.iter())
                .map(|(id, it)| {
                    serde_json::json!({
                        "id": id.as_str(),
                        "direction": it.direction,
                        "text": it.text,
                    })
                })
                .collect();
            let _ = emit(
                &app,
                "deepen:ready",
                serde_json::json!({
                    "parent_id": parent_id_clone.as_str(),
                    "children": children_out,
                }),
            );
        });
    }

    /// v8: drop every trigger_feedback row and reset the in-memory
    /// tuning back to defaults. Also clears the attribution map so
    /// in-flight pills don't try to record against the just-cleared
    /// table after their lifecycle event lands.
    fn on_reset_learning(&mut self) {
        let Ok(db_guard) = self.db.try_lock() else {
            tracing::warn!("reset_learning: db lock contended; skipping");
            return;
        };
        let store = FeedbackStore::new(&db_guard);
        if let Err(e) = store.reset() {
            tracing::warn!(error = %e, "failed to reset trigger_feedback");
            return;
        }
        drop(db_guard);
        self.tuning = TriggerTuning::default();
        self.attribution.clear();
    }

    /// Write one reward observation and refresh `self.tuning` from
    /// the DB so subsequent triggers see the updated sensitivity.
    /// Errors are logged but never panic — the learning loop is
    /// best-effort and must never block pill emission.
    fn record_reward(&mut self, trigger_id: &str, outcome: PillOutcome, mode: WriterMode) {
        let db_arc = self.db.clone();
        // SQLite work is synchronous + short; the orchestrator loop
        // is on its own task so try_lock is fine. If contention
        // hits (heat compute holding the lock), skip this tick;
        // the writer's next outcome event will re-attempt.
        let Ok(db_guard) = db_arc.try_lock() else {
            return;
        };
        let store = FeedbackStore::new(&db_guard);
        if let Err(e) = store.record_outcome(trigger_id, outcome, mode) {
            tracing::warn!(error = %e, trigger = %trigger_id, "failed to record pill outcome");
            return;
        }
        match store.load_sensitivities() {
            Ok(map) => {
                self.tuning = TriggerTuning::new(map);
            }
            Err(e) => {
                tracing::warn!(error = %e, "failed to reload tuning after outcome");
            }
        }
    }

    /// M5: recompute the three local-only heat metrics (Pacing,
    /// Presence, WorldRefs) after a SceneState arrival, write them to
    /// `HeatStore`, update the trigger-facing `analysis.heat_*_tail`
    /// fields, and emit `heat:updated` so the renderer refetches.
    ///
    /// LLM-backed metrics (Valence + Coherence) are NOT computed here
    /// in M5 v1 — they require the per-paragraph budget-gated dispatch
    /// path that lands in the M5 follow-up. The renderer's strip
    /// renders empty tracks for those metrics until that work lands.
    fn recompute_heat_local(
        &mut self,
        scene_id: water_core::Id,
        characters_present: Vec<water_core::Id>,
        body: String,
    ) {
        use water_core::heat::{
            compute_entity_mentions, compute_pacing, partition, Entity, HeatMetricKind,
            HeatStore,
        };

        let paragraphs = partition(&body);
        if paragraphs.is_empty() {
            return;
        }
        let paragraph_count = paragraphs.len() as u32;

        // Build the character + world entity lists once.
        let character_entities: Vec<Entity> = characters_present
            .iter()
            .filter_map(|cid| {
                self.characters.list().iter().find(|row| &row.id == cid).map(|row| {
                    let aliases = row
                        .data
                        .get("main")
                        .and_then(|m| m.get("aliases"))
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|v| v.as_str().map(str::to_string))
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default();
                    let mut names = vec![row.name.clone()];
                    names.extend(aliases);
                    Entity { names }
                })
            })
            .collect();
        let world_entities: Vec<Entity> = self
            .world_registry
            .entries_by_segment_slug("locations")
            .into_iter()
            .map(|snap| {
                let mut names = vec![snap.name.clone()];
                names.extend(snap.aliases.clone());
                Entity { names }
            })
            .collect();

        // Pull typing history (best-effort).
        let history = read_typing_history(&self.db, &scene_id);

        // Compute local metrics.
        let pacing_scores = compute_pacing(&history, paragraph_count);
        let presence_scores =
            compute_entity_mentions(&paragraphs, &character_entities, true);
        let world_refs_scores =
            compute_entity_mentions(&paragraphs, &world_entities, false);

        // Write batches + update trigger-facing tails. Hold the db
        // lock briefly per kind so the orchestrator's other handlers
        // don't starve.
        let writes: [(HeatMetricKind, &[f32]); 3] = [
            (HeatMetricKind::Pacing, pacing_scores.as_slice()),
            (HeatMetricKind::Presence, presence_scores.as_slice()),
            (HeatMetricKind::WorldRefs, world_refs_scores.as_slice()),
        ];
        let db = self.db.clone();
        let app = self.app.clone();
        // Pre-bake the rows per kind so the spawned task doesn't need
        // to re-derive paragraph hashes.
        let rows_per_kind: Vec<(HeatMetricKind, Vec<(u32, f32, String)>)> = writes
            .iter()
            .map(|(kind, scores)| {
                let rows: Vec<(u32, f32, String)> = scores
                    .iter()
                    .enumerate()
                    .filter_map(|(ix, val)| {
                        let p = paragraphs.get(ix)?;
                        Some((ix as u32, *val, p.text_hash.clone()))
                    })
                    .collect();
                (*kind, rows)
            })
            .collect();
        let scene_id_for_task = scene_id.clone();
        tokio::spawn(async move {
            let g = db.lock().await;
            let store = HeatStore::new(&g);
            for (kind, rows) in &rows_per_kind {
                if rows.is_empty() {
                    continue;
                }
                let borrowed: Vec<(u32, f32, &str)> = rows
                    .iter()
                    .map(|(ix, v, h)| (*ix, *v, h.as_str()))
                    .collect();
                let _ = store.write_batch(&scene_id_for_task, *kind, &borrowed);
            }
            drop(g);
            let _ = emit(
                &app,
                "heat:updated",
                serde_json::json!({ "scene_id": scene_id_for_task.as_str() }),
            );
        });

        // Update trigger-facing tails synchronously off the freshly-
        // computed local metrics. Tail = average of last 3 paragraphs.
        // Coherence isn't computed yet — leave its tail at None.
        self.analysis.heat_pace_tail = tail_average(&pacing_scores, 3);
        // heat_coherence_tail intentionally left None until LLM-side
        // compute lands.
    }

    /// Sidecar bridge — spawn a non-blocking analyze request for the
    /// just-finished paragraph. Three gates:
    ///   1. sidecar is `Some` (failed-boot projects stay dormant);
    ///   2. there's a current scene to attribute the response to
    ///      (no scene → don't even mint the request);
    ///   3. the block hasn't been analyzed in the last
    ///      `BLOCK_ANALYZE_DEBOUNCE` (paragraphs don't change every
    ///      idle pulse, but they fire one anyway).
    /// On success the spawned task re-enters the loop with
    /// `BlockAnalysis`, which patches `self.analysis` so the NEXT
    /// telemetry tick's trigger evaluation sees the fresh metrics.
    fn maybe_kick_sidecar_analyze(&mut self, block_id: &str, text: &str) {
        let Some(sidecar) = self.sidecar.clone() else {
            return;
        };
        let Some(scene) = self.scene.as_ref() else {
            return;
        };
        let now = std::time::Instant::now();
        if let Some(last) = self.block_analyze_throttle.get(block_id) {
            if now.duration_since(*last) < BLOCK_ANALYZE_DEBOUNCE {
                return;
            }
        }
        // Drop empty paragraphs on the floor — they'd just round-trip
        // a uniform default response and burn CPU.
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return;
        }
        self.block_analyze_throttle
            .insert(block_id.to_string(), now);
        let scene_id = scene.id.clone();
        let block_id_owned = block_id.to_string();
        let text_owned = trimmed.to_string();
        let tx = self.self_tx.clone();
        tokio::spawn(async move {
            let req = water_core::ipc::AnalyzeRequest {
                text: text_owned,
                scene_id: scene_id.as_str().to_string(),
            };
            match sidecar.analyze(&req).await {
                Ok(response) => {
                    let _ = tx
                        .send(OrchestratorRequest::BlockAnalysis {
                            scene_id,
                            block_id: block_id_owned,
                            response,
                        })
                        .await;
                }
                Err(e) => {
                    // Sidecar may be transiently unhealthy
                    // (supervisor will surface it through the
                    // `sidecar:status` event). Log + move on; next
                    // idle pulse will retry once the debounce
                    // expires.
                    tracing::debug!(error = %e, "sidecar analyze failed");
                }
            }
        });
    }

    /// Sidecar bridge — patch in a fresh per-paragraph analysis result.
    /// Scene-level fields take the latest values (a recent paragraph is
    /// a better proxy for "the writer's current state" than an
    /// arithmetic average across the scene). `block_metrics[block_id]`
    /// is upserted so `block_anchored_drift` can read its
    /// paragraph-specific divergence/coherence. `valence_history`
    /// appends, bounded by `VALENCE_HISTORY_CAP` so a long writing
    /// session doesn't grow the vector unboundedly.
    ///
    /// `scene_id` guard: if the response arrives after a scene switch,
    /// drop it on the floor rather than poison the active scene's
    /// snapshot with stale signal.
    fn on_block_analysis(
        &mut self,
        scene_id: Id,
        block_id: String,
        response: water_core::ipc::AnalyzeResponse,
    ) {
        if let Some(scene) = self.scene.as_ref() {
            if scene.id != scene_id {
                return;
            }
        } else {
            return;
        }
        apply_block_analysis(&mut self.analysis, block_id, &response);
    }

    async fn on_telemetry(&mut self, t: TypingTelemetry, last_block_text: Option<String>) {
        // Apply the renderer's idle-pulse block text BEFORE evaluating
        // triggers so `character_dissonance` (which reads
        // `analysis.last_block_text`) sees the latest paragraph the writer
        // paused on. During typing bursts the renderer sends `None`; we
        // preserve the prior value in that case so a trigger that fired on
        // the previous idle pulse can still gate against it.
        if let Some(text) = last_block_text {
            // Sidecar bridge: kick a non-blocking analyze for this
            // paragraph BEFORE moving the text into the snapshot. The
            // response (debounced + per-block-throttled) lands in the
            // loop later as `BlockAnalysis` and patches scene-level
            // flow/coherence/divergence/pace/intensity/valence plus
            // `block_metrics[block_id]`, lifting the five sidecar-
            // dependent triggers (block_anchored_drift, topic_drift,
            // pace_floor, valence_spike, scene_flow_dip) out of the
            // dormant `AnalysisSnapshot::default()` state.
            self.maybe_kick_sidecar_analyze(&t.block_id, &text);
            self.analysis.last_block_text = Some(text);
        }
        // Gate 1: never surface mid-sentence (spec § 6.1).
        if t.cursor_classification == CursorClassification::MidSentence {
            return;
        }
        // Gate 2: need a scene snapshot to evaluate triggers.
        let Some(scene) = self.scene.clone() else {
            return;
        };

        // Gate 3: global pill-emission interval. Even if a trigger fires
        // and Stage 2 confirms, don't dispatch faster than
        // `MIN_PILL_INTERVAL_MS` so the writer has time to read each
        // pill. This is independent of per-speaker cooldowns (which
        // are 45-90s each) — without it, the writer can see a new pill
        // from a different speaker every idle pulse (~3s), which the
        // M4 smoke walk surfaced as too fast to read.
        if let Some(last) = self.last_pill_emit_at {
            if last.elapsed().as_millis() < u128::from(MIN_PILL_INTERVAL_MS) {
                return;
            }
        }

        // Highest-priority candidate among the 10 built-in triggers.
        let Some(cand) = pick_best_trigger(
            &t,
            &self.analysis,
            &scene,
            &self.project,
            &self.characters,
            &self.world_registry,
            &self.prompts,
            &self.tuning,
        ) else {
            return;
        };

        // Stage 2 (M3 T11): if the candidate carries a
        // `requires_confirmation` request, run a small yes/no LLM call
        // before dispatching the level-0 pill. ~150 tokens in, 1 token
        // out. Non-"yes" responses (and provider errors) drop the
        // candidate silently. All three branches (yes / no / error)
        // append to the replay log when configured so the eval harness
        // can audit Stage-2 decisions independently of Stage-1 lemma
        // gating.
        if let Some(req) = cand.requires_confirmation.as_ref() {
            // Brief lock: clone the optional router Arc and drop the
            // guard before the LLM await. Mirrors the level-0/expand
            // pattern below so we never hold `self.router` across an
            // await.
            let router_arc = {
                let g = self.router.lock().await;
                g.clone()
            };
            let Some(router_arc) = router_arc else {
                tracing::debug!(
                    trigger = %cand.trigger_id,
                    "no LlmRouter configured; skipping confirmation candidate"
                );
                return;
            };
            if !run_stage2_confirmation(
                &router_arc,
                req,
                &cand.trigger_id,
                self.replay_log.as_deref(),
            )
            .await
            {
                return;
            }
        }

        // Voice-route the candidate (cooldown-respecting, POV-prefer for
        // character-track triggers). `None` means every relevant speaker
        // is cooled down — skip this tick.
        let Some(speaker) = route_with_chars(
            &cand,
            &self.personas,
            &self.characters,
            &scene,
            &self.cooldowns,
            std::time::Instant::now(),
        ) else {
            return;
        };

        // Assemble the level-0 prompt. ~400-char window centered on the
        // anchored block (or scene start when there's no anchor).
        let scene_excerpt = self.scene_excerpt_for(&t.block_id);
        let owned_ctx = self.build_owned_prompt_context(speaker.id()).await;
        let ctx = owned_ctx.as_borrowed();
        let prompt = match assemble_level_0(
            &self.prompts,
            &*speaker,
            &cand.trigger_id,
            &scene_excerpt,
            &ctx,
        ) {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(error = %e, trigger = %cand.trigger_id, "prompt assembly failed");
                return;
            }
        };

        // Snapshot router + filter chain for the spawned LLM task. The
        // filter chain is built fresh per call because `Box<dyn PostFilter>`
        // is not Clone — cheap to rebuild (regex compilation happens once
        // per pattern, but the patterns are bounded by `tone.toml`).
        let router_arc = {
            let g = self.router.lock().await;
            g.clone()
        };
        let Some(router_arc) = router_arc else {
            tracing::debug!("no LlmRouter configured; skipping pill dispatch");
            return;
        };
        let filters: Vec<Box<dyn PostFilter>> =
            builtin_post_filters(&self.prompts.tone.blacklist_regex.patterns);

        // Mint the pill + record the speaker cooldown synchronously so a
        // burst of telemetry ticks doesn't pile up emits before any LLM
        // response lands.
        let speaker_id = speaker.id().to_string();
        let trigger_id = cand.trigger_id.to_string();
        let hue = speaker.hue_token().to_string();
        let block_target_id = cand.block_target_id.clone();
        let pill = Pill::new_generating(
            speaker_id.clone(),
            trigger_id.clone(),
            block_target_id.clone(),
            None,
        );
        let pill_id_str = pill.id.as_str().to_string();
        self.pills.push(pill);
        self.cooldowns.note_emit(&speaker_id);
        self.last_pill_emit_at = Some(std::time::Instant::now());

        // v8: record attribution at emerge time. Mode is classified
        // from the telemetry tick that fired this pill — captured
        // here so terminal-event rewards stay anchored to *that
        // moment*, not the (potentially much later) moment of
        // interaction.
        let writer_mode = classify_writer_mode(&t);
        self.attribution.insert(
            pill_id_str.clone(),
            PillAttribution {
                trigger_id: trigger_id.clone(),
                mode: writer_mode,
                clicked: false,
            },
        );

        // Eviction: pick_evictee operates on `OnScreen` pills only. The
        // service currently never transitions service-side pills out of
        // `Generating` (the renderer is source of truth for what's on
        // screen), so this is effectively a no-op today. Kept here so the
        // plumbing is in place for when service-side lifecycle catches up
        // with the renderer (see KNOWN_FRAGILE).
        if let Some(idx) = pick_evictee(&self.pills) {
            let evicted = self.pills.remove(idx);
            let _ = emit(
                &self.app,
                "pill:evicted",
                serde_json::json!({ "pill_id": evicted.id.as_str() }),
            );
        }

        let app = self.app.clone();
        let replay_log = self.replay_log.clone();
        // Clone the prompt strings up front so we can both (a) hand the
        // owned strings to `generate_raw_with_default` and (b) keep
        // copies to log alongside the response below. The `LlmProvider`
        // trait takes `String` by value, so we'd lose the originals
        // otherwise.
        let prompt_system = prompt.system.clone();
        let prompt_user = prompt.user.clone();
        let log_trigger_id = trigger_id.clone();
        tokio::spawn(async move {
            // Request row (kind = trigger id, e.g. "missing_sensory"). We
            // log unconditionally before the LLM call so a panic or
            // hang inside the provider still leaves a request breadcrumb.
            if let Some(log) = replay_log.as_ref() {
                let _ = log.append(&ReplayEntry {
                    ts: chrono::Utc::now().to_rfc3339(),
                    kind: &log_trigger_id,
                    request_system: &prompt_system,
                    request_user: &prompt_user,
                    response_raw: None,
                    post_filter_decision: None,
                    anti_loop_overlap: None,
                });
            }

            let raw = match router_arc
                .generate_raw_with_default(prompt_system.clone(), prompt_user.clone())
                .await
            {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!(error = %e, "level-0 LLM call failed");
                    let _ = emit(
                        &app,
                        "pill:dismissed",
                        serde_json::json!({ "pill_id": pill_id_str }),
                    );
                    return;
                }
            };

            // Decide final post-filter outcome, then both emit and log.
            // We compute the decision string ("pass", "drop:pass-sentinel",
            // "drop:<filter-id>:<reason>") before any early-return so the
            // log entry is always written when a raw response landed.
            let decision: String = if raw.trim() == "PASS" {
                "drop:pass-sentinel".to_string()
            } else {
                let mut d = "pass".to_string();
                for f in &filters {
                    if let FilterDecision::Drop { reason } = f.evaluate(&raw) {
                        d = format!("drop:{}:{}", f.id(), reason);
                        break;
                    }
                }
                d
            };

            if let Some(log) = replay_log.as_ref() {
                let _ = log.append(&ReplayEntry {
                    ts: chrono::Utc::now().to_rfc3339(),
                    kind: "response",
                    request_system: &prompt_system,
                    request_user: &prompt_user,
                    response_raw: Some(&raw),
                    post_filter_decision: Some(&decision),
                    anti_loop_overlap: None,
                });
            }

            // The "PASS" sentinel (spec § 8.1) lets the model decline to
            // speak. Treat trimmed PASS as an explicit dismissal.
            if decision == "drop:pass-sentinel" {
                let _ = emit(
                    &app,
                    "pill:dismissed",
                    serde_json::json!({ "pill_id": pill_id_str }),
                );
                return;
            }
            if decision != "pass" {
                tracing::info!(decision = %decision, "post-filter dropped pill");
                let _ = emit(
                    &app,
                    "pill:dismissed",
                    serde_json::json!({ "pill_id": pill_id_str }),
                );
                return;
            }
            let payload = serde_json::json!({
                "pill_id": pill_id_str,
                "speaker_id": speaker_id,
                "hue_token": hue,
                "text": raw.trim(),
                "block_target_id": block_target_id,
                "trigger_id": trigger_id,
            });
            let _ = emit(&app, "pill:emerged", payload);
        });
    }

    async fn on_expand(&mut self, parent_pill_id: Id, regenerate: bool) {
        // Resolve the parent pill from the service-side list. If it isn't
        // there (e.g. the service was restarted after the pill was emitted),
        // we can still proceed using the pinned-pill row as a fallback —
        // but M2 ships without that path. For now, miss = no-op.
        let Some(parent) = self.pills.iter().find(|p| p.id == parent_pill_id).cloned() else {
            tracing::debug!(
                parent = parent_pill_id.as_str(),
                "expand: parent pill not found in service-side list"
            );
            return;
        };
        let parent_text = parent.text.clone().unwrap_or_default();
        let Some(speaker): Option<SpeakerArc> = self.personas.by_id(&parent.speaker_id) else {
            tracing::warn!(
                speaker = parent.speaker_id.as_str(),
                "expand: persona not found"
            );
            return;
        };
        let scene_excerpt =
            self.scene_excerpt_for(&parent.block_target_id.clone().unwrap_or_default());

        let history_key = parent_pill_id.as_str().to_string();
        let prior_first_words: Vec<String> = {
            let h = self.bouquet_history.lock().await;
            h.get(&history_key)
                .cloned()
                .unwrap_or_default()
                .iter()
                .map(|t| t.split_whitespace().take(8).collect::<Vec<_>>().join(" "))
                .collect()
        };

        let owned_ctx = self.build_owned_prompt_context(speaker.id()).await;
        let ctx = owned_ctx.as_borrowed();
        let prompt = if regenerate {
            assemble_pill_regenerate(
                &self.prompts,
                &*speaker,
                &parent.trigger_id,
                &parent_text,
                &scene_excerpt,
                &prior_first_words,
                &ctx,
            )
        } else {
            assemble_pill_expand(
                &self.prompts,
                &*speaker,
                &parent.trigger_id,
                &parent_text,
                &scene_excerpt,
                &ctx,
            )
        };
        let Ok(prompt) = prompt else {
            tracing::warn!(trigger = %parent.trigger_id, "expand prompt assembly failed");
            return;
        };

        let router_arc = {
            let g = self.router.lock().await;
            g.clone()
        };
        let Some(router_arc) = router_arc else {
            tracing::debug!("no LlmRouter configured; skipping expand dispatch");
            return;
        };

        let app = self.app.clone();
        let threshold = speaker.anti_loop_threshold();
        let history_arc = self.bouquet_history.clone();
        let replay_log = self.replay_log.clone();
        let prompt_system = prompt.system.clone();
        let prompt_user = prompt.user.clone();
        // Distinct kinds so the audit can tell expand vs regenerate apart.
        let log_kind: &'static str = if regenerate {
            "pill_regenerate"
        } else {
            "pill_expand"
        };

        tokio::spawn(async move {
            #[derive(serde::Deserialize, Clone)]
            struct Item {
                angle: String,
                text: String,
            }

            if let Some(log) = replay_log.as_ref() {
                let _ = log.append(&ReplayEntry {
                    ts: chrono::Utc::now().to_rfc3339(),
                    kind: log_kind,
                    request_system: &prompt_system,
                    request_user: &prompt_user,
                    response_raw: None,
                    post_filter_decision: None,
                    anti_loop_overlap: None,
                });
            }

            let items: Vec<Item> = match router_arc
                .generate_structured_with_default(prompt_system.clone(), prompt_user.clone())
                .await
            {
                Ok(v) => v,
                Err(e) => {
                    tracing::warn!(error = %e, "expand LLM call failed");
                    if let Some(log) = replay_log.as_ref() {
                        let err_str = e.to_string();
                        let _ = log.append(&ReplayEntry {
                            ts: chrono::Utc::now().to_rfc3339(),
                            kind: "response",
                            request_system: &prompt_system,
                            request_user: &prompt_user,
                            response_raw: None,
                            post_filter_decision: Some(&format!("drop:error:{err_str}")),
                            anti_loop_overlap: None,
                        });
                    }
                    let _ = emit(
                        &app,
                        "pill:dismissed",
                        serde_json::json!({ "pill_id": history_key }),
                    );
                    return;
                }
            };

            // Anti-loop: drop any variant whose overlap with prior bouquet
            // texts exceeds the speaker's threshold (spec § 6.5). If we
            // run out of candidates the renderer gets an empty bouquet —
            // acceptable degeneration per spec § 9.3.
            let priors = {
                let h = history_arc.lock().await;
                h.get(&history_key).cloned().unwrap_or_default()
            };
            // Track the max overlap across all candidate variants so the
            // replay log captures how close-to-loop this round trip was.
            let mut max_seen_overlap: f32 = 0.0;
            let mut accepted: Vec<Item> = Vec::with_capacity(3);
            for it in items.iter() {
                let ov = max_overlap(&it.text, &priors);
                if ov > max_seen_overlap {
                    max_seen_overlap = ov;
                }
                if ov < threshold {
                    accepted.push(it.clone());
                }
                if accepted.len() >= 3 {
                    break;
                }
            }

            // Response row. We serialize the original `items` (before
            // anti-loop filtering) so the audit can see what the model
            // produced, and stash the post-filter decision as
            // `pass`/`drop:anti_loop:all` to mirror level-0 semantics.
            if let Some(log) = replay_log.as_ref() {
                let raw_items_json = serde_json::to_string(
                    &items
                        .iter()
                        .map(|i| serde_json::json!({ "angle": i.angle, "text": i.text }))
                        .collect::<Vec<_>>(),
                )
                .unwrap_or_else(|_| "[]".to_string());
                let decision = if accepted.is_empty() && !items.is_empty() {
                    "drop:anti_loop:all".to_string()
                } else {
                    "pass".to_string()
                };
                let _ = log.append(&ReplayEntry {
                    ts: chrono::Utc::now().to_rfc3339(),
                    kind: "response",
                    request_system: &prompt_system,
                    request_user: &prompt_user,
                    response_raw: Some(&raw_items_json),
                    post_filter_decision: Some(&decision),
                    anti_loop_overlap: Some(max_seen_overlap),
                });
            }

            // Record accepted texts into history so future regenerates can
            // push the model further away from them.
            {
                let mut h = history_arc.lock().await;
                h.entry(history_key.clone())
                    .or_default()
                    .extend(accepted.iter().map(|i| i.text.clone()));
            }

            let items_json: Vec<serde_json::Value> = accepted
                .iter()
                .enumerate()
                .map(|(i, it)| {
                    serde_json::json!({
                        "sub_pill_id": format!("{}-{}", history_key, i + 1),
                        "angle": it.angle,
                        "text": it.text,
                    })
                })
                .collect();
            let _ = emit(
                &app,
                "bouquet:ready",
                serde_json::json!({
                    "parent_pill_id": history_key,
                    "items": items_json,
                }),
            );
        });
    }
}

/// Evaluate the 10 built-in triggers and return the highest-priority
/// candidate, if any. Extracted so on_telemetry stays readable.
#[allow(clippy::too_many_arguments)]
fn pick_best_trigger(
    t: &TypingTelemetry,
    analysis: &AnalysisSnapshot,
    scene: &SceneSnapshot,
    project: &ProjectSnapshot,
    characters: &CharacterRegistry,
    world_registry: &WorldRegistry,
    prompts: &PromptLibrary,
    tuning: &TriggerTuning,
) -> Option<TriggerCandidate> {
    let ctx = TriggerContext {
        telemetry: t,
        analysis,
        scene,
        project,
        characters,
        world_registry,
        prompts,
        tuning,
    };
    let triggers = builtin_triggers();
    triggers
        .iter()
        .filter_map(|trig| trig.evaluate(&ctx))
        .max_by(|a, b| {
            a.priority
                .partial_cmp(&b.priority)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
}

/// Run the Stage-2 confirmation LLM call for a candidate's
/// `ConfirmationRequest`. Returns `true` when the model answered "yes"
/// (case-insensitive, after trim), `false` for any other outcome
/// (non-"yes" string, provider error). Independent of `AppHandle` /
/// `OrchestratorService` so it can be unit-tested with a `CannedProvider`
/// / `ErrorProvider`.
///
/// Replay-log entries:
/// - `confirmation_yes` — model confirmed; pill dispatch proceeds.
/// - `confirmation_no`  — model declined; candidate dropped.
/// - `confirmation_error` — provider failed; candidate dropped.
///
/// `kind` for all rows is taken from `req.kind` (e.g.
/// `"pill_dissonance_check"`) so the audit can filter by confirmation
/// flavor.
async fn run_stage2_confirmation(
    router: &Arc<LlmRouter>,
    req: &ConfirmationRequest,
    trigger_id: &str,
    replay_log: Option<&ReplayLog>,
) -> bool {
    let confirmation_system = req.system.clone();
    let confirmation_user = req.user.clone();
    let confirmation_kind = req.kind.clone();
    let raw = match router
        .generate_raw_with_default(confirmation_system.clone(), confirmation_user.clone())
        .await
    {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(
                error = %e,
                trigger = %trigger_id,
                "confirmation LLM call failed; dropping candidate"
            );
            if let Some(log) = replay_log {
                let _ = log.append(&ReplayEntry {
                    ts: chrono::Utc::now().to_rfc3339(),
                    kind: &confirmation_kind,
                    request_system: &confirmation_system,
                    request_user: &confirmation_user,
                    response_raw: None,
                    post_filter_decision: Some("confirmation_error"),
                    anti_loop_overlap: None,
                });
            }
            return false;
        }
    };
    let confirmed = raw.trim().to_ascii_lowercase().starts_with("yes");
    if let Some(log) = replay_log {
        let _ = log.append(&ReplayEntry {
            ts: chrono::Utc::now().to_rfc3339(),
            kind: &confirmation_kind,
            request_system: &confirmation_system,
            request_user: &confirmation_user,
            response_raw: Some(&raw),
            post_filter_decision: Some(if confirmed {
                "confirmation_yes"
            } else {
                "confirmation_no"
            }),
            anti_loop_overlap: None,
        });
    }
    if !confirmed {
        tracing::debug!(
            trigger = %trigger_id,
            response = %raw.trim(),
            "confirmation said no; dropping candidate"
        );
    }
    confirmed
}

/// Parse a string into an `Id` for cross-process boundaries. Wraps
/// `Id::from_str` with a `String` error so Tauri command shims can
/// `?`-propagate.
pub fn parse_id(s: &str) -> Result<Id, String> {
    Id::from_str(s).map_err(|e| e.to_string())
}

#[cfg(test)]
mod sidecar_bridge_tests {
    //! Unit tests for the sidecar→orchestrator bridge. The full
    //! end-to-end (telemetry → analyze → BlockAnalysis → patch)
    //! requires a Tauri `AppHandle` + a live mpsc channel, but the
    //! patching logic is pure — covered here directly.
    use super::*;
    use water_core::ipc::AnalyzeResponse;

    fn sample(valence: f64) -> AnalyzeResponse {
        AnalyzeResponse {
            word_count: 12,
            flow: 0.7,
            coherence: 0.6,
            engagement: 0.5,
            divergence: 0.3,
            pace: 0.45,
            intensity: 0.55,
            valence,
            status: "normal".into(),
        }
    }

    #[test]
    fn patches_scene_level_fields_and_inserts_block_metrics() {
        let mut analysis = AnalysisSnapshot::default();
        apply_block_analysis(&mut analysis, "bk-1".into(), &sample(0.4));
        // Scene-level fields populated.
        assert!((analysis.flow.unwrap() - 0.7).abs() < 1e-3);
        assert!((analysis.coherence.unwrap() - 0.6).abs() < 1e-3);
        assert!((analysis.divergence.unwrap() - 0.3).abs() < 1e-3);
        assert!((analysis.pace.unwrap() - 0.45).abs() < 1e-3);
        assert!((analysis.valence.unwrap() - 0.4).abs() < 1e-3);
        // Per-block row keyed by the just-finished block.
        let bm = analysis.block_metrics.get("bk-1").expect("block row");
        assert!((bm.divergence.unwrap() - 0.3).abs() < 1e-3);
        // valence_history starts at length 1.
        assert_eq!(analysis.valence_history.len(), 1);
    }

    #[test]
    fn valence_history_caps_at_constant() {
        let mut analysis = AnalysisSnapshot::default();
        for i in 0..(VALENCE_HISTORY_CAP + 5) {
            apply_block_analysis(&mut analysis, format!("bk-{i}"), &sample(i as f64 / 100.0));
        }
        assert_eq!(analysis.valence_history.len(), VALENCE_HISTORY_CAP);
        // The drained head is the OLDEST values; tail should be the
        // last reading.
        let tail = *analysis.valence_history.last().unwrap();
        let expected_tail = (VALENCE_HISTORY_CAP + 4) as f32 / 100.0;
        assert!((tail - expected_tail).abs() < 1e-3);
    }

    #[test]
    fn upserting_same_block_replaces_row() {
        let mut analysis = AnalysisSnapshot::default();
        apply_block_analysis(&mut analysis, "bk-1".into(), &sample(0.1));
        apply_block_analysis(
            &mut analysis,
            "bk-1".into(),
            &AnalyzeResponse {
                divergence: 0.9,
                ..sample(0.1)
            },
        );
        // One row, latest values.
        assert_eq!(analysis.block_metrics.len(), 1);
        let bm = analysis.block_metrics.get("bk-1").unwrap();
        assert!((bm.divergence.unwrap() - 0.9).abs() < 1e-3);
    }
}

#[cfg(test)]
mod stage2_confirmation_tests {
    //! Unit tests for `run_stage2_confirmation`, the M3 T11 Stage-2
    //! dispatch helper. The full end-to-end flow inside
    //! `OrchestratorService::on_telemetry` depends on a Tauri `AppHandle`
    //! for event emission; testing that surface requires a `tauri::test`
    //! harness that does not yet exist in this crate (zero existing tests
    //! in `app/src-tauri/src/`). The plan's T11 STOP criterion ("Setting
    //! up a confirmation-firing fixture requires >50 lines of helper code")
    //! applies, so we test the dispatch-decision helper directly. The
    //! helper carries all three branches (yes / no / error) plus the
    //! replay-log writes — i.e., everything observable from outside the
    //! `tokio::spawn` that follows level-0 dispatch.
    //!
    //! Phase D's Tauri-command tests will exercise the on_telemetry
    //! surface end-to-end against a mock AppHandle once that scaffolding
    //! lands.
    use super::*;
    use water_core::llm::{CannedProvider, ErrorProvider, LlmProvider, LlmRouter};
    use water_core::orchestrator::ConfirmationRequest;
    use water_core::replay_log::ReplayLog;

    fn make_req() -> ConfirmationRequest {
        ConfirmationRequest {
            system: "sys".to_string(),
            user: "usr".to_string(),
            kind: "pill_dissonance_check".to_string(),
        }
    }

    fn router_with(provider: Arc<dyn LlmProvider>) -> Arc<LlmRouter> {
        Arc::new(LlmRouter::new(vec![provider]))
    }

    #[tokio::test]
    async fn yes_response_returns_true_and_logs_confirmation_yes() {
        // "yes" (any case, after trim) is the only response that allows
        // dispatch to proceed.
        let provider = Arc::new(CannedProvider::with_response("yes\n")) as Arc<dyn LlmProvider>;
        let router = router_with(provider);
        let tmp = tempfile::TempDir::new().unwrap();
        let log = Arc::new(ReplayLog::open(tmp.path(), "t11-yes").unwrap());

        let proceed = run_stage2_confirmation(
            &router,
            &make_req(),
            "character_dissonance",
            Some(log.as_ref()),
        )
        .await;

        assert!(proceed, "Stage-2 yes must allow dispatch");
        let body = std::fs::read_to_string(
            tmp.path()
                .join(".water")
                .join("log")
                .join("llm")
                .join("t11-yes.jsonl"),
        )
        .unwrap();
        assert!(
            body.contains("\"post_filter_decision\":\"confirmation_yes\""),
            "replay log should record confirmation_yes; got: {body}"
        );
        assert!(
            body.contains("\"kind\":\"pill_dissonance_check\""),
            "replay log kind should be the confirmation kind"
        );
    }

    #[tokio::test]
    async fn yes_uppercase_is_still_accepted() {
        // Case-insensitive, leading whitespace tolerated.
        let provider =
            Arc::new(CannedProvider::with_response("  YES, definitely.")) as Arc<dyn LlmProvider>;
        let router = router_with(provider);
        let proceed =
            run_stage2_confirmation(&router, &make_req(), "character_dissonance", None).await;
        assert!(proceed);
    }

    #[tokio::test]
    async fn no_response_returns_false_and_logs_confirmation_no() {
        let provider = Arc::new(CannedProvider::with_response("no")) as Arc<dyn LlmProvider>;
        let router = router_with(provider);
        let tmp = tempfile::TempDir::new().unwrap();
        let log = Arc::new(ReplayLog::open(tmp.path(), "t11-no").unwrap());

        let proceed = run_stage2_confirmation(
            &router,
            &make_req(),
            "character_dissonance",
            Some(log.as_ref()),
        )
        .await;

        assert!(!proceed, "Stage-2 no must drop the candidate");
        let body = std::fs::read_to_string(
            tmp.path()
                .join(".water")
                .join("log")
                .join("llm")
                .join("t11-no.jsonl"),
        )
        .unwrap();
        assert!(
            body.contains("\"post_filter_decision\":\"confirmation_no\""),
            "replay log should record confirmation_no; got: {body}"
        );
    }

    #[tokio::test]
    async fn arbitrary_non_yes_response_drops_candidate() {
        // Anything that doesn't start with "yes" after trim+lowercase
        // is treated as no. A model that hedges or refuses ("Hmm, maybe
        // not.") must drop.
        let provider =
            Arc::new(CannedProvider::with_response("Hmm, maybe not.")) as Arc<dyn LlmProvider>;
        let router = router_with(provider);
        let proceed =
            run_stage2_confirmation(&router, &make_req(), "character_dissonance", None).await;
        assert!(!proceed);
    }

    #[tokio::test]
    async fn provider_error_returns_false_and_logs_confirmation_error() {
        let provider = Arc::new(ErrorProvider::new()) as Arc<dyn LlmProvider>;
        let router = router_with(provider);
        let tmp = tempfile::TempDir::new().unwrap();
        let log = Arc::new(ReplayLog::open(tmp.path(), "t11-err").unwrap());

        let proceed = run_stage2_confirmation(
            &router,
            &make_req(),
            "character_dissonance",
            Some(log.as_ref()),
        )
        .await;

        assert!(
            !proceed,
            "provider error during confirmation must drop the candidate"
        );
        let body = std::fs::read_to_string(
            tmp.path()
                .join(".water")
                .join("log")
                .join("llm")
                .join("t11-err.jsonl"),
        )
        .unwrap();
        assert!(
            body.contains("\"post_filter_decision\":\"confirmation_error\""),
            "replay log should record confirmation_error; got: {body}"
        );
        assert!(
            body.contains("\"response_raw\":null"),
            "error path should have null response_raw"
        );
    }

    #[tokio::test]
    async fn missing_replay_log_does_not_panic() {
        // Production default: WATER_REPLAY_LOG unset, replay_log = None.
        // Helper must still return the correct dispatch decision.
        let yes_provider = Arc::new(CannedProvider::with_response("yes")) as Arc<dyn LlmProvider>;
        let proceed = run_stage2_confirmation(
            &router_with(yes_provider),
            &make_req(),
            "character_dissonance",
            None,
        )
        .await;
        assert!(proceed);

        let err_provider = Arc::new(ErrorProvider::new()) as Arc<dyn LlmProvider>;
        let drop = run_stage2_confirmation(
            &router_with(err_provider),
            &make_req(),
            "character_dissonance",
            None,
        )
        .await;
        assert!(!drop);
    }
}
