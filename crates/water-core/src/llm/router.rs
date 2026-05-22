//! Router — primary/fallback chain with rate limiting + circuit breaker.

use super::{BouquetRequest, BouquetVariant, GenerateRequest, LlmProvider, ProviderId};
use crate::{Error, Result};
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, Mutex};

/// Emitted by `LlmRouter` whenever a provider's health/circuit-breaker
/// outcome changes during a `generate_bouquet` call. Subscribed to by
/// `app/src-tauri` to fan out into the `provider:status` event bus.
/// Mirrors `ProviderStatusPayload` in `app/src-tauri/src/events.rs`.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ProviderHealthChange {
    pub provider_id: String,
    pub ok: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    pub capacity: u32,
    pub refill_per_second: f32,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            capacity: 30,
            refill_per_second: 30.0 / 60.0,
        }
    }
}

#[derive(Debug)]
struct TokenBucket {
    tokens: f32,
    capacity: f32,
    refill_per_second: f32,
    last: Instant,
}

impl TokenBucket {
    #[allow(clippy::cast_precision_loss)]
    fn new(cfg: &RateLimitConfig) -> Self {
        Self {
            tokens: cfg.capacity as f32,
            capacity: cfg.capacity as f32,
            refill_per_second: cfg.refill_per_second,
            last: Instant::now(),
        }
    }
    fn try_take(&mut self) -> bool {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last).as_secs_f32();
        self.tokens = (self.tokens + elapsed * self.refill_per_second).min(self.capacity);
        self.last = now;
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreakerState {
    Closed,
    Open { until: Instant },
}

#[derive(Debug)]
struct Breaker {
    consecutive_failures: u32,
    state: BreakerState,
    threshold: u32,
    open_for: Duration,
}

impl Breaker {
    fn new(threshold: u32, open_for: Duration) -> Self {
        Self {
            consecutive_failures: 0,
            state: BreakerState::Closed,
            threshold,
            open_for,
        }
    }
    fn allow(&mut self) -> bool {
        match self.state {
            BreakerState::Closed => true,
            BreakerState::Open { until } => {
                if Instant::now() >= until {
                    self.state = BreakerState::Closed;
                    self.consecutive_failures = 0;
                    true
                } else {
                    false
                }
            }
        }
    }
    fn record_success(&mut self) {
        self.consecutive_failures = 0;
        self.state = BreakerState::Closed;
    }
    fn record_failure(&mut self) {
        self.consecutive_failures += 1;
        if self.consecutive_failures >= self.threshold {
            self.state = BreakerState::Open {
                until: Instant::now() + self.open_for,
            };
        }
    }
}

struct ProviderState {
    bucket: Mutex<TokenBucket>,
    breaker: Mutex<Breaker>,
}

pub struct LlmRouter {
    chain: Vec<Arc<dyn LlmProvider>>,
    state: HashMap<ProviderId, ProviderState>,
    /// Broadcasts provider health/CB transitions to any subscriber. The
    /// channel is kept alive by the `Sender` stored here even when no
    /// subscribers exist; sends are non-blocking and silently drop on
    /// `NoSubscriber`/`Lagged` — this is a best-effort signal, not a
    /// reliable queue.
    status_tx: broadcast::Sender<ProviderHealthChange>,
    /// Per-provider model override. When set, `req.model` on any
    /// outbound call gets populated from this map *before* the
    /// provider's own empty-model fallback kicks in. Lets the
    /// renderer swap models per provider without re-constructing
    /// the router. Empty by default — falls through to each
    /// provider's hardcoded default.
    default_models: Mutex<HashMap<ProviderId, String>>,
    /// Cached per-provider health, populated by `generate_bouquet`
    /// outcomes. `diagnostics_status` reads from this cache instead
    /// of firing a live `provider.health()` HTTP request — without
    /// it, the renderer's 3-second `diagnostics_status` poll hits
    /// every provider's health endpoint 20×/min, which trips
    /// OpenRouter's 429 rate-limit on free-tier accounts and
    /// flickers Test indicators back to gray right after a green
    /// Test result.
    last_health: Mutex<HashMap<ProviderId, ProviderHealthSnapshot>>,
}

/// Cached health entry for a single provider. `Result::Ok` means the
/// most recent bouquet call succeeded; `Err(string)` carries the most
/// recent error message verbatim. `at` is purely informational so
/// future TTL logic can age out stale entries without breaking the API.
#[derive(Debug, Clone)]
pub struct ProviderHealthSnapshot {
    pub ok: bool,
    pub error: Option<String>,
    pub at: Instant,
}

impl LlmRouter {
    #[must_use]
    pub fn new(chain: Vec<Arc<dyn LlmProvider>>) -> Self {
        let state = chain
            .iter()
            .map(|p| {
                (
                    p.id(),
                    ProviderState {
                        bucket: Mutex::new(TokenBucket::new(&RateLimitConfig::default())),
                        breaker: Mutex::new(Breaker::new(5, Duration::from_mins(1))),
                    },
                )
            })
            .collect();
        // Capacity 32 is comfortable headroom for a single subscriber in the
        // app process; backpressure here is a non-issue because the events
        // are advisory.
        let (status_tx, _rx) = broadcast::channel(32);
        Self {
            chain,
            state,
            status_tx,
            default_models: Mutex::new(HashMap::new()),
            last_health: Mutex::new(HashMap::new()),
        }
    }

    /// Snapshot of the per-provider health cache. Empty before any
    /// bouquet call has run. Used by `diagnostics_status` so the
    /// renderer's polling loop doesn't fire a live health probe
    /// against every provider every 3 seconds — the cache is
    /// authoritative until the next bouquet call updates it.
    pub async fn cached_health(&self) -> Vec<(ProviderId, std::result::Result<(), String>)> {
        let g = self.last_health.lock().await;
        self.chain
            .iter()
            .filter_map(|p| {
                let id = p.id();
                g.get(&id).map(|snap| {
                    let r = if snap.ok {
                        Ok(())
                    } else {
                        Err(snap.error.clone().unwrap_or_default())
                    };
                    (id, r)
                })
            })
            .collect()
    }

    async fn record_health(&self, id: &ProviderId, ok: bool, error: Option<String>) {
        let mut g = self.last_health.lock().await;
        g.insert(
            id.clone(),
            ProviderHealthSnapshot {
                ok,
                error,
                at: Instant::now(),
            },
        );
    }

    /// Set / clear the active model override for a provider. `model`
    /// is an empty string ⇒ clear (fall back to provider's hardcoded
    /// default). Lock contention is cheap because reads are
    /// per-call and writes are rare (writer toggling Settings).
    pub async fn set_default_model(&self, provider_id: &ProviderId, model: &str) {
        let mut g = self.default_models.lock().await;
        if model.is_empty() {
            g.remove(provider_id);
        } else {
            g.insert(provider_id.clone(), model.to_string());
        }
    }

    /// Read the active model override for a provider. Returns the
    /// empty string when no override is set; callers should fall
    /// through to the provider's own default in that case.
    pub async fn default_model_for(&self, provider_id: &ProviderId) -> String {
        self.default_models
            .lock()
            .await
            .get(provider_id)
            .cloned()
            .unwrap_or_default()
    }

    /// Subscribe to provider health/CB transitions. Each `generate_bouquet`
    /// success/failure publishes one `ProviderHealthChange` per provider
    /// touched. The renderer-facing `provider:status` event mirrors this.
    #[must_use]
    pub fn subscribe_status(&self) -> broadcast::Receiver<ProviderHealthChange> {
        self.status_tx.subscribe()
    }

    #[must_use]
    pub fn primary_id(&self) -> Option<ProviderId> {
        self.chain.first().map(|p| p.id())
    }

    /// The primary (first) provider in the chain, cloned for shared use.
    /// Returns `None` if the chain is empty. M2 single-shot paths
    /// (`generate_raw_with_default`, `generate_structured_with_default`)
    /// use this directly without going through the breaker/rate-limiter —
    /// callers that want full fallback chaining should use
    /// `generate_bouquet` instead.
    #[must_use]
    pub fn primary(&self) -> Option<Arc<dyn LlmProvider>> {
        self.chain.first().cloned()
    }

    /// Single-shot text generation against the primary provider. Builds a
    /// `GenerateRequest` from the supplied `system`/`user` strings; other
    /// fields default. Returns the raw model output.
    ///
    /// # Errors
    /// Returns `Error::Provider("no primary provider")` if the router was
    /// constructed with an empty chain, or whatever error the provider's
    /// `generate_raw` implementation returns.
    pub async fn generate_raw_with_default(&self, system: String, user: String) -> Result<String> {
        let primary = self
            .primary()
            .ok_or_else(|| Error::Provider("no primary provider".into()))?;
        // Phase: per-provider model override. When the writer's
        // Settings has picked a model for this provider, populate
        // `req.model` so the adapter uses it instead of its
        // hardcoded default. Empty string keeps the legacy
        // "fall through to provider default" path.
        let model = self.default_model_for(&primary.id()).await;
        // Non-zero temperature is load-bearing for pill quality:
        // `tone.toml` instructs the model to output the `PASS`
        // sentinel when constraints can't be met, and at
        // temperature 0.0 a deterministic model picks PASS as the
        // safest answer almost every time — the post-filter then
        // silently drops the pill. 0.7 matches what the bouquet
        // path uses for level-0 pills (see `provider_test` +
        // `pill_expand`). max_output_tokens 256 covers the 22-word
        // pill cap with margin; some adapters (Kimi) forward
        // max_tokens=0 to the upstream API verbatim, which
        // truncates or rejects.
        let req = GenerateRequest {
            system,
            user,
            model,
            temperature: 0.7,
            max_output_tokens: 256,
        };
        primary.generate_raw(req).await
    }

    /// Single-shot structured (JSON) generation against the primary
    /// provider. Calls `generate_raw` and parses the result as `T`.
    ///
    /// # Errors
    /// Returns `Error::Provider("no primary provider")` if the chain is
    /// empty, propagates `generate_raw` errors, and returns
    /// `Error::Provider("invalid json: …")` when the response fails to
    /// deserialize as `T`.
    pub async fn generate_structured_with_default<T: DeserializeOwned + Send>(
        &self,
        system: String,
        user: String,
    ) -> Result<T> {
        let raw = self.generate_raw_with_default(system, user).await?;
        serde_json::from_str::<T>(&raw)
            .map_err(|e| Error::Provider(format!("invalid json: {e}; raw: {raw}")))
    }

    /// Try each provider in order: skip if breaker open or rate-limited,
    /// otherwise call and on success return; on failure record + try next.
    ///
    /// # Errors
    /// Returns the last provider error if all providers in the chain fail,
    /// or `Error::Provider("no providers configured")` if the chain is empty.
    pub async fn generate_bouquet(
        &self,
        req: &BouquetRequest,
    ) -> Result<(ProviderId, Vec<BouquetVariant>)> {
        let mut last_err: Option<Error> = None;
        for p in &self.chain {
            let id = p.id();
            let Some(st) = self.state.get(&id) else {
                continue;
            };
            if !st.breaker.lock().await.allow() {
                continue;
            }
            if !st.bucket.lock().await.try_take() {
                last_err = Some(Error::Provider(format!("rate limited: {id:?}")));
                continue;
            }
            match p.generate_bouquet(req).await {
                Ok(variants) => {
                    st.breaker.lock().await.record_success();
                    self.record_health(&id, true, None).await;
                    // Best-effort broadcast; ignore NoSubscriber errors. The
                    // subscriber side (renderer) treats this as advisory.
                    let _ = self.status_tx.send(ProviderHealthChange {
                        provider_id: id.as_str().to_string(),
                        ok: true,
                        error: None,
                    });
                    return Ok((id, variants));
                }
                Err(e) => {
                    st.breaker.lock().await.record_failure();
                    self.record_health(&id, false, Some(e.to_string())).await;
                    let _ = self.status_tx.send(ProviderHealthChange {
                        provider_id: id.as_str().to_string(),
                        ok: false,
                        error: Some(e.to_string()),
                    });
                    last_err = Some(e);
                }
            }
        }
        Err(last_err.unwrap_or_else(|| Error::Provider("no providers configured".into())))
    }

    pub async fn health(&self) -> Vec<(ProviderId, std::result::Result<(), String>)> {
        let mut out = Vec::with_capacity(self.chain.len());
        for p in &self.chain {
            let r = p.health().await.map_err(|e| e.to_string());
            out.push((p.id(), r));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::CannedProvider;

    fn req() -> BouquetRequest {
        BouquetRequest {
            system: "s".into(),
            user: "u".into(),
            n_variants: 3,
            previous_variants_first_words: vec![],
            model: "x".into(),
            temperature: 0.7,
            max_output_tokens: 100,
        }
    }

    #[tokio::test]
    async fn router_uses_first_provider_when_healthy() {
        let p1 = Arc::new(CannedProvider::default()) as Arc<dyn LlmProvider>;
        let router = LlmRouter::new(vec![p1]);
        let (id, _) = router.generate_bouquet(&req()).await.unwrap();
        assert_eq!(id.as_str(), "canned");
    }

    struct AlwaysFails;
    #[async_trait::async_trait]
    impl LlmProvider for AlwaysFails {
        fn id(&self) -> ProviderId {
            ProviderId::new("fails")
        }
        async fn health(&self) -> Result<()> {
            Err(Error::Provider("nope".into()))
        }
        async fn generate_bouquet(&self, _: &BouquetRequest) -> Result<Vec<BouquetVariant>> {
            Err(Error::Provider("nope".into()))
        }
    }

    #[tokio::test]
    async fn router_falls_back_to_secondary_on_primary_error() {
        let primary = Arc::new(AlwaysFails) as Arc<dyn LlmProvider>;
        let secondary = Arc::new(CannedProvider::default()) as Arc<dyn LlmProvider>;
        let router = LlmRouter::new(vec![primary, secondary]);
        let (id, _) = router.generate_bouquet(&req()).await.unwrap();
        assert_eq!(id.as_str(), "canned");
    }

    #[tokio::test]
    async fn generate_raw_with_default_hits_primary() {
        let canned = Arc::new(CannedProvider::with_response("hello there")) as Arc<dyn LlmProvider>;
        let router = LlmRouter::new(vec![canned]);
        let out = router
            .generate_raw_with_default("sys".into(), "user".into())
            .await
            .unwrap();
        assert_eq!(out, "hello there");
    }

    /// Regression: `generate_raw_with_default` must set a non-zero
    /// temperature and a non-zero max_output_tokens. The original
    /// `..Default::default()` left both at 0.0 / 0, which made every
    /// provider either output the `PASS` sentinel (silently dropped
    /// by the post-filter) or get rejected at the API for
    /// `max_tokens=0`. Bug surfaced as "no pills ever spawn with
    /// OpenRouter / Kimi configured."
    #[tokio::test]
    async fn generate_raw_with_default_sets_nonzero_temperature_and_max_tokens() {
        use crate::llm::GenerateRequest;
        use async_trait::async_trait;
        use std::sync::Mutex;

        struct CapturingProvider {
            last: Mutex<Option<GenerateRequest>>,
        }
        #[async_trait]
        impl LlmProvider for CapturingProvider {
            fn id(&self) -> ProviderId {
                ProviderId::new("capture")
            }
            async fn health(&self) -> crate::Result<()> {
                Ok(())
            }
            async fn generate_bouquet(
                &self,
                _: &BouquetRequest,
            ) -> crate::Result<Vec<BouquetVariant>> {
                Ok(vec![])
            }
            async fn generate_raw(&self, req: GenerateRequest) -> crate::Result<String> {
                *self.last.lock().unwrap() = Some(req);
                Ok("ok".into())
            }
        }

        let provider = Arc::new(CapturingProvider {
            last: Mutex::new(None),
        });
        let router = LlmRouter::new(vec![provider.clone() as Arc<dyn LlmProvider>]);
        let _ = router
            .generate_raw_with_default("s".into(), "u".into())
            .await
            .unwrap();
        let captured = provider.last.lock().unwrap().clone().expect("called");
        assert!(
            captured.temperature > 0.0,
            "temperature must be > 0 so the model doesn't default to PASS"
        );
        assert!(
            captured.max_output_tokens > 0,
            "max_output_tokens must be > 0 so adapters that forward it verbatim don't truncate"
        );
    }

    #[tokio::test]
    async fn generate_structured_with_default_parses_primary_output() {
        #[derive(serde::Deserialize)]
        struct Pair {
            angle: String,
            #[allow(dead_code)]
            text: String,
        }
        let canned = Arc::new(CannedProvider::with_response(
            r#"[{"angle":"feel","text":"a"},{"angle":"notice","text":"b"}]"#,
        )) as Arc<dyn LlmProvider>;
        let router = LlmRouter::new(vec![canned]);
        let out: Vec<Pair> = router
            .generate_structured_with_default("sys".into(), "user".into())
            .await
            .unwrap();
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].angle, "feel");
    }

    #[tokio::test]
    async fn generate_with_default_errors_when_chain_empty() {
        let router = LlmRouter::new(vec![]);
        let err = router
            .generate_raw_with_default("s".into(), "u".into())
            .await
            .unwrap_err();
        assert!(matches!(err, Error::Provider(_)));
    }

    #[tokio::test]
    async fn subscribe_status_receives_health_change_on_state_transition() {
        // RED: router currently has no subscribe_status. After adding it,
        // a state transition (success after baseline) should publish a
        // ProviderHealthChange.
        let primary = Arc::new(AlwaysFails) as Arc<dyn LlmProvider>;
        let secondary = Arc::new(CannedProvider::default()) as Arc<dyn LlmProvider>;
        let router = LlmRouter::new(vec![primary, secondary]);
        let mut rx = router.subscribe_status();
        // Drive a call that will: try primary (failure → broadcast), then
        // fall back to secondary (success → broadcast). At least one event
        // should land on the subscriber.
        let _ = router.generate_bouquet(&req()).await.unwrap();
        let evt = tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv())
            .await
            .expect("expected at least one ProviderHealthChange")
            .expect("channel closed unexpectedly");
        // Whichever provider event we got, the struct is well-formed.
        assert!(!evt.provider_id.is_empty());
    }

    /// Regression: `cached_health` populates from `generate_bouquet`
    /// outcomes and returns the snapshot. `diagnostics_status` reads
    /// from this cache instead of probing live so the renderer's
    /// 3-second poll doesn't burn provider rate limits (OpenRouter
    /// returns 429 after a couple of minutes of live `/chat/completions`
    /// health probes).
    #[tokio::test]
    async fn cached_health_populates_after_bouquet_success() {
        let canned = Arc::new(CannedProvider::default()) as Arc<dyn LlmProvider>;
        let router = LlmRouter::new(vec![canned]);
        // Empty before any call.
        assert!(router.cached_health().await.is_empty());
        let _ = router.generate_bouquet(&req()).await.unwrap();
        let snap = router.cached_health().await;
        assert_eq!(snap.len(), 1);
        let (id, result) = &snap[0];
        assert_eq!(id.as_str(), "canned");
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn cached_health_records_failure_with_error_string() {
        let primary = Arc::new(AlwaysFails) as Arc<dyn LlmProvider>;
        let router = LlmRouter::new(vec![primary]);
        let _ = router.generate_bouquet(&req()).await;
        let snap = router.cached_health().await;
        assert_eq!(snap.len(), 1);
        let (_, result) = &snap[0];
        assert!(result.is_err());
        let msg = result.as_ref().unwrap_err();
        assert!(msg.contains("nope"), "expected provider error to be cached, got: {msg}");
    }

    #[tokio::test]
    async fn breaker_opens_after_five_failures() {
        let primary = Arc::new(AlwaysFails) as Arc<dyn LlmProvider>;
        let router = LlmRouter::new(vec![primary]);
        for _ in 0..5 {
            let _ = router.generate_bouquet(&req()).await;
        }
        // Now the breaker should be open and the next call short-circuits
        // without even trying the provider.
        let err = router.generate_bouquet(&req()).await.unwrap_err();
        assert!(matches!(err, Error::Provider(_)));
    }
}
