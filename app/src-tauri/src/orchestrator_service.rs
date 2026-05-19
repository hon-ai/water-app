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
    AnalysisSnapshot, CursorClassification, ProjectSnapshot, SceneSnapshot, TriggerContext,
    TriggerCandidate, TypingTelemetry,
};
use water_core::post_filter::{builtin_post_filters, FilterDecision, PostFilter};
use water_core::prompts::{
    assemble_level_0, assemble_pill_expand, assemble_pill_regenerate, PromptLibrary,
};
use water_core::replay_log::{ReplayEntry, ReplayLog};
use water_core::voice::registry::PersonaRegistry;
use water_core::voice::router::{route, CooldownState};
use water_core::voice::speaker::SpeakerArc;
use water_core::Id;

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
    Telemetry(TypingTelemetry),
    Analysis(AnalysisSnapshot),
    /// Scene + project snapshot AND the full scene body text. The
    /// orchestrator caches the body so each telemetry tick can build a
    /// prompt excerpt without re-reading from disk.
    SceneState(SceneSnapshot, ProjectSnapshot, String),
    Expand { parent_pill_id: Id },
    Regenerate { parent_pill_id: Id },
    Dismiss { pill_id: Id },
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
    prompts: Arc<PromptLibrary>,
    pills: Vec<Pill>,
    cooldowns: CooldownState,
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
        project_root: PathBuf,
    ) -> OrchestratorHandle {
        // Channel depth 64 was picked to comfortably absorb a burst of
        // typing-telemetry ticks (renderer fires ~one per keystroke). If
        // the queue ever fills, `send` waits instead of dropping — that's
        // fine because telemetry isn't latency-critical and back-pressure
        // naturally rate-limits the renderer.
        let (tx, mut rx) = mpsc::channel::<OrchestratorRequest>(64);
        let prompts = Arc::new(
            PromptLibrary::load_builtin().expect("built-in prompts must load at startup"),
        );

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
            prompts,
            pills: Vec::new(),
            cooldowns: CooldownState::default(),
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
            OrchestratorRequest::Telemetry(t) => self.on_telemetry(t).await,
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
                self.scene = Some(s);
                self.project = p;
                self.scene_text = text;
            }
            OrchestratorRequest::Expand { parent_pill_id } => {
                self.on_expand(parent_pill_id, false).await;
            }
            OrchestratorRequest::Regenerate { parent_pill_id } => {
                self.on_expand(parent_pill_id, true).await;
            }
            OrchestratorRequest::Dismiss { pill_id } => {
                if let Some(p) = self.pills.iter_mut().find(|p| p.id == pill_id) {
                    p.state = water_core::orchestrator::state::transition(
                        p,
                        &PillEvent::UserDismiss,
                    );
                }
            }
            OrchestratorRequest::Shutdown => {}
        }
    }

    async fn on_telemetry(&mut self, t: TypingTelemetry) {
        // Gate 1: never surface mid-sentence (spec § 6.1).
        if t.cursor_classification == CursorClassification::MidSentence {
            return;
        }
        // Gate 2: need a scene snapshot to evaluate triggers.
        let Some(scene) = self.scene.clone() else {
            return;
        };

        // Highest-priority candidate among the 10 built-in triggers.
        // M3 T7: empty character registry stub. T8 wires `SceneState` to
        // load the project's registry and stores it on the service; until
        // then `character_dissonance` correctly evaluates to None because
        // there are no characters present in the registry to gate against.
        let characters = CharacterRegistry::empty();
        let Some(cand) = pick_best_trigger(&t, &self.analysis, &scene, &self.project, &characters)
        else {
            return;
        };

        // Voice-route the candidate (cooldown-respecting). `None` means
        // every relevant speaker is cooled down — skip this tick.
        let Some(speaker) = route(
            &cand,
            &self.personas,
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
            cand.trigger_id,
            &scene_excerpt,
        ) {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(error = %e, trigger = cand.trigger_id, "prompt assembly failed");
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
                .map(|t| {
                    t.split_whitespace()
                        .take(8)
                        .collect::<Vec<_>>()
                        .join(" ")
                })
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
) -> Option<TriggerCandidate> {
    let ctx = TriggerContext {
        telemetry: t,
        analysis,
        scene,
        project,
        characters,
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

/// Parse a string into an `Id` for cross-process boundaries. Wraps
/// `Id::from_str` with a `String` error so Tauri command shims can
/// `?`-propagate.
pub fn parse_id(s: &str) -> Result<Id, String> {
    Id::from_str(s).map_err(|e| e.to_string())
}

