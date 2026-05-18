//! Opt-in replay log. JSONL at `.water/log/llm/{session_ulid}.jsonl`.
//!
//! Enabled via the `WATER_REPLAY_LOG=1` env var or
//! `settings.replay_log_enabled = true` in the project DB. Each LLM
//! request + response pair surfaces around the orchestrator's
//! `generate_*_with_default` call sites; the file is `append`-mode so
//! concurrent spawned tasks each write a full JSON line atomically per
//! `writeln!` (POSIX `O_APPEND` semantics; Windows append is also
//! single-line atomic for short writes).
//!
//! The M2 tone audit (T29) and the M5 eval harness both consume these
//! files. Until Settings UI lands (M7), only the env var path is wired.

use serde::Serialize;
use std::fs::{create_dir_all, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// One JSONL row. Each LLM round trip produces two rows: a request row
/// (kind = trigger / `pill_expand` / `pill_regenerate`) and a response
/// row (kind = `response`). They are correlated by adjacency within a
/// single session file.
#[derive(Serialize)]
pub struct ReplayEntry<'a> {
    pub ts: String,
    pub kind: &'a str,
    pub request_system: &'a str,
    pub request_user: &'a str,
    pub response_raw: Option<&'a str>,
    pub post_filter_decision: Option<&'a str>,
    pub anti_loop_overlap: Option<f32>,
}

/// A live append handle to a session's replay-log file. Cheap to clone
/// through an `Arc`; concurrent appends serialize on the inner `Mutex`.
pub struct ReplayLog {
    file: Mutex<std::fs::File>,
}

impl ReplayLog {
    /// Open (creating if needed) `<project_root>/.water/log/llm/<session_id>.jsonl`
    /// in append mode. Returns an error string on any IO failure rather
    /// than panicking — callers treat replay logging as best-effort.
    pub fn open(project_root: &Path, session_id: &str) -> Result<Self, String> {
        let dir = project_root.join(".water").join("log").join("llm");
        create_dir_all(&dir).map_err(|e| e.to_string())?;
        let path: PathBuf = dir.join(format!("{session_id}.jsonl"));
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|e| e.to_string())?;
        Ok(Self {
            file: Mutex::new(file),
        })
    }

    /// Serialize `entry` to a single JSON line and append it. The
    /// trailing `\n` is supplied by `writeln!`. Errors are returned as
    /// `String` so callers can `tracing::warn!` and move on.
    pub fn append(&self, entry: &ReplayEntry<'_>) -> Result<(), String> {
        let line = serde_json::to_string(entry).map_err(|e| e.to_string())?;
        let mut f = self.file.lock().map_err(|e| e.to_string())?;
        writeln!(f, "{line}").map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn writes_jsonl_line_to_session_file() {
        let dir = TempDir::new().unwrap();
        let log = ReplayLog::open(dir.path(), "session-1").unwrap();
        log.append(&ReplayEntry {
            ts: "2026-05-17T00:00:00Z".to_string(),
            kind: "level_0",
            request_system: "sys",
            request_user: "u",
            response_raw: Some("hello"),
            post_filter_decision: Some("pass"),
            anti_loop_overlap: None,
        })
        .unwrap();
        let path = dir
            .path()
            .join(".water")
            .join("log")
            .join("llm")
            .join("session-1.jsonl");
        let body = std::fs::read_to_string(&path).unwrap();
        assert!(body.contains("\"kind\":\"level_0\""));
        assert!(body.contains("\"response_raw\":\"hello\""));
    }
}
