//! Watches a [`Sidecar`] and emits status changes.

use crate::sidecar::Sidecar;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{watch, Notify};
use tokio::time::{interval, Duration};

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
    #[must_use]
    pub fn spawn(sidecar: Arc<Sidecar>) -> (Self, watch::Receiver<SidecarStatusEvent>) {
        let (tx, rx) = watch::channel(SidecarStatusEvent {
            status: SidecarStatus::Loading,
            detail: None,
        });
        let stop = Arc::new(Notify::new());
        let stop_clone = stop.clone();
        let tx_clone = tx.clone();
        tokio::spawn(async move {
            let mut consecutive_failures = 0u32;
            let mut ticker = interval(Duration::from_secs(5));
            loop {
                tokio::select! {
                    _ = ticker.tick() => {
                        match sidecar.health().await {
                            Ok(_) => {
                                if consecutive_failures > 0 {
                                    let _ = tx_clone.send(SidecarStatusEvent {
                                        status: SidecarStatus::Ready,
                                        detail: None,
                                    });
                                }
                                consecutive_failures = 0;
                                // Send Ready once at start, too.
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
                                if consecutive_failures >= 3 {
                                    let _ = tx_clone.send(SidecarStatusEvent {
                                        status: SidecarStatus::Error,
                                        detail: Some("sidecar unhealthy after 3 attempts".into()),
                                    });
                                    break;
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
        let (sup, mut rx) = SidecarSupervisor::spawn(sc);
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
        let (sup, mut rx) = SidecarSupervisor::spawn(sc);
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
}
