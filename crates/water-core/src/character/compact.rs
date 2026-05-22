//! Phase 6 — character_compact: pure heuristic distillation of an LSM
//! v2.1 sheet into a ~200-word block usable in pill prompts.
//!
//! The full sheet runs several thousand tokens. The compact pulls the
//! load-bearing fields — name, role, want, need, the lie they believe,
//! values, fears, voice — into a single labelled block. This stops the
//! model from inventing motivations the writer hasn't established (the
//! (scrubbed-project) lesson: pills about character without a sheet drift into
//! generic prose).
//!
//! The output is dropped into the system prompt verbatim. Fields are
//! omitted when empty so a half-finished sheet doesn't burn tokens on
//! blank labels. List fields (`values`, `fears`) are comma-joined.
//!
//! No caching column yet (UX_SPEC §F.3 calls for one). The function is
//! cheap (~microseconds against a parsed JSON value) and runs once per
//! prompt assembly; caching can land if it ever shows up in profiles.

use serde_json::Value;

/// Compose a compact prompt block from an LSM v2.1 character sheet.
/// Returns the empty string when the sheet has no usable fields, so
/// callers can `if !s.is_empty()` to decide whether to emit the
/// `[character sheet]` line at all.
#[must_use]
pub fn character_compact(sheet: &Value) -> String {
    let name = read_str(sheet, "main", "full_name");
    let role = read_str(sheet, "main", "role_in_story");
    let want = read_str(sheet, "main", "want");
    let need = read_str(sheet, "main", "need");
    let lie = read_str(sheet, "main", "lie_they_believe");
    let voice = read_str(sheet, "bonus_traits", "voice");
    let values = read_list_joined(sheet, "bonus_traits", "values");
    let fears = read_list_joined(sheet, "bonus_traits", "fears");

    let mut lines: Vec<String> = Vec::new();
    // Lead line — name + role compressed onto one line when both
    // are present, otherwise whichever exists.
    match (name.is_empty(), role.is_empty()) {
        (false, false) => lines.push(format!("{name} — {role}.")),
        (false, true) => lines.push(format!("{name}.")),
        (true, false) => lines.push(format!("Role: {role}.")),
        (true, true) => {}
    }
    if !want.is_empty() {
        lines.push(format!("Wants: {want}."));
    }
    if !need.is_empty() {
        lines.push(format!("Needs: {need}."));
    }
    if !lie.is_empty() {
        lines.push(format!("The lie they believe: {lie}."));
    }
    if !values.is_empty() {
        lines.push(format!("Values: {values}."));
    }
    if !fears.is_empty() {
        lines.push(format!("Fears: {fears}."));
    }
    if !voice.is_empty() {
        lines.push(format!("Voice: {voice}."));
    }
    lines.join("\n")
}

fn read_str(sheet: &Value, section: &str, key: &str) -> String {
    sheet
        .get(section)
        .and_then(|s| s.get(key))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string()
}

fn read_list_joined(sheet: &Value, section: &str, key: &str) -> String {
    let arr = sheet
        .get(section)
        .and_then(|s| s.get(key))
        .and_then(|v| v.as_array());
    arr.map(|items| {
        items
            .iter()
            .filter_map(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(", ")
    })
    .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn empty_sheet_returns_empty_string() {
        assert_eq!(character_compact(&json!({})), "");
    }

    #[test]
    fn minimal_sheet_with_only_name_emits_lead_line() {
        let s = json!({ "main": { "full_name": "Marcus" } });
        assert_eq!(character_compact(&s), "Marcus.");
    }

    #[test]
    fn full_sheet_emits_every_labeled_line_in_order() {
        let s = json!({
            "main": {
                "full_name": "Marcus",
                "role_in_story": "the apprentice",
                "want": "to find his father's letters",
                "need": "to stop performing his grief",
                "lie_they_believe": "if I keep busy I won't feel it"
            },
            "bonus_traits": {
                "values": ["loyalty", "patience"],
                "fears": ["being unseen", "another funeral"],
                "voice": "spare, measured, slightly dry"
            }
        });
        let out = character_compact(&s);
        assert!(out.starts_with("Marcus — the apprentice."));
        assert!(out.contains("Wants: to find his father's letters."));
        assert!(out.contains("Needs: to stop performing his grief."));
        assert!(out.contains("The lie they believe: if I keep busy I won't feel it."));
        assert!(out.contains("Values: loyalty, patience."));
        assert!(out.contains("Fears: being unseen, another funeral."));
        assert!(out.contains("Voice: spare, measured, slightly dry."));
    }

    #[test]
    fn empty_fields_are_skipped() {
        let s = json!({
            "main": { "full_name": "Marcus", "want": "" },
            "bonus_traits": { "values": [], "fears": ["the dark"] }
        });
        let out = character_compact(&s);
        assert_eq!(out, "Marcus.\nFears: the dark.");
    }

    #[test]
    fn whitespace_trimmed_from_values() {
        let s = json!({
            "main": { "full_name": "  Marcus  " },
            "bonus_traits": { "values": [" loyalty ", "  patience"] }
        });
        let out = character_compact(&s);
        assert!(out.starts_with("Marcus."));
        assert!(out.contains("Values: loyalty, patience."));
    }
}
