//! `LlmBudget` — per-session token ceiling for the Heatmap's LLM-backed
//! metrics (Valence + Coherence).
//!
//! Without a budget, opening a long scene against a real provider can
//! issue hundreds of LLM calls per autosave. The budget caps the total
//! input + output tokens consumed by heat-compute calls in a single
//! session. When exceeded, [`LlmBudget::try_charge`] returns
//! [`BudgetExceeded`]; the orchestrator catches this and writes a
//! sentinel `NaN` value into `HeatStore` so the renderer can show a
//! quiet "budget reached" hint on the affected track without spinning.
//!
//! Reset semantics: budget is zeroed when the project closes
//! (orchestrator service drops, the budget drops with it). Re-opening
//! the same project starts a fresh budget — by design, the writer who
//! returns to a session and starts editing gets a fresh allowance.

use std::sync::Mutex;

/// Default cap: 20k input + 5k output tokens per session. With typical
/// paragraph sizes (~200 input + 1 output per call) this works out to
/// ~95 valence + coherence calls per session at the model side; with
/// the text_hash cache, that's enough to cover several full
/// 5000-word-scene recomputes before the budget bites.
pub const DEFAULT_INPUT_CAP: u32 = 20_000;
pub const DEFAULT_OUTPUT_CAP: u32 = 5_000;

/// Returned by [`LlmBudget::try_charge`] when a charge would exceed the
/// configured cap. The compute path surfaces this as an error so the
/// orchestrator can write a sentinel and skip.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BudgetExceeded {
    /// Which side of the budget was exhausted — input or output. Useful
    /// for the renderer's hint copy ("budget: input reached").
    pub side: BudgetSide,
    /// How many tokens were already consumed on this side.
    pub used: u32,
    /// How many tokens this side caps at.
    pub cap: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BudgetSide {
    Input,
    Output,
}

impl std::fmt::Display for BudgetExceeded {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let side = match self.side {
            BudgetSide::Input => "input",
            BudgetSide::Output => "output",
        };
        write!(
            f,
            "LLM heat budget exhausted: {side} {used}/{cap}",
            side = side,
            used = self.used,
            cap = self.cap
        )
    }
}

impl std::error::Error for BudgetExceeded {}

#[derive(Debug)]
pub struct LlmBudget {
    state: Mutex<BudgetState>,
}

#[derive(Debug, Clone, Copy)]
struct BudgetState {
    input_used: u32,
    output_used: u32,
    input_cap: u32,
    output_cap: u32,
}

impl Default for LlmBudget {
    fn default() -> Self {
        Self::new(DEFAULT_INPUT_CAP, DEFAULT_OUTPUT_CAP)
    }
}

impl LlmBudget {
    #[must_use]
    pub fn new(input_cap: u32, output_cap: u32) -> Self {
        Self {
            state: Mutex::new(BudgetState {
                input_used: 0,
                output_used: 0,
                input_cap,
                output_cap,
            }),
        }
    }

    /// Attempt to reserve `(input, output)` tokens. On success, the
    /// reservation is added to the running total. On failure (either
    /// side would exceed its cap) the totals are left untouched and
    /// the offending side is returned.
    ///
    /// # Errors
    /// Returns [`BudgetExceeded`] when the requested charge would push
    /// either side past its cap.
    pub fn try_charge(&self, input: u32, output: u32) -> Result<(), BudgetExceeded> {
        let mut s = self.state.lock().expect("LlmBudget mutex poisoned");
        let new_in = s.input_used.saturating_add(input);
        let new_out = s.output_used.saturating_add(output);
        if new_in > s.input_cap {
            return Err(BudgetExceeded {
                side: BudgetSide::Input,
                used: s.input_used,
                cap: s.input_cap,
            });
        }
        if new_out > s.output_cap {
            return Err(BudgetExceeded {
                side: BudgetSide::Output,
                used: s.output_used,
                cap: s.output_cap,
            });
        }
        s.input_used = new_in;
        s.output_used = new_out;
        Ok(())
    }

    /// Reset usage to zero. Caps are unchanged. Called on project close.
    pub fn reset(&self) {
        let mut s = self.state.lock().expect("LlmBudget mutex poisoned");
        s.input_used = 0;
        s.output_used = 0;
    }

    /// Snapshot the current usage + caps for the renderer.
    #[must_use]
    pub fn snapshot(&self) -> BudgetSnapshot {
        let s = self.state.lock().expect("LlmBudget mutex poisoned");
        BudgetSnapshot {
            input_used: s.input_used,
            output_used: s.output_used,
            input_cap: s.input_cap,
            output_cap: s.output_cap,
        }
    }
}

/// Renderer-facing budget snapshot. `serde::Serialize` so it can be
/// returned through IPC alongside `heat:updated` events in Phase D.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BudgetSnapshot {
    pub input_used: u32,
    pub output_used: u32,
    pub input_cap: u32,
    pub output_cap: u32,
}

/// Rough heuristic for converting `text` length to a token count.
/// ~4 characters per token across English prose; we under-estimate
/// intentionally so a paragraph that ends up costing slightly more
/// than expected doesn't blow past the cap silently.
#[must_use]
pub fn estimate_tokens(text: &str) -> u32 {
    #[allow(clippy::cast_possible_truncation)]
    let n = (text.len() / 4) as u32;
    n.max(1) // every call costs at least 1 token of context
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_caps_match_constants() {
        let b = LlmBudget::default();
        let snap = b.snapshot();
        assert_eq!(snap.input_cap, DEFAULT_INPUT_CAP);
        assert_eq!(snap.output_cap, DEFAULT_OUTPUT_CAP);
        assert_eq!(snap.input_used, 0);
        assert_eq!(snap.output_used, 0);
    }

    #[test]
    fn try_charge_within_budget_succeeds_and_accumulates() {
        let b = LlmBudget::new(1000, 100);
        b.try_charge(300, 30).unwrap();
        b.try_charge(400, 40).unwrap();
        let snap = b.snapshot();
        assert_eq!(snap.input_used, 700);
        assert_eq!(snap.output_used, 70);
    }

    #[test]
    fn try_charge_exceeding_input_fails_with_input_side() {
        let b = LlmBudget::new(500, 500);
        b.try_charge(400, 100).unwrap();
        let err = b.try_charge(200, 50).unwrap_err();
        assert_eq!(err.side, BudgetSide::Input);
        assert_eq!(err.used, 400);
        assert_eq!(err.cap, 500);
    }

    #[test]
    fn try_charge_exceeding_output_fails_with_output_side() {
        let b = LlmBudget::new(1000, 100);
        b.try_charge(100, 80).unwrap();
        let err = b.try_charge(100, 50).unwrap_err();
        assert_eq!(err.side, BudgetSide::Output);
        assert_eq!(err.used, 80);
        assert_eq!(err.cap, 100);
    }

    #[test]
    fn failed_charge_leaves_totals_unchanged() {
        let b = LlmBudget::new(500, 500);
        b.try_charge(400, 100).unwrap();
        let _ = b.try_charge(200, 50); // exceeds input
        let snap = b.snapshot();
        assert_eq!(snap.input_used, 400);
        assert_eq!(snap.output_used, 100);
    }

    #[test]
    fn reset_zeroes_usage_without_changing_caps() {
        let b = LlmBudget::new(200, 50);
        b.try_charge(150, 30).unwrap();
        b.reset();
        let snap = b.snapshot();
        assert_eq!(snap.input_used, 0);
        assert_eq!(snap.output_used, 0);
        assert_eq!(snap.input_cap, 200);
        assert_eq!(snap.output_cap, 50);
    }

    #[test]
    fn estimate_tokens_floor_one_for_short_text() {
        assert_eq!(estimate_tokens(""), 1);
        assert_eq!(estimate_tokens("a"), 1);
        assert_eq!(estimate_tokens("abc"), 1);
    }

    #[test]
    fn estimate_tokens_scales_at_four_chars_per_token() {
        // 12 chars → ~3 tokens. 40 chars → ~10 tokens.
        assert_eq!(estimate_tokens("aaaa aaaa aa"), 3);
        assert_eq!(estimate_tokens(&"a".repeat(40)), 10);
    }

    #[test]
    fn budget_exceeded_implements_display_with_side() {
        let err = BudgetExceeded {
            side: BudgetSide::Input,
            used: 19000,
            cap: 20000,
        };
        let s = format!("{err}");
        assert!(s.contains("input"));
        assert!(s.contains("19000"));
        assert!(s.contains("20000"));
    }
}
