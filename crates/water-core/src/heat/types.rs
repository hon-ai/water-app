//! Core types for the heat module — metric enum + cached row shape.

use serde::{Deserialize, Serialize};

/// The five metrics rendered by the Heatmap strip. Stored on disk as
/// snake-case strings (see the `metric` column of `heat_metric`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HeatMetricKind {
    /// Rolling word-count density. Local-only; never needs an LLM.
    Pacing,
    /// Per-paragraph emotional valence (-1 cold .. +1 warm). LLM-backed.
    Valence,
    /// Per-paragraph semantic continuity vs. the preceding N paragraphs.
    /// LLM-backed.
    Coherence,
    /// Count of character mentions per paragraph, normalized 0..=1 by
    /// scene-max. Local-only — reuses the M3 character autosuggest scanner.
    Presence,
    /// Count of world-entry name+alias matches per paragraph, normalized
    /// 0..=1 by scene-max. Local-only — reuses the M4 WorldRegistry token
    /// index.
    WorldRefs,
}

impl HeatMetricKind {
    /// String representation used as the `metric` column value in
    /// `heat_metric`. Snake-case, matches serde tag emission.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pacing => "pacing",
            Self::Valence => "valence",
            Self::Coherence => "coherence",
            Self::Presence => "presence",
            Self::WorldRefs => "world_refs",
        }
    }

    /// Parse a stored metric string back into a kind. Returns `None` for
    /// strings written by a future schema (forward compatibility hatch).
    #[must_use]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "pacing" => Some(Self::Pacing),
            "valence" => Some(Self::Valence),
            "coherence" => Some(Self::Coherence),
            "presence" => Some(Self::Presence),
            "world_refs" => Some(Self::WorldRefs),
            _ => None,
        }
    }

    /// `true` if computing this metric requires an LLM round-trip. Used
    /// by the orchestrator to gate recompute on router availability +
    /// the per-session token budget.
    #[must_use]
    pub fn requires_llm(self) -> bool {
        matches!(self, Self::Valence | Self::Coherence)
    }

    /// Iteration helper for enumerating all five kinds (e.g. when the
    /// renderer's metric-picker draws toggles).
    #[must_use]
    pub fn all() -> [Self; 5] {
        [
            Self::Pacing,
            Self::Valence,
            Self::Coherence,
            Self::Presence,
            Self::WorldRefs,
        ]
    }
}

/// One row of the per-paragraph heat cache. Returned by `HeatStore::read`
/// and consumed by the renderer. Ordered by `paragraph_ix` ascending.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeatRow {
    pub paragraph_ix: u32,
    /// The numeric score. Semantics depend on the metric:
    /// - Pacing/Presence/WorldRefs: 0.0 .. 1.0 (scene-max normalized).
    /// - Valence: -1.0 .. 1.0.
    /// - Coherence: 0.0 .. 1.0 (cosine-similarity proxy).
    pub value: f32,
    /// SHA-style hash of the paragraph text that produced `value`. The
    /// compute path can skip re-evaluation when the live paragraph's
    /// hash still matches the cached row's. Stored as a hex string.
    pub text_hash: String,
    /// RFC3339 timestamp of the most recent compute pass that wrote
    /// this row.
    pub updated_at: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn as_str_round_trips_through_from_str() {
        for kind in HeatMetricKind::all() {
            let s = kind.as_str();
            assert_eq!(HeatMetricKind::from_str(s), Some(kind));
        }
    }

    #[test]
    fn from_str_returns_none_for_unknown_metric() {
        assert_eq!(HeatMetricKind::from_str("topicality"), None);
        assert_eq!(HeatMetricKind::from_str(""), None);
        assert_eq!(HeatMetricKind::from_str("Pacing"), None); // case-sensitive
    }

    #[test]
    fn requires_llm_only_for_valence_and_coherence() {
        assert!(!HeatMetricKind::Pacing.requires_llm());
        assert!(HeatMetricKind::Valence.requires_llm());
        assert!(HeatMetricKind::Coherence.requires_llm());
        assert!(!HeatMetricKind::Presence.requires_llm());
        assert!(!HeatMetricKind::WorldRefs.requires_llm());
    }

    #[test]
    fn all_returns_five_distinct_kinds() {
        let kinds = HeatMetricKind::all();
        assert_eq!(kinds.len(), 5);
        // serde tag values are unique by construction; double-check.
        let strs: std::collections::HashSet<&'static str> =
            kinds.iter().map(|k| k.as_str()).collect();
        assert_eq!(strs.len(), 5);
    }

    #[test]
    fn serde_round_trip_uses_snake_case() {
        let kind = HeatMetricKind::WorldRefs;
        let json = serde_json::to_string(&kind).unwrap();
        assert_eq!(json, "\"world_refs\"");
        let back: HeatMetricKind = serde_json::from_str(&json).unwrap();
        assert_eq!(back, kind);
    }
}
