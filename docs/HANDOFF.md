# Handoff — Water — next agent

Last working session ended after a major visual overhaul. The codebase is healthy: typecheck green, 202 frontend tests + Rust workspace tests pass. This doc covers (a) the current shape of the app, (b) what was just shipped, (c) the **pill-feature backlog** that's next.

---

## TL;DR — what you're picking up

- All milestones M1–M6 are complete and tagged.
- The visual system is locked: **OpenCode-warm substrate, deep-sea-blue palette, real-time morphing water-ribbon, matte-glass floating UI, StreamMark logo**. Spec: `docs/UX_SPEC.md` (v2).
- The pill engine + heat engine + canvas + worlds + characters surfaces all work end-to-end. Most recent commits were polish.
- **Next: pill features per the spec — rabbit hole, editor pills, prompt overhaul.** See §3 below.

---

## 1. Where to start the day

```
cd /c/Users/H\ BLAUNTE/Water
git log --oneline -20
npm --prefix app test
cargo test --workspace
```

If anything's red, fix that first. Otherwise dive in.

Dev loop:
```
cd app && npm run tauri dev
```

---

## 2. What was just shipped (last ~24h)

### Visual system (UX_SPEC v2 implementation)

1. **Tokens** — `app/src/styles/tokens.css`. OpenCode-warm substrate (h≈0–30, s ≤ 8%). Deep-sea-blue palette with three alt variants (sunrise / clear / earth) switchable via `[data-palette="..."]`. Matte-glass utility classes:
   - `.water-floating-panel` — large surfaces (IconRail, ScenesPanel, sheets)
   - `.water-floating-chip` — small chips (canvas toggles, heatmap strip)
   - Both have `water-glass-in` enter animation on mount.

2. **StreamMark logo** — `app/src/chrome/StreamMark.tsx`. Three nested arcs in currentColor. Used in EmptyState splash + IconRail brand corner. The IconRail icon is now also a **home button** (closes the project, returns to splash).

3. **WaterRibbon** — `app/src/chrome/WaterRibbon.tsx` + `app/src/chrome/ribbonState.ts`. Single global instance at the App root, behind everything. Cross-surface eased anchor state lives in `ribbonState.ts` so the stream's shape is continuous across navigation. Key properties:
   - Real-time path morphing via `performance.now()`-driven phase shifts (no CSS translateX).
   - Anchors mode (canvas) blends with ambient noise mode via total weight (`ambientFactor = exp(-totalWeight * 0.35)`).
   - `streamMode: "open" | "narrow"` controls visible width via CSS `mask-size` transitions.
   - `widthBump` capped at `MAX_BUMP = 160`, with sub-linear `sqrt` scaling, then 2-pass 5-tap low-pass on the widths array — keeps the ribbon smooth even at vertical scene stacks.
   - Droplets have random lifetimes (6–16s) with smoothstep fade in/out envelopes.

4. **Canvas anchor pipeline** — `CanvasSurface.tsx` calls `setRibbonTarget(...)` directly when scenes change. Anchors are translated from canvas-space to window-space via `pan + zoom + container.offsetX`. Cleanup clears anchors on unmount; ribbon eases back to ambient.

5. **Surface backgrounds** — all surface mains are transparent so the global ribbon flows through. The editor's prose column carries a horizontal-gradient fade backdrop (`linear-gradient(transparent → bg-paper → transparent)`) so the stream bleeds softly into the column. CharactersSurface, WorldsSurface containers transparent; their card/tile classes have glass treatment with backdrop-filter.

6. **Glass theme rollout** — characters page (search/sort/button/cards), worlds page (tiles/entry cards/aliases editor), ProjectMenu, ScenesPanel all wear matte glass with subtle inset-highlight. Gradients (e.g., card-hue washes) layer ON TOP of the translucent paper.

### Pre-pill polish

- Heatmap mouse-tracking tooltip removed (was distracting).
- Dark-mode brown title underline → sea-300 accent.
- Selection toolbar anchored above selection w/ 14px gap.
- Hover-highlight tightened (no vertical overhang).
- ScenesPanel collapse/expand uses single persistent wrapper with width transition + `water-scenes-slide-in` keyframe.
- Word count in ScenesPanel no longer collides with 3-dots button.

---

## 3. Pill features — the next track (per UX_SPEC.md v2)

The spec is locked. Implementation phases below match the spec's track letters. The full design is in `docs/UX_SPEC.md` §§ 3–6 — read those sections first.

### Phase 3 — Pill UX refinement (`UX_SPEC.md §C`)

Status: visual system in place; pill internals NOT yet refactored.

What to build:
- Persona collapses to a 12×12 chip-icon at the left of the pill (glyph on persona-hued circle). Outer pill hue freed for **content signal** (observation / suggestion / warning / praise) via a 1.5px left rail.
- Pill size drops ~15%.
- Chevron-down affordance on the right edge appears on hover — signals "click to deepen" (rabbit hole).
- Replace the hover-line pattern with the chevron + selection-toolbar pattern that's already in place.

Files to touch:
- `app/src/pill/PillCapsule.tsx` — pill anatomy.
- `app/src/pill/Bouquet.tsx` — expanded view.

Spec details: §C.1–C.4.

### Phase 3.5 — Hover-highlight subsystem (`UX_SPEC.md §C.6`)

This is critical. The writer flagged precision as a requirement: **hover must light the exact trigger phrase, not the whole paragraph**.

What to build:
- Extend `pinned_pill` row + emitted pill payload with: `anchor_block_id`, `anchor_snippet` (3–10 word phrase), `anchor_block_hash` (first 80 chars of block, normalized), `anchor_offset_hint`.
- 4-tier resolver: block-id + snippet → block-hash + snippet → tolerant fuzzy (≤2 char edits) → fallback to block + "anchor drifted" pip on the pill.
- ProseMirror decoration plugin scoped to character ranges (port the pattern from Orbit's `setHighlightedParagraph`, which lives at `/c/Users/H BLAUNTE/Desktop/Orbit/frontend/src/components/MarginAnnotations.tsx`).
- Unit test battery: identity, paragraph split, paragraph merge, typo correction, partial deletion, full deletion.

Spec details: §C.6 entire section.

### Phase 4 — Rabbit hole (`UX_SPEC.md §D`)

Persisted on-device per writer choice. Memory-capped (5000 thoughts / 25MB per project, auto-trim non-resonant leaves).

Schema (v8 migration):
```sql
CREATE TABLE rabbit_thought (
    id TEXT PRIMARY KEY,
    scene_id TEXT NOT NULL REFERENCES scene(id) ON DELETE CASCADE,
    parent_id TEXT REFERENCES rabbit_thought(id) ON DELETE CASCADE,
    speaker_kind TEXT NOT NULL,
    speaker_id TEXT NOT NULL,
    message TEXT NOT NULL,
    depth INTEGER NOT NULL,
    siblings_at_depth INTEGER NOT NULL,
    sibling_index INTEGER NOT NULL,
    resonance INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL
);
CREATE INDEX rabbit_thought_by_scene ON rabbit_thought(scene_id, parent_id);
```

UX: click pill → DeepenPanel slides in (360×680 max) with 4 child thoughts. Click child → it becomes new parent; breadcrumb shows ancestry; 4 new children spawn. Esc to ascend; "Pin thread" button stores ancestry.

New prompts:
- `prompts/tasks/rabbit_fan_4.toml` — 4 directions: closer / wider / opposite / deeper. Strict JSON output.
- `prompts/tasks/rabbit_deepen_inherit.toml` — children inherit parent's stance.

Cost: ~$0.005/click at Sonnet 4.6 (600 input / 400 output). Raise `LlmBudget` per-session cap to 40k input / 15k output.

Resonance: marking a child resonant doesn't immediately change generation but flags the row. Future pill-prompts read recent resonance as a `voice_preference` signal.

### Phase 5 — Editor pills (`UX_SPEC.md §E`)

Hybrid rule-based + LLM polish. Sticky until dismissed. **Voice discipline is critical** — see §E.7.

Rule layer (Rust, `water-core::editor::diagnostics`):
| Rule | Detection |
|---|---|
| `spelling` | hunspell-compatible dict embedded in binary |
| `passive_voice` | `(was/were/been/is/are/be) + past-participle` |
| `weak_verb` | `(to be) + adjective` where stronger verb exists |
| `adverb_density` | `> 2 -ly adverbs / 100 words` |
| `sentence_length_variance` | 5 consecutive sentences within ±3 words |
| `repetition` | same word ≥ 4× in 200 words (excluding stopwords) |
| `dialog_tag_overuse` | `said` + adverb |
| `common_mistake` | port Orbit's table (`their is`, `could of`, etc.) |

LLM polish layer: one pass per modified paragraph (30s debounce after last edit). Surfaces one observation the rule layer can't see ("the metaphor of drowning recurs three times"). 5 polish passes per scene max before manual re-request.

**Voice rules** (apply to BOTH layers):
- ❌ Never: "Consider rewriting", "Try using", "You should", "Maybe rephrase as".
- ✅ Always: present-tense observation in the Editor persona's voice, ≤ 12 words, no second-person directive.

Template phrasings table is in §E.7. Each goes through `tone.toml`'s blacklist regex before surfacing.

Schema (v8 with rabbit_thought, OR v9):
```sql
CREATE TABLE editor_pill (
    id TEXT PRIMARY KEY,
    scene_id TEXT NOT NULL REFERENCES scene(id) ON DELETE CASCADE,
    rule TEXT NOT NULL,
    severity TEXT NOT NULL,
    message TEXT NOT NULL,
    suggestion TEXT,
    anchor_block_id TEXT NOT NULL,
    anchor_start INTEGER NOT NULL,
    anchor_end INTEGER NOT NULL,
    text_snippet TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    dismissed INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX editor_pill_by_scene ON editor_pill(scene_id, dismissed);
```

Surface: inline dotted underline (`sea-300` dotted) + a "diagnostics" tab in a right-panel. Both share the same `editor_pill` rows.

### Phase 6 — Prompt overhaul (`UX_SPEC.md §F`)

Existing pill prompts at `prompts/tasks/pill_*.toml` underuse Water's structured context. Bring in:
- `arc_position` — computed from `scene.ordering / manuscript.scene_count`, mapped to labeled buckets (e.g., "opening sequence", "midpoint pivot").
- `character.sheet_compact` — 200-word distillation from LSM v2.1, cached in SQLite (one-time per character).
- Recent resonance picks (from rabbit hole) as last 3 thoughts.

Speaker prompts get a single-line "won't do" clause each (ported from Orbit's blacklist pattern):
- Echo: "Won't analyze. Will notice."
- Architect: "Won't praise. Will name the load-bearing structure."
- Editor: "Won't soften. Will point at the specific phrase."
- Cartographer: "Won't summarize the world. Will point at where it leaks."
- Chorus: "Won't take sides. Will reflect the chord."

---

## 4. Palette switcher (deferred work)

`UX_SPEC v2 §A.5` defines four palettes (deep / sunrise / clear / earth) with full ramps in `tokens.css` already. **The switcher UI isn't wired yet.** When you build it:

- Add `project.palette TEXT NOT NULL DEFAULT 'deep'` column (could be v8 alongside rabbit_thought).
- On project open, read it and set `[data-palette="deep|sunrise|clear|earth"]` on `<html>`.
- Setting UI in `SettingsSheet`.

The palette CSS rules in `tokens.css` are already wired — just need the toggle.

---

## 5. Code surfaces you'll touch most

```
app/src/pill/                 — pill engine + capsule rendering
app/src/heat/                 — heatmap strip + metric picker
app/src/canvas/               — macro spatial canvas
app/src/chrome/               — IconRail, ScenesPanel, WaterRibbon, ribbonState, StreamMark
app/src/worlds/               — world bible surface
app/src/characters/           — characters surface
app/src/scenes/               — scene metadata sheets
app/src-tauri/src/commands/   — Tauri IPC bridges
crates/water-core/src/        — Rust core: scene, character, pill engine
crates/water-core/sql/        — SQLite migrations (currently v7)
prompts/                      — LLM prompts
```

---

## 6. Conventions / preferences saved as memory

The writer has stated these explicitly across sessions — they're in your memory dir:

- Use Water-dedicated LLM keys, NOT shared with other projects. Per-project usage tracking matters.
- Selection toolbar must anchor ABOVE the selection, not over it.
- Location should auto-detect, not require manual dropdown selection (worldbible autosuggest chips are the canonical flow).
- Pill UX polish: persona collapsed to chip-icon, hue freed for content signal, smaller size, chevron affordance not hover-line.
- Tone discipline: never break the writer's trance. No "consider", "try", "as an AI", "you should". See `prompts/tone.toml`.

---

## 7. State of tests

- 202 frontend tests passing (vitest)
- 354 + 41 + 6 + 1 + 1 = 403 Rust tests passing (workspace)
- Manual smoke walk recommended after migrations (palette switcher / rabbit hole / editor pills)

---

## 8. Things to ask the writer before starting

If anything in §3 isn't clear, defer to the writer. Specifically:
- Rabbit hole panel position — current spec says side-slide; confirm before building.
- Editor pills inline vs tab — spec has both; check if both are still wanted.
- Palette switcher UI placement.

Otherwise build to the spec.

Good luck. The visual system is solid — the next phase is where Water becomes uniquely useful.
