//! Sidecar handle — manage the `FastAPI` sidecar process and call it.

use crate::ipc::{AnalyzeRequest, AnalyzeResponse, HealthResponse};
use crate::{Error, Result};
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;

#[derive(Debug, Clone)]
pub struct SidecarSpec {
    pub working_dir: PathBuf, // path containing pyproject.toml
    pub uv_bin: PathBuf,      // path to `uv`
    pub port: u16,            // explicit port; caller picks (avoid 0 for v1)
    pub host: String,         // "127.0.0.1"
    pub boot_timeout: Duration,
}

pub struct Sidecar {
    base_url: String,
    child: Mutex<Option<Child>>,
    http: reqwest::Client,
}

impl Sidecar {
    /// External mode: connect to an already-running sidecar at `base_url`.
    ///
    /// # Panics
    /// Panics if the underlying `reqwest` client fails to build, which should
    /// not happen with the static configuration used here.
    #[must_use]
    pub fn external(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            child: Mutex::new(None),
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(8))
                .build()
                .expect("reqwest client"),
        }
    }

    /// Managed mode: spawn the sidecar and wait for /health.
    ///
    /// # Panics
    /// Panics if the underlying `reqwest` client fails to build, which should
    /// not happen with the static configuration used here.
    pub async fn spawn(spec: SidecarSpec) -> Result<Self> {
        let mut cmd = Command::new(&spec.uv_bin);
        cmd.arg("run")
            .arg("uvicorn")
            .arg("water_sidecar.main:app")
            .arg("--host")
            .arg(&spec.host)
            .arg("--port")
            .arg(spec.port.to_string())
            .current_dir(&spec.working_dir)
            // Force-flush Python stdout. When uvicorn is spawned with
            // piped stdout (no TTY), Python's stdout is block-buffered
            // by default — the `WATER_SIDECAR_PORT=` line we wait for
            // in `main.py` may sit in the buffer until uvicorn does
            // its own flush, which can exceed `boot_timeout`. Setting
            // PYTHONUNBUFFERED switches stdout/stderr to unbuffered
            // mode for the lifetime of this child process, so the
            // marker reaches us as soon as `print()` returns.
            .env("PYTHONUNBUFFERED", "1")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        let mut child = cmd
            .spawn()
            .map_err(|e| Error::Sidecar(format!("failed to spawn uv: {e}")))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| Error::Sidecar("no stdout".into()))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| Error::Sidecar("no stderr".into()))?;
        let mut reader = BufReader::new(stdout).lines();

        // Wait for the WATER_SIDECAR_PORT line within the boot_timeout.
        let port = tokio::time::timeout(spec.boot_timeout, async {
            while let Ok(Some(line)) = reader.next_line().await {
                if let Some(rest) = line.strip_prefix("WATER_SIDECAR_PORT=") {
                    return rest.trim().parse::<u16>().ok();
                }
            }
            None
        })
        .await
        .map_err(|_| Error::Sidecar("timeout waiting for port".into()))?
        .ok_or_else(|| Error::Sidecar("sidecar did not announce port".into()))?;

        // Drain remaining stdout (post-port lines) so the OS pipe buffer never fills.
        tokio::spawn(async move {
            while let Ok(Some(line)) = reader.next_line().await {
                tracing::info!(target: "sidecar.stdout", "{line}");
            }
        });

        // Drain stderr in parallel (uvicorn writes startup banners + errors here).
        let mut stderr_reader = BufReader::new(stderr).lines();
        tokio::spawn(async move {
            while let Ok(Some(line)) = stderr_reader.next_line().await {
                tracing::warn!(target: "sidecar.stderr", "{line}");
            }
        });

        let base_url = format!("http://{}:{}", spec.host, port);
        let me = Self {
            base_url,
            child: Mutex::new(Some(child)),
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(8))
                .build()
                .expect("reqwest client"),
        };

        // Poll /health until ready or timeout.
        let deadline = tokio::time::Instant::now() + spec.boot_timeout;
        loop {
            if tokio::time::Instant::now() > deadline {
                return Err(Error::Sidecar("timeout waiting for /health".into()));
            }
            if me.health().await.is_ok() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(150)).await;
        }
        Ok(me)
    }

    #[must_use]
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub async fn health(&self) -> Result<HealthResponse> {
        let url = format!("{}/health", self.base_url);
        let r = self
            .http
            .get(url)
            .send()
            .await
            .map_err(|e| Error::Sidecar(format!("health: {e}")))?;
        let r = r
            .error_for_status()
            .map_err(|e| Error::Sidecar(format!("health http: {e}")))?;
        let body: HealthResponse = r
            .json()
            .await
            .map_err(|e| Error::Sidecar(format!("health json: {e}")))?;
        Ok(body)
    }

    pub async fn analyze(&self, req: &AnalyzeRequest) -> Result<AnalyzeResponse> {
        let url = format!("{}/analyze", self.base_url);
        let r = self
            .http
            .post(url)
            .json(req)
            .send()
            .await
            .map_err(|e| Error::Sidecar(format!("analyze: {e}")))?;
        let r = r
            .error_for_status()
            .map_err(|e| Error::Sidecar(format!("analyze http: {e}")))?;
        let body: AnalyzeResponse = r
            .json()
            .await
            .map_err(|e| Error::Sidecar(format!("analyze json: {e}")))?;
        Ok(body)
    }

    pub async fn shutdown(self) -> Result<()> {
        if let Some(mut c) = self.child.lock().await.take() {
            let _ = c.kill().await;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn external_health_round_trip() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/health"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "status": "ready",
                "version": "0.1.0",
                "uptime_seconds": 1.2,
                "pid": 999
            })))
            .mount(&server)
            .await;

        let sc = Sidecar::external(server.uri());
        let h = sc.health().await.unwrap();
        assert_eq!(h.status, "ready");
        assert_eq!(h.pid, 999);
    }

    #[tokio::test]
    async fn external_analyze_round_trip() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/analyze"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "word_count": 7,
                "flow": 0.5, "coherence": 0.5, "engagement": 0.5,
                "divergence": 0.0, "pace": 0.5, "intensity": 0.5, "valence": 0.5,
                "status": "normal"
            })))
            .mount(&server)
            .await;
        let sc = Sidecar::external(server.uri());
        let resp = sc
            .analyze(&AnalyzeRequest {
                text: "Some sentence here.".into(),
                scene_id: "01H8X4".into(),
            })
            .await
            .unwrap();
        assert_eq!(resp.word_count, 7);
        assert_eq!(resp.status, "normal");
    }

    #[tokio::test]
    async fn health_propagates_http_errors() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/health"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;
        let sc = Sidecar::external(server.uri());
        assert!(sc.health().await.is_err());
    }

    #[tokio::test]
    #[ignore = "requires uv and the sidecar workspace; run with --ignored"]
    async fn managed_spawn_against_real_sidecar() {
        let workspace = std::env::var("WATER_SIDECAR_DIR").map_or_else(
            |_| std::path::PathBuf::from("../../sidecar"),
            std::path::PathBuf::from,
        );
        let uv = which::which("uv").expect("uv not found on PATH");
        let port: u16 = 18765;
        let sc = Sidecar::spawn(SidecarSpec {
            working_dir: workspace,
            uv_bin: uv,
            port,
            host: "127.0.0.1".into(),
            boot_timeout: Duration::from_secs(20),
        })
        .await
        .unwrap();
        let h = sc.health().await.unwrap();
        assert_eq!(h.status, "ready");
        sc.shutdown().await.unwrap();
    }
}
