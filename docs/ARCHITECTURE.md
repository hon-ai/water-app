# Water — Platform Architecture

A technical overview of the entire system as it stands today.
Companion reading: `UX_SPEC.md` (the locked design direction) and
`HANDOFF.md` (current state-of-the-world for the next agent).

This document is descriptive of the codebase as it is — every claim
points at a file or module you can verify. When something is
fragile, deferred, or load-bearing it's called out explicitly.

---

## 1. What Water is

A local-first writing app for novelists, built on the premise that
the writer never sees a chat box and the LLM never sees a chat box.
The model's role is to surface small ambient observations ("pills")
in the margin as the writer works — never to generate prose. Every
moment the model speaks is gated through:

- A trigger that detected something *specific* worth speaking on.
- A speaker (persona or character) selected by a voice router.
- A tone-blacklist regex that strips assistant-bot phrasings.
- Per-trigger sensitivity learned from the writer's interactions.
- A 15 s global minimum pill-emission interval.

The result feels less like "an AI feature" and more like a quiet
council in a long room.

---

## 2. Process topology

Three processes, in order of statefulness:

```
┌──────────────────────────────────┐   IPC (Tauri commands +
│  Renderer  (React / TypeScript)  │ ──events──>  the Rust core,
│  app/src/                        │              never sidecar)
└──────────────────────────────────┘
                  │
                  │  Tauri 2 invoke / event bus
                  ▼
┌──────────────────────────────────┐
│  Rust core  (water-app shell)    │   Owns: SQLite, secrets,
│  app/src-tauri/                  │   replay log, LlmRouter,
│  + crates/water-core/            │   orchestrator state.
└──────────────────────────────────┘
                  │
                  │  spawned subprocess
                  ▼
┌──────────────────────────────────┐
│  Sidecar  (Python / FastAPI)     │   Continuous per-paragraph
│  sidecar/                        │   ML: embeddings, lemma
└──────────────────────────────────┘   tokenization, coherence /
                                       divergence / flow / pace /
                                       valence.
```

**Key boundary**: the renderer NEVER talks to the sidecar; only the
Rust core does. The sidecar feeds `AnalysisSnapshot` into the
orchestrator, which feeds `TriggerContext` into the 10 built-in
triggers. The renderer just observes Tauri events.

---

## 3. Repository layout

```
Water/
├── app/                          The Tauri shell + the React renderer.
│   ├── src/                      TS frontend (React + ProseMirror).
│   │   ├── chrome/               IconRail, ScenesPanel, WaterRibbon,
│   │   │                         StreamMark, EmptyState.
│   │   ├── editor/               ProseMirror schema + plugins.
│   │   ├── pill/                 PillLayer, PillCapsule, DeepenPanel,
│   │   │                         anchorResolver, RabbitHole, PinnedColumn.
│   │   ├── heat/                 HeatmapStrip + MetricPicker.
│   │   ├── characters/, worlds/, canvas/, scenes/, sheets/, intake/,
│   │   ├── theme/                fonts.ts, providerModels.ts, ThemeProvider.
│   │   ├── ipc/                  commands.ts (Tauri invokes), events.ts.
│   │   └── styles/               tokens.css (the design system).
│   └── src-tauri/                Tauri shell, IPC commands, orchestrator.
│       ├── src/
│       │   ├── commands/         Per-domain Tauri commands.
│       │   ├── orchestrator_service.rs   The big one. See §5.
│       │   ├── state.rs, events.rs, main.rs
│       │   └── icons/            StreamMark icon assets.
│       └── tauri.conf.json
├── crates/
│   └── water-core/               Rust core library. Used by water-app.
│       ├── src/
│       │   ├── llm/              Provider adapters + LlmRouter.
│       │   ├── orchestrator/     Triggers + feedback + state machine.
│       │   ├── editor/           Phase-5 diagnostics + EditorPillStore.
│       │   ├── rabbit.rs         RabbitStore (Phase 4 deepening).
│       │   ├── prompts/          Loader + assembler.
│       │   ├── voice/            Persona / character speakers + router.
│       │   ├── heat/             Per-paragraph metric store + budget.
│       │   ├── character/, world/, scene*, canvas/,
│       │   ├── tone_audit/, post_filter/, replay_log/
│       │   ├── sidecar/, sidecar_supervisor.rs
│       │   └── migrations.rs     v1 → v10 schema.
│       └── sql/                  v{N}_*.sql migrations.
├── sidecar/                      Python FastAPI sidecar (NLP/ML).
├── prompts/                      TOML prompts (speakers, triggers, tasks,
│                                 tone, heat phrasebank).
└── docs/                         HANDOFF, UX_SPEC, ARCHITECTURE (this),
                                  acceptance checklists, superpowers/.
```

---

## 4. Data model — disk + SQLite

### 4.1 Per-project on-disk layout

Each project is a directory under the writer's choice (e.g.
`~/Novels/Marina/`):

```
<project_root>/
├── .water/
│   ├── project.db                SQLite (see §4.2).
│   ├── replay-log/               Append-only JSONL of LLM calls,
│   │                             opt-in via WATER_REPLAY_LOG=1.
│   ├── snapshots/                Forward-only manuscript snapshots.
│   └── fonts/                    (future) imported custom fonts.
├── water.toml                    Project manifest (name, palette,
│                                 default manuscript id).
├── manuscript/
│   ├── chapters.toml             Ordered list of scene ids.
│   └── scenes/
│       └── <scene_id>.md         Per-scene markdown body + frontmatter.
├── characters/
│   └── <character_id>.toml       LSM v2.1 character sheets.
└── worlds/
    └── <segment_slug>/<entry_id>.toml   World bible entries.
```

The markdown is the source of truth for prose. SQLite is an index +
cache for fast lookups; if the DB is lost it can be rebuilt from disk
(see `crates/water-core/src/rebuild.rs`).

### 4.2 SQLite schema — v1 → v10

Forward-only migrations in `crates/water-core/sql/v{N}_*.sql`. The
runner in `migrations.rs` ratchets a DB to latest on open via
`rusqlite_migration::Migrations::to_latest`.

| Ver | Migration | What it adds |
|-----|-----------|-------------|
| v1 | `v1_init.sql` | `project`, `manuscript`, `scene`, `block`, `scene_character_presence`, `pinned_pill`, `schema_version`. |
| v2 | `v2_pill_engine.sql` | `pinned_pill` gains `parent_pill_id`, `pinned_at`, `trigger_class`, `bouquet_position`; backfills pinned_at. |
| v3 | `v3_character_hue.sql` | `character` gains `hue_token` with round-robin backfill. |
| v4 | `v4_world_bible.sql` | `world_segment` + `world_entry`; segment slugs; entry aliases; `pinned_pill.origin_trigger`. |
| v5 | `v5_heatmap.sql` | `heat_metric`, `scene_typing_history`, cascade-on-scene-delete. |
| v6 | `v6_canvas.sql` | `scene.canvas_x / canvas_y / canvas_group` for the spatial canvas. |
| v7 | `v7_location_presence.sql` | `scene_location_presence` (multi-location scenes). |
| v8 | `v8_trigger_feedback.sql` | `trigger_feedback` — adaptive Stage-1 sensitivity learning. |
| v9 | `v9_rabbit_hole.sql` | `rabbit_thought` tree + trim-order index. |
| v10 | `v10_editor_pill.sql` | `editor_pill` — diagnostic surface (Phase 5). |

478 tests live across the workspace; the migration-level suite alone
runs 27 assertions covering ratchets, column presence, indexes, and
cascade behavior.

---

## 5. The pill engine

The pill engine is Water's load-bearing system. End-to-end, a pill
takes this path:

```
typing telemetry ──► AnalysisSnapshot
       │                  │
       ▼                  ▼
┌────────────────────────────────┐
│  pick_best_trigger()           │   10 triggers evaluate against
│  (Stage 1, threshold checks)   │   the same TriggerContext.
└────────────────────────────────┘
                 │
                 ▼ best candidate
┌────────────────────────────────┐
│  Stage 2 LLM yes/no (optional) │   Only character_dissonance +
│  150 in / 1 out                │   world_drift; rest pass-through.
└────────────────────────────────┘
                 │
                 ▼ yes
┌────────────────────────────────┐
│  route_with_chars()            │   Persona vs. character; POV
│  (voice routing, cooldowns)    │   prefer, 45-90s per-speaker.
└────────────────────────────────┘
                 │
                 ▼ speaker selected
┌────────────────────────────────┐
│  assemble_level_0()            │   tone + speaker + trigger +
│                                │   manuscript context (arc pos,
│                                │   POV, location, recent
│                                │   resonance, character compact).
└────────────────────────────────┘
                 │
                 ▼ PromptRequest
┌────────────────────────────────┐
│  LlmRouter::generate_raw       │   Per-provider model override
│                                │   applied here (Phase model-swap).
└────────────────────────────────┘
                 │
                 ▼ raw text
┌────────────────────────────────┐
│  tone-blacklist + PASS check   │   PASS sentinel = drop silently.
└────────────────────────────────┘
                 │
                 ▼
            pill:emerged
            (Tauri event → renderer's PillLayer)
```

### 5.1 The 10 built-in triggers

`crates/water-core/src/orchestrator/triggers/`. Each is a 20–100 LOC
`evaluate(&TriggerContext) -> Option<TriggerCandidate>` with a
priority and an optional Stage-2 confirmation. The full list, by
Stage-1 priority:

| Trigger | Priority | Stage-2? | Sensitivity-tuned? | What it detects |
|---|---|---|---|---|
| `block_anchored_drift` | 8.0 | — | ✓ | divergence > 0.6 OR coherence < 0.35 on the just-left paragraph |
| `topic_drift` | 7.0 | — | ✓ | scene-wide coherence < 0.35 AND divergence > 0.5 |
| `pace_floor` | 6.5 | — | reads only | sustained low pacing (heat-tail aware) |
| `valence_spike` | 6.0 | — | reads only | swing in valence history vs trailing baseline |
| `scene_flow_dip` | 6.0 | — | reads only | trailing coherence-tail floor |
| `character_dissonance` | 5.5 | ✓ | ✓ | Jaccard lemma overlap ≥ 0.30 between paragraph and any present character's `values` / `fears` / `lie_they_believe` |
| `world_drift` | 5.5 | ✓ | reads only | paragraph contradicts an established `world_entry` |
| `structural_inflection` | 5.5×mult | — | reads only | `new_scene`/`new_chapter`/`pov_change`/`location_change` |
| `no_universe_yet` | 4.5 | — | reads only | scene mentions location/world entity that doesn't exist yet |
| `idle_pause_with_present_character` | 4.0 | — | reads only | idle ≥ 3s with `characters_present` non-empty |

Mid-sentence cursors and the 15 s global rate limit gate everything
before any trigger runs.

### 5.2 Stage 2 — LLM confirmation

Two triggers carry a `requires_confirmation: Some(ConfirmationRequest)`:
- `character_dissonance` → `pill_dissonance_check.toml` (yes/no
  whether the paragraph actually contradicts the character's stated
  belief).
- `world_drift` → `world_drift_check.toml` (yes/no whether the
  paragraph contradicts a world entry).

The Stage-2 gate is ~150 input / 1 output tokens; cheap.

### 5.3 Voice routing

`crates/water-core/src/voice/router.rs`. `route_with_chars()` picks
between persona track and character track based on the candidate's
`preferred_track`, then applies per-speaker cooldowns + POV-prefer
for character-track triggers. Returns `None` when every relevant
speaker is on cooldown, which silently drops the tick.

The 5 personas: **Echo**, **Architect**, **Editor**, **Cartographer**,
**Chorus**. Each has a TOML manifest at
`prompts/speakers/persona/<id>.toml` carrying `voice_profile`,
`wont_do`, `cooldown_ms`, `anti_loop_threshold`.

Each persona's prompt now also names the *other four* voices and
their lanes — the "council of gods" effect: pills feel like one
voice in a chorus without ever directly addressing each other.

### 5.4 Adaptive sensitivity (Phase 8)

`crates/water-core/src/orchestrator/feedback.rs`. Each trigger has
a learned sensitivity ∈ [0.2, 0.8], default 0.5, updated by a
mode-aware reward EMA.

**Writer-mode classification** at pill-emerge time:
- `pour` mode: `recent_word_delta ≥ 15 in 10s` OR `idle < 4000 ms`.
- `reflect` mode: `idle ≥ 4000 ms AND recent_word_delta < 15`.

**Reward weights** (computed in `compute_reward`):

| Outcome | Pour | Reflect |
|---|---|---|
| Pin | +1.5 | +1.0 |
| Click (no pin) | +0.6 | +0.4 |
| Dismiss (×) | −0.1 | −0.7 |
| Evict (no interaction) | 0.0 | −0.3 |

The mode-asymmetry is the key insight: a dismiss during pour mode
is near-zero signal (writer was deep in flow and reflexively cleared
the chrome), but a dismiss during reflect mode is the writer
actually evaluating the pill and rejecting it.

EMA: `r_ema ← α·reward + (1−α)·r_ema_old`, α = 0.1.
Sensitivity: `clamp(0.5 + 0.3·r_ema, 0.2, 0.8)`, blended with the
default under `COLD_START_N = 10` observations.

Three triggers currently apply sensitivity to their thresholds
(`block_anchored_drift`, `topic_drift`, `character_dissonance`); the
other seven read the value but don't yet act on it. Wiring them is a
per-trigger decision because their thresholds are heterogeneous
(count-based, event-based).

### 5.5 The renderer side

`app/src/pill/PillLayer.tsx` orchestrates the on-screen presence:
- Subscribes to `pill:emerged` / `pill:dismissed` / `pill:evicted`.
- Max 4 on-screen at once; FIFO eviction calls `pillEvicted` IPC so
  the orchestrator records the reward attribution.
- Per-pill anchor captured at emerge time (block_id + first-sentence
  snippet + content_hash) for the Phase-3.5 trigger-phrase highlight.
- Hover → `dispatchHighlightFor()` runs the 4-tier resolver and
  fires `water:set-trigger-highlight` to the editor's PM plugin.
- Click → `DeepenPanel` mounts (Phase 4) and dispatches `pillDeepen`
  with the renderer's pill text (the service-side `Pill.text` is
  never written back after LLM response — see §11 Fragilities).

The pill column is positioned to anchor on the right edge of the
prose column, NOT the window — `left: calc(min(...))` snaps to the
manuscript wrapper's actual right edge as the window narrows.

---

## 6. Anchor resolver (Phase 3.5)

`app/src/pill/anchorResolver.ts`. Pure 4-tier function:

```
resolveAnchor(payload, blocks) -> ResolvedAnchor | null
```

`payload = { blockId, snippet, blockHash, offsetHint }`.

1. **Tier `id`** — block-id present + snippet substring inside it.
2. **Tier `hash`** — block-id gone, but a block whose
   `computeBlockHash(text.slice(0,80))` matches contains snippet.
3. **Tier `fuzzy`** — sliding-window Levenshtein ≤ 2 across every
   block.
4. **Tier `fallback`** — original block still exists but snippet's
   gone; highlight the whole block + flag `anchorDrifted` on the pill.

`null` only when the block was deleted AND no fuzzy match exists.

ProseMirror integration: `triggerHighlightPlugin` holds a single
`DecorationSet` driven by a transaction-meta key. The pill margin
fires `water:set-trigger-highlight` / `water:clear-trigger-highlight`
CustomEvents; the editor's `useEffect` forwards into the plugin.

12-test battery covers identity / paragraph split / paragraph merge /
typo correction / partial deletion / full deletion / multi-occurrence
near-bias / empty-snippet fallback.

---

## 7. Rabbit hole (Phase 4)

Click a pill → side-slide `DeepenPanel` (`app/src/pill/DeepenPanel.tsx`).
Four child thoughts fan out — `closer / wider / opposite / deeper`.
Click a child → it becomes the new parent; breadcrumb shows ancestry;
Esc ascends. Mark resonance to flag the path.

### 7.1 Schema (v9)

```sql
CREATE TABLE rabbit_thought (
    id, scene_id, parent_id,             -- tree shape
    speaker_kind, speaker_id, message,
    depth, siblings_at_depth, sibling_index,
    direction,                            -- closer/wider/opposite/deeper
    resonance INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    bytes INTEGER NOT NULL DEFAULT 0      -- length(message), for trim
);
```

### 7.2 Prompts

- `prompts/tasks/rabbit_fan_4.toml` — first fan from a root pill.
- `prompts/tasks/rabbit_deepen_inherit.toml` — descent fans; inherit
  the parent's stance instead of re-fanning from the original.

Both produce strict JSON arrays of `{direction, text}` ×4.

### 7.3 Auto-trim policy (`RabbitStore::auto_trim`)

Spec UX_SPEC §D.5.a:

- Per-project caps: 5000 rows / 25 MB messages.
- Pass 1: oldest non-resonant *leaves* first.
- Pass 2: oldest non-resonant *interior* nodes with reparenting
  (children hoisted to grandparent so the depth chain survives).
- Resonant nodes + every ancestor are sacred.
- Runs in a single SQLite write transaction.

Wired on project open via the orchestrator's startup path; reset on
project close (service drops).

---

## 8. Editor pills (Phase 5)

A second class of pill — diagnostic, sticky, per-paragraph. Distinct
from the generative pill engine. Renders inline as a dotted underline
on the offending span; also lists in a panel below the editor.

### 8.1 Rule layer (Rust, no LLM)

`crates/water-core/src/editor/diagnostics.rs`. Seven rules in v1:

| Rule | Detection | Severity |
|---|---|---|
| `passive_voice` | `(was/were/been/is/are/be/am/being) + \w{3,}ed` | suggestion |
| `adverb_density` | > 2 `-ly` per 100 words | suggestion |
| `repetition` | non-stopword ≥ 4× in block | suggestion |
| `dialog_tag_overuse` | `said + adverb` | suggestion |
| `common_mistake` | 15-entry lookup table | warning |
| `weak_verb` | `to-be + adjective-suffixed word` (suffix-gated) | suggestion |
| `sentence_length_variance` | 5 consecutive sentences within ±3 words | observation |
| *(deferred)* `spelling` | needs embedded hunspell dict | — |

### 8.2 Voice discipline (`phrasebank.rs`)

Each rule has 3 Editor-voice templates picked by FNV-1a hash of
`(rule, snippet)` (deterministic, same span → same prose). Every
rendered template runs through `tone.toml`'s blacklist regex; a hit
drops the message rather than ship a tone-violating one.

### 8.3 LLM polish layer (Phase 5.8)

`prompts/tasks/editor_polish.toml` — one paragraph in, ≤22-word
Editor-voice observation out, or `PASS` sentinel for "nothing to
say." Triggered post-save for every block ≥ 25 words.

Throttles:
- **Per-scene cap**: 5 polish calls per session.
- **Per-(scene, block) cooldown**: 30 s.

Both enforced in `orchestrator_service.rs:on_editor_polish`.

### 8.4 Persistence + lifecycle

`editor_pill` table (v10). `EditorPillStore::run_and_upsert` is
idempotent at `(scene, rule, block, snippet)` — re-runs refresh
`updated_at`. Dismissed rows never resurface. `cleanup_orphaned_blocks`
drops rows whose anchor block is gone.

### 8.5 Surface

- **Inline underline**: `editorUnderlinePlugin` decorates spans
  dotted in `--water-sea-{200,300,600}` per severity.
- **`DiagnosticsList`** under the editor: grouped by rule, with
  per-row Accept (when `suggestion` is set) and Dismiss buttons.
- **Accept**: client-side splice via `acceptSuggestion(view, anchor)`
  → PM transaction replaces the range with the suggestion.

---

## 9. Heatmap

`app/src/heat/HeatmapStrip.tsx` — the 28-px strip above the editor.

### 9.1 Visual

Lava-lamp surface: deep-sea / baby-blue gradient (theme-driven via
`--water-heat-*` CSS vars in `tokens.css`), three drifting
radial-gradient gas blobs, 24 spray orbs with lifecycle keyframes
(`opacity: 0 → peak → 0` over 10–26 s, then `setTimeout` repositions
the orb so it reappears elsewhere — mirrors the WaterRibbon's
droplet lifecycle).

### 9.2 Segments

One per scene in the manuscript. Lit when `word_count ≥ 500`
(placeholder heuristic for "scene finished").

### 9.3 Metric overlays (Phase 6 wire-up)

For each scene, `ipc.heatRead(scene_id)` returns per-paragraph
metrics; the strip averages the active metric (first enabled in
`pacing / valence / coherence / presence / world_refs`) and uses the
mean to scale segment glow intensity:
- Lit + high intensity → brighter glow, bigger inner blur.
- Non-lit + intensity > 0.4 → faint sea-300 tint.

Refresh on `heat:updated` (per-scene, not full re-fetch).

### 9.4 Sidecar (where the metrics come from)

`sidecar/` is a Python FastAPI process spawned by the Rust core. It
runs the continuous mid-cost analysis (sentence embeddings, lemma
tokenization, coherence/divergence/flow/pace/valence per paragraph)
and pushes `AnalysisSnapshot` events into the orchestrator. The
renderer never talks to the sidecar directly.

---

## 10. Visual system

`app/src/styles/tokens.css` is the design system. Highlights:

- **Substrate** — five-stop warm-neutral ladder (h≈0–30, s ≤ 8%) so
  warmth is felt not seen.
- **Sea palette** — single ramp `--water-sea-{50,100,200,300,400,500,600,700,800,glow}`
  with three variant blocks under `[data-palette="sunrise|clear|earth"]`.
  Default is deep-sea blue.
- **Theme** — `[data-theme="light"]` / `[data-theme="dark"]` overrides;
  also responds to `prefers-color-scheme`. The heatmap + title-bar
  vars (`--water-heat-bg`, etc.) swap per theme.
- **Glass surfaces** — `.water-floating-panel` and `.water-floating-chip`
  utility classes for the matte-glass aesthetic
  (backdrop-filter blur + saturation + low-alpha bg).
- **Pill anatomy** — `.water-pill` with per-content-signal data
  attributes (`observation / suggestion / warning / praise`) driving
  a left rail; chip + persona label + chevron-down affordance.
- **Manuscript fonts** — six curated serifs (IBM Plex Serif default,
  Iowan, Charter, Georgia, Palatino, system-serif) persisted in
  `localStorage`; applier in `theme/fonts.ts` overrides
  `--water-font-serif` on `<html>`. Picker UI in Settings; custom
  font import deferred.

### 10.1 The WaterRibbon

`app/src/chrome/WaterRibbon.tsx`. A single global SVG behind
everything — real-time path morphing via `performance.now()`-driven
phase shifts. Two anchor modes:
- **Ambient** — free-flowing noise wave.
- **Scene-anchored** — when scenes are placed on the canvas surface,
  the ribbon's strand is shaped by the per-anchor weight via kernel
  smoothing.

Extends 240 px past the viewport on each side so the strand's start
and end live outside the visible bounds.

---

## 11. LLM provider stack

`crates/water-core/src/llm/`. Six built-in adapters:

| Provider | id | Default model | Notes |
|---|---|---|---|
| Anthropic | `anthropic` | `claude-sonnet-4-6` | Strongest observational quality for pills. |
| OpenAI | `openai` | `gpt-4o-mini` | Cheap default. |
| Kimi (Moonshot) | `kimi` | `kimi-k2-0905-preview` | 256k context for long-draft embedding. |
| OpenRouter | `openrouter` | `moonshotai/kimi-k2` | Aggregator — any model on their catalog. |
| Ollama | `ollama` | `qwen2.5:3b` | Local. |
| llama.cpp | `llamacpp` | (server-config) | Local. |

Plus a `CannedProvider` test stub.

### 11.1 LlmRouter

`crates/water-core/src/llm/router.rs`. Holds:
- A chain of `Arc<dyn LlmProvider>` (primary + optional fallbacks).
- Per-provider token bucket + circuit breaker.
- **Runtime model override map** (`default_models: Mutex<HashMap<ProviderId, String>>`)
  — populated via `provider_set_model` IPC. `generate_raw_with_default`
  reads this before falling through to the adapter's own default.

### 11.2 Secrets resolution

`crates/water-core/src/llm/secrets.rs`. Lookup order:

```
OS keychain  →  ~/.water/dev-keys.toml  →  WATER_<ID>_API_KEY env var
```

`provider_set_key` IPC writes to the OS keychain (Windows Credential
Manager via the `keyring` crate). No keys are ever embedded in the
repo; the dev-keys file is gitignored.

### 11.3 Model swap UX

`app/src/theme/providerModels.ts` — curated model lists per provider
plus a "Custom…" free-text input for arbitrary model ids. The choice
persists in `localStorage` (`water:provider-model:<id>`) and re-applies
via `ipc.providerSetModel` on app boot.

### 11.4 Two paths into the LLM

- **Bouquet** (`generate_bouquet`) — fans through the full chain with
  circuit-breaker + rate limit. Used by Stage-2 confirmation calls
  and the legacy pill_expand bouquet.
- **Single-shot** (`generate_raw_with_default` / `generate_structured_with_default`)
  — primary provider only, no breaker. Used by everything else
  (level-0 pills, rabbit fan, editor polish).

---

## 12. Prompt assembly (Phase 6)

`crates/water-core/src/prompts/`.

### 12.1 PromptLibrary

Loads built-in TOML at startup via `include_str!`:
- `tone.toml` — global voice clauses + blacklist regex.
- 10 trigger framings.
- 6 task instructions (`pill_level_0`, `pill_expand`, `pill_regenerate`,
  `rabbit_fan_4`, `rabbit_deepen_inherit`, `editor_polish`).
- 2 Stage-2 confirmation prompts (`pill_dissonance_check`,
  `world_drift_check`).
- 5 persona manifests + 1 character template.

### 12.2 PromptContext

Optional fields the assembler renders conditionally:

```rust
struct PromptContext<'a> {
    scene_name, scene_ordering, manuscript_scene_count,
    arc_position,                  // computed enum from ordering/total
    pov_character_name,
    location_name, location_brief,
    character_compact,             // 200-word LSM distillation
    recent_resonance,              // last 3 resonant rabbit thoughts
}
```

Built by `OrchestratorService::build_owned_prompt_context()` from
the live `SceneSnapshot` + `CharacterRegistry` + `WorldRegistry` +
`RabbitStore`. Each line in the rendered `[manuscript context]` /
`[character sheet]` / `[recent resonance picks]` block is gated on
the relevant field being present, so a cold-boot project still
produces a clean (if smaller) prompt.

### 12.3 Arc position derivation

`crates/water-core/src/orchestrator/arc.rs`. Pure function bucketing
`ordering / total - 1` into six labels:
- `opening sequence` (≤ 0.10)
- `rising action` (≤ 0.40)
- `midpoint pivot` (≤ 0.55)
- `approaching climax` (≤ 0.80)
- `climax` (≤ 0.92)
- `resolution` (> 0.92)
- `standalone scene` (total ≤ 1)

### 12.4 Character compact

`crates/water-core/src/character/compact.rs`. Pure heuristic
distillation from LSM v2.1 fields:

```
{full_name} — {role_in_story}.
Wants: {want}.
Needs: {need}.
The lie they believe: {lie_they_believe}.
Values: {values_joined}.
Fears: {fears_joined}.
Voice: {voice}.
```

Empty fields skipped. Caching deferred — function is cheap; runs
once per prompt assembly.

---

## 13. IPC catalog

Tauri commands defined in `app/src-tauri/src/commands/`, mirrored by
the TS facade in `app/src/ipc/commands.ts`. The major surfaces:

| Domain | Commands |
|---|---|
| Project | `create_project`, `open_project`, `close_project` |
| Scenes | `scene_create`, `scene_read`, `scene_write_body`, `scene_list`, `scene_rename`, `scene_read_metadata`, `scene_set_location`, `scene_set_summary` |
| Characters | `character_create`, `character_read`, `character_list`, `character_update_field`, `character_delete`, `character_link_to_scene`, `character_unlink_from_scene`, `character_set_pov`, `character_autosuggest_for_scene`, `intake_schema` |
| World | `world_segment_list/create/update_template/set_hidden/delete`, `world_intake_schema`, `world_single_doc_read/update_field`, `world_entry_list/read/create/update_field/update_aliases/delete`, `world_autosuggest` |
| Canvas | `canvas_read`, `canvas_set_position` (M6) |
| Heat | `heat_read`, `heat_read_settings`, `heat_set_metric_enabled` |
| Providers | `provider_test`, `provider_set_key`, `provider_set_model` |
| Pills | `pill_expand`, `pill_regenerate`, `pill_pin`, `pill_dismiss`, `pill_evicted`, `pill_deepen`, `rabbit_deepen_thought`, `rabbit_set_resonance`, `feedback_reset`, `pinned_list` |
| Editor pills | `editor_pills_run`, `editor_pills_list`, `editor_pill_dismiss`, `editor_polish_request` |
| Telemetry | `typing_telemetry`, `scene_state` |
| Diagnostics | `diagnostics_status` |

Events flow the other direction (Rust → renderer via `emit()`):
`pill:emerged/dismissed/evicted/pinned/unpinned`, `bouquet:ready`,
`deepen:ready`, `deepen:failed`, `heat:updated`, `editor_pills:updated`,
`sidecar:status`, `provider:status`, `typing:telemetry`.

---

## 14. Editor stack

`app/src/editor/`. ProseMirror-based.

### 14.1 Schema

`schema.ts`. Extends `prosemirror-schema-basic`:
- Block kinds: paragraph, heading (h2/h3), `scene_break`, `dialogue`,
  ordered/bullet lists with per-item `blockId`.
- Marks: strong, em, link (with title hint), **strike** (Phase 5.6+).
- Every block-level node carries a `blockId` (`^bk-XXXX`) attribute.

### 14.2 Plugins (mounted in order)

- `history()` (undo/redo).
- `keymap` — Mod-Z/Y/Shift-Z, Mod-B/I, **Mod-Shift-X** (strike), Mod-K
  (link), Enter (split block / split list item), **Shift-Enter** (hard_break).
- `baseKeymap`.
- `blockIdPlugin` — assigns + dedupes `blockId` on every transaction.
- `triggerHighlightPlugin` — Phase 3.5 trigger-phrase decorations.
- `editorUnderlinePlugin` — Phase 5 diagnostic underlines.
- `smartInputPlugin` — `--` → `—`, `...` → `…`, smart quotes.

### 14.3 Markdown round-trip

`serialize.ts`. Custom serializer that emits `^bk-XXXX` anchors
before each block. Marks: `**bold**`, `*em*`, `~~strike~~`, `[text](url)`,
with priority `link > strike > strong > em`. Deserializer mirrors.

---

## 15. Migrations + tests

| Crate / package | Tests | What's covered |
|---|---|---|
| `water-core` lib | 429 | every module (orchestrator, pills, rabbit, editor, llm adapters, prompts, voice, heat, character, world, canvas, scene, db, migrations) |
| `water-core` `tone_audit_200` | 1 | full 200-pill canned-provider gate |
| `water-core` `tone_audit_battery` | 1 | tone blacklist regex battery |
| `water-core` `orchestrator_integration` | 1 | end-to-end synthetic trigger → speaker → prompt → fake LLM |
| `water-core` doctests | 6 | docstring examples |
| `water-app` | 41 | Tauri commands, secrets, orchestrator wiring |
| Frontend (vitest) | 214 | components, anchor resolver, IPC bindings, ProseMirror plugins, schema |

**478 Rust + 214 frontend** tests pass at the time of writing. The
test suite is fast (≈ 5 s frontend, ≈ 4 s Rust workspace).

---

## 16. Build + run

From the workspace root:

```powershell
pnpm --filter @water/app tauri dev      # dev (Vite + Tauri shell)
pnpm --filter @water/app tauri build    # release bundle (msi/nsis on Win)
cargo test --workspace                  # all Rust tests
pnpm --filter @water/app test           # vitest
pnpm --filter @water/app tauri icon src-tauri/icons/stream-mark.svg
                                         # regenerate the full icon set
                                         # (StreamMark logo)
```

`tauri.conf.json`:
- `decorations: true` — native OS title bar.
- `width: 1280, height: 800, minWidth: 900, minHeight: 600`.
- `bundle.icon: ["icons/icon.ico"]` (StreamMark, generated from SVG).

---

## 17. Known fragilities

Recording these here so they don't get rediscovered the hard way.

### 17.1 Service-side `Pill.text` never written back

The orchestrator emits the LLM-generated text directly to the
renderer via `pill:emerged` but never updates the matching `Pill`
record in `self.pills.text`. Anything that needs the text server-side
(like the rabbit-hole deepen path) has to receive it from the
renderer via IPC. Fixed in v.current for `pillDeepen`; future paths
need the same treatment.

Lasting fix is one of:
- Send a `OrchestratorRequest::PillTextLanded { pill_id, text }` from
  the spawn task back through the channel after the LLM returns; the
  handler writes it into `self.pills`.
- Drop `Pill.text` from the server-side record entirely and treat the
  renderer as authoritative.

### 17.2 FIFO eviction is renderer-driven

The service-side `pick_evictee` runs but the service never
transitions service-side pills out of `Generating` (the renderer is
source-of-truth for "what's on screen"). The renderer's FIFO trim
calls `pill_evicted` IPC; the service's own eviction path is
effectively dormant. Documented in `KNOWN_FRAGILE.md`.

### 17.3 Cargo + Tauri exe lock on Windows

`cargo build --workspace` sometimes fails with `Access is denied (os
error 5)` on `target/debug/water-app.exe` if the dev binary is still
running. `cargo build -p water-app --tests` (which writes to a
different output) works around it. Restart the dev shell to drop the
lock cleanly.

### 17.4 Native title bar vs. `decorations: false`

Earlier prototype tried a custom title bar with `decorations: false`.
Result on Windows: both bars rendered (native + custom) because the
`decorations` config requires a full Tauri rebuild — Vite HMR can't
apply it. Reverted: native bar stays.

### 17.5 Test pill clicks need a configured LLM

A pill won't successfully deepen unless an LLM provider is configured
(key + Test → green dot). Without it, `deepen:failed` fires
immediately with a clear reason — the panel surfaces "no LLM provider
configured — open Settings → Providers and Test one." Not a bug per
se, but it confuses on first run.

### 17.6 Custom font import isn't wired

`theme/fonts.ts` has a curated list + persistence; an "Import font…"
flow that lets writers add their own typeface (Tauri file picker →
save to `.water/fonts/` → @font-face inject) is the obvious next
step. Stub UI lives in Settings but the import button isn't wired.

### 17.7 Sidecar may not be running

The Python sidecar is spawned per-project. When it's alive, the
orchestrator's sidecar-bridge (`maybe_kick_sidecar_analyze` +
`apply_block_analysis`) fans out per-paragraph analyze requests
on every idle pulse and patches the result back into
`self.analysis` via the `BlockAnalysis` request. Per-block
debounce: `BLOCK_ANALYZE_DEBOUNCE` (4 s) so the same paragraph
isn't re-analyzed every 3-s idle tick.

When the sidecar fails to boot (or is killed mid-session), the
bridge becomes a no-op: `self.sidecar` is `None` (or the analyze
HTTP call errors and logs at `debug`), and the five
sidecar-dependent triggers — `block_anchored_drift`,
`topic_drift`, `pace_floor`, `valence_spike`, `scene_flow_dip` —
stay dormant. The other five triggers still fire on telemetry
alone: `structural_inflection`, `idle_pause_with_present_character`,
`no_universe_yet`, `character_dissonance`, `world_drift`.

---

## 18. Phases shipped — high-level milestone summary

| Phase | What | Status |
|---|---|---|
| M1 | Project / scene / manuscript skeleton | ✓ |
| M2 | Pill engine + bouquet | ✓ |
| M3 | Character sheets (LSM v2.1) | ✓ |
| M4 | World bible | ✓ |
| M5 | Heatmap (data layer) | ✓ |
| M6 | Spatial canvas | ✓ |
| M7 | UX_SPEC v2 visual overhaul | ✓ |
| Phase 3 | Pill UX refinement (chip + signal rail) | ✓ |
| Phase 3.5 | Precise hover-highlight subsystem | ✓ |
| Phase 4 | Rabbit hole (deepen panel) | ✓ |
| Phase 5 | Editor pills (rule + LLM polish) | ✓ |
| Phase 6 | Prompt overhaul (rich context, won't-do, council) | ✓ |
| Phase 8 | Adaptive trigger sensitivity (renumbered) | ✓ |

---

## 19. Where to look first

| If you're working on… | Start with |
|---|---|
| Pill generation flow | `app/src-tauri/src/orchestrator_service.rs` — `on_telemetry()` |
| A new trigger | `crates/water-core/src/orchestrator/triggers/` — copy an existing one |
| A new LLM provider | `crates/water-core/src/llm/` — `kimi.rs` is the cleanest reference |
| Editor diagnostics | `crates/water-core/src/editor/diagnostics.rs` |
| Prompt tone / persona voice | `prompts/speakers/persona/*.toml`, `prompts/tone.toml` |
| Rabbit-hole UI | `app/src/pill/DeepenPanel.tsx` |
| Heatmap visual | `app/src/heat/HeatmapStrip.tsx` + `tokens.css` `--water-heat-*` |
| New IPC command | `app/src-tauri/src/commands/<domain>.rs` + register in `main.rs` + add to `app/src/ipc/commands.ts` |
| Schema change | new `crates/water-core/sql/v{N}_*.sql` + register in `migrations.rs` + bump `schema_version_row_matches_latest_migration` |
| New visual token | `app/src/styles/tokens.css` — add to `:root`, then mirror in the two dark blocks (`@media` + `[data-theme="dark"]`) |

---

End.
