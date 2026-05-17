//! IPC contract shared with the sidecar.
//!
//! Kept in sync by hand for v1; later milestones can switch to `ts-rs` or
//! a generated schema.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub uptime_seconds: f64,
    pub pid: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AnalyzeRequest {
    pub text: String,
    pub scene_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AnalyzeResponse {
    pub word_count: u64,
    pub flow: f64,
    pub coherence: f64,
    pub engagement: f64,
    pub divergence: f64,
    pub pace: f64,
    pub intensity: f64,
    pub valence: f64,
    pub status: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn analyze_request_round_trips_json() {
        let req = AnalyzeRequest {
            text: "hi".into(),
            scene_id: "01H8X4".into(),
        };
        let json = serde_json::to_string(&req).unwrap();
        let parsed: AnalyzeRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(req, parsed);
    }

    #[test]
    fn analyze_response_matches_sidecar_schema() {
        // Hand-pinned: this should match the FastAPI AnalyzeResponse shape.
        let sample = r#"{"word_count":12,"flow":0.5,"coherence":0.5,"engagement":0.5,"divergence":0.0,"pace":0.5,"intensity":0.5,"valence":0.5,"status":"normal"}"#;
        let parsed: AnalyzeResponse = serde_json::from_str(sample).unwrap();
        assert_eq!(parsed.word_count, 12);
        assert_eq!(parsed.status, "normal");
    }
}
