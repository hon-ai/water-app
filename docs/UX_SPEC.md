# Water — UX Spec v1

**Status:** Draft for review. Locks decisions for the visual + interaction overhaul; replaces the M7 polish target with a directed redesign.
**Theme:** Running water. Substrate is cool, near-paper gray. Signal is deep-sea blue. Pills are nuanced annotations; rabbit holes are how thought deepens.
**References:**
- OpenCode (`anomalyco/opencode`) — `packages/web/src/styles/custom.css` for shading discipline.
- (scrubbed-project) (`~/Desktop/(scrubbed-project)`) — `frontend/src/services/openai.ts` for character-embodied + craft prompt patterns; `backend/app/agents/grammar_agent.py` for rule-based editor heuristics.

---

## 0. Decisions locked in this draft

| # | Decision | Choice |
|---|---|---|
| 1 | Spec format | One doc, iterate |
| 2 | Heatmap visual | Single deep-sea-blue scale (metric = stripe position, intensity = saturation) |
| 3 | Editor-pill engine | Hybrid: rule-based + LLM polish |
| 4 | Rabbit hole persistence | Persisted to SQLite from day one (v8 migration) |
| 5 | Neutral palette hue | **OPEN** — needs answer below. Recommend cool blue-gray (h ≈ 215, s ≤ 4%). |
| 6 | Pill speaker hues | **OPEN** — recommend collapse persona color to a small chip-icon, free hue for content signal (warning / suggestion / praise / observation). |

Open items 5–6 are flagged in §1 and §3. Everything downstream can be drafted regardless.

---

## 1. Track A — Tokens, palette, shading

### A.1 Substrate ladder

OpenCode's discipline is the *near-imperceptible* step between bg layers. Five stops, all in the cool blue-gray family. Light mode primary; dark mode mirrors (see A.4).

```css
--water-bg-paper:    hsl(215, 12%, 99%);  /* main canvas; nearly white */
--water-bg-raised:   hsl(215, 10%, 97%);  /* cards, tiles, pills */
--water-bg-weak:     hsl(215,  8%, 94%);  /* hover state, secondary panels */
--water-bg-strong:   hsl(215, 14%, 12%);  /* nav rail bg, code blocks, inverted chips */
--water-bg-stronger: hsl(215, 14%,  8%);  /* dialog overlay scrim */
```

The 99 → 97 → 94 step is the OpenCode trick: enough to register as elevation under a 1px hairline border, never enough to feel "panel-y". The hue (215) sits between azure and steel — reads as cool, never as cyan.

### A.2 Hairlines + elevation

Borders are the primary depth tool. Saturation barely above zero so they don't fight the substrate.

```css
--water-hairline:        hsl(215,  6%, 88%);   /* default 1px border */
--water-hairline-weak:   hsl(215,  6%, 92%);   /* whisper-line for divider rows */
--water-elev-1:
  0 1px 2px hsl(215, 18%, 12% / 0.05),
  0 0 0 1px var(--water-hairline);
--water-elev-2:
  0 4px 12px hsl(215, 18%, 12% / 0.08),
  0 0 0 1px var(--water-hairline);
--water-elev-3:
  0 12px 32px hsl(215, 18%, 12% / 0.14),
  0 0 0 1px var(--water-hairline);
```

Rule: **never combine elev shadow with a stronger background**. Pick depth or pick contrast, not both. Cards = elev-1 on `bg-raised`; popovers = elev-2 on `bg-paper`; dialogs = elev-3 on `bg-paper` with `bg-stronger` scrim.

### A.3 Foreground

```css
--water-fg-strong:  hsl(215, 18%, 10%);  /* titles, scene names, primary CTAs */
--water-fg-default: hsl(215, 10%, 24%);  /* body text */
--water-fg-muted:   hsl(215,  6%, 48%);  /* meta lines, secondary chips */
--water-fg-faint:   hsl(215,  4%, 68%);  /* placeholders, disabled, dividers */
```

Text on `bg-strong` flips: use `fg-inverted: hsl(215, 12%, 96%)`.

### A.4 Dark mode

```css
--water-bg-paper:    hsl(215, 14%,  7%);
--water-bg-raised:   hsl(215, 14%, 10%);
--water-bg-weak:     hsl(215, 14%, 13%);
--water-bg-strong:   hsl(215, 12%, 90%);   /* inverted: bright chip on dark canvas */
--water-fg-default:  hsl(215,  8%, 78%);
--water-fg-muted:    hsl(215,  4%, 56%);
--water-fg-faint:    hsl(215,  3%, 36%);
--water-elev-*:      none;                  /* glow carries depth in dark */
```

Dark-mode glow stays — the existing pill fade-in + hue-tinted card backgrounds carry the depth that drop-shadows do in light mode.

### A.5 The blue (signal color)

A single deep-sea ramp drives every signal in the app:

```css
--water-deep-50:  hsl(208, 60%, 96%);   /* heatmap baseline */
--water-deep-100: hsl(208, 58%, 88%);
--water-deep-200: hsl(208, 56%, 75%);
--water-deep-300: hsl(208, 56%, 60%);
--water-deep-400: hsl(208, 60%, 46%);
--water-deep-500: hsl(208, 64%, 36%);   /* primary brand */
--water-deep-600: hsl(208, 68%, 26%);
--water-deep-700: hsl(208, 72%, 18%);
--water-deep-800: hsl(214, 80%, 11%);   /* deepest, abyss */
--water-deep-glow: hsl(196, 90%, 64%);  /* cyan accent for active glow */
```

`--water-deep-500` is the brand color (logo stroke, primary buttons, active toggles, focused borders). `--water-deep-glow` is used **only** for the heatmap intensity peak and pill-active outline. Everything else stays in the 50–800 ramp.

### A.6 Replacing the existing hue tokens

Current `--water-hue-*` tokens (mint, periwinkle, rose, peach, etc.) get retired or remapped:

| Old token | Replacement |
|---|---|
| `--water-hue-flow` | `--water-deep-300` |
| `--water-hue-coherence` | `--water-deep-400` |
| `--water-hue-intensity` | `--water-deep-500` |
| `--water-hue-pace` | `--water-deep-200` |
| `--water-hue-valence-pos` | `--water-deep-100` (with `--water-deep-glow` for peak) |
| `--water-hue-valence-neg` | `--water-deep-600` |
| `--water-hue-drift` | `--water-deep-700` |
| `--water-hue-muse` / `architect` / `editor` / `cartographer` / `chorus` | (see §3 — collapses to icon, hue freed for content signal) |
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
  - **suggestion** → left-rail `--water-deep-300` (1.5px)
  - **warning** → left-rail `--water-deep-600`
  - **praise** → left-rail `--water-deep-glow` at 50% saturation (gentle, non-shouty)
- **Hover state** lifts elevation `elev-1` → `elev-2`, no scale transform (current scale 1.02 feels twitchy).
- **Affordance hint** — a single chevron-down at the right edge appears on hover, signalling "click to deepen" (rabbit hole). Replaces the current hover line that occludes the pill.

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

### C.5 Open decision

Confirm the persona-collapse before implementation: do you want persona color *eliminated entirely* (chip-icon goes neutral, glyph carries identity), or *retained inside the chip* (glyph on persona-hued circle)?

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

### D.6 Rendering

Reuse the existing pill components inside the deepen panel — Bouquet → DeepenPanel sharing internals. Breadcrumb is a horizontal scroller; each crumb shows the speaker chip + truncated message + depth.

### D.7 What "resonance" unlocks

Marking a child as resonant doesn't change generation immediately. It tags the thought row. Future feature: a pill prompt can read the writer's recent resonance picks (last N per scene) as a `voice_preference` signal — the system learns the writer's taste in pills.

---

## 5. Track E — Editor pills (Autocrit/Scrivener-style)

### E.1 Why a separate track

The pill engine is *generative*. Editor pills are *diagnostic*. They surface mechanical and stylistic issues that the writer can fix or dismiss. They're sticky (don't disappear until acted on) and rule-based first, LLM-light second.

### E.2 Rule-based layer (Rust, free, instant)

Runs in `water_core::editor::diagnostics`. Each rule emits zero or more `EditorPill`s.

| Rule | Detection | Severity |
|---|---|---|
| `spelling` | Word not in dict + not capitalized | warning |
| `passive_voice` | `(was|were|been|is|are|be) + past-participle` (ports (scrubbed-project)'s heuristic) | suggestion |
| `weak_verb` | `(to be) + adjective` where a stronger verb exists | suggestion |
| `adverb_density` | `> 2 -ly adverbs per 100 words` | suggestion |
| `sentence_length_variance` | All 5 consecutive sentences within ±3 words | observation |
| `repetition` | Same word ≥ 4 times in 200 words (excluding stopwords) | suggestion |
| `dialog_tag_overuse` | `said` + adverb (`said quickly`) | suggestion |
| `common_mistake` | `their is`, `could of`, `your welcome` ((scrubbed-project)'s table) | warning |

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

The `text_snippet` + `content_hash` columns mirror (scrubbed-project)'s two-layer anchor validation — re-runs locate the right span even if the writer's edits shift offsets.

### E.6 Why Editor not a new speaker

The existing Editor persona is the natural home for diagnostics. We extend its scope (currently editorial craft commentary) to include the rule-based diagnostics. The persona prompt is updated to acknowledge both modes.

---

## 6. Track F — Prompt overhaul

### F.1 Lessons from (scrubbed-project)

(scrubbed-project)'s prompts that landed (~30% by user judgment):
- **Character-embodied** in first person, 80-char ceiling. They felt like the character thinking aloud.
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

Each persona prompt gets a single-line "what this speaker *won't* do" block, ported from (scrubbed-project)'s blacklist pattern. Examples:

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

## 9. Open questions for redline

1. **Neutral hue (h=215 vs 0 vs custom)** — confirm cool blue-gray substrate or propose alternative.
2. **Persona color in chip** — eliminate entirely, or retain inside chip backing?
3. **Editor pills: separate tab vs inline-only** — current spec has both; one might suffice.
4. **Rabbit hole panel: side-slide vs popover** — side-slide preserves manuscript context; popover is cheaper to build.
5. **Resonance signal scope** — should resonance picks leak across scenes (writer-wide voice preference) or stay scene-scoped?
6. **Logo concept** — three flowing arcs okay, or should I sketch 2-3 alternatives before committing?
