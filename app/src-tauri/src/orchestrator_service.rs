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
    state::{Pill, PillEvent},
    triggers::builtin_triggers,
    AnalysisSnapshot, ConfirmationRequest, CursorClassification, ProjectSnapshot, SceneSnapshot,
    TriggerCandidate, TriggerContext, TypingTelemetry,
};
use water_core::post_filter::{builtin_post_filters, FilterDecision, PostFilter};
use water_core::prompts::{
    assemble_level_0, assemble_pill_expand, assemble_pill_regenerate, PromptLibrary,
};
use water_core::replay_log::{ReplayEntry, ReplayLog};
use water_core::voice::registry::PersonaRegistry;
use water_core::voice::router::{route_with_chars, CooldownState};
use water_core::voice::speaker::SpeakerArc;
use water_core::world::WorldRegistry;
use water_core::Id;

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
    Shutdown,
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
}

impl OrchestratorService {
    /// ~400-char window centered on the anchored block id, or first 400
    /// chars if `block_id` is empty or not present in `scene_text`.
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
                let start = back_to_char_boundary(&self.scene_text, pos.saturating_sub(200));
                let end = forward_to_char_boundary(
                    &self.scene_text,
                    (pos + 200).min(self.scene_text.len()),
                );
                return self.scene_text[start..end].to_string();
            }
        }
        let end = forward_to_char_boundary(&self.scene_text, 400.min(self.scene_text.len()));
        self.scene_text[..end].to_string()
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
    ) -> OrchestratorHandle {
        // Channel depth 64 was picked to comfortably absorb a burst of
        // typing-telemetry ticks (renderer fires ~one per keystroke). If
        // the queue ever fills, `send` waits instead of dropping — that's
        // fine because telemetry isn't latency-critical and back-pressure
        // naturally rate-limits the renderer.
        let (tx, mut rx) = mpsc::channel::<OrchestratorRequest>(64);
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
            OrchestratorRequest::Shutdown => {}
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

    async fn on_telemetry(&mut self, t: TypingTelemetry, last_block_text: Option<String>) {
        // Apply the renderer's idle-pulse block text BEFORE evaluating
        // triggers so `character_dissonance` (which reads
        // `analysis.last_block_text`) sees the latest paragraph the writer
        // paused on. During typing bursts the renderer sends `None`; we
        // preserve the prior value in that case so a trigger that fired on
        // the previous idle pulse can still gate against it.
        if let Some(text) = last_block_text {
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
        let prompt = match assemble_level_0(
            &self.prompts,
            &*speaker,
            &cand.trigger_id,
            &scene_excerpt,
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

        let prompt = if regenerate {
            assemble_pill_regenerate(
                &self.prompts,
                &*speaker,
                &parent.trigger_id,
                &parent_text,
                &scene_excerpt,
                &prior_first_words,
            )
        } else {
            assemble_pill_expand(
                &self.prompts,
                &*speaker,
                &parent.trigger_id,
                &parent_text,
                &scene_excerpt,
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
fn pick_best_trigger(
    t: &TypingTelemetry,
    analysis: &AnalysisSnapshot,
    scene: &SceneSnapshot,
    project: &ProjectSnapshot,
    characters: &CharacterRegistry,
    world_registry: &WorldRegistry,
    prompts: &PromptLibrary,
) -> Option<TriggerCandidate> {
    let ctx = TriggerContext {
        telemetry: t,
        analysis,
        scene,
        project,
        characters,
        world_registry,
        prompts,
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
