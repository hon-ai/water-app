# Known Fragile Heuristics

A living catalogue of intentionally-shallow heuristics in Water. If a feature listed here is misbehaving, **start your investigation here** — these are the most likely culprits.

Each entry has:

- **What it is** — a one-line description.
- **Where it lives** — pointers into the spec and (later) code paths.
- **Why it's fragile** — the reason we accepted the fragility.
- **What success looks like** — when it's working as intended.
- **First-look mitigations** — quick checks before a deeper rewrite.

---

## 1. `character_dissonance` trigger — lemma-overlap heuristic

**What it is.** A pill trigger that fires when the most recent paragraph contains language that contradicts a present character's stated `values`, `fears`, or `lie_they_believe` field on their LSM v2.1 sheet. The check is a shallow lemma-overlap between the paragraph and those character-sheet fields.

**Where it lives.**

- Design spec: `docs/superpowers/specs/2026-05-16-water-design.md` § 6.1 (trigger taxonomy), § 7.1 R3.
- (Future) Rust code: `crates/water-core/src/orchestrator/triggers/character_dissonance.rs`.
- (Future) Eval task family: `eval/tasks/character_dissonance/*.json`.

**Why it's fragile.**

- Lemma overlap is a bag-of-words approximation. It cannot detect semantic contradictions phrased without keyword overlap ("she walked away from the help she'd asked for" contradicts `values = [honesty, kept_promises]` without sharing any lemmas).
- It produces false positives when a character is *internally negotiating* a value rather than violating it (a meaningful narrative moment, not dissonance).
- Sensitivity is hard to tune across genres and registers.

**What success looks like.**

- Fires when a character with a clear `lie_they_believe` acts *as if the lie were true* in a moment where the writer's text shows it's costing them.
- Does not fire when a character is simply experiencing tension or change.
- Eval-harness scorecard `character_dissonance.precision` and `.recall` both > 0.7.

**First-look mitigations.**

1. Check the per-character `dissonance_sensitivity` slider in settings — a single user may have it too high.
2. Inspect `replay logs` (settings → diagnostics → enable replay) for the offending trigger; the log records the paragraph text, character sheet snapshot, and the lemma-overlap score.
3. Compare the firing paragraph to the character's `lie_they_believe`, `values`, `fears` fields directly — if they don't *feel* related, the heuristic is doing what it does.
4. If the false-positive rate is high in the field, the next step is to replace the lemma-overlap with a cheap embedding-similarity check (MiniLM cosine between paragraph and sheet excerpts), or to gate the trigger behind a secondary LLM confirmation pass.

---

## 2. Block-id duplicate tolerance

**What it is.** `ensure_block_ids` (in `crates/water-core/src/block.rs`) does not
de-duplicate colliding `^bk-XXXX` tokens within a single scene body. If the
writer manually edits the file and creates two blocks with the same id, both
ids are preserved.

**Where it lives.** `crates/water-core/src/block.rs::ensure_block_ids`.

**Why it's fragile.** Pill anchoring uses the snippet (Section 3.3 of the spec)
as canonical, so duplicate ids do not actually break pill resolution. But
duplicate ids would confuse a future block-id-keyed feature (e.g., paragraph-
level metric pinning).

**What success looks like.** No false positives reported by writers; the
duplicate case is rare because Water never *introduces* duplicates.

**First-look mitigations.**
1. If duplicates cause downstream confusion, upgrade `ensure_block_ids` to
   rename the second occurrence on next save.
2. Add a one-shot repair option to renumber all duplicates.

---

*(More entries will be added as fragile heuristics are introduced. Keep this file in repo root.)*
