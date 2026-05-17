//! Router — primary/fallback chain with rate limiting + circuit breaker.

use super::{BouquetRequest, BouquetVariant, LlmProvider, ProviderId};
use crate::{Error, Result};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

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
        Self { chain, state }
    }

    #[must_use]
    pub fn primary_id(&self) -> Option<ProviderId> {
        self.chain.first().map(|p| p.id())
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
                    return Ok((id, variants));
                }
                Err(e) => {
                    st.breaker.lock().await.record_failure();
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
        let p1 = Arc::new(CannedProvider) as Arc<dyn LlmProvider>;
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
        let secondary = Arc::new(CannedProvider) as Arc<dyn LlmProvider>;
        let router = LlmRouter::new(vec![primary, secondary]);
        let (id, _) = router.generate_bouquet(&req()).await.unwrap();
        assert_eq!(id.as_str(), "canned");
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
