# M2 — Editor & Pill Engine Spec

**Status:** Approved
**Date:** 2026-05-17
**Base:** tag `m1.5.1` (commit `404fadf`)
**Master spec:** `docs/superpowers/specs/2026-05-16-water-design.md` § 4.4, § 6
**Handoff:** `docs/superpowers/handoffs/2026-05-17-m2-handoff.md`

---

## 1. Background

M1 → M1.5.1 built a complete writing-app shell on top of a 16-table SQLite schema, a snapshot scheduler, a sidecar supervisor, an LLM router with five adapters, and a sidebar-first chrome with a Plex Serif title + Plex Sans body. The writer can create projects, scenes, type, save. **There is no pill engine yet** — the editor body is a raw `<textarea>` and the only LLM use is `provider_test`.

M2 builds the engine that defines the product. After M2, idling for three seconds at a paragraph break produces a pastel-glow capsule in the right margin where a named persona (Echo / Architect / Editor / Cartographer / Chorus) observes what just happened. Clicking the capsule opens a vertical bouquet of three sub-pills; clicking a sub-pill expands its own bouquet (unlimited depth, prior siblings collapse to glow-lines on the left edge); a translucent regenerate icon produces three new sub-pills; a pin icon migrates the pill to a half-opacity 56 px column glued to the right edge of `<main>`; an X dismisses the whole thread.

Persona-only pills ship in M2; character voices ride the same engine in M3.

---

## 2. Goals & non-goals

### 2.1 Goals (verbatim from master spec § 4.4)

1. Editor bake-off — ProseMirror vs Lexical, six criteria, winner committed before rest of M2 proceeds.
2. Block editor — paragraph, scene break, dialogue, h2/h3, ordered/unordered list. Block IDs maintained automatically.
3. Typing telemetry — idle detector, cursor classifier (sentence-end / paragraph-end / mid-sentence), structural-inflection detector. Events to Rust core.
4. Pill orchestrator (Rust) — deterministic state machine. Pure, unit-tested.
5. Voice router (Rust) — chooses persona-track speaker. Deterministic for `(trigger, scene_state, project_state)`.
6. Prompt library (Rust) — TOML templates per `(speaker_kind × trigger × task)`. Hot-reloadable in dev. Tone enforced globally.
7. Bouquet generator — exactly N=3 variants in structured JSON. Regenerate adds prior-variant-exclusion.
8. Anti-loop — temperature bump + diverge clause when consecutive bouquets at the same node have ≥ persona-configured token overlap.
9. Pill UI — pastel-glow margin capsule; hover dims rest of page + glow line to snippet; click expands to bouquet of 3; regenerate / pin / X; pinned column at right edge.
10. Rabbit hole — unlimited depth; prior siblings collapse; breadcrumb chain; X closes thread.
11. Persona registry — 5 default named personas (renameable per project), each a distinct hue and tone profile.

### 2.2 Non-goals (deferred to later milestones)

- Character voices (M3 — wired into same engine).
- Conversational Intake popup (M3).
- LSM v2.1 character sheets (M3).
- World Bible (M4).
- Heatmap audiovisualizer (M5).
- Macro spatial scene canvas (M6).
- Bundled local model file in installer (M7 — engine works with any user-configured local provider via existing llamacpp/ollama/mlx adapters; M1.5 eval harness should sweep Kimi and Qwen variants in upcoming iterations so M7 has data when it picks).
- Settings UI for pill cadence / cooldown / sensitivity sliders (M7).
- Per-persona-rename UI (M7 — data path lives in `settings` table per § 6.2 here).
- Plugin loader / dynamic-load API (v2 — seams ship in M2 per § 11).

---

## 3. Hard constraints (inherited from master spec § 1.2 and § 6)

The reviewer will reject any PR that breaches these.

1. **No conversational LLM input.** The LLM sees manuscript text + pill click history only. No chat textbox in this app, hidden or otherwise.
2. **Reactive / observational pill tone.** `you should`, `consider`, `try`, `I think you`, `as an AI` are blacklisted in three layers: prompt clause, `PASS` self-rejection, post-hoc regex filter.
3. **Universe-first, personas-second.** Character voices outrank personas (M3 wires this). Personas are the fallback speaker.
4. **Local-first.** Engine functions against any configured local provider. Cloud is opt-in per project.
5. **Determinism on the orchestrator side.** `(trigger, scene_state, project_state, persona_state)` produces the same prompt assembly + speaker selection. Variation lives only in the LLM's sampling.
6. **Human-readable on disk.** Markdown + TOML are truth. `project.db` is a rebuildable index. M2 introduces no new binary on-disk artifacts that lose information if the DB is deleted.
7. **Plugin seams now, loader in v2.** Trigger / Speaker / PostFilter traits ship in M2. No discovery, no dynamic load.
8. **Pastel-glow visual identity.** All pill UI uses tokens in `app/src/styles/tokens.css`. No shadows < 6 px blur; no 1 px borders; no opaque overlays. Hover dims the rest of the page 8 %; nothing else.
9. **Notion/Apple-fluid motion.** Pill fade-in ~280 ms; bouquet glide ~240 ms ease-out-soft. No bounce, no spring. Reduced-motion → 0 ms (already in tokens).

---

## 4. Architecture overview

```
┌──────────────────────────── renderer ────────────────────────────┐
│  ProseMirror block editor                                         │
│      │  typing:telemetry  (~10 Hz when active)                    │
│      │                                                            │
│      ▼                                                            │
│  ┌──────────────────────── Tauri event bus ──────────────────┐   │
│  │  bidirectional: renderer ↔ water-app (core)               │   │
│  └──────────────────────────────┬─────────────────────────────┘   │
│      ▲                          │                                 │
│      │  pill:emerged,           │  typing:telemetry,              │
│      │  bouquet:ready,          │  pill:click, pill:expand,       │
│      │  sidecar:status, ...     │  pill:regenerate, pin, dismiss  │
└──────│──────────────────────────┼─────────────────────────────────┘
       │                          ▼
       │                  ┌────────────────────────────────────────┐
       │                  │ water-core::orchestrator (state machine)│
       │                  │   ├─ trigger trait + 10 built-ins       │
       │                  │   ├─ eviction (FIFO, max 2 on-screen)   │
       │                  │   └─ anti-loop (Jaccard)                │
       │                  └────────────────┬───────────────────────┘
       │                                   ▼
       │                  ┌────────────────────────────────────────┐
       │                  │ water-core::voice (router + registry)  │
       │                  │   ├─ Speaker trait                      │
       │                  │   └─ persona TOML loaders               │
       │                  └────────────────┬───────────────────────┘
       │                                   ▼
       │                  ┌────────────────────────────────────────┐
       │                  │ water-core::prompts (loader + assembler)│
       │                  │   tone + speaker + trigger + task + in │
       │                  └────────────────┬───────────────────────┘
       │                                   ▼
       │                  ┌────────────────────────────────────────┐
       │                  │ water-core::llm::router                 │
       │                  │   structured-JSON mode, retries, CB     │
       │                  └────────────────┬───────────────────────┘
       │                                   ▼
       │                  ┌────────────────────────────────────────┐
       │                  │ PostFilter chain (tone blacklist, ...)  │
       │                  └────────────────┬───────────────────────┘
       │                                   ▼
       └──────────────────────── pill:emerged / bouquet:ready ──────
```

Plus the analysis stream:

```
sidecar (FastAPI /analyze)  ──► analysis:update event  ──►  orchestrator (Stream B input)
```

Everything left of the LLM call is pure and unit-testable with a fake provider. The orchestrator never blocks on the LLM — generation runs in a tokio task and dispatches a `bouquet:ready` event when done.

---

## 5. Editor & telemetry

### 5.1 Bake-off (Phase B.1)

Two prototypes built in parallel under `app/src/editor-bakeoff-{pm,lexical}/`, each a standalone harness with:

- Paragraph + h2 + h3 + ordered/unordered list + scene-break + dialogue blocks
- Block IDs (`^bk-XXXX`) assigned on insert, preserved across split/merge/delete (verified by a 50-step random-edit fuzz test)
- A decoration API exercise: highlight a random paragraph with a fake "pill underline" + a soft outer-glow box on a different paragraph
- A 50 000-word load test (paste lorem; measure typing latency at the cursor)
- Long-undo stability (200 undo/redo steps, no decoration drift)

Scoring criteria (6, weighted equal):

1. Block-ID maintenance ergonomics (how natural is the idiom)
2. Decoration API for pill highlights + snippet underlines
3. Selection/mark stability under autosave write-backs (re-applying serialized doc state without losing the cursor)
4. Bundle size impact (gzipped delta on `pnpm --filter @water/app build`)
5. 50 k-word perf (median + p95 keypress-to-paint latency)
6. Long-undo behavior (no decoration leaks / no ID renumbering)

**Tie-breaker:** if the criteria do not produce a clear winner, **ProseMirror is selected**. Rationale: decoration API is the most battle-tested for our anchor-stable use case, block-id idiom via `nodeSpec.attrs` + transaction filter is well-trodden, 50k-word perf path is established. Lexical's wins (bundle size, React-native ergonomics) matter less in a desktop Tauri app.

The loser is committed to a `bakeoff/loser-{name}` branch with notes; harness directory is deleted after.

### 5.2 Block editor (Phase B.2)

Block kinds shipped in M2:

| Kind | Markdown | Notes |
|---|---|---|
| paragraph | `paragraph text\n` | most common; block-id at start `^bk-XXXX` |
| heading_2 | `## heading\n` | chapter heading inside a scene is rare; reserved |
| heading_3 | `### heading\n` | section break with title |
| scene_break | `---\n` | hard scene divider within a scene file (uncommon) |
| dialogue | `"..." attribution\n` | paragraph variant; styled differently |
| list_ordered | `1. item\n` | items each carry their own block-id |
| list_unordered | `- item\n` | items each carry their own block-id |

Block IDs are assigned on block insert and preserved across split, merge, and delete via the chosen editor's transaction filter (PM) or node-mutation listener (Lexical). The existing `crate::block::ensure_block_ids` Rust helper (idempotent, tolerant of pre-existing IDs per KNOWN_FRAGILE #2) runs on serialize to guarantee IDs land in the persisted Markdown.

### 5.3 Typing telemetry (Phase B.3)

Emitted as Tauri events from renderer to `water-app` (which forwards to the orchestrator):

```ts
type TypingTelemetry = {
  idle_for_ms: number;              // 0 if actively typing
  cursor_classification:
    | "at_sentence_end"
    | "at_paragraph_end"
    | "mid_sentence";
  block_id: string;                 // ^bk-XXXX of cursor's current block
  recent_word_delta: number;        // words added in last 10 s
  structural_inflection:            // most-recent detected; "none" if stale
    | "pov_change"
    | "location_change"
    | "new_scene"
    | "new_chapter"
    | "none";
};
```

**Cursor classifier (hybrid rule):**

```
cursor is at sentence end iff:
  (cursor is at end of line)
  OR
  (the last non-whitespace token matches /[.!?][")\]]?$/)
```

Single classifier across all block kinds. Correct for dialogue (`"I love you," she said.` — period at EOL = sentence-end; `,"` mid-line = mid-sentence), list items (cursor at EOL of `- Buy milk` = sentence-end), and headings (EOL = sentence-end). Mid-sentence typing **never** surfaces a pill.

**Idle detector:** 3 s with no keypress. Tick rate during active typing: ~10 Hz (cheap event payload; fire-and-forget).

**Structural inflection detector:**

- `new_scene` and `new_chapter` are user-initiated: emitted directly by the editor when the writer inserts a scene-break block or a heading-2 block.
- `pov_change` and `location_change` are sidecar heuristics: pattern match on pronoun shifts and named-place mentions; emitted as part of the sidecar's `/analyze` response and forwarded through the event bus.
- All four are always emitted as candidates. The orchestrator weights them: **priority × 1.5** when scene metadata (`pov_character_id`, `location_id`) is set and the detected inflection deviates from it; **priority × 0.6** when the corresponding metadata is null (eliciting-mode territory, lower priority so other triggers can win).

---

## 6. Pill orchestrator (Rust core)

### 6.1 Module layout

```
crates/water-core/src/orchestrator/
├── mod.rs              # public API: Orchestrator, OrchestratorEvent
├── state.rs            # state machine: Idle / Listening / Evaluating / Generating / Surfacing / OnScreen / Pinned / Dismissed / Expired / Evicted
├── triggers/
│   ├── mod.rs          # Trigger trait + factory
│   ├── block_anchored_drift.rs
│   ├── scene_flow_dip.rs
│   ├── topic_drift.rs
│   ├── valence_spike.rs
│   ├── structural_inflection.rs
│   ├── pace_floor.rs
│   ├── world_drift.rs
│   ├── character_dissonance.rs  # stub; M3 wires sheets
│   ├── no_universe_yet.rs
│   └── idle_pause_with_present_character.rs  # stub; M3 wires characters
├── eviction.rs         # FIFO eviction (max 2 on-screen)
└── anti_loop.rs        # Jaccard overlap on stopword-stripped lemma sets
```

### 6.2 State machine

```
            ┌──────── typing:telemetry / analysis:update
            │
            ▼
         Idle ──► Listening ──► Evaluating (trigger.evaluate for each Trigger)
                                    │
                            (no candidate)       (candidate)
                                    │                │
                                    ▼                ▼
                                  Idle           Generating (router → prompts → LLM in tokio task)
                                                       │
                                            (LLM result / PostFilter)
                                                       │
                                                       ▼
                                                  Surfacing (pill:emerged event)
                                                       │
                                                       ▼
                                                  OnScreen ──► Pinned (user click pin)
                                                       │
                                                       │── Dismissed (user click X)
                                                       │── Expired (soft TTL elapsed)
                                                       └── Evicted (newer candidate, FIFO)
```

State transitions are pure functions of `(current_state, event)`. The orchestrator owns no I/O; all I/O lives in services it consumes (router, prompts, post-filter, persistence). Unit tests drive it with synthetic event streams and a fake LLM provider; the state machine produces a deterministic event log.

### 6.3 Trigger trait

```rust
pub struct TriggerContext<'a> {
    pub telemetry: &'a TypingTelemetry,
    pub analysis: &'a AnalysisSnapshot,
    pub scene: &'a SceneSnapshot,
    pub project: &'a ProjectSnapshot,
}

pub struct TriggerCandidate {
    pub trigger_id: &'static str,
    pub priority: f32,                      // 0.0 .. 10.0
    pub preferred_track: SpeakerTrack,      // Persona / Character / Either
    pub reason: TriggerReason,              // structured payload for voice router
    pub block_target_id: Option<String>,    // ^bk-XXXX if block-anchored
}

pub trait Trigger: Send + Sync {
    fn id(&self) -> &'static str;
    fn evaluate(&self, ctx: &TriggerContext<'_>) -> Option<TriggerCandidate>;
}

pub fn builtin_triggers() -> Vec<Box<dyn Trigger>>;
```

Per-track cooldowns live on the **Speaker** (not the trigger): a speaker's `cooldown_ms()` is the minimum gap before that speaker can fire again. The orchestrator skips a candidate whose chosen speaker is currently cooled down and tries the next-best candidate this tick.

### 6.4 Eviction

FIFO. Max two pills on screen at once. When a third candidate is promoted, evict the older of the two visible pills. Pinned pills do not count toward the 2-max (they live in the right column). Dismissed/expired pills exit normally. Eviction emits `pill:evicted` so the renderer can animate the fade-out.

### 6.5 Anti-loop

Computed on regenerate (and on every bouquet response as a sanity check). For each new variant text, compute Jaccard overlap against each prior variant in this bouquet's history:

```
tokens(text) = strip_punctuation(lowercase(text))
             |> tokenize_on_whitespace
             |> filter(t => t not in STOPWORDS)
             |> map(t => strip_suffix(t, ["s","es","ed","ing","ly"]))

jaccard(a, b) = |tokens(a) ∩ tokens(b)| / |tokens(a) ∪ tokens(b)|
```

Stopword list: ~120 common English function words shipped as `crates/water-core/data/stopwords-en.txt`. Suffix-stripper is a fixed-table heuristic (no Porter stemmer; cheaper, deterministic, English-only — see KNOWN_FRAGILE expected entry #8).

**Threshold:** per-speaker. Read from speaker's TOML `anti_loop_threshold` (default 0.70 when absent). When `max(jaccard(new, prior)) ≥ threshold`, the orchestrator single-retries with `temperature += 0.15` and adds a diverge clause to the prompt. Second failure: the bouquet ships with whatever the second call returned (we don't block UX on a perfect bouquet).

---

## 7. Voice router & persona registry

### 7.1 Module layout

```
crates/water-core/src/voice/
├── mod.rs              # public API: VoiceRouter
├── router.rs           # implements master spec § 6.2 algorithm
├── registry.rs         # PersonaRegistry, CharacterRegistry (stub for M3)
└── speaker.rs          # Speaker trait + PersonaSpeaker / CharacterSpeaker impls
```

### 7.2 Speaker trait

```rust
pub enum SpeakerKind { Persona, Character }
pub enum SpeakerTrack { Persona, Character, Either }

pub trait Speaker: Send + Sync {
    fn id(&self) -> &str;
    fn kind(&self) -> SpeakerKind;
    fn display_name(&self) -> &str;
    fn hue_token(&self) -> &str;          // e.g. "--water-hue-muse"
    fn prompt_fragment(&self) -> &str;    // voice profile, from TOML
    fn anti_loop_threshold(&self) -> f32; // default 0.70
    fn cooldown_ms(&self) -> u64;         // default 45_000
}
```

`PersonaSpeaker` loads from `prompts/speakers/persona/*.toml` (see § 8). `CharacterSpeaker` is a stub in M2 — the trait exists, the registry exists, but `CharacterRegistry::list()` always returns empty. M3 fills it from LSM v2.1 sheets.

### 7.3 Routing algorithm

Implements master spec § 6.2 verbatim. The router is a pure function:

```rust
pub fn route(
    candidate: &TriggerCandidate,
    scene: &SceneSnapshot,
    characters: &CharacterRegistry,
    personas: &PersonaRegistry,
    cooldowns: &CooldownState,
) -> Option<Arc<dyn Speaker>>
```

Tie-break: least-recently-used among non-cooled-down candidates. Determinism is verified by a property test that runs the router with shuffled candidate lists and asserts identical output.

### 7.4 Persona registry

Five built-in personas, each a TOML file under `prompts/speakers/persona/`:

| File | Persona | Hue token | Role |
|---|---|---|---|
| `echo.toml` | Echo | `--water-hue-muse` | muse; emerges what's almost-there |
| `architect.toml` | Architect | `--water-hue-architect` | structure / pace |
| `editor.toml` | Editor | `--water-hue-editor` | diction / clarity |
| `cartographer.toml` | Cartographer | `--water-hue-cartographer` | world / setting consistency |
| `chorus.toml` | Chorus | `--water-hue-chorus` | eliciting mode only |

**Per-project rename.** Renaming a persona stores `{persona_id, display_name}` in the `settings` table keyed as `persona.rename.<persona_id> = "<new name>"`. `PersonaSpeaker::display_name()` consults the project settings first, falls back to the TOML default. Renaming does not affect prompt assembly — the persona's `prompt_fragment` is hue-and-voice, not name.

### 7.5 Persona TOML schema

```toml
# prompts/speakers/persona/echo.toml
version = "1"
id = "echo"
display_name = "Echo"
hue_token = "--water-hue-muse"
anti_loop_threshold = 0.70
cooldown_ms = 45000

[prompt]
voice_profile = """
You are Echo. You speak gently and rarely, as if listening through fog.
You notice what is almost-there in the prose: a feeling almost named,
a rhythm almost found. You never instruct.
"""
```

---

## 8. Prompt library

### 8.1 Module layout

```
crates/water-core/src/prompts/
├── mod.rs              # public API: PromptLibrary, PromptRequest
├── loader.rs           # TOML loading + dev hot-reload (cfg(debug_assertions))
└── assembler.rs        # tone + speaker + trigger + task + inputs
```

### 8.2 File layout

```
prompts/
├── tone.toml                           # global tone clauses, blacklist
├── anti_loop.toml                      # diverge-clause snippets used on regenerate retry
├── speakers/
│   ├── persona/
│   │   ├── echo.toml
│   │   ├── architect.toml
│   │   ├── editor.toml
│   │   ├── cartographer.toml
│   │   └── chorus.toml
│   └── character/
│       └── template.toml               # M3 will render this per LSM sheet
├── triggers/
│   ├── block_anchored_drift.toml
│   ├── scene_flow_dip.toml
│   ├── topic_drift.toml
│   ├── valence_spike.toml
│   ├── structural_inflection.toml
│   ├── pace_floor.toml
│   ├── world_drift.toml
│   ├── character_dissonance.toml
│   ├── no_universe_yet.toml
│   └── idle_pause_with_present_character.toml
└── tasks/
    ├── pill_level_0.toml               # single pill on first surface
    ├── pill_expand.toml                # bouquet of 3 sub-pills
    ├── pill_regenerate.toml            # bouquet excluding prior variants
    ├── scene_summary.toml              # M5 will use
    └── beat_label.toml                 # M5 will use
```

Files are loaded at startup via `PromptLibrary::load(project_root)`. In `cfg(debug_assertions)` builds, a `notify`-based watcher reloads any TOML file when it changes on disk (dev convenience; not used in release).

### 8.3 Assembly

A `PromptRequest` is built by composing:

1. `tone.toml` global clauses (always included)
2. The chosen speaker's `prompt.voice_profile`
3. The trigger TOML's `framing` clause
4. The task TOML's `instruction` + `output_format`
5. Inputs (manuscript excerpt, scene metadata excerpt, character/world snippets as relevant)

Target ~600 tokens including the manuscript excerpt for level-0 pills.

### 8.4 Global tone (excerpt of `tone.toml`)

```toml
version = "1"

[clauses]
present_tense = "Speak in present tense as if you are noticing this just now."
not_assistant = "You are not an assistant. You do not give writing advice."
blacklist = "Never say: 'you should', 'consider', 'try', 'maybe you could', 'I think you', 'as an AI', 'this is good', 'this is bad'."
observe = "Observe, react, wonder. Leave space."
shape = "Output exactly one line of prose, ≤ 22 words. No quotation marks. No emoji."
pass = "If you cannot react in your speaker's voice without breaking these rules, output the single token `PASS` and nothing else."

[blacklist_regex]
# Used by the post-hoc PostFilter; compiled at load time.
patterns = [
  "(?i)\\byou should\\b",
  "(?i)\\bconsider\\b",
  "(?i)\\btry\\b",
  "(?i)\\bi think you\\b",
  "(?i)\\bas an AI\\b",
  "(?i)\\bmaybe you could\\b",
]
```

`PASS` handling: a `PASS` response triggers one retry at a different sampling temperature; a second `PASS` quietly drops the pill (orchestrator returns to Idle without surfacing).

### 8.5 Bouquet generation

On click of a level-0 pill (Phase F integration):

1. Orchestrator calls `pill_expand` task with: original pill text, scene excerpt, speaker definition.
2. Router requests structured JSON (see § 9 for the structured-JSON router extension).
3. Provider returns `[{"angle": "...", "text": "..."}, {"angle": "...", "text": "..."}, {"angle": "...", "text": "..."}]` — exactly 3 items.
4. Anti-loop check (§ 6.5) runs against any prior variants stored in session-scoped `bouquet_history` keyed by parent pill id.
5. PostFilter chain runs against each variant; violators drop, replaced by `PASS` retries up to 1.
6. Renderer renders the bouquet inline below the parent pill with soft glide-in.

### 8.6 Regenerate

`pill_regenerate` task adds a `previous_variants_first_words: [...]` clause carrying the first 8 words of each prior variant in this bouquet's history (across all regenerates for this pill — within session only; cleared at project close per § 11). The task prompt instructs the model to produce 3 new angles materially different in opening and idea. Anti-loop retry kicks in if overlap exceeds the speaker's threshold.

---

## 9. LLM router extension — structured JSON

### 9.1 New trait method

```rust
#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn generate(&self, req: GenerateRequest) -> Result<String>;

    // NEW in M2:
    async fn generate_structured<T: DeserializeOwned + Send>(
        &self,
        req: GenerateRequest,
        schema: &JsonSchema,
    ) -> Result<T>;
}
```

### 9.2 Per-adapter strategy

- **OpenAI:** native `response_format: { type: "json_schema", json_schema: ... }`.
- **Anthropic:** native tool-use with a single forced tool whose input matches the schema.
- **llamacpp / ollama / mlx:** prompt-engineering + JSON-grammar constraint where the adapter supports it (llama.cpp's GBNF; ollama's `format: "json"`). One retry on malformed parse before falling back to plain-text and a hand-parsed result.

### 9.3 PostFilter chain

```rust
pub enum FilterDecision { Pass, Drop { reason: &'static str } }

pub trait PostFilter: Send + Sync {
    fn id(&self) -> &'static str;
    fn evaluate(&self, pill: &PillCandidate) -> FilterDecision;
}

pub fn builtin_post_filters() -> Vec<Box<dyn PostFilter>>;
```

Built-in: `ToneBlacklistFilter`, compiles regexes from `tone.toml::blacklist_regex.patterns`. A dropped pill is logged to replay (if enabled) and the bouquet position is back-filled with a retry. After 1 retry, that position drops; bouquet may render with 2 instead of 3 in degenerate cases (this is acceptable — the user sees a slightly smaller bouquet, no error).

---

## 10. Pill UI

### 10.1 Component layout

```
app/src/pill/
├── PillLayer.tsx           # absolute-positioned overlay to right of canvas
├── PillCapsule.tsx         # the pastel-glow level-0 capsule
├── Bouquet.tsx             # 3 sub-capsules + regenerate + pin + X
├── RabbitHole.tsx          # depth > 0 view; sibling glow-lines on left edge
├── PinnedColumn.tsx        # 56 px column glued to right edge of <main>
├── PinnedPillDetail.tsx    # sheet shown on click of a pinned pill
└── hover-dim.tsx           # global hover-dim + glow-line overlay
```

### 10.2 Margin layout

`<main>` keeps the existing 720 px-max text column centered. `PillLayer` is `position: absolute; top: 0; right: 0; pointer-events: none;` overlaying `<main>`. Pill capsules float to the right of the text column at the y-coordinate of their anchored block. The 56 px `PinnedColumn` is glued to the right edge of `<main>` as a separate always-allocated absolute layer at half-opacity.

**Narrow-viewport fallback** (when `<main>` width < 1100 px): pill capsules get an extra `opacity: 0.7` and may overlap the rightmost ~120 px of the canvas. Pinned column collapses to a 24 px tab; click expands to overlay full pinned-column at full width with a backdrop.

### 10.3 Visuals & motion

- **Capsule** — 12 px height auto-grown to text, padding `8px 14px`, `border-radius: var(--water-r-16)`. Background: `color-mix(in oklch, var(--<persona-hue>) 35%, var(--water-bg-paper))`. Outer glow: `box-shadow: 0 0 24px var(--<persona-hue>)`. Soft fade-in over `var(--water-dur-small)` (240 ms) with `var(--water-ease-out-soft)`. (Spec § 9 says ~280 ms; we use the closest existing token, 240 ms.)
- **Hover** — `<main>` opacity drops to 0.92; a soft 1 px glowing line draws from the right edge of the capsule to the left edge of its anchored block's bounding rect. Line is SVG, stroke `var(--<persona-hue>)`, `stroke-opacity: 0.6`, fade-in 160 ms.
- **Bouquet expansion** — capsule swaps in-place for a vertical stack of 3 sub-capsules + a translucent regenerate icon (`<RefreshCw>`) + pin icon (`<Pin>`) + X icon (`<X>`). Stack glides in over 240 ms with `var(--water-ease-out-soft)`.
- **Sub-capsule angle hue** — each of the 3 sub-capsules shifts its hue ±6° from the persona hue (using `color-mix` with `--water-hue-pace` for warm-angle, `--water-hue-coherence` for cool-angle, parent persona hue for center-angle). The `angle` field from the bouquet response drives the choice; not shown as text.

### 10.4 Rabbit hole

Clicking a sub-capsule re-fires `pill_expand` with that sub-capsule's text as parent context. The expansion:

- Renders a new bouquet below the clicked sub-capsule.
- Prior siblings (the other two sub-capsules in the just-clicked bouquet) collapse to thin (~2 px) glow lines on the left edge of the rabbit-hole panel.
- A breadcrumb chain renders along the top: `Echo · "..." > Architect · "..."` (truncated speaker + first 6 words of each level).
- Only the current bouquet is fully rendered; scroll up at any time re-expands prior siblings inline (this is a stretch UX detail — first cut just shows them as glow-lines; click to re-expand).
- X at the top right closes the entire thread (back to a Pill capsule on the manuscript margin); the breadcrumb chain disappears.

**Bouquet history bound:** each rabbit hole keeps the last 20 levels of context in the prompt; older levels collapse into a 1–2 sentence "summary of the path so far" inserted as background. This is implemented in `prompts/assembler.rs::summarize_path_if_deep`.

### 10.5 Pinned column

- Width 56 px, always-allocated, glued to right edge of `<main>`.
- Each pinned pill renders as a soft glowing dot (16 px, hue = speaker hue) at 50 % opacity, vertically stacked top-down in pin order.
- Hover expands to a 240 px-wide peek showing speaker name + first 12 words of pill text, opacity → 1.0.
- Click opens a `PinnedPillDetail` sheet (re-uses M1.5 `Sheet` primitive + the slide-in transform fix landing in this phase) showing the full bouquet at pin time, the breadcrumb chain (if pinned from a rabbit hole), and an "Un-pin" button. Click outside or X dismisses the sheet without un-pinning.

### 10.6 Anchoring under live edits

Pill anchors use the snippet-as-canonical rule (master spec § 3.3). If the writer edits the anchored block such that the snippet no longer matches, the anchor degrades to "block-id only" and the glow-line falls back to the block's mid-point. If the block itself is deleted, the pill drops (orchestrator emits `pill:expired`).

### 10.7 Reduced motion

`@media (prefers-reduced-motion: reduce)` is already wired in tokens (all durations → 1 ms). Pill UI inherits this automatically. Test asserts a pill emerges synchronously in reduced-motion mode.

---

## 11. Persistence & schema migration

### 11.1 Migration runner

A small runner reads `schema_version` and applies pending `vN_*.sql` files forward-only on `open_project`. Lives in `crates/water-core/src/migrations.rs`:

```rust
pub fn run_pending_migrations(db: &mut Connection) -> Result<()>;
```

Migrations registered as a compile-time slice: `&[("v2_pill_engine", include_str!("../sql/v2_pill_engine.sql"))]`. Forward-only; no down-migrations in M2.

### 11.2 Schema v2 — `v2_pill_engine.sql`

Extends `pinned_pill`:

```sql
ALTER TABLE pinned_pill ADD COLUMN parent_pill_id TEXT NULL
    REFERENCES pinned_pill(id) ON DELETE SET NULL;
ALTER TABLE pinned_pill ADD COLUMN pinned_at TEXT NOT NULL DEFAULT '';
ALTER TABLE pinned_pill ADD COLUMN trigger_class TEXT NOT NULL DEFAULT '';
ALTER TABLE pinned_pill ADD COLUMN bouquet_position INTEGER NULL;

UPDATE pinned_pill SET pinned_at = created_at WHERE pinned_at = '';

UPDATE schema_version SET version = 2;
```

The `DEFAULT ''` + `UPDATE` dance is required because SQLite can't add a NOT NULL column without a default; we backfill `pinned_at` from `created_at` for any pre-existing rows, then enforce non-empty going forward at the Rust layer.

### 11.3 Session-only state

These never touch disk (per Q3 decision in brainstorm):

- Live (un-pinned, un-dismissed) pills
- Bouquet history
- Rabbit-hole breadcrumb context
- Prior-variant exclusion lists for regenerate
- Anti-loop temperature-bump counters
- Speaker LRU table (cooldown tracking)

On project close: all dropped. On project reopen: pinned pills load from `pinned_pill`; orchestrator starts fresh.

### 11.4 Replay logs (opt-in)

JSONL at `.water/log/llm/*.jsonl`, opt-in via a `settings.replay_log_enabled` boolean (Settings UI in M7; M2 ships the boolean default-false + a hidden dev override via env var `WATER_REPLAY_LOG=1`). One file per session, named by ULID. Each line: `{ts, kind, request, response, post_filter_decision, anti_loop_overlap?}`. Used by the tone-audit nightly job and eval harness.

---

## 12. Tone enforcement & audit

### 12.1 Three layers

1. **Prompt clause** — `tone.toml` clauses always concatenated into the prompt.
2. **Model `PASS`** — model self-rejects with `PASS` if it can't produce a clean pill. One retry at different temperature; second `PASS` drops.
3. **Post-hoc `PostFilter`** — regex blacklist runs against returned text. Match → drop + log to replay; bouquet position back-fills via one retry.

### 12.2 Audit harness

Lives at `crates/water-core/src/tone_audit/`. Two entry points:

- `tone_audit::run_gate(fixtures_path, llm_provider) -> AuditReport` — runs the 200-pill fixture sweep; gate requires `report.layer3_catches == 0 && report.audit_violations == 0`.
- `tone_audit::run_nightly(fixtures_path, llm_provider) -> AuditReport` — same sweep; writes scorecard to `eval/tone_audit/scorecards/{YYYY-MM-DD}-{model_id}.json` for trend tracking.

### 12.3 Fixtures

Committed at `eval/tone_audit/fixtures/`. 200 entries spanning all trigger classes × all 5 personas × a curated set of manuscript excerpts. Each entry:

```json
{
  "id": "tone-001",
  "trigger": "block_anchored_drift",
  "speaker": "editor",
  "scene_excerpt": "...",
  "expected_pass": true
}
```

Fixture generation: an offline script (committed) uses the eval harness's existing infrastructure to assemble representative excerpts from public-domain prose corpora.

### 12.4 Gate placement

- **One-time gate** before tagging `m2`: `tone_audit::run_gate` must return 0/0.
- **Nightly job** from M2 onward: GitHub Actions workflow (`/.github/workflows/tone-audit.yml`) runs `run_nightly` against the current default provider + tracks leak rate. Failing nightly does not block CI but raises an issue tag.
- **CI smoke** on every PR: a 5-pill subset via `cargo test --features tone-audit-smoke`. Fast, deterministic with a fake provider.

---

## 13. Plugin contract surface

Three traits in `water-core`, each consumed by a factory function. M2 ships built-ins only; v2 will swap the factory to also include dynamically-loaded impls.

```rust
// triggers
pub trait Trigger: Send + Sync { /* see § 6.3 */ }
pub fn builtin_triggers() -> Vec<Box<dyn Trigger>>;

// speakers
pub trait Speaker: Send + Sync { /* see § 7.2 */ }
pub fn builtin_personas(project_root: &Path) -> Result<Vec<Arc<dyn Speaker>>>;

// post-filters
pub trait PostFilter: Send + Sync { /* see § 9.3 */ }
pub fn builtin_post_filters() -> Vec<Box<dyn PostFilter>>;
```

All three trait signatures are frozen by the M2 tag. v2 plugin loader will add `load_plugin_triggers(manifest)` etc. that append to the factory output. No registry-struct ceremony in M2 — keep the surface minimal.

---

## 14. Carried-over debt from M1.5 review

All four fold into M2 at their natural phase:

### 14.1 KNOWN_FRAGILE #7 — scene write-race (Phase A foundation)

Introduce `SceneWriteLocks: Arc<DashMap<Id, Arc<tokio::Mutex<()>>>>` on `OpenProject`. Both `SceneStore::rename` and `SceneStore::write_body` acquire the per-scene lock before any disk read/write. Orchestrator correctness depends on this: prompt assembly reads scene text, and a concurrent write would tear.

```rust
let lock = open_project.scene_write_locks.entry(scene_id.clone())
    .or_insert_with(|| Arc::new(tokio::Mutex::new(())))
    .value()
    .clone();
let _guard = lock.lock().await;
// ... read/mutate/write whole file ...
```

Drop the `dashmap` entry on scene delete (lock is no longer needed). Test: spawn 100 concurrent `rename` + `write_body` against the same scene, assert no torn writes.

### 14.2 KNOWN_FRAGILE #6 — sidecar respawn (Phase A foundation)

Replace `SidecarSupervisor`'s 3-strike-break with respawn-with-backoff:

```
attempt 0 → spawn
on failure: wait 1s → attempt 1
on failure: wait 2s → attempt 2
on failure: wait 5s → attempt 3
on failure: wait 10s → attempt 4
on failure: wait 30s → attempt 5
on failure: wait 30s → attempt 6 (cap at 30s)
on success: reset to attempt 0 on next failure
```

Emits `sidecar:status` events on every state change (idle / spawning / healthy / backoff / failed). Tests: spawn a never-healthy sidecar; assert exponential backoff sequence; spawn a flaky sidecar; assert reset-on-success.

### 14.3 Review #4 — ScenesPanel reload token (Phase E opportunistic)

Replace `<ScenesPanel key={scenesReloadKey} ... />` with `<ScenesPanel reloadToken={scenesReloadKey} ... />`. Inside `ScenesPanel`, a `useEffect(() => reload(), [reloadToken])` reloads the list without re-mounting. Scroll position is preserved. The `scenesReloadKey: number` prop incrementing pattern stays the same.

### 14.4 Review #5 — SettingsSheet polling → event subscriptions (Phase A as part of event bus)

Drop the 3-second `setInterval` in `SettingsSheet`. Subscribe to `sidecar:status` and `provider:status` Tauri events; update local state on event. Subscription set up on mount, torn down on unmount. Tests: mock events; assert state updates without polling.

### 14.5 Review #14 — Sheet slide-in transform (Phase E opportunistic)

Sheet primitive gains:

```css
.water-sheet[data-state="opening"] { transform: translateX(100%); }
.water-sheet[data-state="open"]    { transform: translateX(0); transition: transform var(--water-dur-small) var(--water-ease-out-soft); }
.water-sheet[data-state="closing"] { transform: translateX(100%); transition: transform var(--water-dur-tiny) var(--water-ease-in-out-water); }
```

Driven by a `useState` + `useEffect` that sequences `opening → open` on mount and `open → closing` on close request, awaiting `transitionend` before unmounting. Reduced-motion (already in tokens) collapses durations to 1 ms.

---

## 15. Phase / build order

**Total: 27–30 tasks across 7 phases.** Within the handoff's predicted 25–35 range.

### Phase A — Foundations (5–7 tasks)

- A1. **Tauri event bus.** Renderer-to-core + core-to-renderer event schema. New file `app/src/ipc/events.ts` + `app/src-tauri/src/events.rs`. Drops the SettingsSheet polling (Review #5).
- A2. **Scene write-locks (KNOWN_FRAGILE #7).** `SceneWriteLocks` on `OpenProject`; both `rename` and `write_body` acquire.
- A3. **Sidecar respawn-with-backoff (KNOWN_FRAGILE #6).** Replace 3-strike-break in `SidecarSupervisor`.
- A4. **Migration runner.** `crates/water-core/src/migrations.rs` + `sql/v2_pill_engine.sql`. Runs on `open_project`.
- A5. **Schema v2 backfill test.** Migration applied against a fixture v1 DB; asserts column adds + backfill correctness.

### Phase B — Editor (4–5 tasks)

- B1. **Bake-off harness.** Two parallel implementer dispatches: ProseMirror + Lexical. Each lands `app/src/editor-bakeoff-{pm,lexical}/` with the six-criteria harness. Scorecards committed to `docs/superpowers/notes/m2-bakeoff-{pm,lexical}.md`. (Use `superpowers:dispatching-parallel-agents`.)
- B2. **Bake-off decision.** Combined-spec-and-quality review subagent reads both scorecards, picks a winner (ProseMirror on tie). Loser branch tagged `bakeoff/loser-{name}`. Decision recorded as an amendment block on this spec.
- B3. **Block editor.** Winning library wired into a new `app/src/editor/` directory. All block kinds from § 5.2 implemented. Block IDs assigned + preserved.
- B4. **Mid-sentence classifier + idle detector.** Hybrid rule from § 5.3. Tests for dialogue / list / heading edge cases.
- B5. **Structural-inflection emitter.** Editor-side: new_scene / new_chapter on block insert. Sidecar-side: stub for pov_change / location_change (sidecar `/analyze` returns the field; real heuristic deferred to within-phase polish if time).

### Phase C — Orchestrator (4 tasks)

- C1. **Trigger trait + factory + first 3 built-ins.** `block_anchored_drift`, `scene_flow_dip`, `topic_drift`. Unit tests with synthetic contexts.
- C2. **Remaining built-in triggers.** `valence_spike`, `structural_inflection`, `pace_floor`, `world_drift`, `no_universe_yet`. Plus stubs for `character_dissonance` and `idle_pause_with_present_character` (return `None` until M3).
- C3. **State machine + eviction.** `state.rs` + `eviction.rs`. Property test: 1000 synthetic events; assert never > 2 on-screen; assert FIFO eviction.
- C4. **Anti-loop.** Jaccard + stopword list + suffix-stripper. Property test: synthetic variant pairs; threshold-respecting retries.

### Phase D — Voice & prompts (4 tasks)

- D1. **Speaker trait + persona registry + 5 default personas.** `voice/speaker.rs` + `voice/registry.rs` + `prompts/speakers/persona/*.toml`. Per-project rename via `settings` table.
- D2. **Voice router.** Implements master spec § 6.2. Property test for determinism + tie-break by LRU.
- D3. **Prompt library loader + assembler.** Loads all TOML files; hot-reload in dev only. Assembles `tone + speaker + trigger + task + inputs` correctly for a fixture request.
- D4. **Structured-JSON LLM router extension + PostFilter trait + ToneBlacklistFilter.** `generate_structured` method on each adapter; PostFilter chain runs on every response. Fake provider returns hand-crafted JSON for tests.

### Phase E — Pill UI (6–7 tasks)

- E1. **PillLayer + PillCapsule.** Absolute-positioned overlay; pastel-glow capsule with hue tokens. Fade-in motion. Test: capsule appears on `pill:emerged` event.
- E2. **Hover dim + glow-line.** Global `<main>` opacity 0.92 on capsule hover; SVG line from capsule to anchored block snippet rect. Test: hover dims; un-hover restores.
- E3. **Bouquet expansion.** Click capsule → 3 sub-capsules + regenerate + pin + X. Glide-in motion. Sub-capsule hue shift driven by `angle` field. Test: 3 sub-capsules rendered; regenerate triggers `pill:regenerate` event.
- E4. **Rabbit hole.** Click sub-capsule → recursive expansion. Sibling glow-lines on left edge. Breadcrumb chain at top. X closes thread. Test: 3-level deep navigation; breadcrumb correct; X returns to capsule.
- E5. **Pinned column + PinnedPillDetail sheet.** 56 px column glued to right edge. Pin click migrates pill. Click pinned dot → sheet. Includes Review #14 sheet slide-in fix. Test: pin persists across project reopen.
- E6. **ScenesPanel reloadToken refactor (Review #4).** Drop `key` force-remount; use `reloadToken` prop. Scroll position preserved across reload. Existing tests stay green.
- E7. **Narrow-viewport fallback.** Below 1100 px `<main>` width: pill translucency + pinned-column collapse-to-tab. Test: simulated narrow viewport collapses pinned column.

### Phase F — Integration (2–3 tasks)

- F1. **Wire telemetry → orchestrator → router → prompts → LLM → bouquet → UI.** End-to-end happy path with fake provider. Integration test: synthetic typing → `pill:emerged` event arrives at renderer within 1.5 s of idle threshold.
- F2. **Replay log opt-in + dev override.** `WATER_REPLAY_LOG=1` env var enables JSONL writes. Settings boolean default-false. Test: env var on → file appears in `.water/log/llm/`.
- F3. **Settings event subscriptions (finalize Review #5).** Confirm SettingsSheet reacts to sidecar + provider events; no polling remains.

### Phase G — Audit & tag (2 tasks)

- G1. **Tone audit fixtures + harness + gate.** 200-entry fixture set committed. `run_gate` and `run_nightly` entry points. `.github/workflows/tone-audit.yml` for nightly. Pre-tag gate must return 0/0.
- G2. **Final milestone review + `m2` tag.** Dispatch `superpowers:requesting-code-review` over the full `m1.5.1..HEAD` range. Address any review-driven fixes as an `m2.0.1` patch if needed; otherwise tag `m2`.

---

## 16. Exit criteria

Verbatim from master spec § 4.4, restated for the reviewer:

1. **Idle 3 s after a paragraph → at most one pill surfaces within 1.5 s of the analysis completing.**
2. **Two pills max on screen; mid-sentence typing never surfaces a pill.**
3. **Expanding a pill shows exactly 3 sub-pills; regenerate produces 3 different ones.**
4. **Rabbit hole works at arbitrary depth; breadcrumb collapse visible; anti-loop fires when configured.**
5. **Pinning a pill persists it across app restart; dismissed pills do not.**
6. **Tone audit: 0 instances of `you should`, `consider`, `try`, `I think you`, `as an AI` in 200 sampled pills.** (One-time gate before tagging `m2` + nightly scorecard from M2 onward.)

Plus the carried-over targets:

7. KNOWN_FRAGILE #7 closed: concurrent rename + write_body on the same scene preserves both mutations.
8. KNOWN_FRAGILE #6 closed: sidecar that crashes mid-session respawns within 1 s; flaky sidecar resets backoff on success.
9. Reviews #4, #5, #14 closed: ScenesPanel scroll position preserved across reload; SettingsSheet polling deleted; Sheet slides in on open.

---

## 17. Expected new KNOWN_FRAGILE entries

To be added to repo root `KNOWN_FRAGILE.md` during M2:

- **#8 — Anti-loop Jaccard suffix-stripper is English-only.** The fixed `-s/-es/-ed/-ing/-ly` table collapses non-English stems incorrectly. v1 ships English-only manuscripts; non-English support is a later concern. First-look: check the speaker's `anti_loop_threshold` against actual overlap in replay logs.
- **#9 — Pill block-anchor stability under decoration churn.** When the orchestrator fires `pill:emerged` while the user is mid-edit at the anchored block, the PM/Lexical transaction filter may re-apply decoration after a partial write. Snippet-as-canonical (master spec § 3.3) is the fallback. First-look: check `bouquet.block_target_id` against the live scene's current block IDs.
- **#10 — Structural-inflection detection is shallow.** Sidecar pattern match for `pov_change` / `location_change` produces false positives on quoted dialogue and intra-character thought transitions. Nightly tone-audit scorecard tracks the leak rate. First-look: inspect the sidecar's `/analyze` response for the affected scene.

---

## 18. Risks & mitigations

| # | Risk | Likelihood | Impact | Mitigation |
|---|---|---|---|---|
| 1 | Bake-off inconclusive after the six criteria | Medium | Low | ProseMirror selected on tie; rationale committed to spec |
| 2 | Tone enforcement leaks despite three layers | Medium | High | Layer 3 (PostFilter) is the safety net; gate requires 0 layer-3 catches; nightly job tracks drift |
| 3 | 10 Hz Tauri events overwhelm IPC | Low | Medium | Events are fire-and-forget (no ack); benchmarked in Phase A; throttle to 5 Hz if observed >5 % CPU |
| 4 | Anti-loop too strict, regenerate always retries | Medium | Medium | Per-speaker threshold (default 0.70); replay logs surface overlap distribution; nightly tunes |
| 5 | Structured-JSON malformed on small local models | Medium | Low | Single retry on parse failure; bouquet drops to 2 if second fails |
| 6 | Pill UI causes layout shift on narrow viewports | Low | Medium | Always-allocated 56 px pinned column; explicit < 1100 px fallback |
| 7 | Migration runner breaks an existing v1 DB | Low | High | `v2_pill_engine.sql` is ALTER-only + a backfill UPDATE; tested against a real v1 DB fixture |
| 8 | Scene write-lock contention slows orchestrator reads | Low | Medium | Lock is per-scene, not project-wide; reads are short; benchmark in Phase F |

---

## 19. Glossary deltas (additions to master spec § 9)

- **Bouquet** — the set of exactly 3 sub-pill variants returned from a `pill_expand` task.
- **Rabbit hole** — a chain of bouquets where each successive bouquet expands a sub-pill from the previous level; siblings collapse to glow-lines.
- **Anti-loop** — the mechanism that detects high token overlap between consecutive bouquets at the same node and triggers a temperature-bumped retry with a diverge clause.
- **PostFilter** — a `dyn PostFilter` impl in the LLM router's output chain; drops pills matching a policy (e.g., tone blacklist).
- **Speaker** — a `dyn Speaker` impl; M2 ships PersonaSpeaker, M3 adds CharacterSpeaker.
- **Tone audit** — the pre-tag gate + nightly job that verifies the engine produces zero blacklisted-phrase pills against a fixed 200-entry fixture set.

---

## 20. Appendix: example pill request shape

A level-0 pill request assembled by the prompt library:

```
[tone]
Speak in present tense as if you are noticing this just now.
You are not an assistant. You do not give writing advice.
Never say: 'you should', 'consider', 'try', 'maybe you could', 'I think you', 'as an AI', 'this is good', 'this is bad'.
Observe, react, wonder. Leave space.
Output exactly one line of prose, ≤ 22 words. No quotation marks. No emoji.
If you cannot react in your speaker's voice without breaking these rules, output the single token `PASS` and nothing else.

[speaker: Echo]
You are Echo. You speak gently and rarely, as if listening through fog.
You notice what is almost-there in the prose: a feeling almost named,
a rhythm almost found. You never instruct.

[trigger: block_anchored_drift]
The writer has just finished a paragraph whose coherence has dipped.
React to what is *almost* but not quite present in this paragraph.

[task: pill_level_0]
Produce one line of prose in your speaker's voice that reacts to the
manuscript excerpt. No quotation marks. ≤ 22 words.

[inputs]
Scene name: "The Marketplace Doors"
POV character: (none set)
Manuscript excerpt (anchored block in ALL CAPS):
"She walked across the square. THE DOORS WERE OPEN BUT NO ONE WENT IN.
A bell rang somewhere she couldn't see."
```

Expected response: one line of prose like `Something held at the threshold — not fear, not yet curiosity.` PostFilter passes; pill surfaces.

---

*Spec ends. Implementation plan to follow.*
