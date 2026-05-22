# Water — UX Spec v2

**Status:** Locked direction. v1 open questions resolved by the writer (2026-05-21). Locks decisions for the visual + interaction overhaul; replaces the M7 polish target with a directed redesign.
**Theme:** Running water. Substrate is warm-neutral paper (OpenCode-tinted). Primary signal is deep-sea blue, with sea-adjacent palette variants (sunrise horizon, clear forest stream, earth/moss). Pills are nuanced annotations that breathe with a near-imperceptible gradient; rabbit holes are how thought deepens.
**References:**
- OpenCode (`anomalyco/opencode`) — `packages/web/src/styles/custom.css` for shading discipline.
- A prior writing-tooling codebase — character-embodied + craft prompt patterns; rule-based editor heuristics.

---

## 0. Decisions locked

| # | Decision | Choice |
|---|---|---|
| 1 | Spec format | One doc, iterate |
| 2 | Heatmap visual | Collapse all metrics to a single per-palette scale; multiple sea-adjacent palettes (deep-sea / sunrise / clear / earth) selectable per project |
| 3 | Editor-pill engine | Hybrid: rule-based for mechanical, LLM for nuanced — *all phrased in Water's voice discipline* |
| 4 | Rabbit hole persistence | SQLite on-device with a memory ceiling + auto-trim |
| 5 | Neutral palette hue | **OpenCode-warm** — h ≈ 0–30, s ≤ 8% on substrates. Warmth comes from `s`, not `h`. |
| 6 | Pill speaker hues | Persona collapses to a 12×12 chip-icon (glyph on persona-hued circle); pill outer hue freed for content signal. **Plus: every pill carries a near-imperceptible gradient animation.** |
| 7 | Pill anchoring | **Precise span highlight required** — hover must light only the trigger phrase, not the paragraph. Mandates a robust anchor-recovery subsystem (see §C.6). |
| 8 | Editor voice | Editor pills (rule-based and LLM) inherit `tone.toml` blacklist + speaker discipline. No "consider rewriting"; phrase as the Editor persona noticing craft. |
| 9 | Logo concept | Three flowing arcs + wordmark, as drafted. Iterate during implementation. |

---

## 1. Track A — Tokens, palette, shading

### A.1 Substrate ladder

OpenCode's discipline is the *near-imperceptible* step between bg layers. Five stops, all warm-neutral. Hue stays near 0 (red/gray) or 20–30 (sand/parchment) at extremely low saturation. The warmth is felt, not seen — putting these tokens against a true neutral, they read as paper.

```css
--water-bg-paper:    hsl(  0, 20%, 99%);  /* main canvas; nearly white, faintest warm tint */
--water-bg-raised:   hsl(  0,  8%, 97%);  /* cards, tiles, pills */
--water-bg-weak:     hsl(  0,  8%, 94%);  /* hover state, secondary panels */
--water-bg-strong:   hsl(  0,  5%, 12%);  /* nav rail bg, code blocks, inverted chips */
--water-bg-stronger: hsl(  0,  6%,  8%);  /* dialog overlay scrim */
```

The 99 → 97 → 94 step is the OpenCode trick: enough to register as elevation under a 1px hairline border, never enough to feel "panel-y". Tested against OpenCode's tokens (h=0, s=20% at 99% L) — same warmth, same near-zero saturation discipline.

### A.2 Hairlines + elevation

Borders are the primary depth tool. Borders carry a *slightly* warmer hue (h=30, s=2%) than the substrate — a trick borrowed from OpenCode that gives them the faintest hint of pencil-line.

```css
--water-hairline:        hsl( 30,  2%, 81%);   /* default 1px border */
--water-hairline-weak:   hsl(  0,  1%, 85%);   /* whisper-line for divider rows */
--water-elev-1:
  0 1px 2px hsl(  0,  5%, 12% / 0.05),
  0 0 0 1px var(--water-hairline);
--water-elev-2:
  0 4px 12px hsl(  0,  5%, 12% / 0.08),
  0 0 0 1px var(--water-hairline);
--water-elev-3:
  0 12px 32px hsl(  0,  5%, 12% / 0.14),
  0 0 0 1px var(--water-hairline);
```

Rule: **never combine elev shadow with a stronger background**. Pick depth or pick contrast, not both. Cards = elev-1 on `bg-raised`; popovers = elev-2 on `bg-paper`; dialogs = elev-3 on `bg-paper` with `bg-stronger` scrim.

### A.3 Foreground

```css
--water-fg-strong:  hsl(  0,  5%, 12%);  /* titles, scene names, primary CTAs */
--water-fg-default: hsl(  0,  1%, 39%);  /* body text */
--water-fg-muted:   hsl(  0,  1%, 55%);  /* meta lines, icon hue */
--water-fg-faint:   hsl(  0,  3%, 78%);  /* placeholders, disabled, dividers */
```

Text on `bg-strong` flips to `fg-inverted: hsl(0, 20%, 99%)`. Direct ports of OpenCode's foreground ladder.

### A.4 Dark mode

```css
--water-bg-paper:    hsl(  0,  9%,  7%);
--water-bg-raised:   hsl(  0,  6%, 10%);
--water-bg-weak:     hsl(  0,  6%, 13%);
--water-bg-strong:   hsl(  0, 15%, 94%);   /* inverted: bright chip on dark canvas */
--water-fg-default:  hsl(  0,  4%, 71%);
--water-fg-muted:    hsl(  0,  2%, 49%);
--water-fg-faint:    hsl(  0,  3%, 28%);
--water-elev-*:      none;                  /* glow carries depth in dark */
```

Dark-mode glow stays — the existing pill fade-in + hue-tinted card backgrounds carry the depth that drop-shadows do in light mode.

### A.5 The signal — sea palettes

A single ramp drives every signal in the app, but the *ramp itself* is one of four sea-adjacent palettes the writer picks per project. All four ramps share the same step count (50–800 + glow), same role assignments, same visual weight — only the hue shifts. This keeps every component agnostic to palette choice.

**A.5.a Deep-sea** (default) — ocean depth, blue-cyan.

```css
--water-sea-50:   hsl(208, 60%, 96%);
--water-sea-100:  hsl(208, 58%, 88%);
--water-sea-200:  hsl(208, 56%, 75%);
--water-sea-300:  hsl(208, 56%, 60%);
--water-sea-400:  hsl(208, 60%, 46%);
--water-sea-500:  hsl(208, 64%, 36%);   /* primary */
--water-sea-600:  hsl(208, 68%, 26%);
--water-sea-700:  hsl(208, 72%, 18%);
--water-sea-800:  hsl(214, 80%, 11%);   /* abyss */
--water-sea-glow: hsl(196, 90%, 64%);   /* peak intensity, active outline */
```

**A.5.b Sunrise** — horizon over water, warm coral-amber.

```css
--water-sea-50:   hsl( 24, 80%, 96%);
--water-sea-100:  hsl( 22, 78%, 88%);
--water-sea-200:  hsl( 20, 74%, 76%);
--water-sea-300:  hsl( 18, 72%, 64%);
--water-sea-400:  hsl( 14, 70%, 52%);
--water-sea-500:  hsl( 10, 72%, 44%);
--water-sea-600:  hsl(  6, 70%, 34%);
--water-sea-700:  hsl(  2, 64%, 24%);
--water-sea-800:  hsl(358, 60%, 14%);
--water-sea-glow: hsl( 32, 96%, 68%);   /* gold sun-flash */
```

**A.5.c Clear** — forest-stream / iOS "Clear" icon translucency, pale green-aqua with high luminance.

```css
--water-sea-50:   hsl(176, 38%, 97%);
--water-sea-100:  hsl(174, 38%, 90%);
--water-sea-200:  hsl(172, 36%, 80%);
--water-sea-300:  hsl(170, 34%, 68%);
--water-sea-400:  hsl(168, 36%, 54%);
--water-sea-500:  hsl(166, 40%, 42%);
--water-sea-600:  hsl(164, 44%, 32%);
--water-sea-700:  hsl(162, 48%, 22%);
--water-sea-800:  hsl(160, 52%, 14%);
--water-sea-glow: hsl(178, 80%, 72%);   /* pale crystal */
```

**A.5.d Earth** — moss, riverbank, peat — green-brown with warm undertone.

```css
--water-sea-50:   hsl( 84, 24%, 96%);
--water-sea-100:  hsl( 82, 22%, 88%);
--water-sea-200:  hsl( 80, 20%, 74%);
--water-sea-300:  hsl( 78, 22%, 58%);
--water-sea-400:  hsl( 76, 24%, 44%);
--water-sea-500:  hsl( 74, 28%, 34%);
--water-sea-600:  hsl( 70, 32%, 24%);
--water-sea-700:  hsl( 66, 36%, 16%);
--water-sea-800:  hsl( 60, 40%, 10%);
--water-sea-glow: hsl( 96, 60%, 60%);   /* fresh moss */
```

**A.5.e Selection mechanism**

Stored on the project (new column `project.palette TEXT NOT NULL DEFAULT 'deep'`). On project open, the renderer sets `[data-palette="deep|sunrise|clear|earth"]` on `<html>`; each variant supplies its own concrete `--water-sea-*` values via CSS attribute selector. Components never reference the variant directly — they reference `--water-sea-300`, etc., and the active palette resolves them.

`--water-sea-500` is always the brand/primary; `--water-sea-glow` is always the peak. Logo, brand buttons, focused borders all use the active palette automatically.

### A.6 Replacing the existing hue tokens

Current `--water-hue-*` tokens (mint, periwinkle, rose, peach, etc.) collapse into the sea-palette ramp. The mapping below is hue-agnostic — substitute `--water-sea-N` and the active palette decides the actual color.

| Old token | Replacement |
|---|---|
| `--water-hue-flow` | `--water-sea-300` |
| `--water-hue-coherence` | `--water-sea-400` |
| `--water-hue-intensity` | `--water-sea-500` |
| `--water-hue-pace` | `--water-sea-200` |
| `--water-hue-valence-pos` | `--water-sea-100` (peak: `--water-sea-glow`) |
| `--water-hue-valence-neg` | `--water-sea-600` |
| `--water-hue-drift` | `--water-sea-700` |
| `--water-hue-muse` / `architect` / `editor` / `cartographer` / `chorus` | (collapses to chip-icon; see §3) |
| `--water-hue-character-1..6` | retained as small accent for character-card avatars only |

### A.7 Migration path

1. Add new `--water-deep-*`, cool-shifted bg/fg/hairline tokens alongside existing ones in `tokens.css`.
2. Keep old `--water-hue-*` as aliases mapped to the closest `--water-deep-*` so nothing breaks during transition.
3. Component-by-component, refactor references. Heatmap is first (largest visual lift). Pills second.
4. Drop the alias layer once nothing references the old names. Run `grep -r "water-hue-flow"` etc. to verify.

---

## 2. Track B — Logo

### B.1 Concept

A flowing stream rendered as three nested arcs of varying weight, suggesting current. Not a literal drop. The name "Water" is wordmark-quiet — the mark itself is a stream, the wordmark sits beside it in IBM Plex Serif with a thin tracking.

### B.2 SVG mark

Two implementations exist, picked by surface:

- **Inline mark** (favicon, app icon, splash): pure SVG, fills container, single color (currentColor).
- **Animated mark** (splash + first-run overlay): same SVG with a subtle `stroke-dashoffset` animation on load — current pulls through.

Sketch (final SVG pinned in implementation):

```
      ╭───╮
   ╭──╯   ╰──╮
──╮ ╰──╮  ╭──╯
  ╰────╯  ╰──
```

Three flowing horizontal curves at decreasing weights (3px, 2px, 1px), stacked with a small vertical gap, slightly offset horizontally to imply motion. Stroke color = `--water-deep-500` on light, `--water-deep-glow` on dark.

### B.3 Wordmark

`water` set in IBM Plex Serif 600, all lowercase, letter-spacing -0.5px, color `--water-fg-strong`. Mark sits to the left at 1.5× the cap height.

### B.4 Where the mark appears

- Window title bar (Tauri custom chrome) — 18×18 currentColor.
- Splash screen — 96×96 animated.
- About dialog — 64×64 static.
- README + docs — 96×96 with wordmark.
- Favicon — 32×32 static.

### B.5 Decision needed

Final SVG is generated during implementation. Sketch above is the intent; we'll iterate on weight and curvature once it's on-screen at 18px.

---

## 3. Track C — Pill UX refinement

The pill engine is the heart of Water. The refinement preserves the architecture (speakers, origin triggers, hue-tinted glow) and tightens the visual language.

### C.1 What changes

- **Size** drops ~15%. Current pills are too tall; truncate metadata, prioritize the message.
- **Persona color** is no longer the dominant signal. The speaker's identity moves to a small chip-icon at the left edge (12×12 round, persona-hued, with a 1-char glyph: E/A/D/C/H for Echo/Architect/Editor/Cartographer/Chorus, or the character's monogram for character speakers).
- **Hue** is freed for *content signal*:
  - **observation** → no border tint, `bg-raised` (default state)
  - **suggestion** → left-rail `--water-sea-300` (1.5px)
  - **warning** → left-rail `--water-sea-600`
  - **praise** → left-rail `--water-sea-glow` at 50% saturation (gentle, non-shouty)
- **Hover state** lifts elevation `elev-1` → `elev-2`, no scale transform (current scale 1.02 feels twitchy).
- **Affordance hint** — a single chevron-down at the right edge appears on hover, signalling "click to deepen" (rabbit hole). Replaces the current hover line that occludes the pill.
- **Living surface** — every pill carries a near-imperceptible gradient animation (see §C.5).

### C.2 What stays

- Speakers (Echo, Architect, Editor, Cartographer, Chorus, character speakers).
- Origin triggers — pills still carry their trigger as a `data-origin` attribute for telemetry.
- The 22-word ceiling on pill prose.
- Pinning, dismissing.
- The hue-tinted card-background glow (now using `--water-deep-*` per content signal instead of per persona).

### C.3 What dies

- Per-persona dominant border colors (rose-gold, sage, lilac, amber, pearl). These persona hues live now only inside the 12×12 speaker chip-icon.
- The hover line + tooltip pattern (replaced by the chevron-down affordance + click-to-open).
- Center-aligned chip text (occasional regression on metric picker — already fixed but worth pinning the rule).

### C.4 Anatomy

```
┌──────────────────────────────────────────────┐
│ ●  she's still avoiding his eyes.            │
│    something she doesn't want him to see. ▾  │
└──────────────────────────────────────────────┘
 │   │                                       │
 │   └─ message (max 22 words, 2 lines)      │
 │                                           └─ chevron affordance, hover-revealed
 └─ speaker chip (persona hue or character monogram)
```

Left-rail color absent on observation; rendered for suggestion/warning/praise.

### C.5 Living surface — subtle gradient animation

Every pill, including editor pills and rabbit-hole children, gets a very-low-amplitude gradient animation on its background. The motion is slow enough to feel ambient — the writer notices it only on second glance.

```css
@keyframes water-pill-breathe {
  0%   { background-position: 0% 50%; }
  50%  { background-position: 100% 50%; }
  100% { background-position: 0% 50%; }
}

.water-pill {
  background:
    linear-gradient(
      110deg,
      var(--water-bg-raised) 0%,
      color-mix(in oklch, var(--water-bg-raised), var(--water-sea-50) 35%) 50%,
      var(--water-bg-raised) 100%
    );
  background-size: 220% 100%;
  animation: water-pill-breathe 14s var(--water-ease-in-out-water) infinite;
}
```

**Discipline:**
- Amplitude is small (background-color moves only ~3–5% toward `--water-sea-50`).
- Period is 14s — slow enough to be ambient, not distracting.
- `prefers-reduced-motion: reduce` disables it entirely.
- Active pill (hovered, focused, or carrying a rabbit-hole thread the user opened) gets a slightly stronger animation (12s period, ~7% amplitude) so it visually breathes more — the user's attention "wakes" the pill.

The animation reuses `--water-sea-50` from the active palette, so a sunrise project breathes amber, a clear project breathes pale aqua, etc.

### C.6 Hover-highlight subsystem (precise anchor recovery)

**Requirement:** hovering a pill highlights *only* the text span that triggered the pill — never the whole paragraph — and the highlight lands on the correct text *every time*, even after the writer has edited.

#### C.6.a Pill anchor payload

Every pill row gains four anchor fields (already partially present; this formalizes them):

```sql
ALTER TABLE pinned_pill ADD COLUMN anchor_block_id TEXT NOT NULL DEFAULT '';
ALTER TABLE pinned_pill ADD COLUMN anchor_snippet TEXT NOT NULL DEFAULT '';
-- The 3–10 word exact phrase from the manuscript at trigger time.
ALTER TABLE pinned_pill ADD COLUMN anchor_block_hash TEXT NOT NULL DEFAULT '';
-- First 80 chars of the block, normalized (lowercase, single-space). Lets us
-- locate the block if its block_id has changed (paragraph split/merge).
ALTER TABLE pinned_pill ADD COLUMN anchor_offset_hint INTEGER NOT NULL DEFAULT 0;
-- Character offset of the snippet within the block at trigger time.
-- Used only as a search starting-point; not authoritative.
```

The same four fields apply to rabbit-hole thoughts and editor pills.

#### C.6.b Resolution algorithm

On hover, the renderer resolves the anchor in this order:

1. **Block-id + snippet substring** — the cheap, common case. Find the block; locate the snippet within it; highlight that exact range.
2. **Block-hash + snippet substring** — if block-id is missing or its content no longer contains the snippet, search blocks whose first-80-char hash matches. Locate snippet there.
3. **Snippet-only fuzzy** — if both fail, run a tolerant search across all blocks for the snippet (allowing ≤2 character-level edits, e.g., a typo correction the writer made since trigger time). Highlight the first match.
4. **Fallback to block + visual marker** — if even fuzzy fails, fall back to highlighting the *block* (paragraph) the block_id pointed to, AND show a small "anchor drifted" pip on the pill so the writer knows the highlight is approximate. They can dismiss and the pill clears itself.

#### C.6.c Visual

```css
.water-trigger-highlight {
  background: color-mix(in oklch, var(--water-sea-200), transparent 70%);
  border-bottom: 1.5px solid var(--water-sea-400);
  border-radius: 2px;
  padding: 0 1px;
  transition: background var(--water-dur-tiny) var(--water-ease-out-soft);
}
```

The highlight uses the active palette so a sunrise project lights triggers in amber, a clear project in aqua. Subtle but legible.

#### C.6.d Implementation notes

- Built as a ProseMirror decoration plugin (paragraph-range highlight from a prior codebase, here scoped to a character range instead of a node range).
- The plugin exposes a single transaction-meta key: `setTriggerHighlight({ blockId, start, end })`. Hover handler computes the range via the resolver, dispatches a transaction with that meta. Plugin applies decoration.
- On pointer-leave or scroll-away, dispatch a clearing transaction.
- Re-resolves on every hover (cheap) so anchors that drift between hovers stay correct.

#### C.6.e Test contract

Anchor resolution is unit-tested with a battery covering: identity hit, paragraph split, paragraph merge, typo correction, partial deletion, full deletion. The fallback path is tested for visual cleanliness (no flicker, no double-highlight).

### C.7 Open decision (closed)

Persona color is retained *inside* the chip backing (glyph on persona-hued circle), not eliminated. Hue outside the chip is freed for content signal.

---

## 4. Track D — Rabbit hole (Midjourney-style deepening)

### D.1 Vision

Every pill is a doorway. Click it → four child thoughts fan out, each a more-specific direction. Click one of those → that child becomes the parent, four new children spawn. The thread of thinking deepens. The writer can mark a child as "this one resonates" → that path informs future pills.

### D.2 Interaction

- Click pill → pill stays in place; a **deepen panel** slides in from the right (or pops over, at 360×680 max), showing four child cards.
- Each child card: same anatomy as a pill (speaker chip + message), but with a `depth` indicator at the top-right (e.g., `↳ depth 1`).
- Click a child → it slides in as the new "parent" at the top of the panel; the old parent compacts into a breadcrumb above; four new children spawn below.
- "Back" arrow at the top steps up the tree. Breadcrumb shows the chain.
- "Pin this thread" button at the top stores the full ancestry to the manuscript's pinned-pills list with a `is_rabbit_hole_root` flag.

### D.3 Data model (v8 migration)

```sql
CREATE TABLE rabbit_thought (
    id TEXT PRIMARY KEY,
    scene_id TEXT NOT NULL REFERENCES scene(id) ON DELETE CASCADE,
    parent_id TEXT REFERENCES rabbit_thought(id) ON DELETE CASCADE,
    -- NULL parent_id = root (the originating pill).
    speaker_kind TEXT NOT NULL,         -- "persona" | "character"
    speaker_id TEXT NOT NULL,
    message TEXT NOT NULL,
    depth INTEGER NOT NULL,             -- 0 for root pill, 1+ for descendants
    siblings_at_depth INTEGER NOT NULL, -- usually 4
    sibling_index INTEGER NOT NULL,     -- 0..3
    resonance INTEGER NOT NULL DEFAULT 0, -- writer's "this one" mark; 0/1
    created_at TEXT NOT NULL
);
CREATE INDEX rabbit_thought_by_scene ON rabbit_thought(scene_id, parent_id);
```

### D.4 Generation prompt

A new task prompt `rabbit_fan_4.toml` produces exactly four siblings:

- Input: parent thought (message + speaker), scene metadata, character sheet (if speaker is character), scene's position in the manuscript arc.
- Output: 4 JSON entries, each a different direction. Constrained to stay in the parent's speaker voice. Each ≤ 22 words.
- The four directions are heuristic-prompted: **closer** (zoom in on the same beat), **wider** (zoom out to the arc), **opposite** (what if the character is wrong about this?), **deeper** (subtext the character isn't admitting). The prompt frames the four directions explicitly so the LLM produces variety, not four near-duplicates.

### D.5 Cost shape

Each click = one LLM call generating 4 messages, ~600 input tokens (parent + scene + sheet + arc summary), ~400 output. At Sonnet 4.6 rates that's ~$0.005/click. A long session of 30 clicks = $0.15. Wire into the existing `LlmBudget` per-session cap (default 20k input / 5k output → raise to 40k input / 15k output to make space for rabbit-hole sessions).

### D.5.a Memory ceiling + auto-trim

The tree lives on-device (SQLite) but with a bounded footprint so it doesn't grow unbounded for prolific writers.

**Hard caps (per project, total):**
- 5,000 `rabbit_thought` rows.
- 25 MB of `message` text (sum of all messages).

**Trim policy (runs on app close + once at startup):**
1. Never trim a thought with `resonance = 1`, nor any of its ancestors. Resonant threads are sacred.
2. Among non-resonant thoughts, prefer trimming the *oldest leaves* (no children) first.
3. If still over the cap after trimming all non-resonant leaves, trim non-resonant interior nodes oldest-first. Reparent any orphaned children to the trimmed node's parent (preserving the depth chain visually, even if context is lost).
4. Log a `trim_event` row with count + freed-bytes so the writer can see if their resonance-marking habits are losing them history.

A user-facing setting (`settings.rabbit_hole_ceiling`) lets the writer raise or lower the cap. Default 5,000 / 25MB; minimum 500 / 2MB; maximum 50,000 / 250MB.

The auto-trim runs in a background task with a single SQLite write transaction, no UI blocking.

### D.6 Rendering

Reuse the existing pill components inside the deepen panel — Bouquet → DeepenPanel sharing internals. Breadcrumb is a horizontal scroller; each crumb shows the speaker chip + truncated message + depth.

### D.7 What "resonance" unlocks

Marking a child as resonant doesn't change generation immediately. It tags the thought row. Future feature: a pill prompt can read the writer's recent resonance picks (last N per scene) as a `voice_preference` signal — the system learns the writer's taste in pills.

---

## 5. Track E — Editor pills (Autocrit/Scrivener-style)

### E.1 Why a separate track

The pill engine is *generative*. Editor pills are *diagnostic*. They surface mechanical and stylistic issues that the writer can fix or dismiss. They're sticky (don't disappear until acted on) and rule-based first, LLM-light second.

**Critical constraint — voice discipline.** Editor pills must keep the writer inside their universe. They never sound like a tooltip from a word processor; they sound like the Editor persona doing its job. This applies to both rule-based and LLM-generated diagnostics. See §E.7 for the voice rules.

### E.2 Rule-based layer (Rust, free, instant)

Runs in `water_core::editor::diagnostics`. Each rule emits zero or more `EditorPill`s.

| Rule | Detection | Severity |
|---|---|---|
| `spelling` | Word not in dict + not capitalized | warning |
| `passive_voice` | `(was|were|been|is|are|be) + past-participle` (standard heuristic) | suggestion |
| `weak_verb` | `(to be) + adjective` where a stronger verb exists | suggestion |
| `adverb_density` | `> 2 -ly adverbs per 100 words` | suggestion |
| `sentence_length_variance` | All 5 consecutive sentences within ±3 words | observation |
| `repetition` | Same word ≥ 4 times in 200 words (excluding stopwords) | suggestion |
| `dialog_tag_overuse` | `said` + adverb (`said quickly`) | suggestion |
| `common_mistake` | `their is`, `could of`, `your welcome` (15-entry lookup) | warning |

The dictionary uses an embedded `hunspell`-compatible affix file; ships with the binary.

### E.3 LLM-polish layer (Sonnet, paragraph-level, optional)

One small pass per *modified paragraph* (debounced 30s after last edit). Asks for one observation that the rule layer can't see — e.g., "the metaphor of drowning recurs three times in this paragraph and once two paragraphs ago." Output ≤ 22 words. Speaker is the **Editor** persona. Marked `editor_polish` origin.

Cost: ~200 input + 50 output per paragraph at the polish trigger rate (debounced). Session cap: 5 polish passes per scene before the writer must explicitly request more (button in the editor pills panel).

### E.4 Surface

Editor pills live in their own column or tab — distinct from the generative pills bouquet so they don't drown out the creative voice. Two interaction patterns:

- **Inline underline** in the manuscript (like spell-check, but in `--water-deep-300` dotted) at the rule's anchor span. Hover → tooltip with the message + accept/dismiss buttons.
- **List view** in a "diagnostics" tab in the right panel — every active editor pill, grouped by rule type, dismissable individually or per-group.

### E.5 Sticky semantics

Editor pills persist in SQLite (`editor_pill` table, v8 alongside rabbit thought). They don't expire on session end. They expire only when:
- The writer dismisses them.
- The anchor span no longer exists in the manuscript.
- The rule re-runs and no longer fires (e.g., the typo was fixed).

Schema:

```sql
CREATE TABLE editor_pill (
    id TEXT PRIMARY KEY,
    scene_id TEXT NOT NULL REFERENCES scene(id) ON DELETE CASCADE,
    rule TEXT NOT NULL,             -- "spelling" | "passive_voice" | etc.
    severity TEXT NOT NULL,
    message TEXT NOT NULL,
    suggestion TEXT,                -- replacement, when applicable
    anchor_block_id TEXT NOT NULL,
    anchor_start INTEGER NOT NULL,
    anchor_end INTEGER NOT NULL,
    text_snippet TEXT NOT NULL,     -- 3-10 word excerpt for anchor recovery
    content_hash TEXT NOT NULL,     -- 50-char paragraph prefix
    dismissed INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX editor_pill_by_scene ON editor_pill(scene_id, dismissed);
```

The `text_snippet` + `content_hash` columns implement a two-layer anchor validation — re-runs locate the right span even if the writer's edits shift offsets.

### E.6 Why Editor not a new speaker

The existing Editor persona is the natural home for diagnostics. We extend its scope (currently editorial craft commentary) to include the rule-based diagnostics. The persona prompt is updated to acknowledge both modes.

### E.7 Voice discipline (the editor stays in the universe)

Rule-based pills are template-string outputs — there's a temptation to write tooltip prose. We resist.

**Rules** (these inherit from `tone.toml`'s blacklist regex and add editor-specific clauses):

- ❌ Never: "Consider rewriting", "Try using", "You should", "This is too...", "Use active voice", "Maybe rephrase as".
- ✅ Always: present-tense observation in the Editor's voice, ≤ 12 words, no second-person directive.

**Rule → message templates** (each rule has 3–5 phrasings, picked round-robin to avoid repetition fatigue):

| Rule | Old "tooltip" phrasing (banned) | New Editor phrasing |
|---|---|---|
| `passive_voice` | "Consider using active voice." | "the verb is asleep here." |
| `passive_voice` | (alt) | "this sentence is being acted upon." |
| `weak_verb` | "Replace 'was' with a stronger verb." | "'was' is doing thin work." |
| `adverb_density` | "Reduce adverbs in this paragraph." | "the prose is over-explaining." |
| `adverb_density` | (alt) | "too many -lys for one breath." |
| `repetition` | "You've repeated 'just' 4 times." | "'just' is showing up a lot. four times in this paragraph." |
| `dialog_tag_overuse` | "Avoid 'said quickly'." | "'quickly' is doing the verb's job." |
| `spelling` | "Possible misspelling: 'wierd'." | "weird, not wierd." |
| `common_mistake` | "Use 'there is' not 'their is'." | "'their is' wants to be 'there is'." |
| `sentence_length_variance` | "Vary sentence length for rhythm." | "the cadence is metronomic right now." |

Each templated message goes through the same blacklist regex check as generative pills before surfacing. A regex hit means the template is broken — we ship without it rather than past it.

**LLM polish prompt** inherits the full Editor persona prompt (including its "won't do" clause from §F.4) and the `tone.toml` clauses. The polish output is treated identically to a generative Editor pill — no special-casing for "editorial" tone.

**Suggestion field** (when applicable, e.g., spelling correction): rendered separately from the message, as a small inline replacement chip. Avoids stuffing the directive into the Editor's voice.

The principle: the writer should feel the Editor sitting at their shoulder, *not* a grammar bot interrupting.

---

## 6. Track F — Prompt overhaul

### F.1 Lessons from prior prompt iterations

What landed (~30% by user judgment):
- **Character-embodied** in first person, 80-char ceiling. Pills felt like the character thinking aloud.
- **textSnippet anchoring** — every insight pointed at a specific phrase. Made the pill feel earned, not generic.
- **Two-layer prompt** — separate system instructions per insight type (character vs craft) instead of one mega-prompt.

What failed (the 70%):
- No character SHEET — the model invented motivations the writer hadn't established.
- No scene context — pills about pacing without knowing where in the arc the scene sat.
- No arc / novel position — character growth comments treated every scene like a turning point.
- Mega-prompt with too many tasks fighting for tokens — model defaulted to safe generic prose.

### F.2 Water's existing advantages

- Character sheets (LSM v2.1) are first-class.
- Scene metadata: POV, location, ordering, beat tags.
- World bible: location entries, lore, established constraints.
- Trigger system: pills only fire when something specific is observed — the prompt knows *why* it was invoked.

### F.3 What changes in prompts

Each task prompt (`pill_level_0`, `pill_expand`, the new `rabbit_fan_4`, plus the editor polish prompt) gets richer context blocks. Pseudo-format:

```
=== System ===
You are {speaker.name}, {speaker.brief}.
Voice: {speaker.voice_guide}
{tone_clauses}

=== Manuscript context ===
Scene: {scene.name} (#{scene.ordering} of {manuscript.scene_count})
Position in arc: {arc_position}  // e.g., "opening sequence", "midpoint pivot", "climax approach"
POV: {pov_character.name}
Location: {location.name} — {location.brief}

=== Character sheet (if speaker is a character) ===
{character.sheet_compact}  // a 200-word distillation generated from LSM v2.1

=== Recent resonance picks (from rabbit hole) ===
{last_3_resonant_thoughts}

=== Excerpt ===
{paragraphs_around_anchor}

=== Task ===
{task.instruction}
```

The `arc_position` is a new derived field — computed locally from `scene.ordering / manuscript.scene_count` mapped to a labeled bucket. Cheap, deterministic, gives the model a frame.

The `character.sheet_compact` is a one-time-per-character distillation cached in SQLite. The sheet itself is long; the compact is what gets injected.

### F.4 Speaker prompt updates

Each persona prompt gets a single-line "what this speaker *won't* do" block (a blacklist pattern that scopes the voice). Examples:

- **Echo**: "Won't analyze. Will notice."
- **Architect**: "Won't praise. Will name the load-bearing structure."
- **Editor**: "Won't soften. Will point at the specific phrase."
- **Cartographer**: "Won't summarize the world. Will point at where it leaks."
- **Chorus**: "Won't take sides. Will reflect the chord."

### F.5 Rabbit hole prompts

New `rabbit_fan_4.toml`: produces 4 child thoughts in the parent's voice, one per direction (closer / wider / opposite / deeper). Output strict JSON, validated against a schema.

New `rabbit_deepen_inherit.toml`: when the writer clicks a child, the next 4 are generated *inheriting* the child's stance — so the thread can sustain a single line of inquiry rather than re-fanning generically each time.

### F.6 Editor polish prompt

New `editor_polish.toml`: one paragraph in, one observation out, ≤ 22 words, in the Editor's voice. Constrained to point at something the rule layer can't see (metaphor recurrence, image collision, tonal mismatch).

---

## 7. Cross-cutting concerns

### G.1 Migration order

`v8` migration introduces both `rabbit_thought` and `editor_pill` tables in a single script. Schema version bumps once.

### G.2 Token rename safety

The alias layer (§A.7) prevents simultaneous-edit breakage. Implementation refactors a component at a time; tests catch token regressions via theme snapshot fixtures.

### G.3 Telemetry

Every pill, rabbit thought, and editor pill carries an origin tag (`trigger`, `parent_id`, `rule`). The existing telemetry pipeline already logs the first; we extend for the other two.

### G.4 Accessibility

- Speaker chip-icons need a `title` and `aria-label` carrying the speaker's full name.
- Left-rail content-signal color must not be the *only* signal — pair with the speaker chip's glyph + a small severity badge for warnings.
- Rabbit hole panel: focusable cards, keyboard nav (J/K to move between siblings, Enter to descend, Esc to ascend).
- Editor underline must respect `prefers-reduced-motion` (no animated dashes).

### G.5 Performance

- Connector arcs (already shipped) remain O(N²) until the manuscript hits 200+ scenes; rabbit-hole tree is bounded by depth × 4 per session, so cheap.
- Editor diagnostics run on text change with a 1.5s debounce; spell-check uses a trie built once at scene load.
- LLM-polish is debounced 30s after last edit + capped per scene.

---

## 8. Phasing

Each phase is a commit-and-ship boundary. Earlier phases unlock later phases.

### Phase 1 — Tokens + heatmap visual (low risk, high visible win)
- Land the new `--water-*` tokens in `tokens.css` alongside aliased legacy ones.
- Refactor heatmap to use the deep-sea-blue ramp.
- Refactor SceneCard hue tint to use the ramp.
- Snapshot tests for theme regressions.

### Phase 2 — Logo
- Generate the final SVG mark, wire into Tauri title bar, splash, About.
- Update favicon.

### Phase 3 — Pill UX refinement
- Reduce pill size, move persona to chip-icon, free hue for content signal.
- Add chevron-down affordance.
- Remove hover-line pattern.
- All existing pill tests updated.

### Phase 4 — Rabbit hole
- v8 migration: `rabbit_thought` table.
- `rabbit_fan_4.toml` + `rabbit_deepen_inherit.toml` prompts.
- DeepenPanel component, breadcrumb, descent + ascent.
- Resonance toggle, persistence.
- Budget integration.

### Phase 5 — Editor pills
- v8 (or v9 if Phase 4 ships first): `editor_pill` table.
- Rust diagnostics module: spelling, passive, weak verb, adverb, repetition, variance, common mistakes.
- Inline underline decoration in ProseMirror.
- Diagnostics tab in right panel.
- Editor polish LLM prompt + 30s debounce trigger.

### Phase 6 — Prompt overhaul
- `character.sheet_compact` derivation + cache.
- `arc_position` derivation.
- Recent-resonance plumb from rabbit hole.
- Every existing task prompt updated to take the richer context block.
- Speaker prompts get the "won't do" line.

---

## 9. Resolved questions (v2)

| # | Question | Resolution |
|---|---|---|
| 1 | Neutral hue | OpenCode-warm (h=0–30, s ≤ 8%) |
| 2 | Persona color in chip | Retained inside chip backing |
| 3 | Editor pills: separate tab vs inline-only | **Both** — inline underline + diagnostics tab |
| 4 | Rabbit hole panel | Side-slide (preserves manuscript context) |
| 5 | Resonance scope | Scene-scoped by default; project-wide opt-in setting `settings.resonance_scope = "scene" \| "project"` |
| 6 | Logo concept | Three flowing arcs as drafted; iterate at 18px before committing |

## 10. Still open (small)

- Heatmap palette switcher UI — chip in settings, or in-canvas chip near the existing metric picker? (Recommend settings; the canvas chip would clutter.)
- "Anchor drifted" pip color — match content-signal warning hue, or use a neutral gray? (Recommend neutral; not the writer's fault.)
- Rabbit hole "back to canvas" gesture — Esc or a back button? (Recommend Esc with button as visual hint.)
