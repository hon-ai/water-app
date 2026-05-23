//! UV bootstrap commands: check-presence + one-click installer.
//!
//! The Python sidecar (which powers stylometric nudges) requires
//! `uv` on PATH. Rather than make testers paste shell commands, the
//! renderer can call `install_uv` here and receive a live stream of
//! `uv:install:log` events while the installer runs.
//!
//! Two notes on the resolver:
//!  - `which::which` only sees PATH at *this* process's startup, so
//!    a fresh install in the user's home dir won't appear there.
//!    `resolve_uv()` therefore also probes the canonical install
//!    locations the astral.sh installer drops binaries into.
//!  - The same resolver is what `project.rs::boot_sidecar_for_project`
//!    should call (it does, after the refactor in this commit) so
//!    the next project open finds uv without an app restart.

use crate::events::emit;
use serde::Serialize;
use std::path::PathBuf;
use std::process::Stdio;
use tauri::AppHandle;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

/// Probe the locations the astral.sh installer drops `uv` into,
/// in addition to PATH. Order matters — PATH first so the writer's
/// explicit choice (Homebrew, manual `cargo install`, etc.) wins.
pub fn resolve_uv() -> Option<PathBuf> {
    if let Ok(p) = which::which("uv") {
        return Some(p);
    }
    let home = dirs_home()?;
    let candidates = if cfg!(windows) {
        vec![
            home.join(".local").join("bin").join("uv.exe"),
            home.join(".cargo").join("bin").join("uv.exe"),
        ]
    } else {
        vec![
            home.join(".local").join("bin").join("uv"),
            home.join(".cargo").join("bin").join("uv"),
        ]
    };
    candidates.into_iter().find(|p| p.is_file())
}

/// `dirs` crate isn't in our dep tree; do it by hand. Windows uses
/// USERPROFILE; everything else uses HOME.
fn dirs_home() -> Option<PathBuf> {
    let var = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    std::env::var_os(var).map(PathBuf::from)
}

#[derive(Serialize)]
pub struct UvStatus {
    pub installed: bool,
    pub path: Option<String>,
}

#[tauri::command]
pub async fn check_uv_installed() -> Result<UvStatus, String> {
    let resolved = resolve_uv();
    Ok(UvStatus {
        installed: resolved.is_some(),
        path: resolved.map(|p| p.to_string_lossy().to_string()),
    })
}

#[derive(Serialize, Clone)]
pub struct UvInstallLog {
    pub line: String,
    pub stream: String, // "stdout" | "stderr"
}

#[derive(Serialize, Clone)]
pub struct UvInstallDone {
    pub success: bool,
    pub error: Option<String>,
    pub path: Option<String>,
}

/// Run the official astral.sh installer for `uv`. On Windows we
/// shell out to PowerShell so the `irm | iex` pipeline behaves like
/// it does in docs; elsewhere we use `sh` for the `curl | sh` form.
///
/// Streams every stdout + stderr line via `uv:install:log` events so
/// the renderer can show progress. Emits one `uv:install:done` event
/// on exit. Returns `Ok` once spawned; failures land in the `done`
/// event, not the return value (so the renderer can keep listening).
#[tauri::command]
pub async fn install_uv(app: AppHandle) -> Result<(), String> {
    if resolve_uv().is_some() {
        // No-op early exit — installer would still succeed but it's
        // cheaper not to download anything and the renderer's UX
        // looks instant.
        let _ = emit(
            &app,
            "uv:install:done",
            UvInstallDone {
                success: true,
                error: None,
                path: resolve_uv().map(|p| p.to_string_lossy().to_string()),
            },
        );
        return Ok(());
    }

    let mut cmd = if cfg!(windows) {
        let mut c = Command::new("powershell");
        c.arg("-NoProfile")
            .arg("-ExecutionPolicy")
            .arg("ByPass")
            .arg("-Command")
            .arg("irm https://astral.sh/uv/install.ps1 | iex");
        c
    } else {
        let mut c = Command::new("sh");
        c.arg("-c")
            .arg("curl -LsSf https://astral.sh/uv/install.sh | sh");
        c
    };
    cmd.stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            let _ = emit(
                &app,
                "uv:install:done",
                UvInstallDone {
                    success: false,
                    error: Some(format!("failed to spawn installer: {e}")),
                    path: None,
                },
            );
            return Ok(());
        }
    };

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let app_out = app.clone();
    let app_err = app.clone();

    if let Some(stdout) = stdout {
        tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = emit(
                    &app_out,
                    "uv:install:log",
                    UvInstallLog {
                        line,
                        stream: "stdout".into(),
                    },
                );
            }
        });
    }
    if let Some(stderr) = stderr {
        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let _ = emit(
                    &app_err,
                    "uv:install:log",
                    UvInstallLog {
                        line,
                        stream: "stderr".into(),
                    },
                );
            }
        });
    }

    let app_done = app.clone();
    tokio::spawn(async move {
        let result = child.wait().await;
        let payload = match result {
            Ok(status) if status.success() => {
                let path = resolve_uv().map(|p| p.to_string_lossy().to_string());
                UvInstallDone {
                    success: path.is_some(),
                    error: if path.is_some() {
                        None
                    } else {
                        Some("Installer exited 0 but uv was not found on disk".into())
                    },
                    path,
                }
            }
            Ok(status) => UvInstallDone {
                success: false,
                error: Some(format!("installer exited with status {status}")),
                path: None,
            },
            Err(e) => UvInstallDone {
                success: false,
                error: Some(format!("installer wait failed: {e}")),
                path: None,
            },
        };
        let _ = emit(&app_done, "uv:install:done", payload);
    });

    Ok(())
}

/// Restart the Tauri app process — used by the post-install
/// "Restart Water" button so the next boot's `boot_sidecar_for_project`
/// inherits a PATH (and ambient env) that includes the freshly
/// installed uv. Tauri does its own teardown so windows close cleanly
/// and autosave fires.
#[tauri::command]
pub async fn restart_app(app: AppHandle) {
    app.restart()
}

// Tests for resolve_uv intentionally omitted — they'd have to mutate
// HOME/USERPROFILE and the test would race with any concurrent test
// that reads those env vars. The function is small enough to inspect.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dirs_home_returns_something() {
        // CI always has HOME or USERPROFILE set.
        assert!(dirs_home().is_some());
    }
}
