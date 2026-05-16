# Water — Design Specification

| | |
|---|---|
| **Status** | Approved (design), pending implementation plan |
| **Date** | 2026-05-16 |
| **Author(s)** | Water core (in collaboration with the brainstorming workflow) |
| **Target release** | v1 internal-test build (~14 weeks calendar) |
| **Supersedes** | — |

---

## Table of contents

1. [Vision & hard principles](#1-vision--hard-principles)
2. [System architecture](#2-system-architecture)
3. [Data model](#3-data-model)
4. [Subsystem decomposition & build order](#4-subsystem-decomposition--build-order)
5. [UX & visual system](#5-ux--visual-system)
6. [Pill engine](#6-pill-engine)
7. [Risks, open questions, and resolutions](#7-risks-open-questions-and-resolutions)
8. [Roadmap summary](#8-roadmap-summary)
9. [Glossary](#9-glossary)
10. [Appendix: schemas, examples, and prompts](#10-appendix-schemas-examples-and-prompts)

---

## 1. Vision & hard principles

### 1.1 Product thesis

Water keeps the writer in flow. The writer never types into an LLM and never sees text generated *into* their manuscript. Instead, LLMs surface as soft pastel **pills** in the margins that read the manuscript and react — sometimes as the writer's own characters, sometimes as ambient personas — turning the act of writing into a continuous conversation with the writer's developing universe.

The defensibility of Water is its UX and the integration of these pills with the universe the writer is constructing. The closer Water gets to the feel of *listening to your story think out loud*, the better it succeeds.

### 1.2 Hard principles (architectural constraints, not preferences)

1. **No conversational input.** The LLM's only inputs from the user are (a) the manuscript text and (b) clicks (which pill, regenerate, dismiss, pin). There is no chat box, no prompt field, no "ask the AI" affordance anywhere in Water.
2. **Universe-first, personas-second.** Character voices dominate. Personas are minimal ambient presences that yield more share as the universe matures. Personas are renameable per-project so they feel *of* the universe.
3. **Flow protection beats feature density.** Pills never appear mid-sentence; they surface only on idle plus signal; max two visible at once; auto-dismiss with soft TTL; per-track cooldowns.
4. **Reactive, never instructive.** Pill copy is observation and reaction; never `you should…`, `consider…`, `try…`, `maybe you could…`, or any tutorial register.
5. **Local-first.** Everything works offline with the bundled local model. Cloud providers (LLMs, future sync) are opt-in.
6. **Human-readable on disk.** A project is a visible folder of Markdown + TOML. SQLite is a *rebuildable* index. The truth is on disk and openable in any editor.
7. **Configurable, with a strong default.** Every cadence/policy knob is exposed in settings; defaults are chosen so most writers never touch them.
8. **Pastel-glow visual identity** layered on a Notion/Apple-fluid, Apple-minimal baseline. Loosely inspired by [Anomaly OpenCode](https://github.com/anomalyco/opencode) `dev` branch at `packages/ui`, `packages/storybook`, `packages/app`, `packages/desktop`, `packages/web` (GUI patterns) and `packages/tui` (density, restraint, monospace usage — for *data*, not as a separate visual register). Light *and* dark mode are first-class; dark mode is the heatmap/macro view's peak moment — a liminal glowing space.
9. **Deterministic onboarding.** Intake (characters, world, scene goals) uses one-question-at-a-time popups bound to a schema. No LLM calls in intake.
10. **Midjourney-style exploration.** Pill rabbit hole = bouquets of three variants with a soft translucent regenerate icon. Depth is unlimited; prior-bouquet siblings collapse to thin glow lines along the panel edge as the writer descends. Pinning at any depth saves to the scene.

### 1.3 Out of scope for v1 (explicit non-goals)

- Generative writing into the manuscript (ever).
- Conversational chat with the LLM (ever).
- Real-time multi-user collaboration.
- Mobile, tablet, or web-only deployment.
- Linux desktop (deferred to v1.1).
- Plugin loader (only the contracts are designed in v1; v2 ships the loader).
- Manual beat-label override (deferred to v2; v1 ships auto with confidence threshold).
- Paragraph Lens zoom level (zoom 5) — stretch goal; ships only if M5 finishes ahead of schedule.
- Sound design.
- In-app per-pill thumbs feedback.

---

## 2. System architecture

### 2.1 Process topology

```
┌───────────────────────────────────────────────────────────────┐
│                       Water (Tauri app)                       │
│ ┌─────────────────────────────────────────────────────────┐   │
│ │  Renderer (web UI)   React + TS + Tailwind + Radix      │   │
│ │   • Editor (Notion-style block editor)                  │   │
│ │   • Pill engine UI (surface, expand, regenerate, pin)   │   │
│ │   • Spatial Continuum (zoom levels 0–4, optional 5)     │   │
│ │   • Heatmap audiovisualizer                             │   │
│ │   • Character / World intake popups                     │   │
│ │   • Settings, light/dark themes, pastel-glow tokens     │   │
│ └─────────────────────────────────────────────────────────┘   │
│                            ▲                                  │
│            Tauri IPC (commands / events)                      │
│                            ▼                                  │
│ ┌─────────────────────────────────────────────────────────┐   │
│ │  Rust core                                              │   │
│ │   • Project store (SQLite + scenes/*.md + snapshots/)   │   │
│ │   • Autosave + snapshot scheduler                       │   │
│ │   • Sidecar lifecycle (spawn / heartbeat / kill)        │   │
│ │   • LLM provider router (cloud + local + MLX)           │   │
│ │   • Pill orchestrator (triggers, cooldowns, routing)    │   │
│ │   • Voice router (character vs persona; speaker pick)   │   │
│ │   • Prompt library (TOML, versioned)                    │   │
│ │   • Secrets vault (OS keychain) for API keys            │   │
│ └─────────────────────────────────────────────────────────┘   │
└───────────────────┬──────────────────────┬────────────────────┘
                    │                      │
        stdio / loopback HTTP        loopback HTTP
                    ▼                      ▼
   ┌──────────────────────────┐  ┌──────────────────────────┐
   │  Analysis sidecar        │  │  LLM providers           │
   │  (Python, bundled)       │  │  • Cloud: Anthropic,     │
   │   • LiveVectorizer       │  │    OpenAI, …             │
   │     (MiniLM+DistilRoBERTa│  │  • Local: Ollama,        │
   │     fusion → 1152-d)     │  │    llama.cpp             │
   │   • Drift tensor         │  │  • Apple Silicon: MLX    │
   │     (flow_net.pth) →     │  │    (feature-flagged)     │
   │     divergence score     │  │  • Bundled tiny model    │
   │   • Style metrics        │  │    (Qwen-or-Kimi pending │
   │   • OCEAN (DistilRoBERTa)│  │    eval-harness sweep)   │
   │   • Coherence / valence  │  │                          │
   │   • Per-paragraph + per- │  │                          │
   │     scene rollups        │  │                          │
   └──────────────────────────┘  └──────────────────────────┘
```

### 2.2 Boundary rationale

- **Tauri shell + Rust core** owns everything that touches disk, secrets, processes, and policy (autosave, snapshots, pill cooldowns, LLM routing). The renderer is *dumb about timing*: it receives events ("here are new pills") but does not decide when pills appear.
- **Renderer** owns presentation, animation, user gestures, and the Spatial Continuum view machinery.
- **Python analysis sidecar** isolates the ML stack (PyTorch, transformers, scikit-learn) from the Rust core. Spawned at app start; killed on app close; restarted on crash. The renderer never talks to it directly. The bundled standalone Python (via [uv](https://github.com/astral-sh/uv)) means testers do not need a system Python.
- **LLM providers** are pluggable behind a single `LlmProvider` trait in Rust. The router holds quota / rate-limit / circuit-breaker state and falls back to a configured secondary on errors. Each provider implements `generate_pill`, `generate_bouquet`, `regenerate_bouquet`, `summarize_scene`, `classify_beats`, and `health`.

### 2.3 Tauri IPC surface (renderer ↔ core)

**Commands (renderer → core):**

- `project.open(path)`, `project.create(...)`, `project.list_recent()`, `project.close()`
- `scene.read(scene_id)`, `scene.write(scene_id, content, cursor)`, `scene.create(after_id)`, `scene.move(scene_id, new_position, new_chapter_id?)`
- `scene.history(scene_id)`, `scene.restore(scene_id, snapshot_id)`
- `character.upsert(...)`, `character.list()`, `character.delete(id)`; identical surface for `world.*`
- `pill.expand(pill_id)` → returns the sub-bouquet
- `pill.regenerate(pill_id)` → returns a fresh bouquet at same level (excluding prior variants)
- `pill.pin(pill_id)`, `pill.dismiss(pill_id)`, `pill.dismiss_all()`
- `analysis.request_scene(scene_id)` → manual re-analysis
- `settings.get/set(...)`, `theme.set(light|dark|auto)`
- `provider.test(provider_id)`, `provider.set_api_key(...)`, `provider.list_models()`
- `zoom.set(level, focus_id?)` → declarative zoom request (renderer animates to it)

**Events (core → renderer)** — push only, never polled:

- `pill.surfaced { pill_id, speaker, target_block_id, target_snippet, message, hue, level }`
- `pill.expired { pill_id, reason }`
- `analysis.updated { scene_id, metrics }`
- `heatmap.updated { manuscript_id, scene_metrics[] }`
- `scene_summary.updated { scene_id, summary, model_id }`
- `autosave.completed { scene_id, snapshot_id? }`
- `sidecar.status { state: "ready"|"loading"|"error", detail? }`
- `provider.status { provider_id, healthy, latency_ms? }`

### 2.4 Pill orchestrator (the heart, in Rust core)

The orchestrator is a pure state machine. Inputs:

1. **Typing telemetry** from renderer (idle duration, cursor position class, structural-inflection events, block-id of cursor).
2. **Sidecar analysis updates** (drift, flow, coherence, valence, OCEAN, per-block metrics).
3. **Scene metadata** (characters present, POV, location, scene goal).
4. **Active pill registry** (currently visible pills, per-track cooldowns, recently dismissed).

Per tick, the orchestrator either (a) emits nothing or (b) emits exactly one `pill.surfaced` event. It is the *only* place that decides *when* a pill appears. See Section 6 for the trigger taxonomy and decision matrix.

### 2.5 Why no direct renderer ↔ sidecar path

Two reasons:

1. All pill timing must flow through the orchestrator. Without this, the renderer could bypass cooldowns / quotas / privacy locks.
2. Keeping the renderer ignorant of the analysis stack means we can swap the sidecar (or eventually move ML into Rust) without touching the UI.

---

## 3. Data model

### 3.1 On-disk project layout

A Water project is a visible folder the user can see in Finder/Explorer, copy to back up, and (later) sync via any folder-syncing tool. **The Markdown is the truth; SQLite is a derived index.**

```
MyNovel.water/                     ← user-visible folder, opens as a Water project
├── water.toml                     ← schema_version, project metadata, name, created_at
├── project.db                     ← SQLite index (rebuildable from the rest)
├── manuscript/
│   ├── chapters.toml              ← ordered list of {chapter_id, name, scene_ids[]}
│   └── scenes/
│       ├── 01H8X4.md              ← one scene per file (ULID filename)
│       ├── 01H8X5.md
│       └── …
├── characters/
│   ├── 01H7AA.toml                ← one LSM v2.1 character per file
│   └── …
├── world/
│   ├── concept.toml
│   ├── locations/
│   │   ├── 01H7B1.toml
│   │   └── …
│   ├── politics-and-social.toml
│   ├── world.toml
│   └── …                          ← segments configurable; each is .toml or a folder
├── pins/
│   └── 01H8X4.toml                ← pinned pills, keyed by scene
├── snapshots/
│   └── 01H8X4/                    ← per-scene history
│       ├── 2026-05-16T09-31-12.zst
│       └── …
├── media/                         ← attachments (reference images, etc.)
└── .water/
    ├── cache/                     ← analysis cache, heatmap rollups
    ├── settings.toml              ← per-project overrides
    └── log/                       ← rotating logs for diagnostics
```

Filenames are [ULIDs](https://github.com/ulid/spec): lexicographically sortable, time-prefixed, never collide.

### 3.2 Scene Markdown format

Plain Markdown with YAML frontmatter. Blocks carry stable IDs via a trailing `^bk-XXXX` token at the end of each paragraph — the Obsidian block-reference convention. Files open cleanly in Obsidian.

```markdown
---
id: 01H8X4
name: "The Lighthouse Awakes"
chapter_id: 01H7Z1
order: 3
pov_character_id: 01H7AA
characters_present: [01H7AA, 01H7AB]
location_id: 01H7B1
scene_goal: "Maren realizes the lighthouse keeper is lying."
status: draft               # draft | revising | done
created_at: 2026-05-12T10:14:00Z
updated_at: 2026-05-16T09:31:12Z
word_count: 842
---

The fog rolled in low over the cliffs, swallowing the harbour
lanterns one by one. Maren watched the last of them go. ^bk-0a3f

"He's lying," she said, more to the gulls than to anyone else. ^bk-0a40

…
```

- `^bk-XXXX` tokens are managed by Water; the editor inserts/maintains them invisibly.
- `characters_present` and `location_id` are *hints* for the pill orchestrator. Empty is fine — personas pick up the slack.
- `scene_goal` is optional and populated via Conversational Intake when the writer creates a scene; always skippable.

### 3.3 Pill anchoring

A pill targets a range inside a block:

```ts
type PillAnchor = {
  scene_id:    string,        // ULID
  block_id:    string,        // "bk-0a3f"
  snippet:     string,        // canonical excerpt, ≤ 120 chars
  offset_hint: { start: number, end: number } | null  // soft, may drift
}
```

Resolution at render time:

1. Renderer searches `block.text` for exact-match `snippet` → highlight that range.
2. If no exact match, fuzzy-match (similarity ≥ 0.85).
3. If still no match, the pill is silently dismissed and a diagnostics entry is recorded. The orchestrator may regenerate one.

**`snippet` is canonical**; offsets are hints. Word-level pills use the same model with shorter snippets.

### 3.4 SQLite schema (the index)

All tables have `created_at` and `updated_at` timestamp columns. Foreign keys are enforced. ULIDs are TEXT primary keys.

```sql
-- Identity / versioning
CREATE TABLE schema_version (version INTEGER NOT NULL);

CREATE TABLE project (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  default_manuscript_id TEXT,
  created_at TEXT, updated_at TEXT
);

-- Story structure (a project can have ≥1 manuscript; v1 default = 1)
CREATE TABLE manuscript (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES project(id),
  name TEXT NOT NULL,
  ordering INTEGER NOT NULL
);

CREATE TABLE chapter (
  id TEXT PRIMARY KEY,
  manuscript_id TEXT NOT NULL REFERENCES manuscript(id),
  name TEXT NOT NULL,
  ordering INTEGER NOT NULL
);

CREATE TABLE scene (
  id TEXT PRIMARY KEY,             -- matches the .md filename (ULID)
  manuscript_id TEXT NOT NULL REFERENCES manuscript(id),
  chapter_id TEXT REFERENCES chapter(id),         -- NULLable: unfiled scenes allowed
  ordering INTEGER NOT NULL,        -- order within manuscript (canonical)
  name TEXT NOT NULL,
  pov_character_id TEXT REFERENCES character(id),
  location_id TEXT REFERENCES world_entry(id),
  scene_goal TEXT,
  status TEXT NOT NULL DEFAULT 'draft',
  word_count INTEGER NOT NULL DEFAULT 0,
  file_path TEXT NOT NULL,
  file_hash TEXT,                   -- for change detection vs disk
  created_at TEXT, updated_at TEXT
);
CREATE INDEX scene_by_manuscript ON scene(manuscript_id, ordering);

CREATE TABLE scene_character_presence (
  scene_id TEXT NOT NULL REFERENCES scene(id),
  character_id TEXT NOT NULL REFERENCES character(id),
  PRIMARY KEY (scene_id, character_id)
);

-- Characters (LSM v2.1)
CREATE TABLE character (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES project(id),
  name TEXT NOT NULL,
  schema_version TEXT NOT NULL DEFAULT 'lsm-v2.1',
  data_json TEXT NOT NULL,          -- full LSM fields (main / bonus / arc / perspectives)
  file_path TEXT NOT NULL,
  file_hash TEXT,
  created_at TEXT, updated_at TEXT
);

-- World/setting (segmented)
CREATE TABLE world_segment (
  id TEXT PRIMARY KEY,              -- e.g. 'concept', 'locations', 'politics-and-social'
  project_id TEXT NOT NULL REFERENCES project(id),
  name TEXT NOT NULL,
  ordering INTEGER NOT NULL,
  is_collection INTEGER NOT NULL DEFAULT 0  -- 0=single doc, 1=folder of entries
);
CREATE TABLE world_entry (
  id TEXT PRIMARY KEY,
  segment_id TEXT NOT NULL REFERENCES world_segment(id),
  name TEXT NOT NULL,
  data_json TEXT NOT NULL,          -- schema depends on segment template
  file_path TEXT NOT NULL,
  file_hash TEXT
);

-- Pinned pills (ephemeral pills do not persist)
CREATE TABLE pinned_pill (
  id TEXT PRIMARY KEY,
  scene_id TEXT NOT NULL REFERENCES scene(id),
  block_id TEXT NOT NULL,
  snippet TEXT NOT NULL,
  speaker_kind TEXT NOT NULL,       -- 'character' | 'persona'
  speaker_id TEXT NOT NULL,         -- character.id or persona slug
  message TEXT NOT NULL,
  hue TEXT NOT NULL,                -- design token name, not raw color
  rabbit_hole_path TEXT,            -- JSON: ordered list of pill texts along the path
  created_at TEXT
);

-- Analysis cache (derivable; safe to wipe)
CREATE TABLE scene_metrics (
  scene_id TEXT PRIMARY KEY REFERENCES scene(id),
  flow REAL, coherence REAL, engagement REAL, divergence REAL,
  pace REAL, intensity REAL, valence REAL,
  lexical_diversity REAL, sentence_complexity REAL,
  ocean_o REAL, ocean_c REAL, ocean_e REAL, ocean_a REAL, ocean_n REAL,
  beat_label TEXT,                  -- 'setup' | 'inciting' | 'rising' | 'midpoint' | ...
  beat_confidence REAL,
  summary TEXT,                     -- 1–2 sentence scene summary (local LLM by default)
  summary_for_hash TEXT,            -- file_hash at which summary was generated
  summary_model_id TEXT,            -- which model produced the summary
  last_analyzed_at TEXT,
  source_file_hash TEXT             -- so we know if cache is stale
);

CREATE TABLE block_metrics (        -- per-paragraph, only kept for the current scene
  scene_id TEXT NOT NULL REFERENCES scene(id),
  block_id TEXT NOT NULL,
  flow REAL, coherence REAL, divergence REAL,
  PRIMARY KEY (scene_id, block_id)
);

-- Snapshot history
CREATE TABLE snapshot (
  id TEXT PRIMARY KEY,              -- ULID
  scene_id TEXT NOT NULL REFERENCES scene(id),
  taken_at TEXT NOT NULL,
  trigger TEXT NOT NULL,            -- 'autosave' | 'hourly' | 'on-close' | 'pre-restore' | 'manual'
  file_path TEXT NOT NULL,          -- snapshots/<scene_id>/<ts>.zst
  byte_size INTEGER NOT NULL
);
CREATE INDEX snapshot_by_scene ON snapshot(scene_id, taken_at DESC);

-- Settings / providers (secrets live in the OS keychain, referenced by id here)
CREATE TABLE settings (key TEXT PRIMARY KEY, value_json TEXT NOT NULL);
CREATE TABLE provider_config (
  id TEXT PRIMARY KEY,              -- 'anthropic' | 'openai' | 'ollama' | 'llamacpp-kimi' | 'mlx-...' | ...
  enabled INTEGER NOT NULL,
  config_json TEXT NOT NULL,        -- model name, base_url, etc. NO secrets here.
  ordering INTEGER NOT NULL         -- primary/fallback order
);

-- Telemetry buffer (opt-out; flushed when online if enabled)
CREATE TABLE telemetry_event (
  id TEXT PRIMARY KEY,
  recorded_at TEXT NOT NULL,
  kind TEXT NOT NULL,               -- 'pill_timing' | 'fallback' | 'crash' | ...
  payload_json TEXT NOT NULL,
  sent INTEGER NOT NULL DEFAULT 0
);
```

### 3.5 Character schema (LSM v2.1) — stored as `data_json`

A flat, well-named object so the LLM can read it directly into a prompt:

```json
{
  "main": {
    "full_name": "...",
    "aliases": ["..."],
    "age": "...",
    "pronouns": "...",
    "role_in_story": "protagonist | antagonist | supporting | ...",
    "want": "...",
    "need": "...",
    "ghost_wound": "...",
    "lie_they_believe": "...",
    "truth": "...",
    "fatal_flaw": "...",
    "strength": "..."
  },
  "bonus_traits": {
    "voice": "...",
    "tells": ["..."],
    "habits": ["..."],
    "speech_patterns": ["..."],
    "physicality": "...",
    "preferences": { "...": "..." },
    "fears": ["..."],
    "values": ["..."]
  },
  "arc": {
    "starting_state": "...",
    "ending_state": "...",
    "inciting_change": "...",
    "midpoint_shift": "...",
    "climax_choice": "..."
  },
  "perspectives": {
    "self_view": "...",
    "others_view": "...",
    "narrator_view": "...",
    "antagonist_view": "..."
  }
}
```

Each field has a `prompt_question` registered in code (used by Conversational Intake). Adding or removing fields is a schema migration.

### 3.6 World/setting templates

Each `world_segment` carries a template (built-in or user-defined). Default templates:

- `concept` — single doc — `{ core_premise, genre, tone, themes[], inspirations[] }`
- `locations` — collection — `{ name, type, sensory_detail, notable_features[], significance }`
- `world` — single doc — `{ era, technology_level, magic_or_speculative_rules, geography }`
- `politics_and_social` — single doc — `{ governance, factions[], conflicts, hierarchies, taboos[] }`
- `culture` — single doc — `{ languages, religions, art_and_ritual, daily_life }`
- `history` — single doc — `{ timeline_beats[], legends[], unresolved_threads[] }`

Users can add/remove segments and customize templates in settings (v1 restriction: built-in segments can be hidden but not deleted). All templates are JSON-Schema-shaped so Conversational Intake walks them automatically.

### 3.7 Snapshot strategy

- **Autosave** (live): every typing pause ≥ 2 s, the renderer flushes block content to the `.md` file via Rust core. Cheap, frequent, **no snapshot taken**.
- **Snapshot** (hourly + on-close + pre-restore + manual): Rust core compresses the current `.md` with zstd and writes to `snapshots/<scene_id>/<timestamp>.zst`. Stores a row in `snapshot`.
- **Retention**: keep all snapshots from the last 24 h, hourly for the last 7 d, daily for the last 90 d, weekly forever. Configurable.
- **Restore UI**: a soft horizontal timeline above the editor lets the writer scrub through snapshots for the current scene. Restoring first takes a `pre-restore` snapshot of the current state so the operation is reversible.

### 3.8 Rebuild-from-truth flow

If `project.db` is missing or its `schema_version` is older than supported:

1. Rust core scans the project folder.
2. Reads all `.toml` and `.md` files, parses frontmatter and content.
3. Rebuilds every index table (`scene`, `character`, `world_entry`, `chapter`, `scene_character_presence`).
4. Marks `scene_metrics` and `block_metrics` as stale — sidecar will re-analyze on demand.
5. Snapshots and pinned pills are reattached by `scene_id` matching filenames.

This is what makes the format Obsidian-like: the user's data is never trapped in the DB.

### 3.9 Schema migrations

`schema_version` is tracked in both `water.toml` and `project.db`. On project open:

- If `project.toml.schema_version > app.schema_version` → refuse to open; ask the user to update Water.
- If `project.toml.schema_version < app.schema_version` → run forward migrations in a transaction, snapshotting the original `.water` folder to `.water/backups/` first.
- Migrations are pure Rust, version-tagged, idempotent. Each migration knows how to upgrade both `project.db` and the on-disk files (e.g., adding `^bk-XXXX` tokens to legacy scenes).

### 3.10 External-edit "repair" pass

When a project opens, Water also runs a repair pass to tolerate users editing files outside Water:

- Regenerate missing `^bk-XXXX` markers (assigning new IDs at the end of paragraphs without them).
- Re-derive `word_count` from file content.
- Reconcile `chapters.toml` against `scene.chapter_id` (chapter wins if they disagree; orphan scenes go to "unfiled").
- Re-link pinned pills referencing dead block IDs to the nearest fuzzy-match block; if no match, archive the pin (kept in `pinned_pill` but soft-hidden).

---

## 4. Subsystem decomposition & build order

Seven internal milestones plus a parallel evaluation harness. Each milestone gets its own internal spec → plan → impl cycle (discipline), but all ship together in v1.

### 4.1 Dependency graph

```
        ┌────────────────────────────────────────────┐
        │  M1 — Foundation                            │
        │  (Tauri shell, design tokens, project store,│
        │   sidecar lifecycle, LLM provider router)   │
        └────────────────────────────────────────────┘
                   │            │           │
        ┌──────────┘            │           └────────────┐
        ▼                       ▼                        ▼
 ┌──────────────┐    ┌────────────────────┐    ┌────────────────┐
 │ M2 — Editor  │    │ M1.5 — Eval Harness│    │ M3 — Character │
 │ & Pill Engine│    │ (parallel, ongoing)│    │ Sheets (LSM)   │
 └──────────────┘    └────────────────────┘    └────────────────┘
        │                                              │
        │              ┌───────────────────────────────┘
        ▼              ▼
 ┌──────────────┐  ┌────────────────┐
 │ M4 — World   │  │ M5 — Heatmap   │
 │ Bible        │  │ Audiovisualizer│
 └──────────────┘  └────────────────┘
                        │
                        ▼
                 ┌──────────────────┐
                 │ M6 — Macro Canvas│
                 └──────────────────┘
                        │
                        ▼
                 ┌──────────────────┐
                 │ M7 — Config &    │
                 │ Polish           │
                 └──────────────────┘
```

### 4.2 M1 — Foundation (~2 weeks)

**Goal.** A Tauri app that opens a project folder, reads/writes scenes, autosaves, snapshots, and can call an LLM provider through a unified interface.

**Components.**

- Tauri shell (Rust core + React/TS renderer scaffold).
- Design-token system: typography, spacing, radii, motion easing, pastel-glow color scale, light/dark theming via CSS variables. Tokens adapted from Opencode `packages/ui` + `packages/storybook`. Aggressive divergence: Notion/Apple-fluid radii (`8/16/24/32/full`), no visible borders by default, no drop shadows (glow-only elevation, with a 1px tinted-border mitigation in light mode).
- Project store: SQLite + on-disk Markdown/TOML, rebuild-from-truth, schema migrations, external-edit repair pass.
- Autosave scheduler + snapshot scheduler + retention.
- Sidecar lifecycle: spawn standalone Python sidecar (via `uv`) at boot; health-check; restart on crash; kill on shutdown.
- LLM provider router: `LlmProvider` trait with adapters for Anthropic, OpenAI, Ollama, llama.cpp, and a feature-flagged MLX adapter on Apple Silicon. Rate limit + circuit breaker. OS keychain for secrets. Developer key-file support (`~/.water/dev-keys.toml`) for internal-test convenience.
- Diagnostics page (data-dense surface using monospace for numbers; surfaces stay pastel-glow): sidecar status, provider health, recent timings, fallback frequency.

**Exit criteria.**

- Create project, type into a scene, close, reopen → content persists.
- Delete `project.db` → Water rebuilds it from `.md`/`.toml` files.
- Pick a provider in settings, hit "test" → round-trip succeeds and returns a tiny canned bouquet.
- Snapshot timeline shows hourly entries; restoring works and creates a pre-restore snapshot.
- Sidecar boots in < 8 s on a typical laptop and responds to `/health`.

### 4.3 M1.5 — Model Evaluation Harness (parallel, ongoing)

**Goal.** A small offline harness that benchmarks candidate LLMs against Water's specific tasks so we choose defaults based on evidence, not vibes.

**Components.**

- **Gold tasks library**: each task is a JSON file with `manuscript_excerpt`, `character_sheet?`, `world_excerpt?`, `trigger`, and 3–5 human-rated reference pills (good/bad with reasons).
- **Task families**:
  - (a) character-voice pill at level 0
  - (b) bouquet expansion (variant diversity, voice consistency)
  - (c) regenerate without repetition
  - (d) persona pill that respects "reactive not instructive" tone
  - (e) heatmap beat labeling
  - (f) "no-universe-yet" Chorus pill that elicits world/character stubs
  - (g) scene summary
  - (h) `character_dissonance` trigger correctness
- **Runner** that hits each candidate provider/model with identical prompt templates and saves outputs.
- **Scoring**: hybrid rubric — automatic (length, JSON validity for structured outputs, instructive-phrase blacklist match, snippet groundedness) + human spot-check.
- **Report**: data-dense, monospace-rich surface (pastel substrate) showing per-model scores across task families, latency, cost-per-pill.

**Candidates to sweep (initial list — refinable each milestone gate).**

- Cloud: Claude Sonnet 4.5, Claude Haiku 4.5, GPT-4.x, Gemini 2.5
- Local via Ollama: Qwen 2.5 (1.5B / 3B / 7B / 14B / 32B), Llama 3.x (8B / 70B), Mistral-Nemo 12B, Phi-4
- Local via llama.cpp: Kimi K2 (quantized — long context window candidate), Qwen 3 (when available)
- Apple Silicon via MLX: any of the above ported
- Distilled/specialty: smallest models suitable for fast level-0 pill generation and scene summaries

**Bundled default selection.** The eval-harness winner across `context_window × pill_quality × cold_start_latency × installer_size_impact` becomes the bundled local default. Initial expectation is a quantized 1–3 B model (Qwen vs Kimi-quantized vs similar).

**Exit criteria.**

- Harness produces a comparable scorecard for ≥ 6 models.
- Recommended defaults selected and documented in this spec for (a) cloud primary, (b) local primary, (c) tiny/fast tier for level-0 pills and scene summaries.
- MLX adapter benchmarked vs Ollama on M-series.

### 4.4 M2 — Editor & Pill Engine (~3 weeks)

**Goal.** A Notion-style block editor with the full pill experience: surfacing, voice routing, expansion bouquets, regenerate, pin, dismiss. **Persona-only pills** ship first; character voices are wired in M3 against the same engine.

**Components.**

- **Editor bake-off** (Week 3 of M1, Week 1 of M2): build identical small prototypes in ProseMirror and Lexical. Decision criteria:
  - block-ID maintenance ergonomics (insert/delete/split/merge paragraphs preserves IDs)
  - decoration API for pill highlights and pill-snippet underlines
  - selection/mark stability under autosave write-backs
  - bundle size impact
  - perf on a 50 000-word scene
  - long-undo behavior
  Winner is committed before the rest of M2 proceeds.
- **Block editor**: paragraph, scene break, dialogue, heading-2/3, ordered/unordered list. Block IDs (`^bk-XXXX`) maintained automatically.
- **Typing telemetry**: idle detector, sentence-end / paragraph-end / mid-sentence cursor classifier, structural-inflection detector (POV change, new location, new scene, new chapter). Emitted as events to Rust core.
- **Pill orchestrator** (Rust core): state machine in Section 6. Pure, deterministic, unit-tested.
- **Voice router** (Rust core): chooses character-track vs persona-track and selects the specific speaker. Deterministic for `(trigger, scene_state, project_state)`.
- **Prompt library** (Rust): templated prompts per `(speaker_kind × trigger × task)`. TOML files shipped with the app; hot-reloadable in dev. Tone enforced globally (Section 6.3).
- **Bouquet generator**: at expansion, requests exactly N = 3 variants in structured JSON. Regenerate adds prior-variant-exclusion clauses.
- **Anti-loop**: temperature bump + diverge-clause when consecutive bouquets at the same node have ≥ 70 % token overlap.
- **Pill UI**: pastel-glow capsule in margin; hover dims the rest of the page and pulls a soft glowing line to the highlighted block snippet; click expands inline into a soft "bouquet" of 3 sub-pills stacked vertically with breathing-space, plus a translucent regenerate icon, a pin icon, and an X. Pinned pills move to a quiet column at the right edge of the page, always visible at half-opacity until clicked.
- **Rabbit hole**: unlimited depth. Prior-bouquet siblings collapse to thin glow lines along the left edge of the rabbit-hole panel as the writer descends. Click path renders as a breadcrumb chain at the top. Only the current bouquet is fully rendered. Scroll up at any time to re-expand prior siblings. `X` closes the whole thread.
- **Persona registry**: 5 default named personas (each renameable per project), each with a distinct hue and tone profile. Working set: **Echo** (muse — emerges what's almost-there), **Architect** (structure/pace), **Editor** (diction/clarity), **Cartographer** (world/setting consistency), **Chorus** (eliciting mode only).

**Exit criteria.**

- Idle for 3 s after a paragraph → at most one pill surfaces within 1.5 s of the analysis completing.
- Two pills max on screen; mid-sentence typing never surfaces a pill.
- Expanding a pill shows exactly 3 sub-pills; regenerate produces 3 different ones.
- Rabbit hole works at arbitrary depth; breadcrumb collapse visible; anti-loop fires when configured.
- Pinning a pill persists it across app restart; dismissed pills do not.
- Tone audit: 0 instances of "you should", "consider", "try", "I think you", "as an AI" in 200 sampled pills.

### 4.5 M3 — Character Sheets (~1.5 weeks)

**Goal.** Full LSM v2.1 character sheet UI + Conversational Intake popup pattern, reused for World Bible and scene goals. Wire character voices into the M2 pill engine.

**Components.**

- **Conversational Intake** — reusable component. Takes a JSON-Schema-like descriptor of questions (label, helper, type, validation, optional-skip). Renders one question at a time in a soft glowing popup, walks the schema, writes the result. Animations: gentle fade + subtle vertical glide between questions. Skippable per question. Resumable mid-flow.
- **LSM v2.1 schema descriptors** — four schemas: main, bonus_traits, arc, perspectives. Each field carries a `prompt_question`, optional `helper`, optional `examples`.
- **Character sheet view** — fully editable document view of a character. Inline editing, autosave. "Continue intake" button if any sections remain incomplete.
- **Character index** — soft grid of glowing character cards (avatar placeholder, role hue, want/need teaser). Sortable, searchable.
- **Scene ↔ character linking** — multi-select in scene metadata + auto-suggest based on names appearing in scene text.
- **Character voice prompt template** — renders per character via the LSM sheet.

**Exit criteria.**

- Create a new character → walk full intake in ≤ 3 minutes → result matches LSM v2.1 PDF schema.
- Edit any field; on save, the corresponding `.toml` in `characters/` reflects the change.
- Open the character `.toml` in any text editor — it is human-readable.
- Character voice fires in an appropriate trigger; voice routing prefers POV character when present.

### 4.6 M4 — World/Setting Bible (~1 week)

**Goal.** Segmented world bible using the same Conversational Intake pattern, plus the Cartographer trigger and `world_drift` check.

**Components.**

- Default segments (Section 3.6).
- Segment editor — single-doc or collection mode based on `is_collection`.
- Segment template editor (settings) — add/remove fields, add segments. v1 limit: cannot remove built-in segments, only hide.
- World index view — soft glowing tile per segment + collection items.
- `world_drift` trigger — cheap exact-match check between named entities in text and world entries; LLM follow-up only on mismatch; fires Cartographer.

**Exit criteria.**

- All built-in segments work with intake.
- User can add a new segment with a custom template and intake walks it.
- Character voice prompts can pull relevant `world_entry` excerpts (e.g., scene's `location_id`'s `sensory_detail`).
- `world_drift` correctly identifies a planted contradiction in test scenes.

### 4.7 M5 — Heatmap Audiovisualizer (~2 weeks)

**Goal.** The tiered-zoom living arc (Section 5.1).

**Components.**

- **Per-scene metric rollup** (sidecar): pace, intensity, valence, in addition to the mechanical metrics (flow, coherence, divergence) already available. Debounced re-analysis when a scene's `file_hash` changes.
- **Scene summary subsystem**: small fast local LLM (chosen by eval harness) produces 1–2 sentence summaries. Triggered when `file_hash` changes AND `word_count ≥ 100` AND debounced ≥ 30 s since last edit AND last summary > 2 min old. Cached on `(scene_id, source_file_hash, model_id, prompt_version)`. Cloud fallback toggle in settings; auto-fallback if local takes > 8 s wall-clock for 3 consecutive scenes.
- **Beat-label subsystem**: a single LLM call with all scene summaries concatenated produces beat labels + confidences. Pulse markers appear only when `confidence ≥ 0.7`. Triggered when `manuscript-wide word_count` changes ≥ 5 % since last classification.
- **Heatmap cache** in SQLite (`scene_metrics`); invalidated by hash mismatch.
- **Renderer** — SVG/Canvas hybrid:
  - Default zoom (level 0): single glowing intensity curve over scenes, soft pulse markers for beat labels with confidence above threshold.
  - Mid zoom (level 1): curve thickens (width = pace), two translucent bands appear (flow + valence).
  - Deep zoom (level 3 scene-detail, level 5 paragraph-lens — paragraph-lens stretch): full multi-band EKG; hover reveals per-paragraph contributions.
- **Animation**: gentle wave-like easing as new metrics arrive. Reduced-motion replaces with crossfade.
- **Click** a scene → zooms to it; **drag** the curve horizontally → pans the manuscript view.

**Exit criteria.**

- Curve updates within 5 s of a scene reaching a stable state after typing pause.
- Zoom transitions are continuous (no view-switch flicker).
- Beat-label pulse markers appear only above 0.7 confidence; below that, no annotation.

### 4.8 M6 — Macro Spatial Scene Canvas (~2 weeks)

**Goal.** The drag-and-drop, hyper-interactive macro view of scenes as cards. Owns the Spatial Continuum's zoom levels 2–3.

**Components.**

- **Scene card**: pastel-glow tile showing name, POV hue, characters present (mini avatars), word count, intensity sparkline, beat label, scene summary excerpt.
- **Layouts**: **timeline** (linear left → right with chapter groupings), **board** (Kanban by `status`), **tree** (chapter → scene hierarchy), **freeform** (user-positioned cards on a canvas, positions persisted per-project). Switchable via tabs at zoom level 2.
- **Drag-and-drop** reorder (timeline + board + tree); drag-to-reposition (freeform). Reorder updates `scene.ordering` and `chapters.toml`.
- **Zoom-to-editor**: clicking a scene zooms the card smoothly into the editor view; closing zooms back. This is the macro ↔ micro navigation.
- **Chapter management**: create, rename, dissolve, reorder; "unfiled" bin for scenes without a chapter.
- **Zoom rail + menu nav** (dual navigation): a soft glowing zoom rail down the right edge (drag, click, hover-name), plus `⌘ 1…5` keyboard shortcuts and a traditional menu nav for discrete jumps.

**Exit criteria.**

- Drag a scene across chapters → both `.md` frontmatter and `chapters.toml` update; SQLite stays consistent.
- Layout switch preserves selection and scroll position.
- Zoom-in / zoom-out transition is one continuous animation, not a route change.
- Continuous-zoom path and menu-nav path both reach every level without bugs.

### 4.9 M7 — Config, Polish & Tester Build (~1.5 weeks)

**Goal.** Every cadence/policy knob, accessibility pass, onboarding, plugin contracts, and the installer.

**Components.**

- **Settings UI**:
  - Appearance (theme: light/dark/auto, motion, density, font choice)
  - Pills (cadence, max visible, TTL, cooldowns, track mix, persona names + hues, sensitivity slider, snooze)
  - Analysis (sidecar verbosity, metric weights, beat-confidence threshold)
  - Providers (cloud/local primary/fallback, API keys via keychain, MLX toggle on Apple Silicon, scene-summary cloud-allowed toggle)
  - Project (autosave/snapshot retention, segments, per-project privacy lock)
  - Diagnostics (sidecar status, provider health, replay log opt-in, telemetry toggle)
- **Accessibility pass**: reduced motion, color-blind-friendly palettes (deuteranopia/protanopia/tritanopia variants), keyboard nav (`Tab`/`Enter`/`R`/`P`/`Esc` on pills), focus rings, screen-reader labels.
- **Onboarding**: first-launch picks provider(s); names the writer; creates or opens a project; intakes one character (skippable); shows the first pill within 90 s of pressing Enter the first time.
- **Plugin contracts** (locked, not implemented): `PluginManifest` schema, read surfaces, contribute surfaces, sandboxing decision (out-of-process JS vs Wasm — final pick deferred, API shaped to support either), stable IDs, versioned event schema.
- **Crash reporter** (telemetry on by default in internal build) + diagnostics export bundle.
- **Installers**: macOS `.dmg` (universal) + Windows `.msi` via Tauri's builder. Code-signing certificates pursued in M7 if budget allows; otherwise documented tester workaround.

**Exit criteria.**

- All defaults pass the fresh-laptop tester checklist (no Python install needed, no terminal, no API key required for local mode).
- Settings changes apply without restart where possible; otherwise prompt + auto-restart.
- v1 build hands off to internal testers.

---

## 5. UX & visual system

### 5.1 The Spatial Continuum

Water has **one continuous zoomable spatial surface** where zoom level itself selects what the writer sees, plus a parallel **traditional menu navigation** that jumps directly to a level. This dual affordance honors both the "tree that sprouts from its roots" thesis and the experienced writer's preference for direct addressing.

```
zoom 0 ─── Manuscript Arc      audiovisualizer ribbon, full story length
zoom 1 ─── Chapter Tide        chapter waves overlay the ribbon; chapter names emerge
zoom 2 ─── Scene Mosaic        scene cards along the ribbon; timeline/board/tree/freeform
zoom 3 ─── Scene Detail        one scene card enlarged — synopsis, characters, goal, sparkline
zoom 4 ─── Editor              the Notion-style block editor for that scene; pills surface here
zoom 5 ─── Paragraph Lens      paragraph metrics overlay, sentence-level coloring (stretch)
```

**Controls.**

- Scroll wheel / trackpad scroll with `⌘` (macOS) or `Ctrl` (Windows) held; trackpad pinch-to-zoom.
- A soft glowing **zoom rail** down the right edge — drag, click a level, or hover for level names.
- Keyboard: `⌘ +` / `⌘ −` step a level; `⌘ 1…5` jump to a specific level; `⌘ 0` jumps to Scene Mosaic.
- Double-click any visible item zooms in centered on it.

**Transitions.** Cross-zoom is one continuous animation — never a route change. Elements that exist at both zooms (a scene's hue, its intensity sparkline) morph between scales; elements only at one zoom (e.g., editor text) crossfade in over ~280 ms with a soft glow halo. Zoom snaps to canonical levels when the user releases scroll/pinch.

**State preservation.** Zooming up from the editor remembers caret position; zooming back drops the writer there. Zooming into Scene Mosaic from the editor centers and pulses the focused scene card for one beat.

**Implication for milestones.** M5 owns the metric bands at every level and zoom 0–1. M6 owns levels 2–3 and the scene-card design system. The editor at level 4 lives in M2. Paragraph Lens at level 5 is a stretch.

### 5.2 Visual language

**Color philosophy — two layers.**

1. **Substrate** — neutral, calm, Apple-minimal. Slim sans-serif body (Inter or close), careful serif accent for scene/chapter names. Light mode: warm-paper-white. Dark mode: deep cool-neutral with subtle blue-violet undertones — the *liminal glowing space*.
2. **Glow** — semantic pastel hues at low chroma + soft outer glow. **Glow signals meaning.** Pills, metric bands, character/persona avatars, beat markers. Never decorative.

**Glow palette tokens (final shades determined in M7 polish).**

| Token | Used for | Feel |
|---|---|---|
| `hue.flow` | flow band, flow pills | mint-cyan |
| `hue.coherence` | coherence band | soft periwinkle |
| `hue.intensity` | intensity ribbon | dusk-rose |
| `hue.valence-pos` / `hue.valence-neg` | valence band gradient | peach ↔ icy-lavender |
| `hue.pace` | pace (ribbon thickness) | warm vanilla |
| `hue.drift` | drift band, drift pills | dim coral |
| `hue.muse` (Echo) | persona Echo | pale rose-gold |
| `hue.architect` | persona Architect | quiet sage |
| `hue.editor` | persona Editor | lilac-grey |
| `hue.cartographer` | persona Cartographer | dune amber |
| `hue.chorus` | persona Chorus | pearl |
| `hue.character.*` | per-character voices | auto-assigned from a pastel wheel, avoiding collisions with persona hues |

**Typography scale.**

- `display` 28/36 (chapter & scene titles)
- `title` 20/28
- `body` 16/26 (manuscript body — slightly larger and looser than UI body to favor reading)
- `ui` 14/20
- `meta` 12/18 (frontmatter chips, tooltips, diagnostics)
- Optional serif for `display` + `title` on manuscript surfaces; sans for UI. Toggle in settings.

**Spacing & radius (aggressive fluid identity).**

- 4-pt base grid.
- Radius scale: `8 / 16 / 24 / 32 / full`. Default card radius is 24, default pill is fully rounded, default input is 16. Hard corners are forbidden in v1 surfaces.
- No visible borders by default. Surfaces separate via low-chroma fill differences + outer-glow halos.
- A 1px tinted border appears *only* on focus and on light-mode card mitigation.
- Containers trend toward stadium/blob shapes; multi-card groupings share a single rounded plate rather than each card having its own rectangle.
- Subtle morph during transitions: corners breathe ± 2 px during cross-zoom so the layout feels alive, not mechanical.

**Elevation & glow.** No drop shadows. Elevation is a 2-stage glow: inner subtle gradient + outer soft halo at low opacity, hue-matched to the surface's semantic token.

**Motion.**

- Easing: a custom `ease-out-soft` cubic for entrances; `ease-in-out-water` for cross-zoom; linear forbidden except progress indicators.
- Durations: tiny ≤ 120 ms (hover, focus), small 200–280 ms (pill in/out, intake question swap), medium 320–480 ms (cross-zoom), long 600–900 ms (initial page reveal, restoration animations).
- Reduced motion: every animation falls back to a 100 ms crossfade or no animation. Audiovisualizer wave-morph becomes a static stepped curve. Honors OS preference and a manual toggle.

**Light vs dark.**

- Light is the **drafting** mode: warm paper, low contrast for long reading.
- Dark is the **visualizing** mode: at zoom 0–2 the page becomes the *liminal glowing space* — deep neutral background, ribbon and bands glow with their own light, glow palette boosted in chroma by ~15 %.
- Theme toggle: `⌘ ⇧ L`. Default is `auto` (follows `prefers-color-scheme`).

**Sound.** Out of scope for v1. Noted for v1.1.

### 5.3 Surfaces

- **Editor surface (zoom 4)** — paper. Maximally calm. No glow except pill margin halos and a soft underline on the currently-anchored pill snippet. Block-id markers (`^bk-XXXX`) render as zero-width.
- **Pills** — pastel-glow capsule in the right margin; hue = speaker's hue; height auto. Hover dims the rest of the page 8 % and draws a soft 1 px glowing line from pill to highlighted snippet. Click expands inline into a bouquet of 3 sub-pills stacked vertically with breathing-space, plus translucent regenerate, pin, and X. Pinned pills migrate to a quiet column at the right edge of the page, always visible at half-opacity until clicked.
- **Conversational Intake popup** — centered glowing card, one question per beat. Question text is `title`-sized; helper text is `meta`. Input field is borderless, underlined-only, large. "Next" and "Skip" buttons are soft. Progress is a dim glowing dot row at the bottom.
- **Character / World docs** — feel like Notion pages with our pastel-glow color tokens. Inline editing, autosave indicator is a slow soft pulse on the title.
- **Scene Mosaic cards** — soft-glow rounded tiles. Body: name in `title`, intensity sparkline behind the name (very low opacity), POV character avatar dot, characters-present mini avatars, word-count chip, status pip, beat-label chip, scene-summary excerpt. Drag-handle is a 3-dot affordance fading in on hover.
- **Zoom rail** — fixed to the right edge, ~16 px wide, mostly transparent until hover; on hover, soft glow + level names slide out.
- **Diagnostics surfaces** — pastel substrate as everywhere else, but content uses monospace for numbers and tighter density (data-dense, restraint-led — `packages/tui` influence applied to *content*, not to *surface visuals*).

### 5.4 Information hierarchy rules

1. **The manuscript text is always the primary surface.** Pills exist in the margin; metrics exist in zoom-out views; settings live behind a single icon.
2. **Glow signals meaning.** If a thing glows, it carries information (state, hue-semantic, attention). Decorative glow is a bug.
3. **One thing pulses at a time.** Only the most recently surfaced pill pulses; new pulses replace old. Beat markers on the audiovisualizer pulse once on first appearance.
4. **No badges with numbers.** Counts are conveyed by ribbon thickness, sparkline shape, or dot density — never by a circled number.

### 5.5 Accessibility floor

- WCAG AA contrast for text on all surfaces, including glow surfaces (tested against page-bg at actual blend).
- Full keyboard navigation including pill expand/regenerate/dismiss (`Tab` to focus pill row, `Enter` expand, `R` regenerate, `P` pin, `Esc` dismiss).
- Screen-reader labels on every pill: "Echo, level 1, anchored to 'the fog rolled in low'". Voice routing announces speaker.
- Honors `prefers-reduced-motion`, `prefers-color-scheme`, `prefers-contrast`.
- Color-blind-friendly alt palette swap. Selectable in Appearance settings.

---

## 6. Pill engine

This is the moat. The engine is owned by the Rust core's `pill_orchestrator` + `voice_router` + `prompt_library`, with LLM providers performing the generation. Everything except the LLM call is deterministic and unit-testable.

### 6.1 Trigger taxonomy

Two signal streams.

**Stream A — Typing telemetry** (from renderer, ~10 Hz when active):

- `idle_for_ms`
- `cursor_at_sentence_end` / `cursor_at_paragraph_end` / `cursor_mid_sentence`
- `block_id` of cursor's current paragraph
- `recent_word_delta`
- `structural_inflection`: `pov_change` | `location_change` | `new_scene` | `new_chapter` | `none`

**Stream B — Analysis updates** (from sidecar, async, debounced ~3 s):

- `flow`, `coherence`, `engagement`, `divergence`, `pace`, `intensity`, `valence`
- `block_metrics` for the current scene (per-paragraph)
- OCEAN drift vector (slow loop)

**Trigger classes** (non-exclusive; highest-priority firing wins):

| Class | Condition | Default speaker track |
|---|---|---|
| `block_anchored_drift` | block `divergence > 0.6` OR block `coherence < 0.35` on the just-finished paragraph | persona (Editor) or character if POV present |
| `scene_flow_dip` | scene `flow < 0.4` sustained ≥ 30 s | persona (Echo) |
| `topic_drift` | scene `coherence < 0.35` AND `divergence > 0.5` | persona (Architect) |
| `valence_spike` | abs change in valence > 0.4 vs scene mean | character if present, else persona (Echo) |
| `structural_inflection` | event from Stream A | character (POV) or Cartographer if `location_change` |
| `pace_floor` | scene `pace < 0.3` AND `word_count_in_last_3min < 40` | persona (Architect) |
| `world_drift` | named entity contradicts world bible (cheap exact-match; LLM follow-up only on mismatch) | persona (Cartographer) |
| `character_dissonance` | character speaks/acts contrary to stated value/want/lie in their sheet (lemma-overlap heuristic — **listed in `KNOWN_FRAGILE.md`**) | that character |
| `no_universe_yet` | project has 0 characters with sheets AND world is empty AND scene `word_count > 200` | persona (Chorus), eliciting mode |
| `idle_pause_with_present_character` | `idle ≥ 8 s` AND scene has present character AND no recent character pill | random present character (rotated) |

Each tick fires *at most one* pill. Priority order is the table order top to bottom for block-anchored triggers, then by recency-of-signal for the rest. Per-track cooldowns (45 s default) prevent the same speaker from interrupting twice in a row.

### 6.2 Voice router

```
function route(trigger, scene, characters, personas, project_state):
    candidates = []

    if trigger preferred_track == "character" and scene.characters_present is non-empty:
        # POV character bias; otherwise pick the character with the strongest
        # relevant sheet field for the trigger (e.g., valence_spike → character
        # whose fears or values most overlap with the event).
        candidates += relevant_characters(scene, trigger.reason)

    if trigger preferred_track == "persona" or candidates is empty:
        candidates += [matching_persona(trigger.class)]

    # Tie-break by least-recently-used among non-cooled-down candidates.
    return pick_least_recently_used(candidates)
```

Routing is deterministic for `(trigger, scene_state, project_state)` — replayable in tests.

**No-universe-yet mode.** When the project has no characters and no world entries, the orchestrator switches into eliciting mode. Chorus pills surface as gentle observations *of the writer's own text* that imply the existence of an unnamed world ("there's a smell on this page that I don't yet have a word for"). On expansion, the bouquet offers 3 directions the universe might bend in. Pinning a Chorus sub-pill creates a `world_entry` stub (e.g., a new location seeded with the snippet as its `sensory_detail`).

### 6.3 Prompt library

Prompts are TOML files shipped with the app:

```
prompts/
├── tone.toml                          # global tone clauses, instructive-phrase blacklist
├── speakers/
│   ├── persona/
│   │   ├── echo.toml
│   │   ├── architect.toml
│   │   ├── editor.toml
│   │   ├── cartographer.toml
│   │   └── chorus.toml
│   └── character/
│       └── template.toml              # rendered with the LSM sheet of the speaking character
├── triggers/
│   ├── block_anchored_drift.toml
│   ├── scene_flow_dip.toml
│   ├── …
│   └── no_universe_yet.toml
└── tasks/
    ├── pill_level_0.toml              # single pill on first surface
    ├── pill_expand.toml               # bouquet of 3 sub-pills
    ├── pill_regenerate.toml           # bouquet excluding prior variants
    ├── scene_summary.toml             # small LLM, 1–2 sentences
    └── beat_label.toml                # classify scene beat with confidence
```

A pill request is assembled as `tone` + `speaker` + `trigger` + `task` + `inputs` (text excerpt, character sheet excerpt, world excerpt as relevant). Target ~600 tokens including the manuscript excerpt for level-0 pills.

**Global tone clauses (excerpt of `tone.toml`).**

- "Speak in present tense as if you are noticing this just now."
- "You are not an assistant. You do not give writing advice."
- "Never say: 'you should', 'consider', 'try', 'maybe you could', 'I think you', 'as an AI', 'this is good/bad'."
- "Observe, react, wonder. Leave space."
- "Output exactly one line of prose, ≤ 22 words. No quotation marks. No emoji."
- "If you cannot react in your speaker's voice without breaking these rules, output the single token `PASS` and nothing else."

A `PASS` response triggers a single retry with a slightly different sampling temperature; a second `PASS` quietly drops the pill (no surfacing).

**Output shapes.** Bouquet and beat-label requests demand strict JSON. Level-0 single-pill requests demand plain prose (one line). The router validates shape and re-asks once on malformed output before dropping.

**Post-hoc filter.** In addition to the prompt clause, the router runs the instructive-phrase blacklist against returned text and drops pills that match (with a diagnostics log entry, so eval-harness can track leak frequency).

### 6.4 Bouquet generation

On click of a level-0 pill:

1. Orchestrator calls `pill_expand` with: original pill text, scene excerpt, speaker definition.
2. Provider returns JSON `[{"angle": "...", "text": "..."}, …]` — exactly 3 items.
3. `angle` is a one-word tag (e.g., "feel", "notice", "wonder") used only to seed the renderer's micro-animation that makes the three sub-pills visually distinct (slight hue shift). It is **not shown** to the writer.
4. Orchestrator records the 3 texts in session-scoped `bouquet_history` keyed by parent pill id.
5. Renderer renders the bouquet inline below the parent pill with soft glide-in.

**Regenerate** calls `pill_regenerate` with the same context plus `previous_variants_first_words: [...]` (first 8 words of each prior variant in this bouquet's history, across all regenerates for this pill). The prompt instructs the model to produce 3 *new* angles materially different in opening and idea. Repetition guard: any new variant with > 70 % token overlap with any prior is single-retried.

**Sub-bouquet** (clicking a sub-pill to go deeper): same `pill_expand` task with the clicked sub-pill now the parent. **Depth is unlimited**. Mechanisms to keep this sane:

- **Bouquet history bound**: each rabbit hole keeps the last 20 levels of context in the prompt; older levels collapse into a 1–2 sentence "summary of the path so far" inserted as background.
- **Anti-loop**: temperature bumps + diverge clauses (Section 6.3 anti-loop).
- **UI accommodation**: prior bouquets' non-chosen siblings collapse to thin glow lines along the left edge of the panel; breadcrumb chain at top; only current bouquet fully rendered.
- **Always-available close**: `X` ends the entire thread; pinning at any depth saves to the scene before close.

### 6.5 Scene summary subsystem

- **Model tier**: smallest viable model from M1.5 eval; defaults to local; cloud allowed via settings toggle; auto-fallback to cloud if local takes > 8 s wall-clock for 3 consecutive scenes (one-time soft notice on first auto-fallback).
- **When to regenerate**: scene `file_hash` changed AND scene has ≥ 100 words AND debounced ≥ 30 s since last edit AND last summary > 2 min old.
- **Storage**: `scene_metrics.summary`, `.summary_for_hash`, `.summary_model_id`.
- **Prompt** (`tasks/scene_summary.toml`): "Summarize what happens in this scene in 1–2 sentences, present tense, no character interpretation, no judgment. Output prose only."
- **Tone enforcement**: same blacklist as pills.

### 6.6 Beat-label subsystem

- **When**: same trigger as summaries but additionally requires manuscript-wide `word_count` change ≥ 5 % since last whole-manuscript classification.
- **Approach**: single LLM call with all scene summaries concatenated; output JSON `[{scene_id, label, confidence}, …]`.
- **Labels**: `setup`, `inciting`, `rising`, `midpoint`, `complication`, `climax`, `soft_climax`, `falling`, `denouement`, `coda`, `unlabeled`.
- **Confidence threshold**: marker shown only at `confidence ≥ 0.7`.
- **Storage**: `scene_metrics.beat_label`, `.beat_confidence`.

### 6.7 Caching & determinism

- **Pill responses are not cached.** Pills are ephemeral and context-sensitive.
- **Bouquet history is session-scoped** (cleared at app close) so regenerate diversification works.
- **Scene summaries and beat labels are cached** on `(scene_id, source_file_hash, model_id, prompt_version)`. Bumping prompt or model invalidates cleanly.
- **Prompt templates are versioned.** Each TOML has `version = "..."`. Migrations track which prompts changed; affected caches invalidate.
- **Replay logs (opt-in, off by default).** When enabled, every LLM call's inputs + outputs are written to `.water/log/llm/*.jsonl` so we can replay an exact session for debugging without re-running the LLM. Useful for eval-harness regression.

### 6.8 Failure modes

| Mode | Behavior |
|---|---|
| Sidecar down | Pills pause; status indicator shows "analysis paused"; editor and save fully functional |
| LLM provider error (timeout, 5xx) | Fall back to secondary provider if configured; otherwise quietly drop the pill (no error UI) |
| LLM provider rate-limited | Circuit breaker opens for 60 s; user notified once via a tiny status chip |
| Malformed LLM JSON | Single retry; on second failure, drop |
| `PASS` output | Single retry at different temperature; on second `PASS`, drop |
| User offline + cloud-only configured | App switches to local provider if available; otherwise shows "pills offline" chip |
| World-drift trigger fires | Cartographer once, then cools down 5 min |
| Cloud-sync folder write contention | WAL-mode SQLite + warn-once + pause autosave-to-db when racing |

### 6.9 Determinism for tests

A test harness drives the orchestrator with synthetic typing + analysis events and a fake LLM provider, asserting exact pill timing, voice routing, and prompt assembly. The eval harness (M1.5) reuses the fake-provider infrastructure for scoring real models.

---

## 7. Risks, open questions, and resolutions

### 7.1 Technical risks (mitigations live in the spec; this is the inventory)

| # | Risk | Likelihood | Severity | Mitigation |
|---|---|---|---|---|
| R1 | Pill quality varies wildly across models; defaults chosen too early go stale | High | High | M1.5 runs continuously; defaults re-selected each milestone; scorecards versioned in repo |
| R2 | Tone enforcement leaks despite prompt clauses | Medium | High | `PASS` + retry; instructive-phrase blacklist used as both prompt clause AND post-hoc filter; nightly tone-audit job |
| R3 | `character_dissonance` lemma-overlap heuristic misfires | Medium | Medium | Flagged in `KNOWN_FRAGILE.md`; per-character `dissonance_sensitivity` slider; eval task family exercises this trigger |
| R4 | Snippet anchor drifts during pill TTL → pill silently dismissed | Medium | Low | Accepted; mitigation is diagnostics log of dismissal cause so we can audit frequency |
| R5 | Bundled Python sidecar inflates installer (~150–250 MB), slow first-boot model load | High | Medium | `uv` standalone Python; lazy-load models; "warming up" indicator; ONNX-export rewrite deferred to v1.1 once measured |
| R6 | Cross-platform sidecar packaging (notarization mac, code-signing Windows) breaks for testers | Medium | Medium | Document tester install workaround (Gatekeeper, SmartScreen); pursue certificates in M7 if budget allows |
| R7 | User edits `.md` files outside Water → broken block IDs / frontmatter | Medium | Medium | External-edit repair pass on open (Section 3.10) |
| R8 | Long manuscripts (200 k+ words, 1000+ scenes) blow up audiovisualizer perf | Medium | Medium | Virtualize the curve (sample to viewport pixel density); cache scene metric rollups; per-paragraph block_metrics kept only for current scene |
| R9 | Cloud-synced folder → SQLite write contention | High | Medium | WAL mode; warn-once on known cloud-sync paths; pause autosave-to-db on race; keep autosave-to-md |
| R10 | Continuous-zoom UX disorients first-time users | Medium | Medium | Traditional menu nav always available; first-launch tour shows zoom rail once; zoom snaps on release |
| R11 | Glow-only elevation reads "fuzzy" rather than "elevated" on bright light-mode displays | Medium | Low | 1px tinted border mitigation in light mode; tracked in user testing |
| R12 | Unlimited rabbit-hole depth costs LLM tokens unboundedly | Low | Low | Bounded by user clicks; per-session token-budget soft warning; cost meter in diagnostics |
| R13 | Bouquet history compression loses important context after 20 levels | Low | Low | Accepted v1; v1.1 may move to vector-summary memory |
| R14 | Eval harness gold tasks bias toward what *we* think is good | High | Medium | Open eval to internal testers; let testers contribute reference pills; rotate authors of gold tasks |
| R15 | Beat-label LLM mislabels scenes; pulse markers in wrong places | Medium | Low | Confidence threshold 0.7; below threshold no annotation; manual override in v2 |
| R16 | Provider router falls back silently to a worse model and writer never notices quality drop | Medium | Medium | Status chip shows active provider; per-pill diagnostics record provider; fallback frequency in diagnostics |
| R17 | Manuscript text sent to cloud providers without writer realizing | High | Critical | Per-project provider selection; first-time cloud-send one-time consent dialog; cloud-active chip in status bar; minimal excerpts when full manuscript not needed |
| R18 | Linux unsupported in v1 | High | Low | Defer to v1.1; Tauri build target is essentially free, sidecar testing surface is not |

### 7.2 Product risks

| # | Risk | Mitigation |
|---|---|---|
| P1 | Personas distract from the universe-conversation principle | Renameable; Chorus only in eliciting mode; persona share decays as character sheets fill in; eval task family enforces "did this pill privilege the universe?" |
| P2 | Writers find pills annoying despite gentle defaults | Sensitivity slider front-and-center; snooze toggle in status bar; full disable available |
| P3 | "No generation in the page" perceived as "AI doesn't help me when stuck" | Eliciting mode (Chorus) is explicitly for this; pinning a Chorus pill creates a world_entry stub in one click |
| P4 | Continuous zoom is too cinematic and slows experienced writers | Zoom rail hideable; `⌘ 1…5` direct jumps; menu nav always available |
| P5 | Heatmap becomes a vanity metric writers stare at instead of writing | Default opens to the editor (zoom 4); macro view is visited, not lived in |
| P6 | Character voice quality is the moat and hardest to nail | M1.5 prioritizes character-voice tasks; M3 ships sheets but M2 ships persona pills first so we iterate while we wait |

### 7.3 Open-question resolutions (recorded for posterity)

| # | Question | Resolution |
|---|---|---|
| O1 | Default local model packaging | Ship with a tiny quantized local model bundled (~1–2 GB); eval-harness shootout pre-selects between Qwen 2.5 (1.5B/3B quantized) and Kimi or any candidate with a notably larger context window. Decision weight: `context_window × pill_quality × cold_start_latency × installer_size_impact` |
| O2 | Telemetry posture | Opt-out telemetry on by default in internal build; settings toggle present |
| O3 | Linux support | Deferred to v1.1 |
| O4 | Apple Silicon MLX adapter | Worth an eval-harness slot in v1; wired in M1 behind a feature flag |
| O5 | Per-project privacy lock | Not the default. Default is "best available provider"; per-project local-only is a setting. Internal-test build supports a developer-supplied API-key config file (`~/.water/dev-keys.toml` or env vars) |
| O6 | Plugin / extension API | v1 designs the contracts; v2 implements the loader |
| O7 | In-app pill thumbs feedback | Skipped; external feedback channel used instead |

---

## 8. Roadmap summary

### 8.1 Milestone-by-week (target ~14 weeks calendar; ~12 weeks with parallelization)

| Wk | Primary | Parallel | Key deliverable | Exit gate |
|---|---|---|---|---|
| 1 | M1 Foundation | M1.5 setup | Tauri shell boots; project folder open/save round-trip | Type-and-persist works |
| 2 | M1 Foundation | M1.5 gold tasks v1 | Sidecar, provider router, snapshot system | Rebuild-from-truth works |
| 3 | M2 Editor (bake-off) | M1.5 first scorecards | ProseMirror vs Lexical decision; block editor scaffold | Editor pick committed |
| 4 | M2 Pill engine | M1.5 model sweep | Orchestrator state machine, voice router, persona prompts | Persona pill end-to-end |
| 5 | M2 Bouquets | M1.5 character-voice tasks | Expansion + regenerate + pin + dismiss; unlimited-depth UX | Rabbit hole works |
| 6 | M3 Sheets | M2 polish | Conversational Intake; LSM v2.1 schemas; character index | Full LSM intake in ≤ 3 min |
| 7 | M3 + char voice | M1.5 char-voice scorecard | Character speaker track wired; voice routing live | First character pill |
| 8 | M4 World Bible | — | Segmented world + Cartographer trigger + world_drift check | World entries influence pills |
| 9 | M5 Heatmap A | M5 scene summaries | Per-scene metric rollups; basic intensity curve | Curve updates within 5 s |
| 10 | M5 Heatmap B | M5 beat labels | Tiered zoom levels 0–1; beat-label markers | Cross-zoom continuous |
| 11 | M6 Macro A | — | Scene Mosaic (timeline + board); drag-and-drop reorder | Scene reorder consistent |
| 12 | M6 Macro B | — | Tree + freeform; zoom rail + menu nav; zoom levels 2–3 fluid | Continuum end-to-end |
| 13 | M7 Polish | Plugin contracts | Settings UI; onboarding; theming; accessibility pass | All defaults pass tester checklist |
| 14 | M7 Build | Internal docs | Installers, dev-key support, `KNOWN_FRAGILE.md`, plugin-contracts doc | Tester build handed off |

### 8.2 Cross-milestone disciplines

- Each milestone has its own internal spec → plan → impl cycle even though they ship together.
- Tone-audit job runs nightly during M2+ over sampled pills, with an instructive-register blacklist.
- Eval harness re-runs the full sweep at each milestone gate and updates default model choices.
- `KNOWN_FRAGILE.md` lives at repo root; `character_dissonance` heuristic is its first entry.
- Replay logs (opt-in, dev-only) for LLM-call replay debugging.
- Plugin contracts get `docs/plugin-contracts.md` in M7.

---

## 9. Glossary

| Term | Meaning |
|---|---|
| **Audiovisualizer arc** | The tiered-zoom story-shape view; M5's deliverable |
| **Block** | A paragraph or paragraph-equivalent unit in the editor; carries a `^bk-XXXX` ID |
| **Bouquet** | The set of 3 sub-pill variants returned when a pill is expanded or regenerated |
| **Character voice** | A pill spoken in the voice of an in-scene character driven by their LSM sheet |
| **Chorus** | The eliciting-mode persona that surfaces only when the project has no characters and no world entries |
| **Conductor** | (Orbit term — *not* used in Water) Orbit's agentic layer; Water replaces with its own pill orchestrator |
| **Conversational Intake** | The deterministic one-question-at-a-time popup that walks a schema |
| **Drift tensor** | The trained model (`flow_net.pth` from Orbit) used as the style/flow drift detector |
| **Eliciting mode** | The pill engine's behavior when the universe is empty, designed to draw out world/characters |
| **Glow palette** | Semantic pastel hues used for meaningful surfaces; one of two color layers |
| **LiveVectorizer** | The MiniLM + DistilRoBERTa fusion encoder producing the 1152-d vector for the drift tensor |
| **LSM v2.1** | LocalScriptMan Character Map version 2.1 |
| **Pill** | A pastel-glow margin element where a speaker reacts to the manuscript |
| **Persona** | A named ambient speaker (Echo, Architect, Editor, Cartographer, Chorus) |
| **POV character** | The character whose perspective owns a scene |
| **Rabbit hole** | The unlimited-depth tree of sub-pill bouquets reachable by clicking pills |
| **Spatial Continuum** | The single zoomable spatial surface that hosts levels 0–4 (and stretch 5) |
| **Substrate** | Neutral Apple-minimal color layer underneath the glow palette |
| **Voice router** | The Rust-core function that picks the speaker given a trigger and scene state |

---

## 10. Appendix: schemas, examples, and prompts

### 10.1 `water.toml` (project root)

```toml
schema_version = 1
project_id = "01H8AA"
name = "Untitled Project"
default_manuscript_id = "01H8AB"
created_at = "2026-05-16T09:00:00Z"
updated_at = "2026-05-16T09:00:00Z"
```

### 10.2 `manuscript/chapters.toml`

```toml
schema_version = 1

[[chapter]]
id = "01H7Z1"
name = "Part One"
ordering = 0
scene_ids = ["01H8X1", "01H8X2", "01H8X3", "01H8X4"]

[[chapter]]
id = "01H7Z2"
name = "Part Two"
ordering = 1
scene_ids = ["01H8X5", "01H8X6"]
```

### 10.3 `characters/<ulid>.toml` (excerpt)

```toml
id = "01H7AA"
name = "Maren"
schema_version = "lsm-v2.1"

[main]
full_name = "Maren Halloway"
role_in_story = "protagonist"
want = "to bring her father's body home before the storm season"
need = "to forgive him for leaving"
ghost_wound = "found his journal at fifteen and learned he'd planned to leave"
lie_they_believe = "love is something you earn by being useful"
truth = "love is unconditional or it is something else"
fatal_flaw = "refuses help"
strength = "knows the harbour like her own hands"

[bonus_traits]
voice = "spare, low-register, hates fillers"
tells = ["touches her left wrist when lying", "looks at the horizon when grieving"]
fears = ["being a burden", "fog at sea"]
values = ["honesty", "competence", "kept promises"]

[arc]
starting_state = "self-sufficient to a fault"
ending_state = "able to receive"
inciting_change = "the keeper offers help she cannot refuse"
midpoint_shift = "she lets someone hold the lantern"
climax_choice = "asks aloud for help on the cliff path"

[perspectives]
self_view = "useful, careful, alone by choice"
others_view = "carved out of the cliffs themselves"
```

### 10.4 `world/concept.toml` (single-doc segment)

```toml
schema_version = 1
core_premise = "A coastal town where the lighthouses remember the people they've watched."
genre = "literary slipstream"
tone = "spare, weather-worn, with quiet warmth"
themes = ["inheritance", "forgiveness", "the ethics of remembering"]
inspirations = ["Marilynne Robinson", "Kelly Link", "Tove Jansson"]
```

### 10.5 Sample tone clauses in `prompts/tone.toml`

```toml
version = "1.0.0"

[clauses]
present_tense = "Speak in present tense as if you are noticing this just now."
not_assistant = "You are not an assistant. You do not give writing advice."
blacklist = "Never say: 'you should', 'consider', 'try', 'maybe you could', 'I think you', 'as an AI', 'this is good/bad'."
observation = "Observe, react, wonder. Leave space."
shape = "Output exactly one line of prose, no longer than 22 words. No quotation marks. No emoji."
pass_token = "If you cannot react in your speaker's voice without breaking these rules, output the single token `PASS` and nothing else."

blacklist_phrases = [
  "you should", "you could", "you might", "consider", "try ",
  "i think you", "as an ai", "as a language model",
  "this is good", "this is bad", "this works", "this doesn't work"
]
```

### 10.6 Sample structured-output schema for `pill_expand`

```json
{
  "type": "array",
  "minItems": 3,
  "maxItems": 3,
  "items": {
    "type": "object",
    "required": ["angle", "text"],
    "properties": {
      "angle": { "type": "string", "minLength": 2, "maxLength": 16 },
      "text":  { "type": "string", "minLength": 4, "maxLength": 200 }
    }
  }
}
```

### 10.7 Sample tester onboarding checklist (for M7)

- Download installer, double-click, allow Gatekeeper / SmartScreen if unsigned.
- App opens; first-launch wizard asks for theme + provider choice.
- Local provider works out of the box with bundled model.
- Optionally drop `~/.water/dev-keys.toml` for cloud testing.
- Create a project named "Test 1"; type a few paragraphs into a new scene.
- Wait for first pill (target ≤ 90 s after the first sentence is finished).
- Expand the pill, regenerate the bouquet, pin one sub-pill at any depth.
- Close the app; reopen; pinned pill still present; scene content preserved.
- Open `~/.../Test 1.water/manuscript/scenes/*.md` in any text editor; content matches what was typed.
- Delete `~/.../Test 1.water/project.db`; reopen the app; project rebuilds.

---

*End of spec.*
