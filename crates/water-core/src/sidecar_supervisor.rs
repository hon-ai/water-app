//! Watches a [`Sidecar`] and emits status changes.

use crate::sidecar::Sidecar;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{watch, Notify};
use tokio::time::{interval, Duration, MissedTickBehavior};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SidecarStatus {
    Loading,
    Ready,
    Error,
}

#[derive(Debug, Clone, Serialize)]
pub struct SidecarStatusEvent {
    pub status: SidecarStatus,
    pub detail: Option<String>,
}

pub struct SidecarSupervisor {
    tx: watch::Sender<SidecarStatusEvent>,
    stop: Arc<Notify>,
}

impl SidecarSupervisor {
    /// Start the supervisor loop with the given health-poll interval.
    ///
    /// On consecutive health failures the supervisor sleeps with exponential
    /// backoff (1s, 2s, 5s, 10s, then 30s repeating) before retrying. The
    /// backoff counter resets to 0 on any successful health response, and the
    /// supervisor NEVER permanently gives up — only `stop()` (or runtime
    /// shutdown) ends the loop. See `KNOWN_FRAGILE` #6.
    #[must_use]
    pub fn start(
        sidecar: Arc<Sidecar>,
        poll_interval: Duration,
    ) -> (Self, watch::Receiver<SidecarStatusEvent>) {
        let (tx, rx) = watch::channel(SidecarStatusEvent {
            status: SidecarStatus::Loading,
            detail: None,
        });
        let stop = Arc::new(Notify::new());
        let stop_clone = stop.clone();
        let tx_clone = tx.clone();
        tokio::spawn(async move {
            let mut consecutive_failures = 0u32;
            let mut ticker = interval(poll_interval);
            // Without this, ticks accumulate as a burst after each backoff
            // sleep, which would defeat the spacing the backoff is designed
            // to enforce. `Delay` measures the next tick from the moment the
            // previous one completes.
            ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
            loop {
                tokio::select! {
                    _ = ticker.tick() => {
                        match sidecar.health().await {
                            Ok(_) => {
                                consecutive_failures = 0;
                                // Send Ready on first success, and again any
                                // time we transition from a non-Ready state.
                                if tx_clone.borrow().status != SidecarStatus::Ready {
                                    let _ = tx_clone.send(SidecarStatusEvent {
                                        status: SidecarStatus::Ready,
                                        detail: None,
                                    });
                                }
                            }
                            Err(e) => {
                                consecutive_failures += 1;
                                let _ = tx_clone.send(SidecarStatusEvent {
                                    status: SidecarStatus::Error,
                                    detail: Some(format!("{e}")),
                                });
                                // Exponential backoff, capped at 30s. The
                                // supervisor never gives up — we just slow
                                // down retries so a crashed/missing sidecar
                                // doesn't hammer the OS.
                                let backoff_secs: u64 = match consecutive_failures {
                                    1 => 1,
                                    2 => 2,
                                    3 => 5,
                                    4 => 10,
                                    _ => 30,
                                };
                                tokio::select! {
                                    () = tokio::time::sleep(Duration::from_secs(backoff_secs)) => {}
                                    () = stop_clone.notified() => { return; }
                                }
                            }
                        }
                    }
                    () = stop_clone.notified() => { break; }
                }
            }
        });
        (Self { tx, stop }, rx)
    }

    pub fn stop(&self) {
        self.stop.notify_waiters();
    }

    #[must_use]
    pub fn current(&self) -> SidecarStatusEvent {
        self.tx.borrow().clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sidecar::Sidecar;
    use std::sync::Arc;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn supervisor_reports_ready_when_health_succeeds() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/health"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "status": "ready", "version": "0.1.0", "uptime_seconds": 1.0, "pid": 1
            })))
            .mount(&server)
            .await;
        let sc = Arc::new(Sidecar::external(server.uri()));
        let (sup, mut rx) = SidecarSupervisor::start(sc, Duration::from_secs(5));
        // Wait up to 8 seconds for the first ready event (interval is 5s).
        let evt = tokio::time::timeout(Duration::from_secs(8), async {
            loop {
                rx.changed().await.unwrap();
                if rx.borrow().status == SidecarStatus::Ready {
                    return rx.borrow().clone();
                }
            }
        })
        .await
        .unwrap();
        assert_eq!(evt.status, SidecarStatus::Ready);
        sup.stop();
    }

    #[tokio::test]
    async fn supervisor_reports_error_after_health_failures() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/health"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;
        let sc = Arc::new(Sidecar::external(server.uri()));
        let (sup, mut rx) = SidecarSupervisor::start(sc, Duration::from_secs(5));
        let evt = tokio::time::timeout(Duration::from_secs(8), async {
            loop {
                rx.changed().await.unwrap();
                if rx.borrow().status == SidecarStatus::Error {
                    return rx.borrow().clone();
                }
            }
        })
        .await
        .unwrap();
        assert_eq!(evt.status, SidecarStatus::Error);
        sup.stop();
    }

    /// Server returns 500 every time. With the old 3-strike-break the
    /// supervisor would emit one final Error then stop forever. With the
    /// backoff implementation it must keep retrying through 1s/2s/5s/10s/30s
    /// windows, producing repeated Error transitions.
    #[tokio::test(start_paused = true)]
    async fn supervisor_uses_exponential_backoff_on_consecutive_failures() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/health"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;
        let sc = Arc::new(Sidecar::external(server.uri()));
        let (sup, mut rx) = SidecarSupervisor::start(sc, Duration::from_millis(100));
        let mut errors_seen = 0;
        // 8 windows of 35s each — past every backoff cap multiple times.
        // `tokio::time::timeout` wraps `rx.changed()` so iterations where the
        // watch value did not change (supervisor still mid-backoff in real
        // time) don't deadlock the loop.
        for _ in 0..8 {
            tokio::time::advance(Duration::from_secs(35)).await;
            let changed = tokio::time::timeout(
                Duration::from_millis(50),
                rx.changed(),
            )
            .await;
            if changed.is_ok() && matches!(rx.borrow().status, SidecarStatus::Error) {
                errors_seen += 1;
            }
        }
        assert!(
            errors_seen >= 3,
            "expected repeated Error status across backoff, got {errors_seen}",
        );
        sup.stop();
    }

    /// First 2 health calls fail, then the server starts succeeding. The
    /// supervisor must reset its backoff window on success and reach Ready.
    #[tokio::test(start_paused = true)]
    async fn supervisor_resets_backoff_on_success() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static N: AtomicUsize = AtomicUsize::new(0);
        // Reset counter in case the test binary re-runs this test or shares
        // process state with other static-using tests.
        N.store(0, Ordering::SeqCst);
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/health"))
            .respond_with(|_: &wiremock::Request| {
                let n = N.fetch_add(1, Ordering::SeqCst);
                if n < 2 {
                    ResponseTemplate::new(500)
                } else {
                    ResponseTemplate::new(200).set_body_json(serde_json::json!({
                        "status": "ready",
                        "version": "0.1.0",
                        "uptime_seconds": 1.0,
                        "pid": 1,
                    }))
                }
            })
            .mount(&server)
            .await;
        let sc = Arc::new(Sidecar::external(server.uri()));
        let (sup, mut rx) = SidecarSupervisor::start(sc, Duration::from_millis(100));
        let mut saw_ready = false;
        // Two failures cost 1s + 2s of simulated backoff, then a success.
        // Under `start_paused = true` plus real wiremock I/O, the supervisor
        // task makes about one health attempt per `advance(2s)` cycle, so we
        // need ~10 iterations to walk through: 2 fails → 1 success. The
        // 50ms `timeout` wrapper around `rx.changed()` keeps the loop
        // progressing on iterations where no new value was sent (the watch
        // version hasn't advanced) — this matters because health I/O is
        // real-clock and may not complete before the simulated advance
        // returns.
        for _ in 0..20 {
            tokio::time::advance(Duration::from_secs(2)).await;
            let _ = tokio::time::timeout(Duration::from_millis(50), rx.changed()).await;
            if matches!(rx.borrow().status, SidecarStatus::Ready) {
                saw_ready = true;
                break;
            }
        }
        assert!(
            saw_ready,
            "supervisor should reach Ready after intermittent failures",
        );
        sup.stop();
    }
}
