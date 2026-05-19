# M4 World/Setting Bible — Implementation Plan

**Status:** Open. Built on `m3` tag (`846f9a8`, 2026-05-19). Targets the `m4` tag when all tasks ship green.

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship the World/Setting Bible on top of M3: 6 default world segments (concept, locations, politics_and_social, culture, world, history) backed by reuse of M3's `IntakeField[]` descriptor; a 3-level `WorldsSurface` (index → segment → entry) with `ConversationalIntake` reused unchanged; minimal user-extensible template editor; `world_drift` trigger with cheap name+alias scan + contextual-overlap pre-check + LLM YES/NO/unclear confirmation firing a world-aware `CartographerSpeaker`; `CharacterSpeaker` extended to inject location sensory-detail into voice prompts when `scene.location_id` is set; and end-to-end Chorus-pin → `world_entry` stub flow closing the no-universe-yet story. Tag `m4` when done.

**Architecture:** ~34 tasks across 9 phases. **Phase A** (engine foundations, 7 tasks) lays down the v4 SQLite migration, built-in template constants, `WorldStore` CRUD for both single-doc and collection segments, `WorldRegistry` hot-path snapshot, and the `rebuild.rs` extension. **Phase B** (Tauri commands, 5 tasks) adds the `world*` IPC surface, `WorldWriteLocks`, scene-location plumbing, and a polish fmt-pass. **Phase C** (orchestrator wiring, 2 tasks) threads `WorldRegistry` through `OrchestratorContext` and registers `CartographerSpeaker` + the world-track voice-router branch. **Phase D** (`world_drift` trigger, 4 tasks) builds the collision resolver, the Stage 1 evaluator with contextual-overlap pre-check, the Stage 2 confirmation prompt, and the Cartographer voice template with tone-audit. **Phase E** (UI surface, 6 tasks) extracts `flattenSerdeFlatten` and builds `WorldsSurface` routing, index tiles, segment views for both shapes, entry sheet, and intake-reuse sheet. **Phase F** (character speaker extension, 2 tasks) extends `CharacterSpeaker::from_row` with `&WorldRegistry` and adds `{{world.location_*}}` tokens. **Phase G** (cross-feature wiring, 4 tasks) lights up the scene-metadata location selector, the discriminated chip suggestion payload, the Chorus-stub pin handler, and the load-bearing Bouquet pin-context fix. **Phase H** (template editor + settings, 2 tasks) ships the minimal new-segment modal and the visibility/delete controls. **Phase I** (audit, 2 tasks) builds the Pell Library eval fixture with 4 test scenes, walks the manual smoke checklist, tags `m4`, and writes the M5 handoff.

**Tech Stack:** Rust 1.85 + tokio + rusqlite + serde + reqwest (existing); React 18 + TypeScript strict (existing); Tauri 2 (existing). No new third-party deps.

**Spec:** `docs/superpowers/specs/2026-05-19-m4-world-bible-design.md`

**Parent design:** `docs/superpowers/specs/2026-05-16-water-design.md` (§ 3.6 World templates, § 4.6 M4 exit criteria).

**M3 plan (for pattern reference):** `docs/superpowers/plans/2026-05-18-m3-character-sheets.md`.

**Handoff:** `docs/superpowers/handoffs/2026-05-19-m4-handoff.md`.

---

## Per-session prerequisites

**Windows / PowerShell PATH setup at every fresh shell:**

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"
```

Put this at the top of every implementer prompt. (M3 lesson; `NativeCommandError` on cargo stderr is cosmetic and should be ignored.)

**Dev server port:** 5174 (5173 is reserved for the user's other project).

**Replay log (optional):** `$env:WATER_REPLAY_LOG = "1"` to emit per-dispatch JSONL.

---

## Pre-authorized adaptations (from M3 spec § 15)

The plan is pre-authorized to adapt during execution without surfacing a question for:

- **Rust API signatures may shift** to satisfy borrow checker / lifetimes / async fn requirements. The `_core` async-fn extraction (every Tauri command takes unpacked deps so tests can drive it) is **required** for every command in this plan — bake it in from the start.
- **TS prop shapes may shift** to match actual `IntakeField` ergonomics in the React tree.
- **TOML field ordering** on disk may differ from the example blocks (serde's default order).
- **CSS class names and component sub-structure** are unconstrained.
- **Test-helper extraction** (e.g. `mk_world_registry`, `mk_world_entry_file`) is encouraged whenever a test pattern repeats 3+ times.
- **`tracing::warn!` / `tracing::debug!` logs** at silent-failure points (per M3 polish #7, #13 lesson) are encouraged.
- **Const extraction** for magic numbers (`MIN_PARAGRAPH_WORDS`, `MIN_CONTEXT_OVERLAP_WORDS`, `WORLD_DRIFT_COOLDOWN_MS`) is **required**.

The plan is **NOT** pre-authorized to:

- Change the on-disk TOML directory layout (`world/<slug>.toml` vs. `world/<slug>/<entry-ulid>.toml`).
- Change the v4 migration column set without coordinating.
- Change the template descriptor format away from `IntakeField[]`.
- Remove the orphan-draft reaping behavior.
- Surface `unclear` Stage-2 verdicts as pills.

If any of these come up, **stop and surface as a question** in the task's review pass.

---

## Pre-authorized lint fixes (clippy)

Apply silently: `must_use_candidate`, `doc_markdown`, `single_match_else`, `implicit_hasher`, `inherent_to_string`, `cast_precision_loss`, `cast_possible_truncation`, `uninlined_format_args`, `duration_suboptimal_units`, `assigning_clones`, `needless_pass_by_value`, `match_same_arms` (with `#[allow]` when state-machine arms are semantically distinct), `field_reassign_with_default` (use struct-update syntax).

---

## Pre-authorized Vitest patterns (from M3)

- `vi.hoisted(() => ({ mock: vi.fn() }))` for top-level mock identities.
- `vi.useFakeTimers({ shouldAdvanceTime: true })` in `beforeEach` whenever combining fake timers with `waitFor`.
- Cancellation-race fix pattern on any new `useEffect` + async subscription (capture `cancelled` flag; check after await before storing unsub).
- `pnpm --filter @water/app test src/path/to/file.test.tsx` (no `--`).
- **`key={field.id}` on the active input** is REQUIRED for any component conditionally rendering same-kind elements across state changes (M3 T15 lesson; ConversationalIntake gains it transitively for free — do NOT regress it).
- **`useRef` write-path race guard** for any new `useEffect` that awaits a write whose target id can change mid-await (M3 T16 lesson).
- **Mock `vi.mock("../ipc/commands")` to replace the `ipc` singleton.** Factory must be hoist-safe (no outer references). See `CharacterIntakeSheet.test.tsx` for the canonical pattern.

**TS strict mode is on with `noUncheckedIndexedAccess: true`.** Array/regex/Map index lookups need `?.[i] ?? fallback` or explicit guards.

---

## Carry-forward lessons from M1 → M3

- **`Db: Send + !Sync`.** Wrap in `Arc<Mutex<Db>>`. Project lock dropped BEFORE db lock.
- **Lock ordering invariant (KNOWN_FRAGILE #6):** `project lock → drop → write-lock (per-id mutex DashMap) → db lock`. M4 introduces `world_write_locks: WorldWriteLocks` following the M3 `CharacterWriteLocks` pattern.
- **`rusqlite::Statement: !Send`.** Async handlers cannot hold one across `.await`. Drain queries into owned `Vec<T>` inside a sync block.
- **`unchecked_transaction()`** is the documented escape hatch when only `&Connection` is available.
- **`#[serde(flatten)]` Rust types deserialize with section keys at TOP LEVEL on the TS side.** `WorldEntryFile` shape is `{id, segment_id, schema_version, name, aliases, [section_key]: unknown}` — there is NO `data` wrapper, matching M3 `CharacterFile`.
- **`IntakeField.id` is a dotted path** (`"main.name"`, `"lists.notable_features"`), not a leaf name. `ConversationalIntake` looks up `values[field.id]` so the flatten helper must produce dotted-path keys.
- **`IntakeFieldKind` is a discriminated union** `{type: "short_text"} | {type: "long_text"} | {type: "string_list"} | {type: "choice", options: string[]}`. Match on `kind.type`. `optional_skip: boolean` (NOT `required: boolean` — semantics inverted).
- **All IPC lives on the `ipc{}` singleton** in `app/src/ipc/commands.ts`. No per-feature `../ipc/<x>` modules.
- **`Set-Content -Encoding UTF8` writes a BOM in PowerShell 5.1.** Use `[System.IO.File]::WriteAllText(...)` for clean UTF-8 fixtures.
- **CRLF/LF git warnings** are environmental on Windows. Ignore.

---

## Task index

| # | Phase | Title |
|---|---|---|
| 1 | A | v4 SQLite migration — `world_segment`, `world_entry`, `pinned_pill.origin_trigger` |
| 2 | A | `BUILT_IN_TEMPLATES` + `effective_template` resolver |
| 3 | A | `WorldStore` segment CRUD + `seed_builtins` |
| 4 | A | `WorldStore` single-doc TOML round-trip + read/update |
| 5 | A | `WorldStore` collection entry CRUD + TOML round-trip + delete_if_empty |
| 6 | A | `WorldRegistry::from_db` with case-insensitive name+alias index |
| 7 | A | `rebuild.rs` world scan + scene.location_id orphan repair |
| 8 | B | IPC types + `world.rs` segment commands |
| 9 | B | `world.rs` single-doc commands |
| 10 | B | `world.rs` entry commands + autosuggest |
| 11 | B | `scene.rs` `sceneSetLocation` + extended `sceneReadMetadata` |
| 12 | B | `state.rs` `WorldWriteLocks` + `cargo fmt` pass on commands/ |
| 13 | C | `OrchestratorContext` gains `world_registry` + service builds it |
| 14 | C | Voice router `WORLD_TRACK_TRIGGERS` + `CartographerSpeaker` registration |
| 15 | D | `collision::resolve_token_kind` shared helper |
| 16 | D | `WorldDriftEvaluator` Stage 1 (name+alias scan + contextual-overlap + cooldown) |
| 17 | D | `pill_world_drift_check.toml` + Stage 2 confirmation handler |
| 18 | D | Cartographer voice template + tone-audit pass |
| 19 | E | `flattenSerdeFlatten` generic TS refactor |
| 20 | E | `WorldsSurface` routing + `App.tsx` mount + `WorldIndex` + `WorldSegmentTile` |
| 21 | E | `WorldSegmentView` single-doc branch |
| 22 | E | `WorldSegmentView` collection branch + `WorldEntryCard` |
| 23 | E | `WorldEntrySheet` (inline edit + aliases editor) |
| 24 | E | `WorldEntryIntakeSheet` (intake reuse + orphan-draft reaping) |
| 25 | F | `CharacterSpeaker::from_row` gains `&WorldRegistry` + `&SceneContext` |
| 26 | F | `character_template.rs` `{{world.location_*}}` token integration |
| 27 | G | `SceneMetadataSheet` location selector + `sceneSetLocation` UI |
| 28 | G | `ChipSuggestion` discriminated payload + `SceneAutosuggestChips` extension |
| 29 | G | `pinned_pill.origin_trigger` plumbing + Chorus-stub pin handler |
| 30 | G | Bouquet pin-context fix (M2 carry-over) + stub-creation UI route |
| 31 | H | `SegmentTemplateEditor` modal (creation + edit modes) |
| 32 | H | World settings: hide/show toggle + delete user-added |
| 33 | I | `eval/m4_acceptance/pell_library.toml` + 4 test scenes |
| 34 | I | Manual smoke walk + tag `m4` + write M5 handoff |

---

## Phase A — Engine foundations

Lays down the SQL migration, built-in templates, `WorldStore` for both shapes, `WorldRegistry`, and the rebuild extension. All Rust, no Tauri or UI yet. Every task ends with a green `cargo test -p water-core` and a commit.

### Task 1: v4 SQLite migration — `world_segment`, `world_entry`, `pinned_pill.origin_trigger`

Adds the columns needed for templates-as-data, alias indexing, Chorus-stub detection, and timestamp parity with `character`. Forward-only, follows the M3 T2 migration pattern.

**Files:**
- Create: `crates/water-core/sql/v4_world_bible.sql`
- Modify: `crates/water-core/src/migrations.rs`
- Test: `crates/water-core/src/migrations.rs` (add `#[cfg(test)]` cases)

**Pre-authorized adaptations:**
- The exact `DEFAULT ''` placeholder values may be supplemented with sentinel backfill `UPDATE` statements within the same migration; per M3 polish #3, leave a TODO comment OR add a `CHECK` constraint on `hue_token`.

- [ ] **Step 1: Inspect the M3 migration pattern**

Read `crates/water-core/sql/v3_character_hue.sql` and `crates/water-core/src/migrations.rs:23-35`. The pattern is:
1. Append a `const Vn_*: &str = include_str!("../sql/vn_*.sql");` in `migrations.rs`.
2. Append `M::up(Vn_*)` to the `all()` vector.
3. End the SQL script with `UPDATE schema_version SET version = N;`.
4. Never reorder or edit existing migrations.

- [ ] **Step 2: Write the failing migration test**

Append to `crates/water-core/src/migrations.rs` `tests` module:

```rust
#[test]
fn v4_adds_world_segment_template_columns() {
    let db = Db::open_in_memory().unwrap();
    let cols: Vec<String> = db
        .conn()
        .prepare("PRAGMA table_info(world_segment)")
        .unwrap()
        .query_map([], |row| row.get::<_, String>(1))
        .unwrap()
        .collect::<std::result::Result<_, _>>()
        .unwrap();
    for required in [
        "template_json",
        "hidden",
        "hue_token",
        "slug",
        "created_at",
        "updated_at",
    ] {
        assert!(
            cols.iter().any(|c| c == required),
            "world_segment missing column {required}; got {cols:?}"
        );
    }
}

#[test]
fn v4_adds_world_entry_alias_and_timestamp_columns() {
    let db = Db::open_in_memory().unwrap();
    let cols: Vec<String> = db
        .conn()
        .prepare("PRAGMA table_info(world_entry)")
        .unwrap()
        .query_map([], |row| row.get::<_, String>(1))
        .unwrap()
        .collect::<std::result::Result<_, _>>()
        .unwrap();
    for required in [
        "aliases_json",
        "schema_version",
        "created_at",
        "updated_at",
    ] {
        assert!(
            cols.iter().any(|c| c == required),
            "world_entry missing column {required}; got {cols:?}"
        );
    }
}

#[test]
fn v4_adds_pinned_pill_origin_trigger() {
    let db = Db::open_in_memory().unwrap();
    let has_col: bool = db
        .conn()
        .prepare("PRAGMA table_info(pinned_pill)")
        .unwrap()
        .query_map([], |row| row.get::<_, String>(1))
        .unwrap()
        .filter_map(std::result::Result::ok)
        .any(|c| c == "origin_trigger");
    assert!(has_col, "pinned_pill missing origin_trigger column");
}

#[test]
fn v4_schema_version_is_four() {
    let db = Db::open_in_memory().unwrap();
    let version: u32 = db
        .conn()
        .query_row("SELECT version FROM schema_version", [], |r| r.get(0))
        .unwrap();
    assert_eq!(version, 4);
}

#[test]
fn v4_creates_world_entry_by_segment_index() {
    let db = Db::open_in_memory().unwrap();
    let count: i64 = db
        .conn()
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='index' AND name='world_entry_by_segment'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(count, 1, "world_entry_by_segment index missing");
}
```

- [ ] **Step 3: Run the tests, confirm they fail**

```powershell
cargo test -p water-core migrations::tests::v4_ -- --nocapture
```

Expected: ALL FIVE tests FAIL — either the `Db::open_in_memory()` succeeds but the columns don't exist (assertion fails), or migration fails before reaching the asserts.

- [ ] **Step 4: Author the migration SQL**

Create `crates/water-core/sql/v4_world_bible.sql`:

```sql
-- M4 schema v4: World/Setting Bible support.
-- Forward-only. v3 -> v4.
--
-- Adds template-override and visibility/timestamp columns to world_segment
-- (so user-customized templates can be persisted in the DB while built-ins
-- live as Rust consts in crates/water-core/src/world/templates.rs); adds
-- aliases + schema_version + timestamps to world_entry (parity with
-- character + Stage 1 of the world_drift trigger needs the alias index);
-- and adds pinned_pill.origin_trigger so the M4 Chorus-pin -> world_entry
-- stub handler can detect the no_universe_yet origin.

ALTER TABLE world_segment ADD COLUMN template_json TEXT;
ALTER TABLE world_segment ADD COLUMN hidden INTEGER NOT NULL DEFAULT 0;
ALTER TABLE world_segment ADD COLUMN hue_token TEXT NOT NULL DEFAULT '';
ALTER TABLE world_segment ADD COLUMN slug TEXT NOT NULL DEFAULT '';
ALTER TABLE world_segment ADD COLUMN created_at TEXT NOT NULL DEFAULT '';
ALTER TABLE world_segment ADD COLUMN updated_at TEXT NOT NULL DEFAULT '';

ALTER TABLE world_entry ADD COLUMN aliases_json TEXT NOT NULL DEFAULT '[]';
ALTER TABLE world_entry ADD COLUMN schema_version TEXT NOT NULL DEFAULT '';
ALTER TABLE world_entry ADD COLUMN created_at TEXT NOT NULL DEFAULT '';
ALTER TABLE world_entry ADD COLUMN updated_at TEXT NOT NULL DEFAULT '';

ALTER TABLE pinned_pill ADD COLUMN origin_trigger TEXT;

CREATE INDEX IF NOT EXISTS world_entry_by_segment ON world_entry(segment_id);

-- Backfill: any pre-existing world_segment rows (unlikely since the world
-- surface lands in M4) get their slug seeded from the existing name in
-- lowercase. Built-in seeding in Rust (Task 3) will overwrite to the
-- canonical slug ("concept", "locations", ...) when seed_builtins runs.
UPDATE world_segment SET slug = LOWER(REPLACE(name, ' ', '_')) WHERE slug = '';

-- TODO(m5 polish): consider adding
--   CHECK (hue_token = '' OR hue_token LIKE '--water-hue-world-%')
-- once the round-robin assignment in Rust covers all entry points.

UPDATE schema_version SET version = 4;
```

- [ ] **Step 5: Register the migration in `migrations.rs`**

Modify `crates/water-core/src/migrations.rs`:

```rust
// Add near the existing V*_ consts:
const V4_WORLD_BIBLE: &str = include_str!("../sql/v4_world_bible.sql");

// Update all() to:
#[must_use]
pub fn all() -> Migrations<'static> {
    Migrations::new(vec![
        M::up(V1_INIT),
        M::up(V2_PILL_ENGINE),
        M::up(V3_CHARACTER_HUE),
        M::up(V4_WORLD_BIBLE),
    ])
}
```

- [ ] **Step 6: Run the tests, confirm they pass**

```powershell
cargo test -p water-core migrations::tests::v4_ -- --nocapture
```

Expected: ALL FIVE tests PASS.

- [ ] **Step 7: Run full water-core test suite**

```powershell
cargo test -p water-core
```

Expected: 201 (M3 baseline) + 5 new = 206 tests PASS. No failures, no clippy warnings.

- [ ] **Step 8: Commit**

```powershell
git add crates/water-core/sql/v4_world_bible.sql crates/water-core/src/migrations.rs
git commit -m "feat(world): v4 migration — template_json, hue_token, aliases, timestamps, pinned_pill.origin_trigger"
```

---

### Task 2: `BUILT_IN_TEMPLATES` + `effective_template` resolver

Defines the 6 built-in world segment templates as Rust consts (matching M3's `LSM_V2_1` pattern in `character/intake.rs`) and the lookup that returns either a user-override `template_json` or the built-in default. Pure Rust; no DB writes yet — Task 3 wires `seed_builtins` to actually insert the segment rows.

**Files:**
- Create: `crates/water-core/src/world/mod.rs` (convert existing `world.rs` to a directory)
- Create: `crates/water-core/src/world/templates.rs`
- Modify: `crates/water-core/src/world.rs` → move content into `world/mod.rs`
- Modify: `crates/water-core/src/lib.rs` (no change expected if `pub mod world;` resolves automatically)

**Pre-authorized adaptations:**
- `IntakeField` / `IntakeSchemaSection` are imported from `crate::character::intake` for M4. (No need to relocate them to a shared module — the type system already treats them as crate-public.)

- [ ] **Step 1: Convert `world.rs` to a directory**

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"
New-Item -ItemType Directory -Path "crates\water-core\src\world"
Move-Item "crates\water-core\src\world.rs" "crates\water-core\src\world\mod.rs"
cargo build -p water-core
```

Expected: clean build (`pub mod world;` in `lib.rs` resolves to `world/mod.rs` automatically).

- [ ] **Step 2: Write the failing test for `BUILT_IN_TEMPLATES`**

Create `crates/water-core/src/world/templates.rs`:

```rust
//! Built-in world segment templates.
//!
//! Each `BuiltInTemplate` is a fixed schema shipped with the application.
//! User-customized segments persist their template in
//! `world_segment.template_json`; lookup precedence is user-override -> built-in.
//!
//! Field-id convention (matches M3 LSM_V2_1):
//! - `main.<key>` for scalars and long-text.
//! - `lists.<key>` for `string_list` kinds.
//! On disk the corresponding TOML uses `[main]` and `[lists]` sections via
//! `#[serde(flatten)]` on the section enum.

use crate::character::intake::{IntakeField, IntakeFieldKind, IntakeSchemaSection};

pub struct BuiltInTemplate {
    pub slug: &'static str,
    pub display_name: &'static str,
    pub is_collection: bool,
    pub schema: IntakeSchemaSection,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_templates_has_six_segments() {
        let all = built_in_templates();
        assert_eq!(all.len(), 6, "expected 6 built-in segments; got {}", all.len());
    }

    #[test]
    fn built_in_template_slugs_are_canonical() {
        let slugs: Vec<&str> = built_in_templates().iter().map(|t| t.slug).collect();
        assert_eq!(
            slugs,
            vec![
                "concept",
                "locations",
                "politics_and_social",
                "culture",
                "world",
                "history",
            ]
        );
    }

    #[test]
    fn only_locations_is_collection() {
        for t in built_in_templates() {
            if t.slug == "locations" {
                assert!(t.is_collection, "locations should be is_collection=true");
            } else {
                assert!(
                    !t.is_collection,
                    "{} should be is_collection=false",
                    t.slug
                );
            }
        }
    }

    #[test]
    fn concept_template_has_expected_fields() {
        let t = built_in_templates().iter().find(|t| t.slug == "concept").unwrap();
        let ids: Vec<&str> = t.schema.fields.iter().map(|f| f.id.as_str()).collect();
        assert_eq!(
            ids,
            vec![
                "main.core_premise",
                "main.genre",
                "main.tone",
                "lists.themes",
                "lists.inspirations",
            ]
        );
    }

    #[test]
    fn locations_template_uses_main_name_for_canonical_name() {
        let t = built_in_templates()
            .iter()
            .find(|t| t.slug == "locations")
            .unwrap();
        assert!(
            t.schema.fields.iter().any(|f| f.id == "main.name"),
            "locations template must have a main.name field for rename-cascade"
        );
    }

    #[test]
    fn history_template_has_three_list_fields() {
        let t = built_in_templates().iter().find(|t| t.slug == "history").unwrap();
        for f in &t.schema.fields {
            assert!(
                matches!(f.kind, IntakeFieldKind::StringList),
                "history field {} should be string_list",
                f.id
            );
        }
        assert_eq!(t.schema.fields.len(), 3);
    }
}
```

- [ ] **Step 3: Wire `pub mod templates;` into `world/mod.rs`**

At the top of `crates/water-core/src/world/mod.rs` (currently the moved contents of `world.rs`), add:

```rust
pub mod templates;
```

- [ ] **Step 4: Run the tests, confirm they fail**

```powershell
cargo test -p water-core world::templates::tests -- --nocapture
```

Expected: FAIL — `built_in_templates` function not defined.

- [ ] **Step 5: Implement `built_in_templates()`**

Append to `crates/water-core/src/world/templates.rs`:

```rust
fn field(id: &str, label: &str, prompt: &str, kind: IntakeFieldKind, optional: bool) -> IntakeField {
    IntakeField {
        id: id.to_string(),
        label: label.to_string(),
        prompt_question: prompt.to_string(),
        kind,
        optional_skip: optional,
    }
}

fn section(id: &str, label: &str, fields: Vec<IntakeField>) -> IntakeSchemaSection {
    IntakeSchemaSection {
        id: id.to_string(),
        label: label.to_string(),
        fields,
    }
}

#[must_use]
pub fn built_in_templates() -> Vec<BuiltInTemplate> {
    vec![
        BuiltInTemplate {
            slug: "concept",
            display_name: "Concept",
            is_collection: false,
            schema: section(
                "concept",
                "Concept",
                vec![
                    field("main.core_premise", "Core Premise", "What's the core premise of this story?", IntakeFieldKind::LongText, false),
                    field("main.genre", "Genre", "What genre does this sit in?", IntakeFieldKind::ShortText, false),
                    field("main.tone", "Tone", "What's the dominant tone?", IntakeFieldKind::ShortText, false),
                    field("lists.themes", "Themes", "What themes do you want to explore?", IntakeFieldKind::StringList, true),
                    field("lists.inspirations", "Inspirations", "What works inspired this?", IntakeFieldKind::StringList, true),
                ],
            ),
        },
        BuiltInTemplate {
            slug: "locations",
            display_name: "Locations",
            is_collection: true,
            schema: section(
                "locations",
                "Location",
                vec![
                    field("main.name", "Name", "What's this place called?", IntakeFieldKind::ShortText, false),
                    field("main.type", "Type", "What kind of place is it (city, library, ruin, etc.)?", IntakeFieldKind::ShortText, false),
                    field("main.sensory_detail", "Sensory Detail", "What does it look, smell, sound like?", IntakeFieldKind::LongText, false),
                    field("lists.notable_features", "Notable Features", "What features are worth remembering?", IntakeFieldKind::StringList, true),
                    field("main.significance", "Significance", "Why does this place matter to the story?", IntakeFieldKind::LongText, true),
                ],
            ),
        },
        BuiltInTemplate {
            slug: "politics_and_social",
            display_name: "Politics & Social",
            is_collection: false,
            schema: section(
                "politics_and_social",
                "Politics & Social",
                vec![
                    field("main.governance", "Governance", "Who rules and how?", IntakeFieldKind::LongText, false),
                    field("lists.factions", "Factions", "What major factions exist?", IntakeFieldKind::StringList, true),
                    field("main.conflicts", "Conflicts", "What conflicts shape the political landscape?", IntakeFieldKind::LongText, false),
                    field("main.hierarchies", "Hierarchies", "What social hierarchies are in play?", IntakeFieldKind::LongText, true),
                    field("lists.taboos", "Taboos", "What's forbidden, shameful, or dangerous?", IntakeFieldKind::StringList, true),
                ],
            ),
        },
        BuiltInTemplate {
            slug: "culture",
            display_name: "Culture",
            is_collection: false,
            schema: section(
                "culture",
                "Culture",
                vec![
                    field("main.languages", "Languages", "What languages are spoken?", IntakeFieldKind::LongText, true),
                    field("main.religions", "Religions", "What religions or belief systems exist?", IntakeFieldKind::LongText, true),
                    field("main.art_and_ritual", "Art & Ritual", "What art forms and rituals matter?", IntakeFieldKind::LongText, true),
                    field("main.daily_life", "Daily Life", "What does ordinary daily life look like?", IntakeFieldKind::LongText, false),
                ],
            ),
        },
        BuiltInTemplate {
            slug: "world",
            display_name: "World",
            is_collection: false,
            schema: section(
                "world",
                "World",
                vec![
                    field("main.era", "Era", "What time period or era is this?", IntakeFieldKind::ShortText, false),
                    field("main.technology_level", "Technology Level", "Where does technology sit?", IntakeFieldKind::ShortText, false),
                    field("main.magic_or_speculative_rules", "Magic / Speculative Rules", "What rules govern magic or speculative elements (if any)?", IntakeFieldKind::LongText, true),
                    field("main.geography", "Geography", "What does the geography look like?", IntakeFieldKind::LongText, true),
                ],
            ),
        },
        BuiltInTemplate {
            slug: "history",
            display_name: "History",
            is_collection: false,
            schema: section(
                "history",
                "History",
                vec![
                    field("lists.timeline_beats", "Timeline Beats", "What major events anchor the timeline?", IntakeFieldKind::StringList, true),
                    field("lists.legends", "Legends", "What stories does this world tell itself?", IntakeFieldKind::StringList, true),
                    field("lists.unresolved_threads", "Unresolved Threads", "What mysteries or unresolved threads linger?", IntakeFieldKind::StringList, true),
                ],
            ),
        },
    ]
}
```

- [ ] **Step 6: Run the tests, confirm they pass**

```powershell
cargo test -p water-core world::templates::tests -- --nocapture
```

Expected: ALL SIX tests PASS.

- [ ] **Step 7: Write the failing test for `effective_template`**

Append to `crates/water-core/src/world/templates.rs` `tests` module:

```rust
#[test]
fn effective_template_returns_built_in_when_override_is_null() {
    use crate::{Db, ProjectStore};
    use crate::world::WorldStore;
    let db = Db::open_in_memory().unwrap();
    let p = ProjectStore::new(&db).insert("P").unwrap();
    let store = WorldStore::new(&db, std::path::PathBuf::from("/tmp"));
    let id = store
        .upsert_segment(&p.id, "concept", "Concept", 0, false)
        .unwrap();
    let schema = effective_template(&db, &id).unwrap();
    assert_eq!(schema.id, "concept");
    assert!(schema.fields.iter().any(|f| f.id == "main.core_premise"));
}

#[test]
fn effective_template_returns_override_when_present() {
    use crate::{Db, ProjectStore};
    use crate::world::WorldStore;
    let db = Db::open_in_memory().unwrap();
    let p = ProjectStore::new(&db).insert("P").unwrap();
    let store = WorldStore::new(&db, std::path::PathBuf::from("/tmp"));
    let id = store
        .upsert_segment(&p.id, "concept", "Concept", 0, false)
        .unwrap();
    // Write an override.
    let custom = IntakeSchemaSection {
        id: "concept".to_string(),
        label: "Custom Concept".to_string(),
        fields: vec![field(
            "main.tagline",
            "Tagline",
            "One-sentence tagline?",
            IntakeFieldKind::ShortText,
            false,
        )],
    };
    let json = serde_json::to_string(&custom).unwrap();
    db.conn()
        .execute(
            "UPDATE world_segment SET template_json = ?1 WHERE id = ?2",
            (&json, id.as_str()),
        )
        .unwrap();
    let schema = effective_template(&db, &id).unwrap();
    assert_eq!(schema.label, "Custom Concept");
    assert!(schema.fields.iter().any(|f| f.id == "main.tagline"));
}

#[test]
fn effective_template_errors_when_slug_unknown_and_no_override() {
    use crate::{Db, ProjectStore};
    use crate::world::WorldStore;
    let db = Db::open_in_memory().unwrap();
    let p = ProjectStore::new(&db).insert("P").unwrap();
    let store = WorldStore::new(&db, std::path::PathBuf::from("/tmp"));
    // Insert a segment with an unknown slug and no template override.
    let id = store
        .upsert_segment(&p.id, "fictional_kingdoms", "Fictional Kingdoms", 9, true)
        .unwrap();
    // Force-clear the slug-derived backfill to simulate user-added-without-template error state.
    db.conn()
        .execute(
            "UPDATE world_segment SET slug = 'fictional_kingdoms', template_json = NULL WHERE id = ?1",
            [id.as_str()],
        )
        .unwrap();
    let err = effective_template(&db, &id).unwrap_err();
    assert!(
        err.to_string().contains("no template"),
        "expected 'no template' in error; got {err}"
    );
}
```

- [ ] **Step 8: Run, confirm fail**

```powershell
cargo test -p water-core world::templates::tests::effective_ -- --nocapture
```

Expected: FAIL — `effective_template` not defined.

- [ ] **Step 9: Implement `effective_template`**

Append to `crates/water-core/src/world/templates.rs`:

```rust
use crate::{Db, Error, Id, Result};

/// Resolves the active template for a segment: user override if
/// `template_json` is non-null, else the built-in default looked up by slug.
///
/// Errors if the segment has no override AND no matching built-in.
pub fn effective_template(db: &Db, segment_id: &Id) -> Result<IntakeSchemaSection> {
    let (slug, template_json): (String, Option<String>) = db.conn().query_row(
        "SELECT slug, template_json FROM world_segment WHERE id = ?1",
        [segment_id.as_str()],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;

    if let Some(json) = template_json {
        let parsed: IntakeSchemaSection = serde_json::from_str(&json)
            .map_err(|e| Error::Other(format!("template_json parse: {e}")))?;
        return Ok(parsed);
    }

    for t in built_in_templates() {
        if t.slug == slug {
            return Ok(t.schema);
        }
    }

    Err(Error::Other(format!(
        "no template: segment {} has slug '{slug}' which is not a built-in and has no template_json override",
        segment_id.as_str()
    )))
}
```

- [ ] **Step 10: Confirm `IntakeSchemaSection` is `serde::Serialize + Deserialize`**

Check `crates/water-core/src/character/intake.rs` for the existing derives. If `IntakeSchemaSection` lacks `Serialize`, add it:

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct IntakeSchemaSection { /* ... */ }
```

If the derive already exists, skip. Verify with:

```powershell
cargo build -p water-core
```

- [ ] **Step 11: Run all templates tests, confirm pass**

```powershell
cargo test -p water-core world::templates -- --nocapture
```

Expected: ALL NINE tests PASS (6 from Step 2 + 3 from Step 7).

- [ ] **Step 12: Run full water-core suite**

```powershell
cargo test -p water-core
```

Expected: 206 (Task 1) + 9 new = 215 tests PASS.

- [ ] **Step 13: Commit**

```powershell
git add crates/water-core/src/world/ crates/water-core/src/character/intake.rs
git commit -m "feat(world): BUILT_IN_TEMPLATES (6 segments) + effective_template resolver"
```

---

### Task 3: `WorldStore` segment CRUD + `seed_builtins`

Extends the existing `WorldStore` (currently just `upsert_segment` + `list_segments`) with the full segment-management surface plus `seed_builtins`, the idempotent "ensure 6 default segments exist for this project" call that runs on project open.

**Files:**
- Create: `crates/water-core/src/world/store.rs`
- Modify: `crates/water-core/src/world/mod.rs` — split storage logic out of `mod.rs` into `store.rs`; `mod.rs` re-exports

**Pre-authorized adaptations:**
- The existing `WorldStore::upsert_segment` may be kept as-is for now and the new methods added alongside; refactoring it into `create_user_segment` + `update_segment` later is acceptable if the existing tests still pass.
- Hue token assignment uses round-robin against `--water-hue-world-1..6`. Empty-string `hue_token` is allowed at creation; `seed_builtins` assigns hues 1..6 in slug order.

- [ ] **Step 1: Confirm existing WorldStore API**

Read `crates/water-core/src/world/mod.rs` (the moved-from-`world.rs` content) to confirm `WorldStore::new`, `upsert_segment`, `list_segments`, `project_root` are all intact. The new methods will sit alongside.

- [ ] **Step 2: Write the failing test for `seed_builtins`**

Append to `crates/water-core/src/world/mod.rs` `tests` module (or create `crates/water-core/src/world/store.rs` with its own `mod tests`):

```rust
#[test]
fn seed_builtins_inserts_six_segments_idempotently() {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open_in_memory().unwrap();
    let p = ProjectStore::new(&db).insert("P").unwrap();
    let store = WorldStore::new(&db, dir.path().to_path_buf());

    store.seed_builtins(&p.id).unwrap();
    let segs = store.list_segments(&p.id).unwrap();
    assert_eq!(segs.len(), 6, "expected 6 built-in segments; got {}", segs.len());

    // Second call must be idempotent.
    store.seed_builtins(&p.id).unwrap();
    let segs2 = store.list_segments(&p.id).unwrap();
    assert_eq!(segs2.len(), 6);
}

#[test]
fn seed_builtins_assigns_unique_hue_tokens_round_robin() {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open_in_memory().unwrap();
    let p = ProjectStore::new(&db).insert("P").unwrap();
    let store = WorldStore::new(&db, dir.path().to_path_buf());
    store.seed_builtins(&p.id).unwrap();

    let hues: Vec<String> = db
        .conn()
        .prepare("SELECT hue_token FROM world_segment WHERE project_id = ?1 ORDER BY ordering")
        .unwrap()
        .query_map([p.id.as_str()], |row| row.get::<_, String>(0))
        .unwrap()
        .collect::<std::result::Result<_, _>>()
        .unwrap();

    assert_eq!(
        hues,
        vec![
            "--water-hue-world-1",
            "--water-hue-world-2",
            "--water-hue-world-3",
            "--water-hue-world-4",
            "--water-hue-world-5",
            "--water-hue-world-6",
        ]
    );
}

#[test]
fn seed_builtins_sets_correct_slugs_and_is_collection_flags() {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open_in_memory().unwrap();
    let p = ProjectStore::new(&db).insert("P").unwrap();
    let store = WorldStore::new(&db, dir.path().to_path_buf());
    store.seed_builtins(&p.id).unwrap();

    let mut stmt = db
        .conn()
        .prepare("SELECT slug, is_collection FROM world_segment WHERE project_id = ?1 ORDER BY ordering")
        .unwrap();
    let rows: Vec<(String, bool)> = stmt
        .query_map([p.id.as_str()], |row| {
            let s: String = row.get(0)?;
            let c: i64 = row.get(1)?;
            Ok((s, c != 0))
        })
        .unwrap()
        .collect::<std::result::Result<_, _>>()
        .unwrap();

    assert_eq!(
        rows,
        vec![
            ("concept".to_string(), false),
            ("locations".to_string(), true),
            ("politics_and_social".to_string(), false),
            ("culture".to_string(), false),
            ("world".to_string(), false),
            ("history".to_string(), false),
        ]
    );
}

#[test]
fn find_segment_by_slug_returns_some_for_builtin() {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open_in_memory().unwrap();
    let p = ProjectStore::new(&db).insert("P").unwrap();
    let store = WorldStore::new(&db, dir.path().to_path_buf());
    store.seed_builtins(&p.id).unwrap();

    let found = store.find_segment_by_slug(&p.id, "locations").unwrap();
    assert!(found.is_some());
    let s = found.unwrap();
    assert!(s.is_collection);
}

#[test]
fn find_segment_by_slug_returns_none_for_unknown() {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open_in_memory().unwrap();
    let p = ProjectStore::new(&db).insert("P").unwrap();
    let store = WorldStore::new(&db, dir.path().to_path_buf());
    store.seed_builtins(&p.id).unwrap();

    assert!(store.find_segment_by_slug(&p.id, "nonexistent").unwrap().is_none());
}

#[test]
fn create_user_segment_persists_template_json() {
    use crate::character::intake::{IntakeField, IntakeFieldKind, IntakeSchemaSection};
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open_in_memory().unwrap();
    let p = ProjectStore::new(&db).insert("P").unwrap();
    let store = WorldStore::new(&db, dir.path().to_path_buf());

    let custom = IntakeSchemaSection {
        id: "magic_systems".to_string(),
        label: "Magic Systems".to_string(),
        fields: vec![IntakeField {
            id: "main.name".to_string(),
            label: "System Name".to_string(),
            prompt_question: "What's this system called?".to_string(),
            kind: IntakeFieldKind::ShortText,
            optional_skip: false,
        }],
    };
    let id = store
        .create_user_segment(&p.id, "Magic Systems", true, &custom)
        .unwrap();

    let json: String = db
        .conn()
        .query_row(
            "SELECT template_json FROM world_segment WHERE id = ?1",
            [id.as_str()],
            |r| r.get(0),
        )
        .unwrap();
    let parsed: IntakeSchemaSection = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.label, "Magic Systems");
}

#[test]
fn set_segment_hidden_toggles_flag() {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open_in_memory().unwrap();
    let p = ProjectStore::new(&db).insert("P").unwrap();
    let store = WorldStore::new(&db, dir.path().to_path_buf());
    store.seed_builtins(&p.id).unwrap();
    let s = store.find_segment_by_slug(&p.id, "history").unwrap().unwrap();
    store.set_segment_hidden(&s.id, true).unwrap();
    let again = store.find_segment_by_slug(&p.id, "history").unwrap().unwrap();
    assert!(again.hidden);
    store.set_segment_hidden(&s.id, false).unwrap();
    let third = store.find_segment_by_slug(&p.id, "history").unwrap().unwrap();
    assert!(!third.hidden);
}

#[test]
fn delete_user_segment_refuses_builtin() {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open_in_memory().unwrap();
    let p = ProjectStore::new(&db).insert("P").unwrap();
    let store = WorldStore::new(&db, dir.path().to_path_buf());
    store.seed_builtins(&p.id).unwrap();
    let s = store.find_segment_by_slug(&p.id, "concept").unwrap().unwrap();
    let err = store.delete_user_segment(&s.id).unwrap_err();
    assert!(err.to_string().contains("built-in"));
}

#[test]
fn delete_user_segment_removes_user_added() {
    use crate::character::intake::{IntakeField, IntakeFieldKind, IntakeSchemaSection};
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open_in_memory().unwrap();
    let p = ProjectStore::new(&db).insert("P").unwrap();
    let store = WorldStore::new(&db, dir.path().to_path_buf());
    let custom = IntakeSchemaSection {
        id: "test".to_string(),
        label: "Test".to_string(),
        fields: vec![IntakeField {
            id: "main.thing".to_string(),
            label: "Thing".to_string(),
            prompt_question: "?".to_string(),
            kind: IntakeFieldKind::ShortText,
            optional_skip: false,
        }],
    };
    let id = store.create_user_segment(&p.id, "Test", false, &custom).unwrap();
    store.delete_user_segment(&id).unwrap();
    let segs = store.list_segments(&p.id).unwrap();
    assert!(segs.iter().all(|s| s.id != id));
}
```

Note: `WorldSegmentRow` will need new fields (`hidden`, `slug`, `hue_token`, timestamps). Update the struct in Step 3.

- [ ] **Step 3: Update `WorldSegmentRow` shape**

Modify `crates/water-core/src/world/mod.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldSegmentRow {
    pub id: Id,
    pub name: String,
    pub ordering: i64,
    pub is_collection: bool,
    pub slug: String,
    pub hue_token: String,
    pub hidden: bool,
    pub has_template_override: bool,   // computed: template_json IS NOT NULL
}
```

Update the existing `list_segments` query to populate the new fields:

```rust
pub fn list_segments(&self, project_id: &Id) -> Result<Vec<WorldSegmentRow>> {
    let mut stmt = self.db.conn().prepare(
        "SELECT id, name, ordering, is_collection, slug, hue_token, hidden,
                CASE WHEN template_json IS NULL THEN 0 ELSE 1 END AS has_override
         FROM world_segment WHERE project_id = ?1 ORDER BY ordering",
    )?;
    let rows = stmt.query_map([project_id.as_str()], |row| {
        let id: String = row.get(0)?;
        let id = id.parse::<Id>().map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?;
        Ok(WorldSegmentRow {
            id,
            name: row.get(1)?,
            ordering: row.get(2)?,
            is_collection: row.get::<_, i64>(3)? != 0,
            slug: row.get(4)?,
            hue_token: row.get(5)?,
            hidden: row.get::<_, i64>(6)? != 0,
            has_template_override: row.get::<_, i64>(7)? != 0,
        })
    })?;
    rows.collect::<std::result::Result<Vec<_>, _>>().map_err(Into::into)
}
```

Also adjust the existing test `upsert_and_list_segments` if the new struct shape breaks it (most likely needs `..Default` style or explicit construction of the extra fields).

- [ ] **Step 4: Implement the new methods**

Append to `crates/water-core/src/world/mod.rs` `impl<'a> WorldStore<'a>`:

```rust
pub fn seed_builtins(&self, project_id: &Id) -> Result<()> {
    use crate::world::templates::built_in_templates;
    let now = current_timestamp_string();
    for (idx, t) in built_in_templates().iter().enumerate() {
        // Check whether a row with this slug already exists for this project.
        let existing: Option<String> = self.db.conn().query_row(
            "SELECT id FROM world_segment WHERE project_id = ?1 AND slug = ?2",
            (project_id.as_str(), t.slug),
            |r| r.get(0),
        ).optional()?;
        if existing.is_some() {
            continue;   // idempotent
        }
        let id = Id::new();
        let hue = format!("--water-hue-world-{}", (idx % 6) + 1);
        self.db.conn().execute(
            "INSERT INTO world_segment
             (id, project_id, name, ordering, is_collection, slug, hue_token, hidden, created_at, updated_at, template_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, ?8, ?8, NULL)",
            (
                id.as_str(),
                project_id.as_str(),
                t.display_name,
                i64::try_from(idx).unwrap_or(i64::MAX),
                i64::from(t.is_collection),
                t.slug,
                &hue,
                &now,
            ),
        )?;
    }
    Ok(())
}

pub fn find_segment_by_slug(&self, project_id: &Id, slug: &str) -> Result<Option<WorldSegmentRow>> {
    let segs = self.list_segments(project_id)?;
    Ok(segs.into_iter().find(|s| s.slug == slug))
}

pub fn read_segment(&self, segment_id: &Id) -> Result<WorldSegmentRow> {
    let mut stmt = self.db.conn().prepare(
        "SELECT id, name, ordering, is_collection, slug, hue_token, hidden,
                CASE WHEN template_json IS NULL THEN 0 ELSE 1 END
         FROM world_segment WHERE id = ?1",
    )?;
    stmt.query_row([segment_id.as_str()], |row| {
        let id: String = row.get(0)?;
        let id = id.parse::<Id>().map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?;
        Ok(WorldSegmentRow {
            id,
            name: row.get(1)?,
            ordering: row.get(2)?,
            is_collection: row.get::<_, i64>(3)? != 0,
            slug: row.get(4)?,
            hue_token: row.get(5)?,
            hidden: row.get::<_, i64>(6)? != 0,
            has_template_override: row.get::<_, i64>(7)? != 0,
        })
    }).map_err(Into::into)
}

pub fn create_user_segment(
    &self,
    project_id: &Id,
    name: &str,
    is_collection: bool,
    template: &crate::character::intake::IntakeSchemaSection,
) -> Result<Id> {
    let id = Id::new();
    let now = current_timestamp_string();
    let json = serde_json::to_string(template)
        .map_err(|e| crate::Error::Other(format!("template serialize: {e}")))?;
    // Next ordering: max(ordering)+1.
    let next_ord: i64 = self.db.conn().query_row(
        "SELECT COALESCE(MAX(ordering), -1) + 1 FROM world_segment WHERE project_id = ?1",
        [project_id.as_str()],
        |r| r.get(0),
    )?;
    // Hue: round-robin against existing world hues.
    let count: i64 = self.db.conn().query_row(
        "SELECT COUNT(*) FROM world_segment WHERE project_id = ?1",
        [project_id.as_str()],
        |r| r.get(0),
    )?;
    let hue = format!("--water-hue-world-{}", (count % 6) + 1);

    self.db.conn().execute(
        "INSERT INTO world_segment
         (id, project_id, name, ordering, is_collection, slug, hue_token, hidden, created_at, updated_at, template_json)
         VALUES (?1, ?2, ?3, ?4, ?5, '', ?6, 0, ?7, ?7, ?8)",
        (
            id.as_str(),
            project_id.as_str(),
            name,
            next_ord,
            i64::from(is_collection),
            &hue,
            &now,
            &json,
        ),
    )?;
    Ok(id)
}

pub fn update_segment_template(
    &self,
    segment_id: &Id,
    template: &crate::character::intake::IntakeSchemaSection,
) -> Result<()> {
    let json = serde_json::to_string(template)
        .map_err(|e| crate::Error::Other(format!("template serialize: {e}")))?;
    let now = current_timestamp_string();
    self.db.conn().execute(
        "UPDATE world_segment SET template_json = ?1, updated_at = ?2 WHERE id = ?3",
        (&json, &now, segment_id.as_str()),
    )?;
    Ok(())
}

pub fn set_segment_hidden(&self, segment_id: &Id, hidden: bool) -> Result<()> {
    let now = current_timestamp_string();
    self.db.conn().execute(
        "UPDATE world_segment SET hidden = ?1, updated_at = ?2 WHERE id = ?3",
        (i64::from(hidden), &now, segment_id.as_str()),
    )?;
    Ok(())
}

pub fn delete_user_segment(&self, segment_id: &Id) -> Result<()> {
    // Refuse to delete a built-in (slug matches one of the canonical six).
    let slug: String = self.db.conn().query_row(
        "SELECT slug FROM world_segment WHERE id = ?1",
        [segment_id.as_str()],
        |r| r.get(0),
    )?;
    const BUILTIN_SLUGS: &[&str] = &[
        "concept", "locations", "politics_and_social", "culture", "world", "history",
    ];
    if BUILTIN_SLUGS.contains(&slug.as_str()) {
        return Err(crate::Error::Other(format!(
            "cannot delete built-in segment '{slug}' — use set_segment_hidden instead"
        )));
    }
    self.db.conn().execute(
        "DELETE FROM world_segment WHERE id = ?1",
        [segment_id.as_str()],
    )?;
    Ok(())
}

fn current_timestamp_string() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    secs.to_string()
}
```

Note: free `current_timestamp_string()` lives at module level (not inside the impl). If a similar helper already exists in the crate (likely in `time.rs` or `character/mod.rs`), reuse it instead of duplicating — search before adding.

```powershell
Select-String -Path crates\water-core\src\*.rs -Pattern "current_timestamp|seconds_since_epoch|SystemTime"
```

- [ ] **Step 5: Run tests, confirm pass**

```powershell
cargo test -p water-core world -- --nocapture
```

Expected: all `seed_builtins_*`, `find_segment_by_slug_*`, `create_user_segment_*`, `set_segment_hidden_*`, `delete_user_segment_*` tests PASS.

- [ ] **Step 6: Run full water-core suite**

```powershell
cargo test -p water-core
```

Expected: 215 (Task 2) + 9 new = 224 tests PASS.

- [ ] **Step 7: Commit**

```powershell
git add crates/water-core/src/world/
git commit -m "feat(world): WorldStore segment CRUD + seed_builtins (idempotent)"
```

---

### Task 4: `WorldStore` single-doc TOML round-trip + read/update

Single-doc segments (`concept`, `politics_and_social`, `culture`, `world`, `history`) store all their data in one row of `world_entry` per segment, with `is_collection = false`. On disk: `world/<slug>.toml`. This task adds `read_single_doc` + `update_single_doc_field` with disk and DB kept in sync.

**Files:**
- Create or extend: `crates/water-core/src/world/store.rs` OR `world/mod.rs` (per current split)
- Test: in same file

**Pre-authorized adaptations:**
- The exact dotted-path mutation library may differ — `serde_json::Value` field-by-field navigation is acceptable. If the M3 `update_field` in `character/mod.rs` has a reusable helper, prefer reuse.

- [ ] **Step 1: Inspect M3's single-row update_field pattern**

Read `crates/water-core/src/character/mod.rs` lines around `update_field` (the M3 character_update_field, ~line 372 per handoff). Note the dotted-path mutation, file_hash computation, and rename-cascade guard.

- [ ] **Step 2: Define `WorldSingleDocFile` shape**

Append to `crates/water-core/src/world/mod.rs`:

```rust
/// On-disk shape for a single-doc segment. Section keys (e.g. "main",
/// "lists") land at top level via `#[serde(flatten)]`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldSingleDocFile {
    pub id: Id,
    pub schema_version: String,
    pub name: String,
    #[serde(flatten)]
    pub data: serde_json::Map<String, serde_json::Value>,
}
```

- [ ] **Step 3: Write the failing tests**

Append to `tests` module:

```rust
#[test]
fn read_single_doc_returns_empty_data_for_freshly_seeded_segment() {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open_in_memory().unwrap();
    let p = ProjectStore::new(&db).insert("P").unwrap();
    let store = WorldStore::new(&db, dir.path().to_path_buf());
    store.seed_builtins(&p.id).unwrap();
    let seg = store.find_segment_by_slug(&p.id, "concept").unwrap().unwrap();

    let file = store.read_single_doc(&seg.id).unwrap();
    assert_eq!(file.name, "Concept");
    // Pre-edit, no [main] or [lists] sections yet.
    assert!(file.data.get("main").map_or(true, |v| {
        v.as_object().map_or(true, serde_json::Map::is_empty)
    }));
}

#[test]
fn update_single_doc_field_persists_to_disk_and_db() {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open_in_memory().unwrap();
    let p = ProjectStore::new(&db).insert("P").unwrap();
    let store = WorldStore::new(&db, dir.path().to_path_buf());
    store.seed_builtins(&p.id).unwrap();
    let seg = store.find_segment_by_slug(&p.id, "concept").unwrap().unwrap();

    store
        .update_single_doc_field(
            &seg.id,
            "main.core_premise",
            &serde_json::Value::String("A library that remembers".to_string()),
        )
        .unwrap();

    // Re-read from disk via store.
    let file = store.read_single_doc(&seg.id).unwrap();
    let main = file.data.get("main").unwrap().as_object().unwrap();
    assert_eq!(
        main.get("core_premise").unwrap().as_str().unwrap(),
        "A library that remembers"
    );

    // Confirm a TOML file actually landed on disk at world/concept.toml.
    let path = dir.path().join("world").join("concept.toml");
    assert!(path.exists(), "world/concept.toml should exist");
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("A library that remembers"), "TOML body should contain the value");
}

#[test]
fn update_single_doc_field_supports_string_list_kind() {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open_in_memory().unwrap();
    let p = ProjectStore::new(&db).insert("P").unwrap();
    let store = WorldStore::new(&db, dir.path().to_path_buf());
    store.seed_builtins(&p.id).unwrap();
    let seg = store.find_segment_by_slug(&p.id, "concept").unwrap().unwrap();

    let v = serde_json::json!(["memory", "loss", "obligation"]);
    store
        .update_single_doc_field(&seg.id, "lists.themes", &v)
        .unwrap();

    let file = store.read_single_doc(&seg.id).unwrap();
    let lists = file.data.get("lists").unwrap().as_object().unwrap();
    let arr = lists.get("themes").unwrap().as_array().unwrap();
    assert_eq!(arr.len(), 3);
    assert_eq!(arr[0].as_str().unwrap(), "memory");
}

#[test]
fn update_single_doc_field_updates_file_hash_in_db() {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open_in_memory().unwrap();
    let p = ProjectStore::new(&db).insert("P").unwrap();
    let store = WorldStore::new(&db, dir.path().to_path_buf());
    store.seed_builtins(&p.id).unwrap();
    let seg = store.find_segment_by_slug(&p.id, "concept").unwrap().unwrap();

    store
        .update_single_doc_field(
            &seg.id,
            "main.genre",
            &serde_json::Value::String("literary".to_string()),
        )
        .unwrap();

    // The single-doc row lives in world_entry with segment_id = seg.id.
    let hash: String = db
        .conn()
        .query_row(
            "SELECT file_hash FROM world_entry WHERE segment_id = ?1",
            [seg.id.as_str()],
            |r| r.get(0),
        )
        .unwrap();
    assert!(!hash.is_empty(), "file_hash should be populated");
}
```

- [ ] **Step 4: Run, confirm fail**

```powershell
cargo test -p water-core world::tests::read_single_doc_ world::tests::update_single_doc_ -- --nocapture
```

Expected: FAIL — methods undefined.

- [ ] **Step 5: Implement single-doc CRUD**

Append to `crates/water-core/src/world/mod.rs` impl:

```rust
/// Returns the single-doc segment's data, lazily materializing an empty
/// row + file the first time the segment is read.
pub fn read_single_doc(&self, segment_id: &Id) -> Result<WorldSingleDocFile> {
    let seg = self.read_segment(segment_id)?;
    if seg.is_collection {
        return Err(crate::Error::Other(format!(
            "segment {} is a collection; use list_entries / read_entry instead",
            seg.slug
        )));
    }

    // Look up the single row for this segment in world_entry.
    let row: Option<(String, String, String)> = self.db.conn().query_row(
        "SELECT id, name, data_json FROM world_entry WHERE segment_id = ?1 LIMIT 1",
        [segment_id.as_str()],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    ).optional()?;

    if let Some((id_str, name, data_json)) = row {
        let id = id_str.parse::<Id>().map_err(|e| {
            crate::Error::Other(format!("invalid id in world_entry: {e}"))
        })?;
        let data: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&data_json)
            .unwrap_or_default();
        return Ok(WorldSingleDocFile {
            id,
            schema_version: format!("{}@1", seg.slug),
            name: if name.is_empty() { seg.name } else { name },
            data,
        });
    }

    // Lazily create the row.
    let id = Id::new();
    let now = current_timestamp_string();
    let file_path = format!("world/{}.toml", seg.slug);
    self.db.conn().execute(
        "INSERT INTO world_entry (id, segment_id, name, data_json, file_path, file_hash, aliases_json, schema_version, created_at, updated_at)
         VALUES (?1, ?2, ?3, '{}', ?4, '', '[]', ?5, ?6, ?6)",
        (
            id.as_str(),
            segment_id.as_str(),
            &seg.name,
            &file_path,
            &format!("{}@1", seg.slug),
            &now,
        ),
    )?;

    Ok(WorldSingleDocFile {
        id,
        schema_version: format!("{}@1", seg.slug),
        name: seg.name,
        data: serde_json::Map::new(),
    })
}

/// Updates one field in a single-doc segment by dotted path (e.g.
/// "main.core_premise" or "lists.themes"). Writes the new TOML to disk,
/// recomputes file_hash, and updates the DB row in one transaction.
pub fn update_single_doc_field(
    &self,
    segment_id: &Id,
    field_id: &str,
    value: &serde_json::Value,
) -> Result<()> {
    let mut file = self.read_single_doc(segment_id)?;
    apply_dotted_mutation(&mut file.data, field_id, value.clone())?;

    let seg = self.read_segment(segment_id)?;
    let file_path = format!("world/{}.toml", seg.slug);
    let disk_path = self.project_root.join(&file_path);

    // Write TOML to disk.
    if let Some(parent) = disk_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| crate::Error::Other(format!("mkdir {parent:?}: {e}")))?;
    }
    let toml_text = render_single_doc_toml(&file)?;
    std::fs::write(&disk_path, &toml_text)
        .map_err(|e| crate::Error::Other(format!("write {disk_path:?}: {e}")))?;

    // Compute hash.
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(toml_text.as_bytes());
    let hash = hex::encode(h.finalize());

    let now = current_timestamp_string();
    let data_json = serde_json::Value::Object(file.data).to_string();
    self.db.conn().execute(
        "UPDATE world_entry SET data_json = ?1, file_hash = ?2, updated_at = ?3 WHERE id = ?4",
        (&data_json, &hash, &now, file.id.as_str()),
    )?;
    Ok(())
}

fn apply_dotted_mutation(
    data: &mut serde_json::Map<String, serde_json::Value>,
    field_id: &str,
    value: serde_json::Value,
) -> Result<()> {
    let (section, leaf) = field_id
        .split_once('.')
        .ok_or_else(|| crate::Error::Other(format!("field_id '{field_id}' is not dotted")))?;
    let section_obj = data
        .entry(section.to_string())
        .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
    let map = section_obj
        .as_object_mut()
        .ok_or_else(|| crate::Error::Other(format!("section '{section}' is not an object")))?;
    map.insert(leaf.to_string(), value);
    Ok(())
}

fn render_single_doc_toml(file: &WorldSingleDocFile) -> Result<String> {
    // The TOML serializer requires a struct shape, but our data is a
    // dynamic Map<String, Value>. Serialize the WorldSingleDocFile directly;
    // serde_flatten yields the right shape thanks to #[serde(flatten)].
    toml::to_string_pretty(file)
        .map_err(|e| crate::Error::Other(format!("toml render: {e}")))
}
```

If `hex` and `sha2` are not already in the crate's Cargo.toml, add them. Check:

```powershell
Select-String -Path crates\water-core\Cargo.toml -Pattern "sha2|hex"
```

If absent, append to `crates/water-core/Cargo.toml`:

```toml
sha2 = "0.10"
hex = "0.4"
```

(They may already be deps via M3's file_hash work — verify before adding.)

Also confirm `toml` is a dependency (was added in M1/M3 for character TOML round-trip).

- [ ] **Step 6: Run tests, confirm pass**

```powershell
cargo test -p water-core world::tests::read_single_doc_ world::tests::update_single_doc_ -- --nocapture
```

Expected: all 4 PASS.

- [ ] **Step 7: Run full suite**

```powershell
cargo test -p water-core
```

Expected: 224 (Task 3) + 4 = 228 tests PASS.

- [ ] **Step 8: Commit**

```powershell
git add crates/water-core/src/world/ crates/water-core/Cargo.toml
git commit -m "feat(world): WorldStore single-doc TOML round-trip + read/update_field"
```

---

### Task 5: `WorldStore` collection entry CRUD + TOML round-trip + `delete_if_empty`

Collection segments (currently just `locations`) store one row of `world_entry` per entry, with `is_collection = true` on the segment. On disk: `world/<slug>/<entry-ulid>.toml`. This task adds the entry-level CRUD with rename-cascade on `main.name`, alias updates, and the orphan-draft reaping helper.

**Files:**
- Extend: `crates/water-core/src/world/mod.rs`

**Pre-authorized adaptations:**
- The rename-cascade guard from M3 polish #1 (non-string `main.name` → error) is baked in from day one.
- `delete_entry_if_empty` returns `true` only if ALL data sections are absent or all-empty.

- [ ] **Step 1: Define `WorldEntryFile` + `WorldEntryIndexRow`**

Append to `crates/water-core/src/world/mod.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldEntryFile {
    pub id: Id,
    pub segment_id: Id,
    pub schema_version: String,
    pub name: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(flatten)]
    pub data: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldEntryIndexRow {
    pub id: Id,
    pub segment_id: Id,
    pub name: String,
    pub preview: String,
}
```

- [ ] **Step 2: Write the failing tests**

```rust
#[test]
fn create_entry_inserts_row_and_returns_id() {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open_in_memory().unwrap();
    let p = ProjectStore::new(&db).insert("P").unwrap();
    let store = WorldStore::new(&db, dir.path().to_path_buf());
    store.seed_builtins(&p.id).unwrap();
    let loc = store.find_segment_by_slug(&p.id, "locations").unwrap().unwrap();

    let id = store.create_entry(&loc.id, "The Pell Library").unwrap();
    let entries = store.list_entries(&loc.id).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].id, id);
    assert_eq!(entries[0].name, "The Pell Library");
}

#[test]
fn create_entry_with_empty_name_is_allowed_for_chorus_stub() {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open_in_memory().unwrap();
    let p = ProjectStore::new(&db).insert("P").unwrap();
    let store = WorldStore::new(&db, dir.path().to_path_buf());
    store.seed_builtins(&p.id).unwrap();
    let loc = store.find_segment_by_slug(&p.id, "locations").unwrap().unwrap();
    let id = store.create_entry(&loc.id, "").unwrap();
    let entries = store.list_entries(&loc.id).unwrap();
    assert_eq!(entries[0].id, id);
    assert_eq!(entries[0].name, "");
}

#[test]
fn create_entry_seeded_writes_initial_field_value() {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open_in_memory().unwrap();
    let p = ProjectStore::new(&db).insert("P").unwrap();
    let store = WorldStore::new(&db, dir.path().to_path_buf());
    store.seed_builtins(&p.id).unwrap();
    let loc = store.find_segment_by_slug(&p.id, "locations").unwrap().unwrap();

    let id = store
        .create_entry_seeded(&loc.id, "", "main.sensory_detail", "Dust thick enough to read fingertips in")
        .unwrap();
    let entry = store.read_entry(&id).unwrap();
    let main = entry.data.get("main").unwrap().as_object().unwrap();
    assert_eq!(
        main.get("sensory_detail").unwrap().as_str().unwrap(),
        "Dust thick enough to read fingertips in"
    );
}

#[test]
fn update_entry_field_persists_to_disk_with_correct_path() {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open_in_memory().unwrap();
    let p = ProjectStore::new(&db).insert("P").unwrap();
    let store = WorldStore::new(&db, dir.path().to_path_buf());
    store.seed_builtins(&p.id).unwrap();
    let loc = store.find_segment_by_slug(&p.id, "locations").unwrap().unwrap();
    let id = store.create_entry(&loc.id, "Pell").unwrap();

    store
        .update_entry_field(&id, "main.type", &serde_json::json!("underground library"))
        .unwrap();

    let entry = store.read_entry(&id).unwrap();
    let main = entry.data.get("main").unwrap().as_object().unwrap();
    assert_eq!(main.get("type").unwrap().as_str().unwrap(), "underground library");

    // Check the file landed at world/locations/<id>.toml.
    let on_disk = dir
        .path()
        .join("world")
        .join("locations")
        .join(format!("{}.toml", id.as_str()));
    assert!(on_disk.exists(), "expected {on_disk:?} to exist");
}

#[test]
fn update_entry_field_rename_cascade_guard_rejects_non_string_main_name() {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open_in_memory().unwrap();
    let p = ProjectStore::new(&db).insert("P").unwrap();
    let store = WorldStore::new(&db, dir.path().to_path_buf());
    store.seed_builtins(&p.id).unwrap();
    let loc = store.find_segment_by_slug(&p.id, "locations").unwrap().unwrap();
    let id = store.create_entry(&loc.id, "Pell").unwrap();

    let err = store
        .update_entry_field(&id, "main.name", &serde_json::json!(42))
        .unwrap_err();
    assert!(
        err.to_string().contains("main.name must be a string"),
        "expected guard error; got {err}"
    );
}

#[test]
fn update_entry_field_main_name_renames_world_entry_row() {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open_in_memory().unwrap();
    let p = ProjectStore::new(&db).insert("P").unwrap();
    let store = WorldStore::new(&db, dir.path().to_path_buf());
    store.seed_builtins(&p.id).unwrap();
    let loc = store.find_segment_by_slug(&p.id, "locations").unwrap().unwrap();
    let id = store.create_entry(&loc.id, "Pell").unwrap();

    store
        .update_entry_field(&id, "main.name", &serde_json::json!("The Pell Library"))
        .unwrap();

    let name: String = db
        .conn()
        .query_row(
            "SELECT name FROM world_entry WHERE id = ?1",
            [id.as_str()],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(name, "The Pell Library");
}

#[test]
fn update_entry_aliases_persists_to_db_and_disk() {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open_in_memory().unwrap();
    let p = ProjectStore::new(&db).insert("P").unwrap();
    let store = WorldStore::new(&db, dir.path().to_path_buf());
    store.seed_builtins(&p.id).unwrap();
    let loc = store.find_segment_by_slug(&p.id, "locations").unwrap().unwrap();
    let id = store.create_entry(&loc.id, "The Pell Library").unwrap();

    let aliases = vec!["Pell".to_string(), "the library".to_string()];
    store.update_entry_aliases(&id, &aliases).unwrap();

    let entry = store.read_entry(&id).unwrap();
    assert_eq!(entry.aliases, aliases);

    let json: String = db
        .conn()
        .query_row(
            "SELECT aliases_json FROM world_entry WHERE id = ?1",
            [id.as_str()],
            |r| r.get(0),
        )
        .unwrap();
    let parsed: Vec<String> = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, aliases);
}

#[test]
fn delete_entry_removes_row_and_file() {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open_in_memory().unwrap();
    let p = ProjectStore::new(&db).insert("P").unwrap();
    let store = WorldStore::new(&db, dir.path().to_path_buf());
    store.seed_builtins(&p.id).unwrap();
    let loc = store.find_segment_by_slug(&p.id, "locations").unwrap().unwrap();
    let id = store.create_entry(&loc.id, "Pell").unwrap();
    let path = dir
        .path()
        .join("world")
        .join("locations")
        .join(format!("{}.toml", id.as_str()));
    // Force-create the file so delete has something to clean up.
    store
        .update_entry_field(&id, "main.type", &serde_json::json!("library"))
        .unwrap();
    assert!(path.exists());

    store.delete_entry(&id).unwrap();

    let entries = store.list_entries(&loc.id).unwrap();
    assert!(entries.is_empty());
    assert!(!path.exists(), "TOML file should be removed");
}

#[test]
fn delete_entry_if_empty_returns_true_for_blank_entry() {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open_in_memory().unwrap();
    let p = ProjectStore::new(&db).insert("P").unwrap();
    let store = WorldStore::new(&db, dir.path().to_path_buf());
    store.seed_builtins(&p.id).unwrap();
    let loc = store.find_segment_by_slug(&p.id, "locations").unwrap().unwrap();
    let id = store.create_entry(&loc.id, "").unwrap();
    let reaped = store.delete_entry_if_empty(&id).unwrap();
    assert!(reaped);
    let entries = store.list_entries(&loc.id).unwrap();
    assert!(entries.is_empty());
}

#[test]
fn delete_entry_if_empty_returns_false_for_populated_entry() {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open_in_memory().unwrap();
    let p = ProjectStore::new(&db).insert("P").unwrap();
    let store = WorldStore::new(&db, dir.path().to_path_buf());
    store.seed_builtins(&p.id).unwrap();
    let loc = store.find_segment_by_slug(&p.id, "locations").unwrap().unwrap();
    let id = store.create_entry(&loc.id, "Pell").unwrap();
    store
        .update_entry_field(&id, "main.type", &serde_json::json!("library"))
        .unwrap();
    let reaped = store.delete_entry_if_empty(&id).unwrap();
    assert!(!reaped);
    let entries = store.list_entries(&loc.id).unwrap();
    assert_eq!(entries.len(), 1);
}
```

- [ ] **Step 3: Run, confirm fail**

```powershell
cargo test -p water-core world::tests -- --nocapture
```

Expected: all the new tests FAIL (methods undefined).

- [ ] **Step 4: Implement entry CRUD**

Append to `crates/water-core/src/world/mod.rs`:

```rust
pub fn list_entries(&self, segment_id: &Id) -> Result<Vec<WorldEntryIndexRow>> {
    let mut stmt = self.db.conn().prepare(
        "SELECT id, name, data_json FROM world_entry WHERE segment_id = ?1 ORDER BY name",
    )?;
    let rows = stmt.query_map([segment_id.as_str()], |row| {
        let id_str: String = row.get(0)?;
        let id = id_str.parse::<Id>().map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
        })?;
        let name: String = row.get(1)?;
        let data_json: String = row.get(2)?;
        Ok((id, segment_id.clone(), name, data_json))
    })?;
    let mut out = vec![];
    for r in rows {
        let (id, seg_id, name, data_json) = r?;
        let data: serde_json::Value = serde_json::from_str(&data_json).unwrap_or(serde_json::Value::Null);
        let preview = compute_preview(&data);
        out.push(WorldEntryIndexRow { id, segment_id: seg_id, name, preview });
    }
    Ok(out)
}

pub fn read_entry(&self, entry_id: &Id) -> Result<WorldEntryFile> {
    let (segment_id_str, name, data_json, aliases_json, schema_version): (String, String, String, String, String) =
        self.db.conn().query_row(
            "SELECT segment_id, name, data_json, aliases_json, schema_version FROM world_entry WHERE id = ?1",
            [entry_id.as_str()],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )?;
    let segment_id = segment_id_str.parse::<Id>().map_err(|e| {
        crate::Error::Other(format!("invalid segment_id: {e}"))
    })?;
    let data: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&data_json).unwrap_or_default();
    let aliases: Vec<String> = serde_json::from_str(&aliases_json).unwrap_or_default();
    Ok(WorldEntryFile {
        id: entry_id.clone(),
        segment_id,
        schema_version,
        name,
        aliases,
        data,
    })
}

pub fn create_entry(&self, segment_id: &Id, name: &str) -> Result<Id> {
    let seg = self.read_segment(segment_id)?;
    if !seg.is_collection {
        return Err(crate::Error::Other(format!(
            "segment {} is not a collection; cannot create entry",
            seg.slug
        )));
    }
    let id = Id::new();
    let now = current_timestamp_string();
    let file_path = format!("world/{}/{}.toml", seg.slug, id.as_str());
    let schema_version = format!("{}@1", seg.slug);
    self.db.conn().execute(
        "INSERT INTO world_entry (id, segment_id, name, data_json, file_path, file_hash, aliases_json, schema_version, created_at, updated_at)
         VALUES (?1, ?2, ?3, '{}', ?4, '', '[]', ?5, ?6, ?6)",
        (
            id.as_str(),
            segment_id.as_str(),
            name,
            &file_path,
            &schema_version,
            &now,
        ),
    )?;
    Ok(id)
}

pub fn create_entry_seeded(
    &self,
    segment_id: &Id,
    name: &str,
    seed_field_id: &str,
    seed_value: &str,
) -> Result<Id> {
    let id = self.create_entry(segment_id, name)?;
    self.update_entry_field(
        &id,
        seed_field_id,
        &serde_json::Value::String(seed_value.to_string()),
    )?;
    Ok(id)
}

pub fn update_entry_field(
    &self,
    entry_id: &Id,
    field_id: &str,
    value: &serde_json::Value,
) -> Result<()> {
    // Rename-cascade guard (M3 polish #1).
    if field_id == "main.name" && !value.is_string() {
        return Err(crate::Error::Other(
            "main.name must be a string".to_string(),
        ));
    }

    let mut file = self.read_entry(entry_id)?;
    apply_dotted_mutation(&mut file.data, field_id, value.clone())?;

    let seg = self.read_segment(&file.segment_id)?;
    let file_path = format!("world/{}/{}.toml", seg.slug, entry_id.as_str());
    let disk_path = self.project_root.join(&file_path);

    // If main.name changed, update file.name too (it stays in sync with the TOML top-level `name` field).
    let new_name = if field_id == "main.name" {
        let s = value.as_str().expect("guarded above").to_string();
        file.name = s.clone();
        Some(s)
    } else {
        None
    };

    if let Some(parent) = disk_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| crate::Error::Other(format!("mkdir {parent:?}: {e}")))?;
    }
    let toml_text = toml::to_string_pretty(&file)
        .map_err(|e| crate::Error::Other(format!("toml render: {e}")))?;
    std::fs::write(&disk_path, &toml_text)
        .map_err(|e| crate::Error::Other(format!("write {disk_path:?}: {e}")))?;

    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(toml_text.as_bytes());
    let hash = hex::encode(h.finalize());

    let now = current_timestamp_string();
    let data_json = serde_json::Value::Object(file.data).to_string();
    if let Some(name) = new_name {
        self.db.conn().execute(
            "UPDATE world_entry SET data_json = ?1, name = ?2, file_hash = ?3, updated_at = ?4 WHERE id = ?5",
            (&data_json, &name, &hash, &now, entry_id.as_str()),
        )?;
    } else {
        self.db.conn().execute(
            "UPDATE world_entry SET data_json = ?1, file_hash = ?2, updated_at = ?3 WHERE id = ?4",
            (&data_json, &hash, &now, entry_id.as_str()),
        )?;
    }
    Ok(())
}

pub fn update_entry_aliases(&self, entry_id: &Id, aliases: &[String]) -> Result<()> {
    let mut file = self.read_entry(entry_id)?;
    file.aliases = aliases.to_vec();

    let seg = self.read_segment(&file.segment_id)?;
    let file_path = format!("world/{}/{}.toml", seg.slug, entry_id.as_str());
    let disk_path = self.project_root.join(&file_path);
    if let Some(parent) = disk_path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| crate::Error::Other(format!("mkdir {parent:?}: {e}")))?;
    }
    let toml_text = toml::to_string_pretty(&file)
        .map_err(|e| crate::Error::Other(format!("toml render: {e}")))?;
    std::fs::write(&disk_path, &toml_text)
        .map_err(|e| crate::Error::Other(format!("write {disk_path:?}: {e}")))?;

    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(toml_text.as_bytes());
    let hash = hex::encode(h.finalize());

    let now = current_timestamp_string();
    let aliases_json = serde_json::to_string(aliases)
        .map_err(|e| crate::Error::Other(format!("aliases serialize: {e}")))?;
    self.db.conn().execute(
        "UPDATE world_entry SET aliases_json = ?1, file_hash = ?2, updated_at = ?3 WHERE id = ?4",
        (&aliases_json, &hash, &now, entry_id.as_str()),
    )?;
    Ok(())
}

pub fn delete_entry(&self, entry_id: &Id) -> Result<()> {
    let file_path: String = self.db.conn().query_row(
        "SELECT file_path FROM world_entry WHERE id = ?1",
        [entry_id.as_str()],
        |r| r.get(0),
    )?;
    let disk_path = self.project_root.join(&file_path);
    if disk_path.exists() {
        std::fs::remove_file(&disk_path)
            .map_err(|e| crate::Error::Other(format!("remove {disk_path:?}: {e}")))?;
    }
    self.db.conn().execute(
        "DELETE FROM world_entry WHERE id = ?1",
        [entry_id.as_str()],
    )?;
    Ok(())
}

pub fn delete_entry_if_empty(&self, entry_id: &Id) -> Result<bool> {
    let file = self.read_entry(entry_id)?;
    let is_empty = file.name.is_empty()
        && file.aliases.is_empty()
        && file.data.values().all(|v| match v {
            serde_json::Value::Object(m) => m.is_empty()
                || m.values().all(|inner| matches!(
                    inner,
                    serde_json::Value::Null
                        | serde_json::Value::String(s) if s.is_empty(),
                )),
            serde_json::Value::Array(a) => a.is_empty(),
            serde_json::Value::Null => true,
            serde_json::Value::String(s) => s.is_empty(),
            _ => false,
        });
    if is_empty {
        self.delete_entry(entry_id)?;
        return Ok(true);
    }
    Ok(false)
}

fn compute_preview(data: &serde_json::Value) -> String {
    // First non-empty string value found in main.* or first array element's first 80 chars.
    if let Some(main) = data.get("main").and_then(serde_json::Value::as_object) {
        for v in main.values() {
            if let Some(s) = v.as_str() {
                if !s.trim().is_empty() {
                    let truncated: String = s.chars().take(80).collect();
                    return truncated;
                }
            }
        }
    }
    String::new()
}
```

Note: the `delete_entry_if_empty` "all-empty" predicate uses a non-trivial pattern. The matches! on `serde_json::Value::String(s) if s.is_empty()` requires a slight reshape — the actual implementer should write the predicate as a clear helper function `is_value_empty(&Value) -> bool` rather than the complex matches!. The test-derived contract is: `delete_entry_if_empty` returns `true` only when every section is absent OR contains only null/empty-string values, AND the entry's own name and aliases are empty.

- [ ] **Step 5: Run tests, confirm pass**

```powershell
cargo test -p water-core world::tests -- --nocapture
```

Expected: all entry tests PASS.

- [ ] **Step 6: Run full suite**

```powershell
cargo test -p water-core
```

Expected: 228 (Task 4) + 10 = 238 tests PASS.

- [ ] **Step 7: Commit**

```powershell
git add crates/water-core/src/world/
git commit -m "feat(world): WorldStore collection entry CRUD + delete_entry_if_empty"
```

---

### Task 6: `WorldRegistry::from_db` with case-insensitive name+alias index

The hot-path snapshot built once per orchestrator dispatch. Used by `world_drift` Stage 1, by the future `WorldAutosuggest` chip surface, and by `CharacterSpeaker::from_row` for location-context resolution.

**Files:**
- Create: `crates/water-core/src/world/registry.rs`
- Modify: `crates/water-core/src/world/mod.rs` (add `pub mod registry;`)

**Pre-authorized adaptations:**
- The internal HashMap key is `String` (owned); consider `Box<str>` if benchmarking shows it matters (unlikely at expected dataset sizes).

- [ ] **Step 1: Write the failing tests**

Create `crates/water-core/src/world/registry.rs`:

```rust
//! `WorldRegistry` — hot-path read-only snapshot of all world segments
//! and entries for a project. Built once per orchestrator dispatch by
//! `WorldRegistry::from_db`.
//!
//! Name + alias lookups are **case-insensitive** (lowercased on both
//! insertion and query). Character autosuggest (M3) is case-sensitive
//! on word boundaries — this asymmetry is intentional: place names are
//! more case-variable in English prose than character names.

use crate::{world::WorldStore, Db, Id, Result};
use std::collections::HashMap;

pub struct WorldRegistry {
    by_id: HashMap<Id, WorldEntrySnapshot>,
    by_name_or_alias: HashMap<String, Vec<Id>>,
    segments: HashMap<Id, crate::world::WorldSegmentRow>,
    by_segment_slug: HashMap<String, Id>,
}

#[derive(Debug, Clone)]
pub struct WorldEntrySnapshot {
    pub id: Id,
    pub segment_id: Id,
    pub segment_slug: String,
    pub name: String,
    pub aliases: Vec<String>,
    pub data: serde_json::Value,
}

impl WorldRegistry {
    pub fn from_db(db: &Db, project_id: &Id, project_root: std::path::PathBuf) -> Result<Self> {
        let store = WorldStore::new(db, project_root);
        let segments = store.list_segments(project_id)?;
        let mut by_segment_slug = HashMap::new();
        let mut by_id_seg = HashMap::new();
        for s in &segments {
            by_segment_slug.insert(s.slug.clone(), s.id.clone());
            by_id_seg.insert(s.id.clone(), s.clone());
        }

        let mut by_id = HashMap::new();
        let mut by_name_or_alias: HashMap<String, Vec<Id>> = HashMap::new();
        for s in &segments {
            let entries = store.list_entries(&s.id)?;
            for index_row in entries {
                let entry = store.read_entry(&index_row.id)?;
                let snap = WorldEntrySnapshot {
                    id: entry.id.clone(),
                    segment_id: entry.segment_id.clone(),
                    segment_slug: s.slug.clone(),
                    name: entry.name.clone(),
                    aliases: entry.aliases.clone(),
                    data: serde_json::Value::Object(entry.data),
                };
                if !snap.name.trim().is_empty() {
                    by_name_or_alias
                        .entry(snap.name.to_lowercase())
                        .or_default()
                        .push(snap.id.clone());
                }
                for alias in &snap.aliases {
                    if !alias.trim().is_empty() {
                        by_name_or_alias
                            .entry(alias.to_lowercase())
                            .or_default()
                            .push(snap.id.clone());
                    }
                }
                by_id.insert(snap.id.clone(), snap);
            }
        }

        Ok(Self {
            by_id,
            by_name_or_alias,
            segments: by_id_seg,
            by_segment_slug,
        })
    }

    #[must_use]
    pub fn by_id(&self, id: &Id) -> Option<&WorldEntrySnapshot> {
        self.by_id.get(id)
    }

    /// Returns IDs whose `name` or any alias matches `lowercased_token`
    /// (case-insensitive). Caller MUST lowercase before calling.
    #[must_use]
    pub fn find_by_token(&self, lowercased_token: &str) -> &[Id] {
        self.by_name_or_alias
            .get(lowercased_token)
            .map_or(&[], Vec::as_slice)
    }

    pub fn entries_by_segment_slug(&self, slug: &str) -> Vec<&WorldEntrySnapshot> {
        let Some(seg_id) = self.by_segment_slug.get(slug) else {
            return vec![];
        };
        self.by_id
            .values()
            .filter(|e| &e.segment_id == seg_id)
            .collect()
    }

    pub fn segments(&self) -> impl Iterator<Item = &crate::world::WorldSegmentRow> {
        self.segments.values()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ProjectStore;

    #[test]
    fn from_db_indexes_seeded_segments() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        let p = ProjectStore::new(&db).insert("P").unwrap();
        WorldStore::new(&db, dir.path().to_path_buf()).seed_builtins(&p.id).unwrap();

        let reg = WorldRegistry::from_db(&db, &p.id, dir.path().to_path_buf()).unwrap();
        let slugs: Vec<&str> = reg.segments().map(|s| s.slug.as_str()).collect();
        assert!(slugs.contains(&"concept"));
        assert!(slugs.contains(&"locations"));
        assert_eq!(slugs.len(), 6);
    }

    #[test]
    fn find_by_token_is_case_insensitive() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        let p = ProjectStore::new(&db).insert("P").unwrap();
        let store = WorldStore::new(&db, dir.path().to_path_buf());
        store.seed_builtins(&p.id).unwrap();
        let loc = store.find_segment_by_slug(&p.id, "locations").unwrap().unwrap();
        let id = store.create_entry(&loc.id, "The Pell Library").unwrap();
        store
            .update_entry_aliases(&id, &["Pell".to_string(), "the library".to_string()])
            .unwrap();

        let reg = WorldRegistry::from_db(&db, &p.id, dir.path().to_path_buf()).unwrap();

        // Lowercased lookup ALL match the same id.
        let matches_name_lower = reg.find_by_token("the pell library");
        let matches_alias_pell = reg.find_by_token("pell");
        let matches_alias_lib = reg.find_by_token("the library");

        assert_eq!(matches_name_lower, &[id.clone()][..]);
        assert_eq!(matches_alias_pell, &[id.clone()][..]);
        assert_eq!(matches_alias_lib, &[id][..]);
    }

    #[test]
    fn find_by_token_returns_empty_for_no_match() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        let p = ProjectStore::new(&db).insert("P").unwrap();
        WorldStore::new(&db, dir.path().to_path_buf()).seed_builtins(&p.id).unwrap();
        let reg = WorldRegistry::from_db(&db, &p.id, dir.path().to_path_buf()).unwrap();
        assert!(reg.find_by_token("nonexistent").is_empty());
    }

    #[test]
    fn entries_by_segment_slug_returns_only_matching_segment() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        let p = ProjectStore::new(&db).insert("P").unwrap();
        let store = WorldStore::new(&db, dir.path().to_path_buf());
        store.seed_builtins(&p.id).unwrap();
        let loc = store.find_segment_by_slug(&p.id, "locations").unwrap().unwrap();
        store.create_entry(&loc.id, "A").unwrap();
        store.create_entry(&loc.id, "B").unwrap();

        let reg = WorldRegistry::from_db(&db, &p.id, dir.path().to_path_buf()).unwrap();
        let in_locations = reg.entries_by_segment_slug("locations");
        assert_eq!(in_locations.len(), 2);
        let in_concept = reg.entries_by_segment_slug("concept");
        assert_eq!(in_concept.len(), 0);
    }

    #[test]
    fn by_id_returns_some_for_known_entry() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        let p = ProjectStore::new(&db).insert("P").unwrap();
        let store = WorldStore::new(&db, dir.path().to_path_buf());
        store.seed_builtins(&p.id).unwrap();
        let loc = store.find_segment_by_slug(&p.id, "locations").unwrap().unwrap();
        let id = store.create_entry(&loc.id, "X").unwrap();
        let reg = WorldRegistry::from_db(&db, &p.id, dir.path().to_path_buf()).unwrap();
        let snap = reg.by_id(&id).expect("entry must be present");
        assert_eq!(snap.name, "X");
        assert_eq!(snap.segment_slug, "locations");
    }
}
```

In `world/mod.rs`, add:

```rust
pub mod registry;
pub use registry::{WorldRegistry, WorldEntrySnapshot};
```

- [ ] **Step 2: Run, confirm pass**

```powershell
cargo test -p water-core world::registry -- --nocapture
```

Expected: ALL FIVE tests PASS.

- [ ] **Step 3: Run full suite**

```powershell
cargo test -p water-core
```

Expected: 238 (Task 5) + 5 = 243 tests PASS.

- [ ] **Step 4: Commit**

```powershell
git add crates/water-core/src/world/
git commit -m "feat(world): WorldRegistry::from_db with case-insensitive name+alias index"
```

---

### Task 7: `rebuild.rs` world scan + `scene.location_id` orphan repair

Extends `crates/water-core/src/rebuild.rs` (which currently seeds legacy world rows at lines ~116-124) to:

1. Scan `world/` for built-in single-doc files and collection directories on disk.
2. Scan `world/_segments/*.template.toml` for user-added segments.
3. Seed any missing built-ins from `BUILT_IN_TEMPLATES`.
4. Reattach `scene.location_id` references; orphan refs become NULL with a `tracing::warn!`.
5. Rebuild `aliases_json` from each entry's TOML aliases array.

**Files:**
- Modify: `crates/water-core/src/rebuild.rs`

**Pre-authorized adaptations:**
- If `rebuild.rs` doesn't currently scan `world/` at all (the existing lines 116-124 may be a no-op pre-M4), this task implements the scan from scratch.
- A pre-existing `WorldStore::seed_builtins` call satisfies the "ensure built-ins exist" requirement.

- [ ] **Step 1: Read current rebuild.rs**

```powershell
Get-Content crates\water-core\src\rebuild.rs
```

Identify where world rows are currently rebuilt (~line 116). Note the surrounding pattern (transaction handling, error propagation).

- [ ] **Step 2: Write the failing test**

Append to `crates/water-core/src/rebuild.rs` (or wherever rebuild tests live):

```rust
#[test]
fn rebuild_seeds_builtin_segments_on_fresh_project() {
    use crate::{world::WorldStore, ProjectStore};
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open(dir.path().join("project.db")).unwrap();
    let p = ProjectStore::new(&db).insert("P").unwrap();

    // Simulate a freshly-rebuilt DB by directly invoking the rebuild path.
    rebuild_project(&db, &p.id, dir.path()).unwrap();

    let segs = WorldStore::new(&db, dir.path().to_path_buf())
        .list_segments(&p.id)
        .unwrap();
    assert_eq!(segs.len(), 6);
}

#[test]
fn rebuild_nulls_orphan_scene_location_id() {
    use crate::{world::WorldStore, ProjectStore};
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open(dir.path().join("project.db")).unwrap();
    let p = ProjectStore::new(&db).insert("P").unwrap();
    let store = WorldStore::new(&db, dir.path().to_path_buf());
    store.seed_builtins(&p.id).unwrap();
    let loc = store.find_segment_by_slug(&p.id, "locations").unwrap().unwrap();
    let entry_id = store.create_entry(&loc.id, "Pell").unwrap();

    // Create a scene referencing the entry, then delete the entry out-of-band
    // to simulate user manually removing the TOML file.
    let manuscript_id = crate::ManuscriptStore::new(&db).insert(&p.id, "M").unwrap();
    let chapter_id = crate::ChapterStore::new(&db).insert(&manuscript_id, "C").unwrap();
    let scene_id = crate::SceneStore::new(&db)
        .insert(&manuscript_id, Some(&chapter_id), "S", 0)
        .unwrap();
    db.conn().execute(
        "UPDATE scene SET location_id = ?1 WHERE id = ?2",
        (entry_id.as_str(), scene_id.as_str()),
    ).unwrap();
    db.conn().execute(
        "DELETE FROM world_entry WHERE id = ?1",
        [entry_id.as_str()],
    ).unwrap();

    rebuild_project(&db, &p.id, dir.path()).unwrap();

    let loc_after: Option<String> = db.conn().query_row(
        "SELECT location_id FROM scene WHERE id = ?1",
        [scene_id.as_str()],
        |r| r.get(0),
    ).unwrap();
    assert!(loc_after.is_none(), "orphan location_id should be cleared");
}
```

Note: if `ManuscriptStore`, `ChapterStore`, `SceneStore` have different APIs in this codebase, adapt the test setup. Read `crates/water-core/src/lib.rs` to confirm the actual constructors.

- [ ] **Step 3: Run, confirm fail**

```powershell
cargo test -p water-core rebuild::tests::rebuild_seeds_builtin_segments rebuild::tests::rebuild_nulls_orphan -- --nocapture
```

Expected: FAIL — either `rebuild_project` undefined OR doesn't seed/clear.

- [ ] **Step 4: Implement the rebuild extension**

Modify `crates/water-core/src/rebuild.rs`. Find the existing `rebuild_project` (or whichever public entry-point — name may differ) and add a "Phase: world" block before manuscript/scene rebuilds:

```rust
pub fn rebuild_project(db: &Db, project_id: &Id, project_root: &Path) -> Result<()> {
    // ... existing project/manuscript/chapter rebuild steps ...

    // M4: Ensure built-in world segments exist.
    let world = WorldStore::new(db, project_root.to_path_buf());
    world.seed_builtins(project_id)?;

    // M4: Scan world/_segments/*.template.toml for user-added segments
    //     and apply template overrides. Soft-skip on parse error with a warn.
    let segments_dir = project_root.join("world").join("_segments");
    if segments_dir.exists() {
        for entry in std::fs::read_dir(&segments_dir)
            .map_err(|e| Error::Other(format!("read_dir {segments_dir:?}: {e}")))?
        {
            let entry = entry.map_err(|e| Error::Other(format!("dir entry: {e}")))?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("toml") {
                continue;
            }
            match std::fs::read_to_string(&path)
                .map_err(|e| format!("read {path:?}: {e}"))
                .and_then(|text| {
                    toml::from_str::<crate::character::intake::IntakeSchemaSection>(&text)
                        .map_err(|e| format!("parse {path:?}: {e}"))
                }) {
                Ok(template) => {
                    let slug = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                    if let Some(seg) = world.find_segment_by_slug(project_id, slug)? {
                        world.update_segment_template(&seg.id, &template)?;
                    }
                    // If no matching segment exists, the user-added segment
                    // wasn't created via create_user_segment yet — skip;
                    // subsequent project work will surface this.
                }
                Err(e) => {
                    tracing::warn!("skipping malformed segment template {path:?}: {e}");
                }
            }
        }
    }

    // M4: Orphan repair for scene.location_id.
    db.conn().execute(
        "UPDATE scene SET location_id = NULL
         WHERE location_id IS NOT NULL
           AND location_id NOT IN (SELECT id FROM world_entry)",
        [],
    )?;

    // ... continue with remaining rebuild steps ...

    Ok(())
}
```

Note: if `tracing` is not imported, add `use tracing;` at top of file. The crate already uses tracing per M3 (verify with `Select-String -Path crates\water-core\src\*.rs -Pattern "tracing::"`).

- [ ] **Step 5: Run tests, confirm pass**

```powershell
cargo test -p water-core rebuild -- --nocapture
```

Expected: rebuild tests PASS.

- [ ] **Step 6: Run full suite**

```powershell
cargo test -p water-core
```

Expected: 243 (Task 6) + 2 = 245 tests PASS. **Phase A complete.**

- [ ] **Step 7: Commit**

```powershell
git add crates/water-core/src/rebuild.rs
git commit -m "feat(world): rebuild scans world/ + seeds built-ins + clears orphan scene.location_id"
```

**Phase A close-out.** All seven engine-foundation tasks done. `cargo test -p water-core` is green with +44 tests over the M3 baseline (245 vs. 201). Next: Phase B wires the Tauri commands.

---

## Phase B — Tauri commands

Wires the engine surface from Phase A through to Tauri commands using the `_core` async-fn pattern (M3 lesson: every command takes unpacked deps so tests can drive it). All commands go on the `ipc{}` singleton in `app/src/ipc/commands.ts` — no per-feature TS modules.

### Task 8: IPC types + `world.rs` segment commands

Stands up `app/src-tauri/src/commands/world.rs` with the segment-level commands and registers them in `lib.rs`. TS types added to `app/src/ipc/commands.ts`.

**Files:**
- Create: `app/src-tauri/src/commands/world.rs`
- Modify: `app/src-tauri/src/lib.rs` (register commands)
- Modify: `app/src/ipc/commands.ts` (types + method signatures)

**Pre-authorized adaptations:**
- The `_core` extraction pattern (separate async fn taking unpacked deps; thin Tauri command wrapper) is REQUIRED.

- [ ] **Step 1: Inspect M3's character commands**

```powershell
Get-Content "app\src-tauri\src\commands\character.rs" -Head 80
```

Note the `character_list_core` / `character_list` wrapping pattern.

- [ ] **Step 2: Write the failing core-test**

Create `app/src-tauri/src/commands/world.rs`:

```rust
//! World/Setting Bible Tauri commands.
//!
//! All commands follow the `_core` async-fn pattern: the public Tauri
//! command is a thin wrapper that unpacks `tauri::State<OpenProject>`
//! and calls a `*_core` async fn taking the unpacked deps. Tests drive
//! `*_core` directly because `tauri::State<'_, T>` has no test constructor.

use crate::state::OpenProject;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::State;
use tokio::sync::Mutex;
use water_core::character::intake::IntakeSchemaSection;
use water_core::world::{WorldSegmentRow, WorldStore};
use water_core::{Db, Id};

#[derive(Debug, Serialize, Deserialize)]
pub struct WorldSegmentPayload {
    pub id: String,
    pub slug: String,
    pub name: String,
    pub ordering: i64,
    pub is_collection: bool,
    pub hue_token: String,
    pub hidden: bool,
    pub has_template_override: bool,
}

impl From<WorldSegmentRow> for WorldSegmentPayload {
    fn from(r: WorldSegmentRow) -> Self {
        Self {
            id: r.id.to_string(),
            slug: r.slug,
            name: r.name,
            ordering: r.ordering,
            is_collection: r.is_collection,
            hue_token: r.hue_token,
            hidden: r.hidden,
            has_template_override: r.has_template_override,
        }
    }
}

pub async fn world_segment_list_core(
    db: &Arc<Mutex<Db>>,
    project_root: &std::path::Path,
    project_id: &Id,
) -> Result<Vec<WorldSegmentPayload>, String> {
    let db = db.lock().await;
    let store = WorldStore::new(&db, project_root.to_path_buf());
    store
        .list_segments(project_id)
        .map(|rows| rows.into_iter().map(Into::into).collect())
        .map_err(|e| e.to_string())
}

#[derive(Deserialize)]
pub struct CreateSegmentRequest {
    pub name: String,
    pub is_collection: bool,
    pub template: IntakeSchemaSection,
}

pub async fn world_segment_create_core(
    db: &Arc<Mutex<Db>>,
    project_root: &std::path::Path,
    project_id: &Id,
    req: CreateSegmentRequest,
) -> Result<String, String> {
    let db = db.lock().await;
    let store = WorldStore::new(&db, project_root.to_path_buf());
    store
        .create_user_segment(project_id, &req.name, req.is_collection, &req.template)
        .map(|id| id.to_string())
        .map_err(|e| e.to_string())
}

#[derive(Deserialize)]
pub struct UpdateTemplateRequest {
    pub segment_id: String,
    pub template: IntakeSchemaSection,
}

pub async fn world_segment_update_template_core(
    db: &Arc<Mutex<Db>>,
    project_root: &std::path::Path,
    req: UpdateTemplateRequest,
) -> Result<(), String> {
    let db = db.lock().await;
    let store = WorldStore::new(&db, project_root.to_path_buf());
    let seg_id: Id = req.segment_id.parse().map_err(|e: water_core::Error| e.to_string())?;
    store
        .update_segment_template(&seg_id, &req.template)
        .map_err(|e| e.to_string())
}

#[derive(Deserialize)]
pub struct SetHiddenRequest {
    pub segment_id: String,
    pub hidden: bool,
}

pub async fn world_segment_set_hidden_core(
    db: &Arc<Mutex<Db>>,
    project_root: &std::path::Path,
    req: SetHiddenRequest,
) -> Result<(), String> {
    let db = db.lock().await;
    let store = WorldStore::new(&db, project_root.to_path_buf());
    let seg_id: Id = req.segment_id.parse().map_err(|e: water_core::Error| e.to_string())?;
    store
        .set_segment_hidden(&seg_id, req.hidden)
        .map_err(|e| e.to_string())
}

pub async fn world_segment_delete_core(
    db: &Arc<Mutex<Db>>,
    project_root: &std::path::Path,
    segment_id: String,
) -> Result<(), String> {
    let db = db.lock().await;
    let store = WorldStore::new(&db, project_root.to_path_buf());
    let seg_id: Id = segment_id.parse().map_err(|e: water_core::Error| e.to_string())?;
    store
        .delete_user_segment(&seg_id)
        .map_err(|e| e.to_string())
}

pub async fn world_intake_schema_core(
    db: &Arc<Mutex<Db>>,
    segment_id: String,
) -> Result<IntakeSchemaSection, String> {
    let db = db.lock().await;
    let seg_id: Id = segment_id.parse().map_err(|e: water_core::Error| e.to_string())?;
    water_core::world::templates::effective_template(&db, &seg_id)
        .map_err(|e| e.to_string())
}

// === Thin Tauri command wrappers ===

#[tauri::command]
pub async fn world_segment_list(state: State<'_, OpenProject>) -> Result<Vec<WorldSegmentPayload>, String> {
    world_segment_list_core(&state.db, &state.project_root, &state.project_id).await
}

#[tauri::command]
pub async fn world_segment_create(
    state: State<'_, OpenProject>,
    req: CreateSegmentRequest,
) -> Result<String, String> {
    world_segment_create_core(&state.db, &state.project_root, &state.project_id, req).await
}

#[tauri::command]
pub async fn world_segment_update_template(
    state: State<'_, OpenProject>,
    req: UpdateTemplateRequest,
) -> Result<(), String> {
    world_segment_update_template_core(&state.db, &state.project_root, req).await
}

#[tauri::command]
pub async fn world_segment_set_hidden(
    state: State<'_, OpenProject>,
    req: SetHiddenRequest,
) -> Result<(), String> {
    world_segment_set_hidden_core(&state.db, &state.project_root, req).await
}

#[tauri::command]
pub async fn world_segment_delete(
    state: State<'_, OpenProject>,
    segment_id: String,
) -> Result<(), String> {
    world_segment_delete_core(&state.db, &state.project_root, segment_id).await
}

#[tauri::command]
pub async fn world_intake_schema(
    state: State<'_, OpenProject>,
    segment_id: String,
) -> Result<IntakeSchemaSection, String> {
    world_intake_schema_core(&state.db, segment_id).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use water_core::ProjectStore;

    fn mk(dir: &tempfile::TempDir) -> (Arc<Mutex<Db>>, std::path::PathBuf, Id) {
        let db = Db::open(dir.path().join("project.db")).unwrap();
        let p = ProjectStore::new(&db).insert("P").unwrap();
        WorldStore::new(&db, dir.path().to_path_buf()).seed_builtins(&p.id).unwrap();
        (Arc::new(Mutex::new(db)), dir.path().to_path_buf(), p.id)
    }

    #[tokio::test]
    async fn world_segment_list_returns_six_builtins() {
        let dir = tempfile::tempdir().unwrap();
        let (db, root, p) = mk(&dir);
        let segs = world_segment_list_core(&db, &root, &p).await.unwrap();
        assert_eq!(segs.len(), 6);
    }

    #[tokio::test]
    async fn world_segment_create_then_list_includes_new() {
        let dir = tempfile::tempdir().unwrap();
        let (db, root, p) = mk(&dir);
        let req = CreateSegmentRequest {
            name: "Magic".to_string(),
            is_collection: false,
            template: IntakeSchemaSection {
                id: "magic".to_string(),
                label: "Magic".to_string(),
                fields: vec![],
            },
        };
        let _id = world_segment_create_core(&db, &root, &p, req).await.unwrap();
        let segs = world_segment_list_core(&db, &root, &p).await.unwrap();
        assert_eq!(segs.len(), 7);
        assert!(segs.iter().any(|s| s.name == "Magic"));
    }

    #[tokio::test]
    async fn world_intake_schema_returns_builtin_for_concept() {
        let dir = tempfile::tempdir().unwrap();
        let (db, root, p) = mk(&dir);
        let segs = world_segment_list_core(&db, &root, &p).await.unwrap();
        let concept = segs.iter().find(|s| s.slug == "concept").unwrap();
        let schema = world_intake_schema_core(&db, concept.id.clone()).await.unwrap();
        assert!(schema.fields.iter().any(|f| f.id == "main.core_premise"));
    }
}
```

- [ ] **Step 3: Register the commands**

Modify `app/src-tauri/src/lib.rs`. Add to the existing `mod commands { ... }` and to the `invoke_handler![...]` list:

```rust
// In mod commands:
pub mod world;

// In .invoke_handler(tauri::generate_handler![...]) — append:
commands::world::world_segment_list,
commands::world::world_segment_create,
commands::world::world_segment_update_template,
commands::world::world_segment_set_hidden,
commands::world::world_segment_delete,
commands::world::world_intake_schema,
```

Note: this task only registers the segment-level commands. Single-doc + entry commands come in Tasks 9-10.

- [ ] **Step 4: Add TS types to `app/src/ipc/commands.ts`**

Locate the existing type exports and the `ipc{}` object. Append:

```typescript
// === M4 — World/Setting Bible ===

export type WorldSegment = {
  id: string;
  slug: string;
  name: string;
  ordering: number;
  is_collection: boolean;
  hue_token: string;
  hidden: boolean;
  has_template_override: boolean;
};

export type CreateSegmentRequest = {
  name: string;
  isCollection: boolean;
  template: IntakeSchemaSection;
};

export type UpdateTemplateRequest = {
  segmentId: string;
  template: IntakeSchemaSection;
};

export type SetHiddenRequest = {
  segmentId: string;
  hidden: boolean;
};
```

And in the `ipc` object, add methods (use `tauri::invoke` or whatever existing pattern). Check the existing `characterList` method for shape:

```typescript
worldSegmentList: () => invoke<WorldSegment[]>("world_segment_list"),
worldSegmentCreate: (req: CreateSegmentRequest) =>
  invoke<string>("world_segment_create", { req: { name: req.name, is_collection: req.isCollection, template: req.template } }),
worldSegmentUpdateTemplate: (req: UpdateTemplateRequest) =>
  invoke<void>("world_segment_update_template", { req: { segment_id: req.segmentId, template: req.template } }),
worldSegmentSetHidden: (req: SetHiddenRequest) =>
  invoke<void>("world_segment_set_hidden", { req: { segment_id: req.segmentId, hidden: req.hidden } }),
worldSegmentDelete: (segmentId: string) =>
  invoke<void>("world_segment_delete", { segmentId }),
worldIntakeSchema: (segmentId: string) =>
  invoke<IntakeSchemaSection>("world_intake_schema", { segmentId }),
```

Confirm `IntakeSchemaSection` is already exported from `commands.ts` (M3 added it). If not, import or re-export.

- [ ] **Step 5: Run command tests**

```powershell
cargo test -p water-app commands::world::tests -- --nocapture
```

Expected: 3 PASS.

- [ ] **Step 6: Run full suite (Rust + TS smoke)**

```powershell
cargo test -p water-app
pnpm --filter @water/app test
```

Expected: water-app +3 tests; TS suite unchanged (no new TS tests yet).

- [ ] **Step 7: Commit**

```powershell
git add app/src-tauri/src/commands/world.rs app/src-tauri/src/lib.rs app/src/ipc/commands.ts
git commit -m "feat(world): Tauri commands for segment CRUD + intake_schema"
```

---

### Task 9: `world.rs` single-doc commands

Adds the single-doc read/update commands to `world.rs`.

**Files:** Modify `app/src-tauri/src/commands/world.rs`, `app/src/ipc/commands.ts`.

- [ ] **Step 1: Write the failing core-test**

Append to `app/src-tauri/src/commands/world.rs`:

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct WorldEntryFilePayload {
    pub id: String,
    pub segment_id: String,
    pub schema_version: String,
    pub name: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(flatten)]
    pub data: serde_json::Map<String, serde_json::Value>,
}

pub async fn world_single_doc_read_core(
    db: &Arc<Mutex<Db>>,
    project_root: &std::path::Path,
    segment_id: String,
) -> Result<WorldEntryFilePayload, String> {
    let db = db.lock().await;
    let store = WorldStore::new(&db, project_root.to_path_buf());
    let seg_id: Id = segment_id.parse().map_err(|e: water_core::Error| e.to_string())?;
    let file = store.read_single_doc(&seg_id).map_err(|e| e.to_string())?;
    Ok(WorldEntryFilePayload {
        id: file.id.to_string(),
        segment_id: seg_id.to_string(),
        schema_version: file.schema_version,
        name: file.name,
        aliases: vec![],
        data: file.data,
    })
}

#[derive(Deserialize)]
pub struct UpdateSingleDocRequest {
    pub segment_id: String,
    pub field_id: String,
    pub value: serde_json::Value,
}

pub async fn world_single_doc_update_field_core(
    db: &Arc<Mutex<Db>>,
    project_root: &std::path::Path,
    req: UpdateSingleDocRequest,
) -> Result<(), String> {
    let db = db.lock().await;
    let store = WorldStore::new(&db, project_root.to_path_buf());
    let seg_id: Id = req.segment_id.parse().map_err(|e: water_core::Error| e.to_string())?;
    store
        .update_single_doc_field(&seg_id, &req.field_id, &req.value)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn world_single_doc_read(
    state: State<'_, OpenProject>,
    segment_id: String,
) -> Result<WorldEntryFilePayload, String> {
    world_single_doc_read_core(&state.db, &state.project_root, segment_id).await
}

#[tauri::command]
pub async fn world_single_doc_update_field(
    state: State<'_, OpenProject>,
    req: UpdateSingleDocRequest,
) -> Result<(), String> {
    world_single_doc_update_field_core(&state.db, &state.project_root, req).await
}

// In #[cfg(test)] mod tests:
#[tokio::test]
async fn world_single_doc_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let (db, root, p) = mk(&dir);
    let segs = world_segment_list_core(&db, &root, &p).await.unwrap();
    let concept = segs.iter().find(|s| s.slug == "concept").unwrap();

    world_single_doc_update_field_core(
        &db,
        &root,
        UpdateSingleDocRequest {
            segment_id: concept.id.clone(),
            field_id: "main.core_premise".to_string(),
            value: serde_json::json!("A test premise"),
        },
    )
    .await
    .unwrap();

    let file = world_single_doc_read_core(&db, &root, concept.id.clone()).await.unwrap();
    let main = file.data.get("main").unwrap().as_object().unwrap();
    assert_eq!(main.get("core_premise").unwrap().as_str().unwrap(), "A test premise");
}
```

- [ ] **Step 2: Register in lib.rs**

Append to the `tauri::generate_handler![...]` list:

```rust
commands::world::world_single_doc_read,
commands::world::world_single_doc_update_field,
```

- [ ] **Step 3: Add TS surface**

In `app/src/ipc/commands.ts`:

```typescript
export type WorldEntryFile = {
  id: string;
  segment_id: string;
  schema_version: string;
  name: string;
  aliases: string[];
  [key: string]: unknown;
};

// Append to ipc:
worldSingleDocRead: (segmentId: string) =>
  invoke<WorldEntryFile>("world_single_doc_read", { segmentId }),
worldSingleDocUpdateField: (req: { segmentId: string; fieldId: string; value: unknown }) =>
  invoke<void>("world_single_doc_update_field", {
    req: { segment_id: req.segmentId, field_id: req.fieldId, value: req.value },
  }),
```

- [ ] **Step 4: Test + commit**

```powershell
cargo test -p water-app commands::world -- --nocapture
git add app/src-tauri/src/commands/world.rs app/src-tauri/src/lib.rs app/src/ipc/commands.ts
git commit -m "feat(world): Tauri commands for single-doc read/update_field"
```

---

### Task 10: `world.rs` entry commands + autosuggest

Adds the collection entry commands plus `world_autosuggest`.

**Files:** Modify `app/src-tauri/src/commands/world.rs`, `app/src/ipc/commands.ts`.

- [ ] **Step 1: Write the core impls + tests**

Append to `app/src-tauri/src/commands/world.rs`:

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct WorldEntryIndexPayload {
    pub id: String,
    pub segment_id: String,
    pub name: String,
    pub preview: String,
}

pub async fn world_entry_list_core(
    db: &Arc<Mutex<Db>>,
    project_root: &std::path::Path,
    segment_id: String,
) -> Result<Vec<WorldEntryIndexPayload>, String> {
    let db = db.lock().await;
    let store = WorldStore::new(&db, project_root.to_path_buf());
    let seg_id: Id = segment_id.parse().map_err(|e: water_core::Error| e.to_string())?;
    store
        .list_entries(&seg_id)
        .map(|rows| {
            rows.into_iter()
                .map(|r| WorldEntryIndexPayload {
                    id: r.id.to_string(),
                    segment_id: r.segment_id.to_string(),
                    name: r.name,
                    preview: r.preview,
                })
                .collect()
        })
        .map_err(|e| e.to_string())
}

pub async fn world_entry_read_core(
    db: &Arc<Mutex<Db>>,
    project_root: &std::path::Path,
    entry_id: String,
) -> Result<WorldEntryFilePayload, String> {
    let db = db.lock().await;
    let store = WorldStore::new(&db, project_root.to_path_buf());
    let id: Id = entry_id.parse().map_err(|e: water_core::Error| e.to_string())?;
    let file = store.read_entry(&id).map_err(|e| e.to_string())?;
    Ok(WorldEntryFilePayload {
        id: file.id.to_string(),
        segment_id: file.segment_id.to_string(),
        schema_version: file.schema_version,
        name: file.name,
        aliases: file.aliases,
        data: file.data,
    })
}

#[derive(Deserialize)]
pub struct CreateEntryRequest {
    pub segment_id: String,
    pub name: String,
}

pub async fn world_entry_create_core(
    db: &Arc<Mutex<Db>>,
    project_root: &std::path::Path,
    req: CreateEntryRequest,
) -> Result<String, String> {
    let db = db.lock().await;
    let store = WorldStore::new(&db, project_root.to_path_buf());
    let seg_id: Id = req.segment_id.parse().map_err(|e: water_core::Error| e.to_string())?;
    store
        .create_entry(&seg_id, &req.name)
        .map(|id| id.to_string())
        .map_err(|e| e.to_string())
}

#[derive(Deserialize)]
pub struct UpdateEntryFieldRequest {
    pub entry_id: String,
    pub field_id: String,
    pub value: serde_json::Value,
}

pub async fn world_entry_update_field_core(
    db: &Arc<Mutex<Db>>,
    project_root: &std::path::Path,
    req: UpdateEntryFieldRequest,
) -> Result<(), String> {
    let db = db.lock().await;
    let store = WorldStore::new(&db, project_root.to_path_buf());
    let id: Id = req.entry_id.parse().map_err(|e: water_core::Error| e.to_string())?;
    store
        .update_entry_field(&id, &req.field_id, &req.value)
        .map_err(|e| e.to_string())
}

#[derive(Deserialize)]
pub struct UpdateAliasesRequest {
    pub entry_id: String,
    pub aliases: Vec<String>,
}

pub async fn world_entry_update_aliases_core(
    db: &Arc<Mutex<Db>>,
    project_root: &std::path::Path,
    req: UpdateAliasesRequest,
) -> Result<(), String> {
    let db = db.lock().await;
    let store = WorldStore::new(&db, project_root.to_path_buf());
    let id: Id = req.entry_id.parse().map_err(|e: water_core::Error| e.to_string())?;
    store
        .update_entry_aliases(&id, &req.aliases)
        .map_err(|e| e.to_string())
}

pub async fn world_entry_delete_if_empty_core(
    db: &Arc<Mutex<Db>>,
    project_root: &std::path::Path,
    entry_id: String,
) -> Result<bool, String> {
    let db = db.lock().await;
    let store = WorldStore::new(&db, project_root.to_path_buf());
    let id: Id = entry_id.parse().map_err(|e: water_core::Error| e.to_string())?;
    store.delete_entry_if_empty(&id).map_err(|e| e.to_string())
}

pub async fn world_entry_delete_core(
    db: &Arc<Mutex<Db>>,
    project_root: &std::path::Path,
    entry_id: String,
) -> Result<(), String> {
    let db = db.lock().await;
    let store = WorldStore::new(&db, project_root.to_path_buf());
    let id: Id = entry_id.parse().map_err(|e: water_core::Error| e.to_string())?;
    store.delete_entry(&id).map_err(|e| e.to_string())
}

#[derive(Deserialize)]
pub struct AutosuggestRequest {
    pub scene_id: String,
    pub paragraph: String,
}

pub async fn world_autosuggest_core(
    db: &Arc<Mutex<Db>>,
    project_root: &std::path::Path,
    project_id: &Id,
    req: AutosuggestRequest,
) -> Result<Vec<WorldEntryIndexPayload>, String> {
    use water_core::world::WorldRegistry;
    let db = db.lock().await;
    let reg = WorldRegistry::from_db(&db, project_id, project_root.to_path_buf())
        .map_err(|e| e.to_string())?;

    // Tokenize paragraph by whitespace + simple punctuation, lowercase, dedup.
    let tokens: std::collections::HashSet<String> = req
        .paragraph
        .split(|c: char| !c.is_alphanumeric() && c != '\'')
        .filter(|s| !s.is_empty())
        .map(str::to_lowercase)
        .collect();

    let mut hits: std::collections::HashMap<Id, WorldEntryIndexPayload> = std::collections::HashMap::new();
    for token in &tokens {
        for id in reg.find_by_token(token) {
            if let Some(snap) = reg.by_id(id) {
                // Only suggest locations-segment entries (M4 scope).
                if snap.segment_slug != "locations" {
                    continue;
                }
                hits.entry(id.clone()).or_insert_with(|| WorldEntryIndexPayload {
                    id: snap.id.to_string(),
                    segment_id: snap.segment_id.to_string(),
                    name: snap.name.clone(),
                    preview: String::new(),
                });
            }
        }
    }
    let _ = req.scene_id; // future: filter against scene state
    Ok(hits.into_values().collect())
}

// Thin wrappers — register all in lib.rs:
#[tauri::command]
pub async fn world_entry_list(state: State<'_, OpenProject>, segment_id: String) -> Result<Vec<WorldEntryIndexPayload>, String> {
    world_entry_list_core(&state.db, &state.project_root, segment_id).await
}
#[tauri::command]
pub async fn world_entry_read(state: State<'_, OpenProject>, entry_id: String) -> Result<WorldEntryFilePayload, String> {
    world_entry_read_core(&state.db, &state.project_root, entry_id).await
}
#[tauri::command]
pub async fn world_entry_create(state: State<'_, OpenProject>, req: CreateEntryRequest) -> Result<String, String> {
    world_entry_create_core(&state.db, &state.project_root, req).await
}
#[tauri::command]
pub async fn world_entry_update_field(state: State<'_, OpenProject>, req: UpdateEntryFieldRequest) -> Result<(), String> {
    world_entry_update_field_core(&state.db, &state.project_root, req).await
}
#[tauri::command]
pub async fn world_entry_update_aliases(state: State<'_, OpenProject>, req: UpdateAliasesRequest) -> Result<(), String> {
    world_entry_update_aliases_core(&state.db, &state.project_root, req).await
}
#[tauri::command]
pub async fn world_entry_delete_if_empty(state: State<'_, OpenProject>, entry_id: String) -> Result<bool, String> {
    world_entry_delete_if_empty_core(&state.db, &state.project_root, entry_id).await
}
#[tauri::command]
pub async fn world_entry_delete(state: State<'_, OpenProject>, entry_id: String) -> Result<(), String> {
    world_entry_delete_core(&state.db, &state.project_root, entry_id).await
}
#[tauri::command]
pub async fn world_autosuggest(state: State<'_, OpenProject>, req: AutosuggestRequest) -> Result<Vec<WorldEntryIndexPayload>, String> {
    world_autosuggest_core(&state.db, &state.project_root, &state.project_id, req).await
}

// Tests:
#[cfg(test)]
mod entry_tests {
    use super::*;
    use water_core::ProjectStore;

    fn mk(dir: &tempfile::TempDir) -> (Arc<Mutex<Db>>, std::path::PathBuf, Id, Id) {
        let db = Db::open(dir.path().join("project.db")).unwrap();
        let p = ProjectStore::new(&db).insert("P").unwrap();
        let store = WorldStore::new(&db, dir.path().to_path_buf());
        store.seed_builtins(&p.id).unwrap();
        let loc = store.find_segment_by_slug(&p.id, "locations").unwrap().unwrap();
        (Arc::new(Mutex::new(db)), dir.path().to_path_buf(), p.id, loc.id)
    }

    #[tokio::test]
    async fn entry_create_then_list() {
        let dir = tempfile::tempdir().unwrap();
        let (db, root, _p, seg) = mk(&dir);
        let id = world_entry_create_core(&db, &root, CreateEntryRequest {
            segment_id: seg.to_string(),
            name: "Pell".to_string(),
        }).await.unwrap();
        let list = world_entry_list_core(&db, &root, seg.to_string()).await.unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, id);
    }

    #[tokio::test]
    async fn entry_delete_if_empty_returns_true_for_blank() {
        let dir = tempfile::tempdir().unwrap();
        let (db, root, _p, seg) = mk(&dir);
        let id = world_entry_create_core(&db, &root, CreateEntryRequest {
            segment_id: seg.to_string(),
            name: "".to_string(),
        }).await.unwrap();
        let reaped = world_entry_delete_if_empty_core(&db, &root, id).await.unwrap();
        assert!(reaped);
    }

    #[tokio::test]
    async fn autosuggest_matches_alias_case_insensitive() {
        let dir = tempfile::tempdir().unwrap();
        let (db, root, p, seg) = mk(&dir);
        let id = world_entry_create_core(&db, &root, CreateEntryRequest {
            segment_id: seg.to_string(),
            name: "The Pell Library".to_string(),
        }).await.unwrap();
        world_entry_update_aliases_core(&db, &root, UpdateAliasesRequest {
            entry_id: id.clone(),
            aliases: vec!["Pell".to_string()],
        }).await.unwrap();

        let hits = world_autosuggest_core(&db, &root, &p, AutosuggestRequest {
            scene_id: "noop".to_string(),
            paragraph: "She walked past pell at dusk.".to_string(),
        }).await.unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, id);
    }
}
```

- [ ] **Step 2: Register in lib.rs**

Append to `tauri::generate_handler![...]`:

```rust
commands::world::world_entry_list,
commands::world::world_entry_read,
commands::world::world_entry_create,
commands::world::world_entry_update_field,
commands::world::world_entry_update_aliases,
commands::world::world_entry_delete_if_empty,
commands::world::world_entry_delete,
commands::world::world_autosuggest,
```

- [ ] **Step 3: Add TS surface**

Append to `app/src/ipc/commands.ts`:

```typescript
export type WorldEntryIndexEntry = {
  id: string;
  segment_id: string;
  name: string;
  preview: string;
};

// ipc methods:
worldEntryList: (segmentId: string) =>
  invoke<WorldEntryIndexEntry[]>("world_entry_list", { segmentId }),
worldEntryRead: (entryId: string) =>
  invoke<WorldEntryFile>("world_entry_read", { entryId }),
worldEntryCreate: (req: { segmentId: string; name: string }) =>
  invoke<string>("world_entry_create", { req: { segment_id: req.segmentId, name: req.name } }),
worldEntryUpdateField: (req: { entryId: string; fieldId: string; value: unknown }) =>
  invoke<void>("world_entry_update_field", {
    req: { entry_id: req.entryId, field_id: req.fieldId, value: req.value },
  }),
worldEntryUpdateAliases: (req: { entryId: string; aliases: string[] }) =>
  invoke<void>("world_entry_update_aliases", { req: { entry_id: req.entryId, aliases: req.aliases } }),
worldEntryDeleteIfEmpty: (entryId: string) =>
  invoke<boolean>("world_entry_delete_if_empty", { entryId }),
worldEntryDelete: (entryId: string) =>
  invoke<void>("world_entry_delete", { entryId }),
worldAutosuggest: (req: { sceneId: string; paragraph: string }) =>
  invoke<WorldEntryIndexEntry[]>("world_autosuggest", { req: { scene_id: req.sceneId, paragraph: req.paragraph } }),
```

- [ ] **Step 4: Test + commit**

```powershell
cargo test -p water-app commands::world -- --nocapture
git add app/src-tauri/src/commands/world.rs app/src-tauri/src/lib.rs app/src/ipc/commands.ts
git commit -m "feat(world): Tauri commands for entry CRUD + autosuggest"
```

---

### Task 11: `scene.rs` `sceneSetLocation` + extended `sceneReadMetadata`

Extends the M3 T21 `scene_read_metadata` to include the scene's location, and adds `scene_set_location` for setting/clearing `scene.location_id`.

**Files:** Modify `app/src-tauri/src/commands/scene.rs`, `app/src/ipc/commands.ts`.

- [ ] **Step 1: Inspect current scene_read_metadata**

```powershell
Select-String -Path "app\src-tauri\src\commands\scene.rs" -Pattern "scene_read_metadata"
```

Read the function and its return struct (likely `SceneMetadata`).

- [ ] **Step 2: Write the failing test + extend the struct**

In `app/src-tauri/src/commands/scene.rs`, find the `SceneMetadata` struct and add:

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct SceneLocationPayload {
    pub id: String,
    pub name: String,
    pub segment_slug: String,
}

// In SceneMetadata struct:
pub location: Option<SceneLocationPayload>,
```

Append to the test module:

```rust
#[tokio::test]
async fn scene_set_location_updates_db_and_metadata() {
    use water_core::{world::WorldStore, ProjectStore};
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open(dir.path().join("project.db")).unwrap();
    let p = ProjectStore::new(&db).insert("P").unwrap();
    let store = WorldStore::new(&db, dir.path().to_path_buf());
    store.seed_builtins(&p.id).unwrap();
    let loc_seg = store.find_segment_by_slug(&p.id, "locations").unwrap().unwrap();
    let entry_id = store.create_entry(&loc_seg.id, "Pell").unwrap();
    // ... create scene; depends on existing scene-creation helper ...

    let db = Arc::new(Mutex::new(db));
    scene_set_location_core(&db, &scene_id.to_string(), Some(&entry_id.to_string())).await.unwrap();

    let meta = scene_read_metadata_core(&db, &scene_id.to_string()).await.unwrap();
    let loc = meta.location.expect("location populated");
    assert_eq!(loc.name, "Pell");
    assert_eq!(loc.segment_slug, "locations");

    scene_set_location_core(&db, &scene_id.to_string(), None).await.unwrap();
    let meta2 = scene_read_metadata_core(&db, &scene_id.to_string()).await.unwrap();
    assert!(meta2.location.is_none());
}
```

(Adapt scene-creation per the existing scene-store API.)

- [ ] **Step 3: Implement scene_set_location_core + extend scene_read_metadata_core**

Add to `scene.rs`:

```rust
pub async fn scene_set_location_core(
    db: &Arc<Mutex<Db>>,
    scene_id: &str,
    location_id: Option<&str>,
) -> Result<(), String> {
    let db = db.lock().await;
    match location_id {
        Some(loc) => {
            db.conn().execute(
                "UPDATE scene SET location_id = ?1 WHERE id = ?2",
                (loc, scene_id),
            )
        }
        None => {
            db.conn().execute(
                "UPDATE scene SET location_id = NULL WHERE id = ?1",
                [scene_id],
            )
        }
    }
    .map(|_| ())
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn scene_set_location(
    state: State<'_, OpenProject>,
    scene_id: String,
    location_id: Option<String>,
) -> Result<(), String> {
    scene_set_location_core(&state.db, &scene_id, location_id.as_deref()).await
}
```

In `scene_read_metadata_core`, add a JOIN to populate `location`:

```rust
let location: Option<SceneLocationPayload> = {
    let row: Option<(String, String, String)> = db.conn().query_row(
        "SELECT we.id, we.name, ws.slug
         FROM scene s
         LEFT JOIN world_entry we ON we.id = s.location_id
         LEFT JOIN world_segment ws ON ws.id = we.segment_id
         WHERE s.id = ?1 AND s.location_id IS NOT NULL",
        [scene_id],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    ).optional().map_err(|e| e.to_string())?;
    row.map(|(id, name, slug)| SceneLocationPayload {
        id,
        name,
        segment_slug: slug,
    })
};
```

- [ ] **Step 4: Register command + update TS**

Add `commands::scene::scene_set_location` to `lib.rs`. In TS:

```typescript
// Extend SceneMetadata:
export type SceneMetadata = {
  // ... existing fields ...
  location: { id: string; name: string; segment_slug: string } | null;
};

sceneSetLocation: (req: { sceneId: string; locationId: string | null }) =>
  invoke<void>("scene_set_location", { sceneId: req.sceneId, locationId: req.locationId }),
```

- [ ] **Step 5: Test + commit**

```powershell
cargo test -p water-app commands::scene -- --nocapture
git add app/src-tauri/src/commands/scene.rs app/src-tauri/src/lib.rs app/src/ipc/commands.ts
git commit -m "feat(scene): scene_set_location + scene_read_metadata returns location"
```

---

### Task 12: `state.rs` `WorldWriteLocks` + `cargo fmt` pass

Adds the per-entry write-lock DashMap to `OpenProject` (parallel to M3's `CharacterWriteLocks`) and runs the M3-polish #12 fmt-pass on `commands/*.rs`.

**Files:** Modify `app/src-tauri/src/state.rs`, then `cargo fmt -p water-app`.

- [ ] **Step 1: Inspect state.rs**

```powershell
Get-Content app\src-tauri\src\state.rs
```

Note the `SceneWriteLocks` and `CharacterWriteLocks` declarations.

- [ ] **Step 2: Add WorldWriteLocks**

```rust
pub type WorldWriteLocks = DashMap<water_core::Id, std::sync::Arc<tokio::sync::Mutex<()>>>;

pub struct OpenProject {
    pub db: Arc<Mutex<Db>>,
    pub project_root: PathBuf,
    pub project_id: water_core::Id,
    pub scene_write_locks: SceneWriteLocks,
    pub character_write_locks: CharacterWriteLocks,
    pub world_write_locks: WorldWriteLocks,   // NEW
    // ... existing fields ...
}
```

In the constructor (`OpenProject::new` or wherever the struct is built):

```rust
world_write_locks: DashMap::new(),
```

- [ ] **Step 3: Run fmt-pass**

```powershell
cargo fmt -p water-app
```

This applies the M3-polish #12 fmt drift fix. Inspect the diff:

```powershell
git diff --stat
```

Expected: whitespace-only changes to `commands/*.rs` + `state.rs` + `orchestrator_service.rs`.

- [ ] **Step 4: Verify build + test**

```powershell
cargo build -p water-app
cargo test -p water-app
```

Expected: clean build + green tests.

- [ ] **Step 5: Commit**

```powershell
git add app/src-tauri/src/state.rs app/src-tauri/src/commands/ app/src-tauri/src/orchestrator_service.rs
git commit -m "chore(app): WorldWriteLocks on OpenProject + cargo fmt pass (M3 polish #12)"
```

**Phase B close-out.** Tauri command surface complete. Next: orchestrator wiring for the trigger + speaker pipeline.

---

## Phase C — Orchestrator wiring

### Task 13: `OrchestratorContext` gains `world_registry` + service builds it

Threads `WorldRegistry` through `OrchestratorContext` and ensures `orchestrator_service.rs` builds it once per dispatch.

**Files:**
- Modify: `crates/water-core/src/orchestrator/mod.rs`
- Modify: `app/src-tauri/src/orchestrator_service.rs`

- [ ] **Step 1: Inspect current OrchestratorContext**

```powershell
Select-String -Path "crates\water-core\src\orchestrator\mod.rs" -Pattern "struct OrchestratorContext|character_registry"
```

Note the existing `character_registry: &CharacterRegistry` field.

- [ ] **Step 2: Add `world_registry` field**

In `orchestrator/mod.rs`:

```rust
use crate::world::WorldRegistry;

pub struct OrchestratorContext<'a> {
    pub scene: &'a SceneSnapshot,
    pub project: &'a ProjectSnapshot,
    pub character_registry: &'a CharacterRegistry,
    pub world_registry: &'a WorldRegistry,   // NEW
    // ... existing fields ...
}
```

- [ ] **Step 3: Update all OrchestratorContext construction sites**

Every place that constructs `OrchestratorContext` must now pass a `&WorldRegistry`. Search:

```powershell
Select-String -Path "crates\water-core\src","app\src-tauri\src" -Pattern "OrchestratorContext\s*\{" -Recurse
```

Each callsite gets a `&world_registry` parameter. For test sites that don't care about world data, construct an empty registry:

```rust
let world_reg = WorldRegistry::from_db(&db, &project_id, project_root)
    .unwrap_or_else(|_| WorldRegistry::default());   // empty if not seeded
```

If `WorldRegistry::default()` doesn't exist, add one to `world/registry.rs`:

```rust
impl Default for WorldRegistry {
    fn default() -> Self {
        Self {
            by_id: std::collections::HashMap::new(),
            by_name_or_alias: std::collections::HashMap::new(),
            segments: std::collections::HashMap::new(),
            by_segment_slug: std::collections::HashMap::new(),
        }
    }
}
```

- [ ] **Step 4: Update `orchestrator_service.rs` to build world_registry once per dispatch**

In `app/src-tauri/src/orchestrator_service.rs`, find where `CharacterRegistry::from_db` is called per dispatch. Add a parallel `WorldRegistry::from_db` call right after:

```rust
let char_reg = CharacterRegistry::from_db(&db, &project_id)?;
let world_reg = WorldRegistry::from_db(&db, &project_id, project_root.clone())?;
// ... construct OrchestratorContext { ..., world_registry: &world_reg, ... } ...
```

- [ ] **Step 5: Write a smoke test for context threading**

In `crates/water-core/src/orchestrator/mod.rs` tests:

```rust
#[test]
fn orchestrator_context_carries_world_registry() {
    let dir = tempfile::tempdir().unwrap();
    let db = crate::Db::open_in_memory().unwrap();
    let p = crate::ProjectStore::new(&db).insert("P").unwrap();
    crate::world::WorldStore::new(&db, dir.path().to_path_buf())
        .seed_builtins(&p.id)
        .unwrap();
    let world_reg = crate::world::WorldRegistry::from_db(&db, &p.id, dir.path().to_path_buf()).unwrap();
    let char_reg = crate::character::CharacterRegistry::default();
    let scene = SceneSnapshot::default();
    let project = ProjectSnapshot::default();
    let ctx = OrchestratorContext {
        scene: &scene,
        project: &project,
        character_registry: &char_reg,
        world_registry: &world_reg,
    };
    let count = ctx.world_registry.segments().count();
    assert_eq!(count, 6);
}
```

(Adapt `SceneSnapshot::default` / `ProjectSnapshot::default` per the existing types — add `Default` derives if missing.)

- [ ] **Step 6: Run tests + commit**

```powershell
cargo test -p water-core orchestrator
cargo test -p water-app
git add crates/water-core/src/orchestrator/ crates/water-core/src/world/registry.rs app/src-tauri/src/orchestrator_service.rs
git commit -m "feat(orch): OrchestratorContext gains world_registry; service builds per dispatch"
```

---

### Task 14: Voice router `WORLD_TRACK_TRIGGERS` + `CartographerSpeaker` registration

Registers the Cartographer as a `PersonaSpeaker` variant and wires the world-track branch in the voice router. The Cartographer template TOML itself is authored in Task 18 — this task creates the Speaker registration plumbing only.

**Files:**
- Create: `crates/water-core/src/voice/cartographer_template.rs`
- Modify: `crates/water-core/src/voice/router.rs`
- Modify: `crates/water-core/src/voice/mod.rs`

- [ ] **Step 1: Inspect existing persona speakers**

```powershell
Get-Content crates\water-core\src\voice\mod.rs | Select-Object -First 60
Select-String -Path "crates\water-core\src\voice\*.rs" -Pattern "PersonaSpeaker|echo|architect|editor|chorus"
```

Identify where the 5 existing personas are registered.

- [ ] **Step 2: Add `WORLD_TRACK_TRIGGERS` const**

In `crates/water-core/src/voice/router.rs`:

```rust
pub const WORLD_TRACK_TRIGGERS: &[&str] = &["world_drift"];
```

Extend the `select_speaker` (or whatever the existing dispatch fn is called) with:

```rust
pub fn select_speaker(
    trigger_id: &str,
    candidate: &TriggerCandidate,
    scene: &SceneSnapshot,
    char_registry: &CharacterRegistry,
    world_registry: &WorldRegistry,
) -> SpeakerHandle {
    if CHAR_TRACK_TRIGGERS.contains(&trigger_id) {
        return select_char_speaker_with_pov_prefer(/* existing args */);
    }
    if WORLD_TRACK_TRIGGERS.contains(&trigger_id) {
        return SpeakerHandle::Persona("cartographer");
    }
    // ... existing persona rotation ...
}
```

If the existing signature differs, adapt — the principle is: trigger_id check happens BEFORE the persona-rotation fallback.

- [ ] **Step 3: Register Cartographer in the persona enum/lookup**

Wherever the 5 existing personas are listed (likely a `const PERSONAS: &[&str] = &["echo", "architect", "editor", "chorus", "cartographer"]` or a match arm), add `"cartographer"`. If a persona-template file is loaded by slug, the loader will look for `prompts/speakers/cartographer/template.toml` — that file is authored in Task 18; Step 4 below creates a stub.

- [ ] **Step 4: Create a stub Cartographer template**

```powershell
New-Item -ItemType Directory -Path "prompts\speakers\cartographer" -Force
```

Create `prompts/speakers/cartographer/template.toml` with a minimal placeholder so the Speaker registration test passes (the real template lands in Task 18):

```toml
schema_version = 1
persona_slug = "cartographer"
hue_token = "--water-hue-persona-cartographer"

[clauses]
tone_clause = "Placeholder — real Cartographer tone lands in Task 18."

[user]
template = "Cartographer placeholder for {{trigger_kind}}."
```

- [ ] **Step 5: Write the failing test**

In `crates/water-core/src/voice/router.rs` tests:

```rust
#[test]
fn world_drift_routes_to_cartographer() {
    let scene = SceneSnapshot::default();
    let char_reg = crate::character::CharacterRegistry::default();
    let world_reg = crate::world::WorldRegistry::default();
    let candidate = TriggerCandidate {
        trigger_id: "world_drift",
        // ... minimal viable construction ...
    };
    let handle = select_speaker("world_drift", &candidate, &scene, &char_reg, &world_reg);
    assert_eq!(handle, SpeakerHandle::Persona("cartographer"));
}

#[test]
fn world_track_triggers_does_not_collide_with_char_track() {
    for t in WORLD_TRACK_TRIGGERS {
        assert!(!CHAR_TRACK_TRIGGERS.contains(t), "{t} appears in both tracks");
    }
}
```

- [ ] **Step 6: Run, fix, commit**

```powershell
cargo test -p water-core voice -- --nocapture
git add crates/water-core/src/voice/ prompts/speakers/cartographer/
git commit -m "feat(voice): WORLD_TRACK_TRIGGERS + cartographer PersonaSpeaker registration"
```

**Phase C close-out.** Orchestrator now carries world data; voice router knows to dispatch world_drift to Cartographer. Phase D builds the trigger and the real prompts.

---

## Phase D — `world_drift` trigger

### Task 15: `collision::resolve_token_kind` shared helper

Centralizes the character-vs-world collision-resolution policy from spec § 6.2. Used by both `world_drift` Stage 1 (Task 16) and `SceneAutosuggestChips` (Task 28).

**Files:**
- Create: `crates/water-core/src/world/collision.rs`
- Modify: `crates/water-core/src/world/mod.rs` (add `pub mod collision;`)

- [ ] **Step 1: Write the failing tests**

Create `crates/water-core/src/world/collision.rs`:

```rust
//! Character-vs-world name-collision resolution policy.
//!
//! Policy (M4 spec § 6.2, derived from open-question Q8):
//! - If a token matches both a character and a world entry, AND the
//!   character is already in `scene.characters_present`, suppress the
//!   world match (return `CharacterOnly`).
//! - Otherwise: both fire independently (`BothFire`), letting downstream
//!   surfaces (autosuggest chips, world_drift) decide what to do.
//! - Character-only and world-only matches pass through unchanged.

use crate::{character::CharacterRegistry, world::WorldRegistry, Id};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    CharacterOnly(Id),
    WorldOnly(Vec<Id>),
    BothFire { character_id: Id, world_ids: Vec<Id> },
    Neither,
}

pub fn resolve_token_kind(
    token: &str,
    char_registry: &CharacterRegistry,
    world_registry: &WorldRegistry,
    characters_present: &[Id],
) -> TokenKind {
    let char_match = char_registry.find_by_name(token);
    let world_matches: Vec<Id> = world_registry.find_by_token(&token.to_lowercase()).to_vec();
    match (char_match, world_matches.is_empty()) {
        (Some(c), false) if characters_present.contains(&c.id) => TokenKind::CharacterOnly(c.id),
        (Some(c), false) => TokenKind::BothFire {
            character_id: c.id,
            world_ids: world_matches,
        },
        (Some(c), true) => TokenKind::CharacterOnly(c.id),
        (None, false) => TokenKind::WorldOnly(world_matches),
        (None, true) => TokenKind::Neither,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{world::WorldStore, Db, ProjectStore};

    fn setup() -> (
        tempfile::TempDir,
        Db,
        Id,
        Id, // character_id
        Id, // world_entry_id
    ) {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        let p = ProjectStore::new(&db).insert("P").unwrap();
        let store = WorldStore::new(&db, dir.path().to_path_buf());
        store.seed_builtins(&p.id).unwrap();
        let loc = store.find_segment_by_slug(&p.id, "locations").unwrap().unwrap();
        let world_id = store.create_entry(&loc.id, "Aren").unwrap();

        // Use CharacterStore to create a character named Aren.
        let char_id = crate::character::CharacterStore::new(&db, dir.path().to_path_buf())
            .create("Aren")
            .unwrap();

        (dir, db, p.id, char_id, world_id)
    }

    #[test]
    fn character_in_scene_wins() {
        let (dir, db, p, c, _w) = setup();
        let char_reg = crate::character::CharacterRegistry::from_db(&db, &p).unwrap();
        let world_reg = crate::world::WorldRegistry::from_db(&db, &p, dir.path().to_path_buf()).unwrap();
        let result = resolve_token_kind("Aren", &char_reg, &world_reg, &[c.clone()]);
        assert_eq!(result, TokenKind::CharacterOnly(c));
    }

    #[test]
    fn both_fire_when_character_not_present() {
        let (dir, db, p, c, w) = setup();
        let char_reg = crate::character::CharacterRegistry::from_db(&db, &p).unwrap();
        let world_reg = crate::world::WorldRegistry::from_db(&db, &p, dir.path().to_path_buf()).unwrap();
        let result = resolve_token_kind("Aren", &char_reg, &world_reg, &[]);
        assert_eq!(
            result,
            TokenKind::BothFire {
                character_id: c,
                world_ids: vec![w],
            }
        );
    }

    #[test]
    fn world_only_when_no_character_match() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        let p = ProjectStore::new(&db).insert("P").unwrap();
        let store = WorldStore::new(&db, dir.path().to_path_buf());
        store.seed_builtins(&p.id).unwrap();
        let loc = store.find_segment_by_slug(&p.id, "locations").unwrap().unwrap();
        let w = store.create_entry(&loc.id, "Pell").unwrap();

        let char_reg = crate::character::CharacterRegistry::default();
        let world_reg = crate::world::WorldRegistry::from_db(&db, &p.id, dir.path().to_path_buf()).unwrap();
        let result = resolve_token_kind("pell", &char_reg, &world_reg, &[]);
        assert_eq!(result, TokenKind::WorldOnly(vec![w]));
    }

    #[test]
    fn neither_when_no_match_anywhere() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open_in_memory().unwrap();
        let p = ProjectStore::new(&db).insert("P").unwrap();
        let char_reg = crate::character::CharacterRegistry::default();
        let world_reg = crate::world::WorldRegistry::from_db(&db, &p.id, dir.path().to_path_buf()).unwrap();
        let result = resolve_token_kind("Nonexistent", &char_reg, &world_reg, &[]);
        assert_eq!(result, TokenKind::Neither);
    }
}
```

In `world/mod.rs`: `pub mod collision;`.

- [ ] **Step 2: Verify CharacterRegistry has find_by_name + Default**

```powershell
Select-String -Path "crates\water-core\src\character\*.rs" -Pattern "fn find_by_name|impl Default for CharacterRegistry"
```

If `find_by_name` returns a different shape (e.g. `&CharacterSnapshot` with an `.id` field), adapt the helper signature. If `Default` is missing, add it.

- [ ] **Step 3: Run + commit**

```powershell
cargo test -p water-core world::collision
git add crates/water-core/src/world/collision.rs crates/water-core/src/world/mod.rs
git commit -m "feat(world): resolve_token_kind collision helper (character-in-scene wins)"
```

---

### Task 16: `WorldDriftEvaluator` Stage 1 (name+alias scan + contextual-overlap + cooldown)

The trigger evaluator. Emits `TriggerCandidate { requires_confirmation: true, ... }` only when (a) the paragraph is ≥ 12 words, (b) the matched entry's attributes have ≥ 2 content-word overlap with the paragraph, (c) the per-(entry, scene) cooldown hasn't expired.

**Files:**
- Create: `crates/water-core/src/orchestrator/triggers/world_drift.rs`
- Modify: `crates/water-core/src/orchestrator/triggers/mod.rs`
- Modify: `crates/water-core/src/orchestrator/mod.rs` (extend `ConfirmationRequest` enum + `TriggerCandidate` payload variants)

- [ ] **Step 1: Inspect M3 character_dissonance for pattern reference**

```powershell
Get-Content crates\water-core\src\orchestrator\triggers\character_dissonance.rs | Select-Object -First 60
```

Note the `requires_confirmation: true` + `ConfirmationRequest` variant + per-(target, scene) cooldown key shape.

- [ ] **Step 2: Add ConfirmationRequest::WorldDrift + PillSeed::WorldDrift variants**

In `crates/water-core/src/orchestrator/mod.rs`:

```rust
pub enum ConfirmationRequest {
    // ... existing variants (CharacterDissonance, etc.) ...
    WorldDrift {
        entry_id: Id,
        matched_token: String,
        paragraph: String,
    },
}

pub enum PillSeed {
    // ... existing variants ...
    WorldDrift {
        entry_name: String,
        segment_slug: String,
    },
}
```

- [ ] **Step 3: Author the evaluator + tests**

Create `crates/water-core/src/orchestrator/triggers/world_drift.rs`:

```rust
//! `world_drift` — Stage 1 detector for scene-vs-bible contradictions.
//!
//! Three-stage gate:
//!   1. Token match: case-insensitive `\b`-bounded word match against
//!      `world_entry.name` + aliases.
//!   2. Contextual-overlap pre-check: ≥ MIN_CONTEXT_OVERLAP_WORDS shared
//!      content words between paragraph and entry's [main] text (minus
//!      stopwords).
//!   3. LLM confirmation: emitted via `requires_confirmation: true`;
//!      runs the `pill_world_drift_check.toml` prompt elsewhere.
//!
//! Cooldown: per-(entry_id, scene_id) at WORLD_DRIFT_COOLDOWN_MS.

use crate::{
    orchestrator::{
        ConfirmationRequest, OrchestratorContext, PillSeed, SpeakerHint, TriggerCandidate,
        TriggerEvaluator,
    },
    world::WorldEntrySnapshot,
};

pub const MIN_PARAGRAPH_WORDS: usize = 12;
pub const MIN_CONTEXT_OVERLAP_WORDS: usize = 2;
pub const WORLD_DRIFT_COOLDOWN_MS: i64 = 180_000;

pub struct WorldDriftEvaluator;

impl TriggerEvaluator for WorldDriftEvaluator {
    fn id(&self) -> &'static str {
        "world_drift"
    }

    fn evaluate(&self, ctx: &OrchestratorContext) -> Vec<TriggerCandidate> {
        let paragraph = ctx.scene.recent_paragraph_text();
        if word_count(paragraph) < MIN_PARAGRAPH_WORDS {
            return vec![];
        }

        let paragraph_tokens = tokenize_words(paragraph);
        let mut out = vec![];

        for token in &paragraph_tokens {
            let lower = token.to_lowercase();
            for entry_id in ctx.world_registry.find_by_token(&lower) {
                let Some(entry) = ctx.world_registry.by_id(entry_id) else {
                    continue;
                };

                // Collision: character-in-scene wins.
                use crate::world::collision::{resolve_token_kind, TokenKind};
                match resolve_token_kind(
                    token,
                    ctx.character_registry,
                    ctx.world_registry,
                    &ctx.scene.characters_present,
                ) {
                    TokenKind::CharacterOnly(_) | TokenKind::Neither => continue,
                    _ => {}
                }

                if !has_contextual_overlap(&paragraph_tokens, entry, MIN_CONTEXT_OVERLAP_WORDS) {
                    continue;
                }

                if ctx.is_cooled_down("world_drift", &entry.id, &ctx.scene.id, WORLD_DRIFT_COOLDOWN_MS) {
                    continue;
                }

                out.push(TriggerCandidate {
                    trigger_id: "world_drift",
                    speaker_hint: SpeakerHint::Persona("cartographer"),
                    requires_confirmation: true,
                    confirmation: Some(ConfirmationRequest::WorldDrift {
                        entry_id: entry.id.clone(),
                        matched_token: token.to_string(),
                        paragraph: paragraph.to_string(),
                    }),
                    pill_seed_payload: PillSeed::WorldDrift {
                        entry_name: entry.name.clone(),
                        segment_slug: entry.segment_slug.clone(),
                    },
                });
            }
        }

        out
    }
}

fn word_count(s: &str) -> usize {
    s.split_whitespace().count()
}

/// Tokenize on word boundaries, dropping punctuation. Mirrors M3 lemma-gate.
pub fn tokenize_words(s: &str) -> Vec<String> {
    s.split(|c: char| !c.is_alphanumeric() && c != '\'')
        .filter(|t| !t.is_empty())
        .map(str::to_string)
        .collect()
}

/// Returns true iff ≥ `min` content-word overlap (minus stopwords) between
/// the paragraph and the entry's main-section text.
fn has_contextual_overlap(
    paragraph_tokens: &[String],
    entry: &WorldEntrySnapshot,
    min: usize,
) -> bool {
    let stopwords: std::collections::HashSet<&str> = [
        "the", "a", "an", "of", "to", "in", "on", "at", "by", "for", "with",
        "and", "or", "but", "is", "are", "was", "were", "be", "been", "being",
        "this", "that", "these", "those", "it", "its", "he", "she", "they", "her", "his", "their",
    ]
    .into_iter()
    .collect();

    let entry_main = entry.data.get("main").and_then(|v| v.as_object()).cloned().unwrap_or_default();
    let mut entry_words: std::collections::HashSet<String> = std::collections::HashSet::new();
    for v in entry_main.values() {
        if let Some(s) = v.as_str() {
            for w in tokenize_words(s) {
                let lw = w.to_lowercase();
                if !stopwords.contains(lw.as_str()) && lw.len() >= 3 {
                    entry_words.insert(lw);
                }
            }
        }
    }
    let mut overlap = 0;
    for t in paragraph_tokens {
        let lt = t.to_lowercase();
        if entry_words.contains(&lt) {
            overlap += 1;
            if overlap >= min {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    // Tests require construction of OrchestratorContext, scene snapshot,
    // and is_cooled_down stub. Use a test helper that builds a minimal
    // OrchestratorContext with a populated WorldRegistry + a paragraph.

    #[test]
    fn word_count_counts_whitespace_words() {
        assert_eq!(word_count("hello world"), 2);
        assert_eq!(word_count("  one  two  three  "), 3);
    }

    #[test]
    fn tokenize_drops_punctuation() {
        let v = tokenize_words("She walked past Pell, then onward.");
        assert_eq!(v, vec!["She", "walked", "past", "Pell", "then", "onward"]);
    }

    #[test]
    fn contextual_overlap_returns_true_at_threshold() {
        let mut data = serde_json::Map::new();
        let mut main = serde_json::Map::new();
        main.insert(
            "sensory_detail".to_string(),
            serde_json::json!("Dust thick enough to read fingertips in the sub-basement"),
        );
        data.insert("main".to_string(), serde_json::Value::Object(main));
        let entry = WorldEntrySnapshot {
            id: crate::Id::new(),
            segment_id: crate::Id::new(),
            segment_slug: "locations".to_string(),
            name: "Pell".to_string(),
            aliases: vec![],
            data: serde_json::Value::Object(data),
        };
        let paragraph = tokenize_words("She saw the dust on the fingertips in the sub-basement.");
        assert!(has_contextual_overlap(&paragraph, &entry, 2));
    }

    #[test]
    fn contextual_overlap_returns_false_below_threshold() {
        let mut data = serde_json::Map::new();
        let mut main = serde_json::Map::new();
        main.insert(
            "sensory_detail".to_string(),
            serde_json::json!("Dust thick enough to read fingertips in"),
        );
        data.insert("main".to_string(), serde_json::Value::Object(main));
        let entry = WorldEntrySnapshot {
            id: crate::Id::new(),
            segment_id: crate::Id::new(),
            segment_slug: "locations".to_string(),
            name: "Pell".to_string(),
            aliases: vec![],
            data: serde_json::Value::Object(data),
        };
        let paragraph = tokenize_words("She walked past quickly.");
        assert!(!has_contextual_overlap(&paragraph, &entry, 2));
    }

    // Full evaluator integration tests live alongside the rest of the
    // orchestrator integration tests where a real OrchestratorContext can
    // be built with all dependencies (scene snapshot, project snapshot,
    // is_cooled_down implementation, etc.).
}
```

Note: the `ctx.is_cooled_down(...)` API may differ in the actual codebase. Search:

```powershell
Select-String -Path "crates\water-core\src\orchestrator" -Pattern "is_cooled_down|cooldown"
```

If the cooldown API takes different args (e.g. a single key string), adapt the call signature in this task — the principle (per-(entry_id, scene_id) keying) must be preserved.

- [ ] **Step 4: Register the evaluator**

In `crates/water-core/src/orchestrator/triggers/mod.rs`:

```rust
pub mod world_drift;

pub fn standard_evaluators() -> Vec<Box<dyn TriggerEvaluator>> {
    vec![
        // ... existing entries ...
        Box::new(world_drift::WorldDriftEvaluator),
    ]
}
```

- [ ] **Step 5: Test + commit**

```powershell
cargo test -p water-core world_drift
git add crates/water-core/src/orchestrator/
git commit -m "feat(world): world_drift Stage 1 evaluator (name+alias scan + overlap + cooldown)"
```

---

### Task 17: `pill_world_drift_check.toml` + Stage 2 confirmation handler

Authors the LLM confirmation prompt and wires it into the orchestrator's confirmation dispatcher.

**Files:**
- Create: `prompts/pill_world_drift_check.toml`
- Modify: `crates/water-core/src/prompts/loader.rs` or `mod.rs` (register new prompt slug)
- Modify: orchestrator confirmation runner (wherever `pill_dissonance_check.toml` is dispatched today)

- [ ] **Step 1: Inspect M3 confirmation dispatch**

```powershell
Select-String -Path "crates\water-core\src" -Pattern "pill_dissonance_check|ConfirmationPrompt|render_confirmation_request" -Recurse
```

Note where `pill_dissonance_check.toml` is loaded and how the verdict is parsed.

- [ ] **Step 2: Author the prompt**

Create `prompts/pill_world_drift_check.toml`:

```toml
schema_version = 1
purpose = "Detect contradictions between a scene paragraph and an established world-bible entry."

[response_schema]
fields = ["verdict", "reason"]
verdict_values = ["contradicts", "consistent", "unclear"]

[system]
template = """
You are a precise consistency checker for a writer's world bible. You will be shown one paragraph of fiction and one world-bible entry the paragraph references. Decide whether the paragraph contradicts the established entry.

Respond ONLY in JSON: {"verdict": "contradicts" | "consistent" | "unclear", "reason": "<one short sentence>"}.

You are NOT giving writing advice. You are NOT suggesting changes. You report the factual relationship between the paragraph and the entry. The "reason" field is one neutral sentence stating which attribute is in tension (if any) — never instructive, never suggestive.
"""

[user]
template = """
World-bible entry — {{segment_slug}} — "{{entry_name}}":
{{entry_excerpt}}

Scene paragraph:
{{paragraph}}

The paragraph mentions "{{matched_token}}", which refers to this entry. Is the paragraph consistent with the entry, or does it contradict it?
"""
```

- [ ] **Step 3: Add the rendering helper**

In `crates/water-core/src/prompts/loader.rs` (or wherever the M3 confirmation prompt is rendered):

```rust
pub fn render_world_drift_check(
    segment_slug: &str,
    entry_name: &str,
    entry_excerpt: &str,
    paragraph: &str,
    matched_token: &str,
) -> Result<RenderedPrompt> {
    // Load prompts/pill_world_drift_check.toml (use the same loading path
    // that pill_dissonance_check uses). Replace tokens. Return RenderedPrompt.
    // The M3 helper render_confirmation_request can likely be reused if it
    // accepts an arbitrary token map.
}
```

If the M3 helper is generic (takes a `HashMap<String, String>` of tokens), reuse it with the appropriate key set rather than authoring a new fn.

- [ ] **Step 4: Wire the dispatcher**

Find the confirmation dispatcher (in `orchestrator_service.rs` or a sibling). Add a branch:

```rust
match candidate.confirmation {
    Some(ConfirmationRequest::CharacterDissonance { .. }) => { /* existing */ }
    Some(ConfirmationRequest::WorldDrift {
        ref entry_id,
        ref matched_token,
        ref paragraph,
    }) => {
        let snap = ctx.world_registry.by_id(entry_id).ok_or("entry vanished")?;
        let excerpt = render_entry_excerpt(snap);   // [main] block as key:value lines, capped ~400 tokens
        let prompt = render_world_drift_check(
            &snap.segment_slug,
            &snap.name,
            &excerpt,
            paragraph,
            matched_token,
        )?;
        let resp = llm_call_json(&prompt).await?;
        let verdict = resp["verdict"].as_str().unwrap_or("unclear");
        match verdict {
            "contradicts" => /* promote to pill via existing pipeline */,
            "consistent" | "unclear" => {
                tracing::debug!(
                    "world_drift dropped: entry={} verdict={verdict} reason={}",
                    snap.name,
                    resp["reason"].as_str().unwrap_or("")
                );
                /* drop, no pill */
            }
            _ => /* drop */,
        }
    }
    None => { /* existing */ }
}
```

`render_entry_excerpt` is a small helper:

```rust
fn render_entry_excerpt(snap: &WorldEntrySnapshot) -> String {
    let mut out = String::new();
    if let Some(main) = snap.data.get("main").and_then(|v| v.as_object()) {
        for (k, v) in main {
            if let Some(s) = v.as_str() {
                if !s.is_empty() {
                    out.push_str(k);
                    out.push_str(": ");
                    out.push_str(s);
                    out.push('\n');
                }
            }
        }
    }
    if let Some(lists) = snap.data.get("lists").and_then(|v| v.as_object()) {
        for (k, v) in lists {
            if let Some(arr) = v.as_array() {
                let joined: Vec<&str> = arr.iter().filter_map(serde_json::Value::as_str).collect();
                if !joined.is_empty() {
                    out.push_str(k);
                    out.push_str(": ");
                    out.push_str(&joined.join(", "));
                    out.push('\n');
                }
            }
        }
    }
    // Cap at ~1600 chars (~400 tokens).
    if out.len() > 1600 {
        out.truncate(1600);
        out.push_str("…");
    }
    out
}
```

- [ ] **Step 5: Test (best-effort)**

A full end-to-end LLM test requires a mocked LLM. If the M3 codebase has a mock harness (`MockLlmRouter` or similar), use it to verify:
- `contradicts` → pill emitted.
- `consistent` → no pill, no error.
- `unclear` → no pill, no error.

If no mock harness exists, ship a unit test for `render_entry_excerpt` alone + a smoke test that the prompt file parses correctly:

```rust
#[test]
fn pill_world_drift_check_prompt_parses() {
    let text = std::fs::read_to_string("../../prompts/pill_world_drift_check.toml").unwrap();
    let parsed: toml::Value = toml::from_str(&text).unwrap();
    assert!(parsed.get("response_schema").is_some());
    assert!(parsed.get("system").is_some());
    assert!(parsed.get("user").is_some());
}

#[test]
fn render_entry_excerpt_caps_at_1600_chars() {
    let long = "x".repeat(2000);
    let mut data = serde_json::Map::new();
    let mut main = serde_json::Map::new();
    main.insert("description".to_string(), serde_json::json!(long));
    data.insert("main".to_string(), serde_json::Value::Object(main));
    let snap = WorldEntrySnapshot {
        id: crate::Id::new(),
        segment_id: crate::Id::new(),
        segment_slug: "locations".to_string(),
        name: "Test".to_string(),
        aliases: vec![],
        data: serde_json::Value::Object(data),
    };
    let s = render_entry_excerpt(&snap);
    assert!(s.len() <= 1601 + 3);  // 1600 chars + "…"
}
```

- [ ] **Step 6: Commit**

```powershell
git add prompts/pill_world_drift_check.toml crates/water-core/src/prompts/ app/src-tauri/src/orchestrator_service.rs
git commit -m "feat(world): pill_world_drift_check.toml + Stage 2 confirmation handler"
```

---

### Task 18: Cartographer voice template + tone-audit pass

Replaces the stub Cartographer template from Task 14 with the real reactive/observational template, then runs the existing tone-audit harness (M2 carry-over) to verify it passes the 3-layer blacklist.

**Files:**
- Modify: `prompts/speakers/cartographer/template.toml`
- Possibly modify: `crates/water-core/src/voice/cartographer_template.rs` to load + render

- [ ] **Step 1: Replace the stub**

Overwrite `prompts/speakers/cartographer/template.toml`:

```toml
schema_version = 1
persona_slug = "cartographer"
hue_token = "--water-hue-persona-cartographer"

[clauses]
tone_clause = """
You speak as the Cartographer — a quiet observer of the world's coherence. Your voice is reactive and observational. You notice; you do not advise. You never use the words: you should, consider, try, I think you, as an AI.
"""

[user]
template = """
Trigger: {{trigger_kind}}

World entry — "{{matched_entry_name}}" in segment "{{matched_entry_segment}}":
{{relevant_world_excerpt}}

Scene paragraph:
{{scene_paragraph}}

Reason (from consistency check, for your situational awareness, NOT to quote): {{confirmation_reason}}

Speak as the Cartographer: one short sentence observing what you notice about the relationship between the paragraph and the entry. Reactive, not instructive.
"""
```

- [ ] **Step 2: Run the tone-audit harness**

```powershell
cargo test -p water-core --test tone_audit -- --nocapture
```

The M2 tone-audit suite samples each persona template and verifies the prompt clause is present + the blacklist is enforced at PASS time. If the audit fails on Cartographer, the failure surface reveals which blacklist phrase leaked — fix the template.

- [ ] **Step 3: Wire `CartographerSpeaker` to render the template**

In `crates/water-core/src/voice/cartographer_template.rs` (created in Task 14 with a stub):

```rust
use crate::voice::PromptRender;

pub struct CartographerTemplate {
    tone_clause: String,
    user_template: String,
}

impl CartographerTemplate {
    pub fn load(prompts_root: &std::path::Path) -> crate::Result<Self> {
        let path = prompts_root.join("speakers/cartographer/template.toml");
        let text = std::fs::read_to_string(&path)
            .map_err(|e| crate::Error::Other(format!("read {path:?}: {e}")))?;
        let parsed: toml::Value = toml::from_str(&text)
            .map_err(|e| crate::Error::Other(format!("parse {path:?}: {e}")))?;
        let tone_clause = parsed
            .get("clauses")
            .and_then(|v| v.get("tone_clause"))
            .and_then(toml::Value::as_str)
            .ok_or_else(|| crate::Error::Other("missing tone_clause".into()))?
            .to_string();
        let user_template = parsed
            .get("user")
            .and_then(|v| v.get("template"))
            .and_then(toml::Value::as_str)
            .ok_or_else(|| crate::Error::Other("missing user.template".into()))?
            .to_string();
        Ok(Self { tone_clause, user_template })
    }

    pub fn render(&self, tokens: &std::collections::HashMap<&str, String>) -> PromptRender {
        let mut user = self.user_template.clone();
        for (k, v) in tokens {
            user = user.replace(&format!("{{{{{k}}}}}"), v);
        }
        // Apply M3 line-based omission: drop lines that resolved to only whitespace.
        let user = user
            .lines()
            .filter(|l| !l.trim().is_empty() || l.trim_end_matches(char::is_whitespace).is_empty())
            .collect::<Vec<_>>()
            .join("\n");
        PromptRender {
            system_clause: self.tone_clause.clone(),
            user,
        }
    }
}
```

Note: the existing `PromptRender` / `PASS` / regex post-filter infrastructure handles the 3-layer blacklist enforcement — this template just feeds into it.

- [ ] **Step 3: Write a render test**

```rust
#[test]
fn cartographer_template_renders_with_tokens() {
    let tpl = CartographerTemplate {
        tone_clause: "TC".into(),
        user_template: "Entry: {{matched_entry_name}}\nPara: {{scene_paragraph}}".into(),
    };
    let mut tokens = std::collections::HashMap::new();
    tokens.insert("matched_entry_name", "Pell".to_string());
    tokens.insert("scene_paragraph", "She walked past.".to_string());
    let render = tpl.render(&tokens);
    assert!(render.user.contains("Entry: Pell"));
    assert!(render.user.contains("Para: She walked past."));
}
```

- [ ] **Step 4: Commit**

```powershell
cargo test -p water-core voice::cartographer_template
git add prompts/speakers/cartographer/ crates/water-core/src/voice/cartographer_template.rs
git commit -m "feat(world): Cartographer voice template (reactive tone, M2 3-layer audit clean)"
```

**Phase D close-out.** `world_drift` end-to-end: cheap scan → contextual-overlap pre-check → cooldown gate → LLM YES/NO/unclear → Cartographer pill (only on `contradicts`). Phase E now builds the user-facing surface.

---

## Phase E — UI surface

All TS files use strict mode + `noUncheckedIndexedAccess`. Components use the existing M3 pattern (functional, hooks, `Sheet` wrapper, `InlineField`, `ConversationalIntake` reuse).

### Task 19: `flattenSerdeFlatten` generic TS refactor

M3 lesson: `flattenCharacterData.ts` is the prototype. M4 needs the same flatten for `WorldEntryFile`. Extract a generic helper under `app/src/util/`.

**Files:**
- Create: `app/src/util/flattenSerdeFlatten.ts`
- Create: `app/src/util/flattenSerdeFlatten.test.ts`
- Modify: `app/src/characters/flattenCharacterData.ts` (delegates to generic)

- [ ] **Step 1: Inspect existing flattenCharacterData**

```powershell
Get-Content app\src\characters\flattenCharacterData.ts
```

Note its signature and the dotted-path output convention.

- [ ] **Step 2: Author the generic helper + tests**

Create `app/src/util/flattenSerdeFlatten.ts`:

```typescript
/**
 * Flatten an object whose section keys (e.g. "main", "lists", "arc") land
 * at the top level via Rust-side #[serde(flatten)] into a dotted-path
 * key/value map: `{ "main.full_name": "Aren", "lists.themes": ["..."] }`.
 *
 * Top-level non-object scalars (`id`, `name`, `schema_version`) are NOT
 * flattened — those are object metadata, not template-driven content.
 *
 * The `metadataKeys` parameter lists those non-flattenable keys; the helper
 * skips them in the output.
 */
export function flattenSerdeFlatten(
  source: Record<string, unknown>,
  metadataKeys: ReadonlySet<string> = new Set(["id", "name", "schema_version", "segment_id", "aliases"]),
): Record<string, unknown> {
  const out: Record<string, unknown> = {};
  for (const [sectionKey, sectionVal] of Object.entries(source)) {
    if (metadataKeys.has(sectionKey)) continue;
    if (sectionVal && typeof sectionVal === "object" && !Array.isArray(sectionVal)) {
      for (const [leafKey, leafVal] of Object.entries(sectionVal as Record<string, unknown>)) {
        out[`${sectionKey}.${leafKey}`] = leafVal;
      }
    }
  }
  return out;
}
```

Create `app/src/util/flattenSerdeFlatten.test.ts`:

```typescript
import { describe, it, expect } from "vitest";
import { flattenSerdeFlatten } from "./flattenSerdeFlatten";

describe("flattenSerdeFlatten", () => {
  it("flattens section keys into dotted paths", () => {
    const source = {
      id: "01J",
      name: "Aren",
      schema_version: "lsm-v2.1",
      main: { full_name: "Aren Vale", role: "scribe" },
      lists: { themes: ["memory", "obligation"] },
    };
    const out = flattenSerdeFlatten(source);
    expect(out).toEqual({
      "main.full_name": "Aren Vale",
      "main.role": "scribe",
      "lists.themes": ["memory", "obligation"],
    });
  });

  it("skips metadata keys", () => {
    const out = flattenSerdeFlatten({ id: "X", name: "Y", schema_version: "v" });
    expect(out).toEqual({});
  });

  it("respects custom metadataKeys", () => {
    const out = flattenSerdeFlatten(
      { id: "X", custom_meta: "skip", main: { a: 1 } },
      new Set(["id", "custom_meta"]),
    );
    expect(out).toEqual({ "main.a": 1 });
  });

  it("skips top-level arrays at metadata level (e.g. aliases)", () => {
    const out = flattenSerdeFlatten({
      id: "X",
      aliases: ["a", "b"],
      main: { type: "library" },
    });
    expect(out).toEqual({ "main.type": "library" });
  });
});
```

- [ ] **Step 3: Refactor `flattenCharacterData.ts` to delegate**

```typescript
import { flattenSerdeFlatten } from "../util/flattenSerdeFlatten";
import type { CharacterFile } from "../ipc/commands";

export function flattenCharacterData(file: CharacterFile): Record<string, unknown> {
  return flattenSerdeFlatten(file as Record<string, unknown>);
}
```

Confirm the character intake tests still pass after the refactor.

- [ ] **Step 4: Run + commit**

```powershell
pnpm --filter @water/app test src/util/flattenSerdeFlatten.test.ts
pnpm --filter @water/app test src/characters
git add app/src/util/ app/src/characters/flattenCharacterData.ts
git commit -m "refactor(util): extract flattenSerdeFlatten from flattenCharacterData for World reuse"
```

---

### Task 20: `WorldsSurface` routing + `App.tsx` mount + `WorldIndex` + `WorldSegmentTile`

The shell of the world surface — top-level routing skeleton + index tile grid.

**Files:**
- Create: `app/src/worlds/WorldsSurface.tsx`
- Create: `app/src/worlds/WorldIndex.tsx`
- Create: `app/src/worlds/WorldSegmentTile.tsx`
- Modify: `app/src/App.tsx`
- Create: `app/src/worlds/WorldsSurface.test.tsx`

- [ ] **Step 1: Inspect CharactersSurface for routing pattern**

```powershell
Get-Content app\src\chrome\CharactersSurface.tsx | Select-Object -First 60
```

Note the `View` discriminated union, the scroll-position preservation pattern (M3 T20).

- [ ] **Step 2: Author `WorldsSurface.tsx`**

```tsx
import { useState } from "react";
import { WorldIndex } from "../worlds/WorldIndex";
// Forward-declared imports; created in later tasks:
// import { WorldSegmentView } from "./WorldSegmentView";
// import { WorldEntrySheet } from "./WorldEntrySheet";
// import { WorldEntryIntakeSheet } from "./WorldEntryIntakeSheet";
// import { SegmentTemplateEditor } from "./SegmentTemplateEditor";

type View =
  | { kind: "index" }
  | { kind: "segment"; segmentId: string }
  | { kind: "entry"; segmentId: string; entryId: string }
  | { kind: "entry-intake"; segmentId: string; draftEntryId: string }
  | { kind: "new-segment" };

export function WorldsSurface({ projectId }: { projectId: string }) {
  const [view, setView] = useState<View>({ kind: "index" });

  // Scroll position preservation per M3 T20.
  const [indexScrollY, setIndexScrollY] = useState(0);

  function goToSegment(segmentId: string) {
    if (view.kind === "index") setIndexScrollY(window.scrollY);
    setView({ kind: "segment", segmentId });
  }

  function goToIndex() {
    setView({ kind: "index" });
    queueMicrotask(() => window.scrollTo(0, indexScrollY));
  }

  function goToEntry(segmentId: string, entryId: string) {
    setView({ kind: "entry", segmentId, entryId });
  }

  return (
    <div className="worlds-surface">
      {view.kind === "index" && (
        <WorldIndex
          onSelectSegment={goToSegment}
          onNewSegment={() => setView({ kind: "new-segment" })}
        />
      )}
      {view.kind === "segment" && (
        <div>
          <button onClick={goToIndex}>← Back</button>
          {/* WorldSegmentView mounts in Task 21 */}
          <div data-testid="segment-view-placeholder">segment: {view.segmentId}</div>
        </div>
      )}
      {view.kind === "entry" && (
        <div>
          <button onClick={() => goToSegment(view.segmentId)}>← Back</button>
          {/* WorldEntrySheet mounts in Task 23 */}
          <div data-testid="entry-placeholder">entry: {view.entryId}</div>
        </div>
      )}
      {view.kind === "entry-intake" && (
        <div data-testid="intake-placeholder">intake: {view.draftEntryId}</div>
      )}
      {view.kind === "new-segment" && (
        <div data-testid="new-segment-placeholder">new segment modal</div>
      )}
    </div>
  );
}
```

Each placeholder gets replaced by its real component in the relevant task. The skeleton ensures routing works end-to-end before any component is fleshed out.

- [ ] **Step 3: Author `WorldIndex.tsx` + `WorldSegmentTile.tsx`**

`app/src/worlds/WorldIndex.tsx`:

```tsx
import { useEffect, useState } from "react";
import { ipc, type WorldSegment } from "../ipc/commands";
import { WorldSegmentTile } from "./WorldSegmentTile";

export function WorldIndex({
  onSelectSegment,
  onNewSegment,
}: {
  onSelectSegment: (segmentId: string) => void;
  onNewSegment: () => void;
}) {
  const [segments, setSegments] = useState<WorldSegment[]>([]);
  const [loaded, setLoaded] = useState(false);

  useEffect(() => {
    let cancelled = false;
    ipc.worldSegmentList().then((rows) => {
      if (cancelled) return;
      setSegments(rows);
      setLoaded(true);
    });
    return () => {
      cancelled = true;
    };
  }, []);

  if (!loaded) return <div>Loading…</div>;

  return (
    <div className="world-index">
      <h2>World Bible</h2>
      <div className="world-index-grid">
        {segments
          .filter((s) => !s.hidden)
          .map((s) => (
            <WorldSegmentTile
              key={s.id}
              segment={s}
              onClick={() => onSelectSegment(s.id)}
            />
          ))}
        <button
          className="world-index-new-segment"
          onClick={onNewSegment}
          data-testid="new-segment-button"
        >
          + New segment
        </button>
      </div>
    </div>
  );
}
```

`app/src/worlds/WorldSegmentTile.tsx`:

```tsx
import { useEffect, useState } from "react";
import { ipc, type WorldSegment, type WorldEntryIndexEntry } from "../ipc/commands";

export function WorldSegmentTile({
  segment,
  onClick,
}: {
  segment: WorldSegment;
  onClick: () => void;
}) {
  const [preview, setPreview] = useState<string>("");

  useEffect(() => {
    let cancelled = false;
    if (segment.is_collection) {
      ipc.worldEntryList(segment.id).then((rows) => {
        if (cancelled) return;
        if (rows.length === 0) {
          setPreview("(no entries yet)");
        } else {
          const names = rows.slice(0, 2).map((r) => r.name || "(unnamed)").join(", ");
          const suffix = rows.length > 2 ? `, …` : "";
          setPreview(`${rows.length} ${rows.length === 1 ? "entry" : "entries"}: ${names}${suffix}`);
        }
      });
    } else {
      ipc.worldSingleDocRead(segment.id).then((file) => {
        if (cancelled) return;
        const main = (file as Record<string, unknown>).main as
          | Record<string, unknown>
          | undefined;
        for (const v of Object.values(main ?? {})) {
          if (typeof v === "string" && v.trim().length > 0) {
            setPreview(v.length > 80 ? `${v.slice(0, 80)}…` : v);
            return;
          }
        }
        setPreview("(empty)");
      });
    }
    return () => {
      cancelled = true;
    };
  }, [segment.id, segment.is_collection]);

  return (
    <button
      className="world-segment-tile"
      style={{ ["--tile-hue" as string]: `var(${segment.hue_token})` }}
      onClick={onClick}
      data-testid={`segment-tile-${segment.slug || segment.id}`}
    >
      <div className="world-segment-tile-name">{segment.name}</div>
      <div className="world-segment-tile-preview">{preview}</div>
      <div className="world-segment-tile-icon">
        {segment.is_collection ? "▦" : "▢"}
      </div>
    </button>
  );
}
```

- [ ] **Step 4: Mount in `App.tsx`**

Find the `activeNav` switch in `app/src/App.tsx` and replace the current world fall-through:

```tsx
{activeNav === "scenes" && <ScenesSurface .../>}
{activeNav === "characters" && <CharactersSurface .../>}
{activeNav === "world" && <WorldsSurface projectId={projectId} />}
```

Import: `import { WorldsSurface } from "./worlds/WorldsSurface";`

- [ ] **Step 5: Write the failing tests**

Create `app/src/worlds/WorldsSurface.test.tsx`:

```tsx
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { WorldsSurface } from "./WorldsSurface";

vi.mock("../ipc/commands", () => ({
  ipc: {
    worldSegmentList: vi.fn(),
    worldEntryList: vi.fn(),
    worldSingleDocRead: vi.fn(),
  },
}));

import { ipc } from "../ipc/commands";

describe("WorldsSurface", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    (ipc.worldSegmentList as ReturnType<typeof vi.fn>).mockResolvedValue([
      {
        id: "seg-1",
        slug: "concept",
        name: "Concept",
        ordering: 0,
        is_collection: false,
        hue_token: "--water-hue-world-1",
        hidden: false,
        has_template_override: false,
      },
      {
        id: "seg-2",
        slug: "locations",
        name: "Locations",
        ordering: 1,
        is_collection: true,
        hue_token: "--water-hue-world-2",
        hidden: false,
        has_template_override: false,
      },
    ]);
    (ipc.worldEntryList as ReturnType<typeof vi.fn>).mockResolvedValue([]);
    (ipc.worldSingleDocRead as ReturnType<typeof vi.fn>).mockResolvedValue({
      id: "x",
      segment_id: "seg-1",
      schema_version: "concept@1",
      name: "Concept",
      aliases: [],
    });
  });

  it("renders the index with all non-hidden segments", async () => {
    render(<WorldsSurface projectId="p1" />);
    await waitFor(() => screen.getByTestId("segment-tile-concept"));
    expect(screen.getByTestId("segment-tile-locations")).toBeInTheDocument();
  });

  it("navigates to segment view on tile click", async () => {
    render(<WorldsSurface projectId="p1" />);
    await waitFor(() => screen.getByTestId("segment-tile-locations"));
    await userEvent.click(screen.getByTestId("segment-tile-locations"));
    expect(screen.getByTestId("segment-view-placeholder")).toHaveTextContent("seg-2");
  });

  it("back button returns to index from segment view", async () => {
    render(<WorldsSurface projectId="p1" />);
    await waitFor(() => screen.getByTestId("segment-tile-locations"));
    await userEvent.click(screen.getByTestId("segment-tile-locations"));
    await userEvent.click(screen.getByText("← Back"));
    expect(screen.getByTestId("segment-tile-concept")).toBeInTheDocument();
  });

  it("hidden segments are filtered out", async () => {
    (ipc.worldSegmentList as ReturnType<typeof vi.fn>).mockResolvedValue([
      {
        id: "seg-1",
        slug: "concept",
        name: "Concept",
        ordering: 0,
        is_collection: false,
        hue_token: "--water-hue-world-1",
        hidden: true,
        has_template_override: false,
      },
    ]);
    render(<WorldsSurface projectId="p1" />);
    await waitFor(() => screen.getByTestId("new-segment-button"));
    expect(screen.queryByTestId("segment-tile-concept")).not.toBeInTheDocument();
  });
});
```

- [ ] **Step 6: Run + commit**

```powershell
pnpm --filter @water/app test src/worlds/WorldsSurface.test.tsx
git add app/src/worlds/ app/src/App.tsx
git commit -m "feat(worlds): WorldsSurface routing skeleton + WorldIndex + WorldSegmentTile"
```

---

### Task 21: `WorldSegmentView` single-doc branch

The "one inline-editable sheet for all fields" rendering for single-doc segments.

**Files:**
- Create: `app/src/worlds/WorldSegmentView.tsx`
- Create: `app/src/worlds/WorldSingleDocSheet.tsx`
- Create: `app/src/worlds/WorldSegmentView.test.tsx`

- [ ] **Step 1: Author `WorldSegmentView.tsx`**

```tsx
import { useEffect, useState } from "react";
import { ipc, type WorldSegment } from "../ipc/commands";
import { WorldSingleDocSheet } from "./WorldSingleDocSheet";

export function WorldSegmentView({
  segmentId,
  onOpenEntry,
}: {
  segmentId: string;
  onOpenEntry: (entryId: string) => void;
}) {
  const [segment, setSegment] = useState<WorldSegment | null>(null);

  useEffect(() => {
    let cancelled = false;
    ipc.worldSegmentList().then((rows) => {
      if (cancelled) return;
      setSegment(rows.find((s) => s.id === segmentId) ?? null);
    });
    return () => {
      cancelled = true;
    };
  }, [segmentId]);

  if (!segment) return <div>Loading…</div>;

  if (segment.is_collection) {
    // WorldCollectionGrid mounts in Task 22.
    return <div data-testid="collection-grid-placeholder">{segment.name}</div>;
  }

  return <WorldSingleDocSheet segment={segment} />;
}
```

- [ ] **Step 2: Author `WorldSingleDocSheet.tsx`**

```tsx
import { useEffect, useState } from "react";
import { ipc, type WorldSegment, type IntakeSchemaSection } from "../ipc/commands";
import { InlineField } from "../characters/InlineField";
import { flattenSerdeFlatten } from "../util/flattenSerdeFlatten";

export function WorldSingleDocSheet({ segment }: { segment: WorldSegment }) {
  const [schema, setSchema] = useState<IntakeSchemaSection | null>(null);
  const [values, setValues] = useState<Record<string, unknown>>({});
  const [loaded, setLoaded] = useState(false);

  useEffect(() => {
    let cancelled = false;
    Promise.all([
      ipc.worldIntakeSchema(segment.id),
      ipc.worldSingleDocRead(segment.id),
    ]).then(([sch, file]) => {
      if (cancelled) return;
      setSchema(sch);
      setValues(flattenSerdeFlatten(file as Record<string, unknown>));
      setLoaded(true);
    });
    return () => {
      cancelled = true;
    };
  }, [segment.id]);

  if (!loaded || !schema) return <div>Loading…</div>;

  return (
    <div className="world-single-doc-sheet">
      <h2>{segment.name}</h2>
      <div className="world-single-doc-fields">
        {schema.fields.map((field) => (
          <InlineField
            key={field.id}
            field={field}
            value={values[field.id]}
            onChange={async (newValue) => {
              await ipc.worldSingleDocUpdateField({
                segmentId: segment.id,
                fieldId: field.id,
                value: newValue,
              });
              setValues((prev) => ({ ...prev, [field.id]: newValue }));
            }}
          />
        ))}
      </div>
    </div>
  );
}
```

- [ ] **Step 3: Wire into WorldsSurface**

In `WorldsSurface.tsx`, replace the `segment-view-placeholder` div:

```tsx
{view.kind === "segment" && (
  <div>
    <button onClick={goToIndex}>← Back</button>
    <WorldSegmentView segmentId={view.segmentId} onOpenEntry={(entryId) => goToEntry(view.segmentId, entryId)} />
  </div>
)}
```

Add the import: `import { WorldSegmentView } from "./WorldSegmentView";`

- [ ] **Step 4: Write the failing test**

Create `app/src/worlds/WorldSegmentView.test.tsx`:

```tsx
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import { WorldSegmentView } from "./WorldSegmentView";

vi.mock("../ipc/commands", () => ({
  ipc: {
    worldSegmentList: vi.fn(),
    worldIntakeSchema: vi.fn(),
    worldSingleDocRead: vi.fn(),
    worldSingleDocUpdateField: vi.fn(),
  },
}));

import { ipc } from "../ipc/commands";

describe("WorldSegmentView (single-doc branch)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    (ipc.worldSegmentList as ReturnType<typeof vi.fn>).mockResolvedValue([
      {
        id: "seg-c",
        slug: "concept",
        name: "Concept",
        ordering: 0,
        is_collection: false,
        hue_token: "--water-hue-world-1",
        hidden: false,
        has_template_override: false,
      },
    ]);
    (ipc.worldIntakeSchema as ReturnType<typeof vi.fn>).mockResolvedValue({
      id: "concept",
      label: "Concept",
      fields: [
        {
          id: "main.core_premise",
          label: "Core Premise",
          prompt_question: "?",
          kind: { type: "long_text" },
          optional_skip: false,
        },
      ],
    });
    (ipc.worldSingleDocRead as ReturnType<typeof vi.fn>).mockResolvedValue({
      id: "x",
      segment_id: "seg-c",
      schema_version: "concept@1",
      name: "Concept",
      aliases: [],
      main: { core_premise: "An echo through stone." },
    });
  });

  it("renders the single-doc sheet with template fields and current values", async () => {
    render(<WorldSegmentView segmentId="seg-c" onOpenEntry={() => {}} />);
    await waitFor(() => screen.getByText("Core Premise"));
    expect(screen.getByDisplayValue("An echo through stone.")).toBeInTheDocument();
  });

  it("collection segment defers to placeholder (Task 22 wires the real grid)", async () => {
    (ipc.worldSegmentList as ReturnType<typeof vi.fn>).mockResolvedValue([
      {
        id: "seg-l",
        slug: "locations",
        name: "Locations",
        ordering: 1,
        is_collection: true,
        hue_token: "--water-hue-world-2",
        hidden: false,
        has_template_override: false,
      },
    ]);
    render(<WorldSegmentView segmentId="seg-l" onOpenEntry={() => {}} />);
    await waitFor(() => screen.getByTestId("collection-grid-placeholder"));
  });
});
```

- [ ] **Step 5: Run + commit**

```powershell
pnpm --filter @water/app test src/worlds/WorldSegmentView.test.tsx
git add app/src/worlds/
git commit -m "feat(worlds): WorldSegmentView single-doc branch with InlineField sheet"
```

---

### Task 22: `WorldSegmentView` collection branch + `WorldEntryCard`

Adds the grid + `+ New entry` tile for collection segments.

**Files:**
- Create: `app/src/worlds/WorldCollectionGrid.tsx`
- Create: `app/src/worlds/WorldEntryCard.tsx`
- Modify: `app/src/worlds/WorldSegmentView.tsx` (replace placeholder)

- [ ] **Step 1: Author `WorldCollectionGrid.tsx`**

```tsx
import { useEffect, useState } from "react";
import { ipc, type WorldSegment, type WorldEntryIndexEntry } from "../ipc/commands";
import { WorldEntryCard } from "./WorldEntryCard";

export function WorldCollectionGrid({
  segment,
  onOpenEntry,
  onNewEntry,
}: {
  segment: WorldSegment;
  onOpenEntry: (entryId: string) => void;
  onNewEntry: () => void;
}) {
  const [entries, setEntries] = useState<WorldEntryIndexEntry[]>([]);
  const [loaded, setLoaded] = useState(false);

  async function refresh() {
    const rows = await ipc.worldEntryList(segment.id);
    setEntries(rows);
    setLoaded(true);
  }

  useEffect(() => {
    let cancelled = false;
    ipc.worldEntryList(segment.id).then((rows) => {
      if (cancelled) return;
      setEntries(rows);
      setLoaded(true);
    });
    return () => {
      cancelled = true;
    };
  }, [segment.id]);

  if (!loaded) return <div>Loading…</div>;

  return (
    <div className="world-collection-grid">
      <h2>{segment.name}</h2>
      <div className="world-entry-grid">
        {entries.map((e) => (
          <WorldEntryCard
            key={e.id}
            entry={e}
            hueToken={segment.hue_token}
            onClick={() => onOpenEntry(e.id)}
          />
        ))}
        <button
          className="world-entry-card-new"
          onClick={onNewEntry}
          data-testid="new-entry-button"
        >
          + New entry
        </button>
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Author `WorldEntryCard.tsx`**

```tsx
import type { WorldEntryIndexEntry } from "../ipc/commands";

export function WorldEntryCard({
  entry,
  hueToken,
  onClick,
}: {
  entry: WorldEntryIndexEntry;
  hueToken: string;
  onClick: () => void;
}) {
  const displayName = entry.name.trim() === "" ? "(unnamed)" : entry.name;
  return (
    <button
      className="world-entry-card"
      style={{ ["--card-hue" as string]: `var(${hueToken})` }}
      onClick={onClick}
      data-testid={`entry-card-${entry.id}`}
    >
      <div className="world-entry-card-name">{displayName}</div>
      <div className="world-entry-card-preview">{entry.preview}</div>
    </button>
  );
}
```

- [ ] **Step 3: Wire into `WorldSegmentView.tsx`**

Replace the `collection-grid-placeholder`:

```tsx
if (segment.is_collection) {
  return (
    <WorldCollectionGrid
      segment={segment}
      onOpenEntry={onOpenEntry}
      onNewEntry={async () => {
        const newId = await ipc.worldEntryCreate({ segmentId: segment.id, name: "" });
        onOpenIntake(segment.id, newId);
      }}
    />
  );
}
```

Add `onOpenIntake` to `WorldSegmentView`'s props and thread through `WorldsSurface`:

```tsx
// In WorldsSurface:
{view.kind === "segment" && (
  <div>
    <button onClick={goToIndex}>← Back</button>
    <WorldSegmentView
      segmentId={view.segmentId}
      onOpenEntry={(entryId) => goToEntry(view.segmentId, entryId)}
      onOpenIntake={(segId, draftId) => setView({ kind: "entry-intake", segmentId: segId, draftEntryId: draftId })}
    />
  </div>
)}
```

- [ ] **Step 4: Write the failing test**

Append to `app/src/worlds/WorldSegmentView.test.tsx`:

```tsx
describe("WorldSegmentView (collection branch)", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    (ipc.worldSegmentList as ReturnType<typeof vi.fn>).mockResolvedValue([
      {
        id: "seg-l",
        slug: "locations",
        name: "Locations",
        ordering: 1,
        is_collection: true,
        hue_token: "--water-hue-world-2",
        hidden: false,
        has_template_override: false,
      },
    ]);
  });

  it("renders entry cards from worldEntryList", async () => {
    (ipc.worldEntryList as ReturnType<typeof vi.fn>).mockResolvedValue([
      { id: "e1", segment_id: "seg-l", name: "The Pell Library", preview: "Dust thick…" },
      { id: "e2", segment_id: "seg-l", name: "Aren's Atelier", preview: "Light from the…" },
    ]);
    render(<WorldSegmentView segmentId="seg-l" onOpenEntry={() => {}} onOpenIntake={() => {}} />);
    await waitFor(() => screen.getByTestId("entry-card-e1"));
    expect(screen.getByText("The Pell Library")).toBeInTheDocument();
    expect(screen.getByText("Aren's Atelier")).toBeInTheDocument();
  });

  it("clicking + New entry creates a draft and routes to intake", async () => {
    (ipc.worldEntryList as ReturnType<typeof vi.fn>).mockResolvedValue([]);
    (ipc.worldEntryCreate as ReturnType<typeof vi.fn>).mockResolvedValue("draft-1");
    const openIntake = vi.fn();
    render(<WorldSegmentView segmentId="seg-l" onOpenEntry={() => {}} onOpenIntake={openIntake} />);
    await waitFor(() => screen.getByTestId("new-entry-button"));
    await userEvent.click(screen.getByTestId("new-entry-button"));
    await waitFor(() => expect(openIntake).toHaveBeenCalledWith("seg-l", "draft-1"));
  });

  it("unnamed entry shows '(unnamed)' label", async () => {
    (ipc.worldEntryList as ReturnType<typeof vi.fn>).mockResolvedValue([
      { id: "e-stub", segment_id: "seg-l", name: "", preview: "Some snippet…" },
    ]);
    render(<WorldSegmentView segmentId="seg-l" onOpenEntry={() => {}} onOpenIntake={() => {}} />);
    await waitFor(() => screen.getByTestId("entry-card-e-stub"));
    expect(screen.getByText("(unnamed)")).toBeInTheDocument();
  });
});
```

Don't forget to import `userEvent` if missing.

- [ ] **Step 5: Run + commit**

```powershell
pnpm --filter @water/app test src/worlds/WorldSegmentView.test.tsx
git add app/src/worlds/
git commit -m "feat(worlds): WorldCollectionGrid + WorldEntryCard + new-entry intake routing"
```

---

### Task 23: `WorldEntrySheet` (inline edit + aliases editor)

Per-entry sheet — inline-editable template fields + the M4-specific aliases list editor.

**Files:**
- Create: `app/src/worlds/WorldEntrySheet.tsx`
- Create: `app/src/worlds/AliasesEditor.tsx`
- Create: `app/src/worlds/WorldEntrySheet.test.tsx`

- [ ] **Step 1: Author `AliasesEditor.tsx`**

```tsx
import { useState } from "react";

export function AliasesEditor({
  aliases,
  onChange,
}: {
  aliases: string[];
  onChange: (next: string[]) => void;
}) {
  const [draft, setDraft] = useState("");

  function addAlias() {
    const trimmed = draft.trim();
    if (!trimmed) return;
    if (aliases.includes(trimmed)) {
      setDraft("");
      return;
    }
    onChange([...aliases, trimmed]);
    setDraft("");
  }

  function removeAlias(i: number) {
    onChange(aliases.filter((_, idx) => idx !== i));
  }

  return (
    <div className="aliases-editor">
      <label>Aliases</label>
      <ul>
        {aliases.map((a, i) => (
          <li key={`${a}-${i}`}>
            {a}{" "}
            <button onClick={() => removeAlias(i)} data-testid={`remove-alias-${i}`}>
              ×
            </button>
          </li>
        ))}
      </ul>
      <input
        type="text"
        value={draft}
        onChange={(e) => setDraft(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter") {
            e.preventDefault();
            addAlias();
          }
        }}
        placeholder="Add an alias…"
        data-testid="alias-input"
      />
      <button onClick={addAlias} data-testid="alias-add-button">
        Add
      </button>
    </div>
  );
}
```

- [ ] **Step 2: Author `WorldEntrySheet.tsx`**

```tsx
import { useEffect, useState } from "react";
import { ipc, type WorldSegment, type WorldEntryFile, type IntakeSchemaSection } from "../ipc/commands";
import { InlineField } from "../characters/InlineField";
import { flattenSerdeFlatten } from "../util/flattenSerdeFlatten";
import { AliasesEditor } from "./AliasesEditor";

export function WorldEntrySheet({
  segmentId,
  entryId,
}: {
  segmentId: string;
  entryId: string;
}) {
  const [segment, setSegment] = useState<WorldSegment | null>(null);
  const [schema, setSchema] = useState<IntakeSchemaSection | null>(null);
  const [file, setFile] = useState<WorldEntryFile | null>(null);
  const [values, setValues] = useState<Record<string, unknown>>({});

  useEffect(() => {
    let cancelled = false;
    Promise.all([
      ipc.worldSegmentList(),
      ipc.worldIntakeSchema(segmentId),
      ipc.worldEntryRead(entryId),
    ]).then(([segs, sch, f]) => {
      if (cancelled) return;
      setSegment(segs.find((s) => s.id === segmentId) ?? null);
      setSchema(sch);
      setFile(f);
      setValues(flattenSerdeFlatten(f as Record<string, unknown>));
    });
    return () => {
      cancelled = true;
    };
  }, [segmentId, entryId]);

  if (!segment || !schema || !file) return <div>Loading…</div>;

  const displayName = file.name.trim() === "" ? "(unnamed)" : file.name;

  return (
    <div className="world-entry-sheet">
      <h2 data-testid="entry-name">{displayName}</h2>
      <AliasesEditor
        aliases={file.aliases}
        onChange={async (next) => {
          await ipc.worldEntryUpdateAliases({ entryId, aliases: next });
          setFile({ ...file, aliases: next });
        }}
      />
      <div className="world-entry-fields">
        {schema.fields.map((field) => (
          <InlineField
            key={field.id}
            field={field}
            value={values[field.id]}
            onChange={async (newValue) => {
              await ipc.worldEntryUpdateField({
                entryId,
                fieldId: field.id,
                value: newValue,
              });
              setValues((prev) => ({ ...prev, [field.id]: newValue }));
              if (field.id === "main.name" && typeof newValue === "string") {
                setFile({ ...file, name: newValue });
              }
            }}
          />
        ))}
      </div>
    </div>
  );
}
```

- [ ] **Step 3: Wire into `WorldsSurface.tsx`**

Replace the `entry-placeholder` div:

```tsx
{view.kind === "entry" && (
  <div>
    <button onClick={() => goToSegment(view.segmentId)}>← Back</button>
    <WorldEntrySheet segmentId={view.segmentId} entryId={view.entryId} />
  </div>
)}
```

Import: `import { WorldEntrySheet } from "./WorldEntrySheet";`

- [ ] **Step 4: Test + commit**

Create `app/src/worlds/WorldEntrySheet.test.tsx`:

```tsx
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { WorldEntrySheet } from "./WorldEntrySheet";

vi.mock("../ipc/commands", () => ({
  ipc: {
    worldSegmentList: vi.fn(),
    worldIntakeSchema: vi.fn(),
    worldEntryRead: vi.fn(),
    worldEntryUpdateAliases: vi.fn(),
    worldEntryUpdateField: vi.fn(),
  },
}));

import { ipc } from "../ipc/commands";

describe("WorldEntrySheet", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    (ipc.worldSegmentList as ReturnType<typeof vi.fn>).mockResolvedValue([
      {
        id: "seg-l",
        slug: "locations",
        name: "Locations",
        ordering: 1,
        is_collection: true,
        hue_token: "--water-hue-world-2",
        hidden: false,
        has_template_override: false,
      },
    ]);
    (ipc.worldIntakeSchema as ReturnType<typeof vi.fn>).mockResolvedValue({
      id: "locations",
      label: "Location",
      fields: [
        {
          id: "main.name",
          label: "Name",
          prompt_question: "?",
          kind: { type: "short_text" },
          optional_skip: false,
        },
        {
          id: "main.type",
          label: "Type",
          prompt_question: "?",
          kind: { type: "short_text" },
          optional_skip: false,
        },
      ],
    });
    (ipc.worldEntryRead as ReturnType<typeof vi.fn>).mockResolvedValue({
      id: "e1",
      segment_id: "seg-l",
      schema_version: "locations@1",
      name: "The Pell Library",
      aliases: ["Pell"],
      main: { name: "The Pell Library", type: "library" },
    });
    (ipc.worldEntryUpdateAliases as ReturnType<typeof vi.fn>).mockResolvedValue(undefined);
    (ipc.worldEntryUpdateField as ReturnType<typeof vi.fn>).mockResolvedValue(undefined);
  });

  it("renders the entry name + aliases + template fields", async () => {
    render(<WorldEntrySheet segmentId="seg-l" entryId="e1" />);
    await waitFor(() => screen.getByTestId("entry-name"));
    expect(screen.getByText("The Pell Library")).toBeInTheDocument();
    expect(screen.getByText("Pell")).toBeInTheDocument();
    expect(screen.getByDisplayValue("library")).toBeInTheDocument();
  });

  it("adding an alias commits via ipc.worldEntryUpdateAliases", async () => {
    render(<WorldEntrySheet segmentId="seg-l" entryId="e1" />);
    await waitFor(() => screen.getByTestId("alias-input"));
    await userEvent.type(screen.getByTestId("alias-input"), "the library");
    await userEvent.click(screen.getByTestId("alias-add-button"));
    await waitFor(() =>
      expect(ipc.worldEntryUpdateAliases).toHaveBeenCalledWith({
        entryId: "e1",
        aliases: ["Pell", "the library"],
      }),
    );
  });

  it("unnamed entry shows '(unnamed)' label", async () => {
    (ipc.worldEntryRead as ReturnType<typeof vi.fn>).mockResolvedValue({
      id: "e-stub",
      segment_id: "seg-l",
      schema_version: "locations@1",
      name: "",
      aliases: [],
      main: { sensory_detail: "Dust thick enough…" },
    });
    render(<WorldEntrySheet segmentId="seg-l" entryId="e-stub" />);
    await waitFor(() => screen.getByTestId("entry-name"));
    expect(screen.getByText("(unnamed)")).toBeInTheDocument();
  });
});
```

```powershell
pnpm --filter @water/app test src/worlds/WorldEntrySheet.test.tsx
git add app/src/worlds/
git commit -m "feat(worlds): WorldEntrySheet (inline edit + AliasesEditor)"
```

---

### Task 24: `WorldEntryIntakeSheet` (intake reuse + orphan-draft reaping)

Walks `ConversationalIntake` for a new entry. Reaps drafts on close per spec § 4.5.

**Files:**
- Create: `app/src/worlds/WorldEntryIntakeSheet.tsx`
- Create: `app/src/worlds/WorldEntryIntakeSheet.test.tsx`

- [ ] **Step 1: Author the component**

```tsx
import { useEffect, useState } from "react";
import { ipc, type IntakeSchemaSection, type WorldEntryFile } from "../ipc/commands";
import { Sheet } from "../intake/Sheet";
import { ConversationalIntake } from "../intake/ConversationalIntake";
import { flattenSerdeFlatten } from "../util/flattenSerdeFlatten";

export function WorldEntryIntakeSheet({
  segmentId,
  draftEntryId,
  onComplete,
  onClose,
}: {
  segmentId: string;
  draftEntryId: string;
  onComplete: (entryId: string) => void;
  onClose: () => void;
}) {
  const [schema, setSchema] = useState<IntakeSchemaSection | null>(null);
  const [values, setValues] = useState<Record<string, unknown>>({});
  const [loaded, setLoaded] = useState(false);

  useEffect(() => {
    let cancelled = false;
    Promise.all([
      ipc.worldIntakeSchema(segmentId),
      ipc.worldEntryRead(draftEntryId),
    ]).then(([sch, file]: [IntakeSchemaSection, WorldEntryFile]) => {
      if (cancelled) return;
      setSchema(sch);
      setValues(flattenSerdeFlatten(file as Record<string, unknown>));
      setLoaded(true);
    });
    return () => {
      cancelled = true;
    };
  }, [segmentId, draftEntryId]);

  async function handleClose() {
    // Reap orphan draft if entry is still empty.
    await ipc.worldEntryDeleteIfEmpty(draftEntryId);
    onClose();
  }

  if (!loaded || !schema) {
    return <Sheet onClose={handleClose}>Loading…</Sheet>;
  }

  return (
    <Sheet onClose={handleClose}>
      <ConversationalIntake
        schema={schema}
        values={values}
        onAnswer={async (fieldId: string, value: unknown) => {
          await ipc.worldEntryUpdateField({ entryId: draftEntryId, fieldId, value });
          setValues((prev) => ({ ...prev, [fieldId]: value }));
        }}
        onComplete={() => onComplete(draftEntryId)}
      />
    </Sheet>
  );
}
```

- [ ] **Step 2: Wire into `WorldsSurface.tsx`**

Replace the `intake-placeholder`:

```tsx
{view.kind === "entry-intake" && (
  <WorldEntryIntakeSheet
    segmentId={view.segmentId}
    draftEntryId={view.draftEntryId}
    onComplete={(entryId) => setView({ kind: "entry", segmentId: view.segmentId, entryId })}
    onClose={() => setView({ kind: "segment", segmentId: view.segmentId })}
  />
)}
```

Import: `import { WorldEntryIntakeSheet } from "./WorldEntryIntakeSheet";`

- [ ] **Step 3: Write the failing test**

Create `app/src/worlds/WorldEntryIntakeSheet.test.tsx`:

```tsx
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { WorldEntryIntakeSheet } from "./WorldEntryIntakeSheet";

vi.mock("../ipc/commands", () => ({
  ipc: {
    worldIntakeSchema: vi.fn(),
    worldEntryRead: vi.fn(),
    worldEntryUpdateField: vi.fn(),
    worldEntryDeleteIfEmpty: vi.fn(),
  },
}));

import { ipc } from "../ipc/commands";

describe("WorldEntryIntakeSheet", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    (ipc.worldIntakeSchema as ReturnType<typeof vi.fn>).mockResolvedValue({
      id: "locations",
      label: "Location",
      fields: [
        {
          id: "main.name",
          label: "Name",
          prompt_question: "What's this place called?",
          kind: { type: "short_text" },
          optional_skip: false,
        },
      ],
    });
    (ipc.worldEntryRead as ReturnType<typeof vi.fn>).mockResolvedValue({
      id: "draft-1",
      segment_id: "seg-l",
      schema_version: "locations@1",
      name: "",
      aliases: [],
      main: {},
    });
    (ipc.worldEntryUpdateField as ReturnType<typeof vi.fn>).mockResolvedValue(undefined);
    (ipc.worldEntryDeleteIfEmpty as ReturnType<typeof vi.fn>).mockResolvedValue(true);
  });

  it("walks the intake for the segment's template", async () => {
    render(
      <WorldEntryIntakeSheet
        segmentId="seg-l"
        draftEntryId="draft-1"
        onComplete={() => {}}
        onClose={() => {}}
      />,
    );
    await waitFor(() => screen.getByText("What's this place called?"));
  });

  it("close handler reaps the draft if empty", async () => {
    const onClose = vi.fn();
    render(
      <WorldEntryIntakeSheet
        segmentId="seg-l"
        draftEntryId="draft-1"
        onComplete={() => {}}
        onClose={onClose}
      />,
    );
    await waitFor(() => screen.getByText("What's this place called?"));
    // Sheet's close affordance varies by impl; if `onClose` is wired to a
    // button with testid="sheet-close", click it. Otherwise simulate the
    // close prop directly. Read existing CharacterIntakeSheet.test.tsx for
    // the canonical close-trigger pattern.
    await userEvent.click(screen.getByTestId("sheet-close"));
    await waitFor(() => expect(ipc.worldEntryDeleteIfEmpty).toHaveBeenCalledWith("draft-1"));
    expect(onClose).toHaveBeenCalled();
  });

  it("answering a field calls worldEntryUpdateField with dotted-path field_id", async () => {
    render(
      <WorldEntryIntakeSheet
        segmentId="seg-l"
        draftEntryId="draft-1"
        onComplete={() => {}}
        onClose={() => {}}
      />,
    );
    await waitFor(() => screen.getByText("What's this place called?"));
    // ConversationalIntake exposes an answer-submit affordance; match the
    // canonical CharacterIntakeSheet.test.tsx pattern for input + submit.
    const input = screen.getByRole("textbox");
    await userEvent.type(input, "The Pell Library");
    await userEvent.keyboard("{Enter}");
    await waitFor(() =>
      expect(ipc.worldEntryUpdateField).toHaveBeenCalledWith({
        entryId: "draft-1",
        fieldId: "main.name",
        value: "The Pell Library",
      }),
    );
  });
});
```

If `Sheet` doesn't expose a `sheet-close` testid, either add one (small modification to `Sheet.tsx`) OR simulate the close prop directly.

- [ ] **Step 4: Run + commit**

```powershell
pnpm --filter @water/app test src/worlds/WorldEntryIntakeSheet.test.tsx
git add app/src/worlds/
git commit -m "feat(worlds): WorldEntryIntakeSheet (intake reuse + orphan-draft reaping)"
```

**Phase E close-out.** Full world UI surface is now end-to-end: writer can navigate Index → Segment → Entry, edit single-doc or collection segments, create new entries via intake, edit names + aliases + fields inline. Phase F now lights up the character-voice integration.

---

## Phase F — Character speaker extension

### Task 25: `CharacterSpeaker::from_row` gains `&WorldRegistry` + `&SceneContext`

Extends the existing M3 character speaker to accept world context. Implementation just threads the parameters; template token wiring lands in Task 26.

**Files:**
- Modify: `crates/water-core/src/voice/character_template.rs` (or wherever `CharacterSpeaker::from_row` lives)
- Modify: every callsite

- [ ] **Step 1: Find all callsites**

```powershell
Select-String -Path "crates\water-core\src","app\src-tauri\src" -Pattern "CharacterSpeaker::from_row|CharacterSpeaker\s*::\s*from_row" -Recurse
```

- [ ] **Step 2: Extend the signature**

```rust
// Before (M3):
pub fn from_row(row: &CharacterRow, registry: &CharacterRegistry) -> CharacterSpeaker;

// After (M4):
pub fn from_row(
    row: &CharacterRow,
    char_registry: &CharacterRegistry,
    world_registry: &crate::world::WorldRegistry,
    scene: &SceneContext,
) -> CharacterSpeaker;
```

`SceneContext` may already exist; if not, the minimal shape is:

```rust
#[derive(Debug, Clone)]
pub struct SceneContext {
    pub scene_id: Id,
    pub location_id: Option<Id>,
    pub pov_character_id: Option<Id>,
    pub characters_present: Vec<Id>,
}
```

Add to `crates/water-core/src/voice/mod.rs` if absent.

- [ ] **Step 3: Update every callsite to pass the two new args**

Each callsite needs `world_registry` and `scene_context`. The orchestrator service path (after Task 13) already builds `world_registry`. The scene context can be derived from `SceneSnapshot`:

```rust
let scene_ctx = SceneContext {
    scene_id: scene.id.clone(),
    location_id: scene.location_id.clone(),
    pov_character_id: scene.pov_character_id.clone(),
    characters_present: scene.characters_present.clone(),
};
let speaker = CharacterSpeaker::from_row(&row, &char_registry, &world_registry, &scene_ctx);
```

- [ ] **Step 4: Write a thread-through test**

In `crates/water-core/src/voice/character_template.rs` tests:

```rust
#[test]
fn from_row_accepts_world_registry_and_scene_context_without_panic() {
    let dir = tempfile::tempdir().unwrap();
    let db = crate::Db::open_in_memory().unwrap();
    let p = crate::ProjectStore::new(&db).insert("P").unwrap();
    crate::world::WorldStore::new(&db, dir.path().to_path_buf())
        .seed_builtins(&p.id)
        .unwrap();
    let char_reg = crate::character::CharacterRegistry::default();
    let world_reg = crate::world::WorldRegistry::from_db(&db, &p.id, dir.path().to_path_buf()).unwrap();
    let row = mk_test_character_row("Aren");   // use whatever test helper exists
    let scene_ctx = SceneContext {
        scene_id: crate::Id::new(),
        location_id: None,
        pov_character_id: None,
        characters_present: vec![],
    };
    let _speaker = CharacterSpeaker::from_row(&row, &char_reg, &world_reg, &scene_ctx);
}
```

- [ ] **Step 5: Test + commit**

```powershell
cargo test -p water-core voice
cargo test -p water-app   # orchestrator integration test still green
git add crates/water-core/src/voice/ app/src-tauri/src/
git commit -m "feat(voice): CharacterSpeaker::from_row gains &WorldRegistry + &SceneContext"
```

---

### Task 26: `character_template.rs` `{{world.location_*}}` token integration

Wires the three location tokens into the rendered character voice prompt.

**Files:**
- Modify: `crates/water-core/src/voice/character_template.rs`

- [ ] **Step 1: Identify the template render path**

Find where the character voice template's `render` substitutes tokens. M3 uses `replace(&format!("{{{{{key}}}}}"), value)` per the handoff (M3 polish item #9 mentions this hot-spot).

- [ ] **Step 2: Inject world tokens**

In the render function, before the existing token substitution loop:

```rust
let mut tokens: HashMap<&str, String> = /* existing token map */;

// M4: location tokens. Empty string when scene.location_id is None
// OR when the referenced entry is not in the locations segment OR is
// missing fields.
let (loc_name, loc_sensory, loc_type) = if let Some(loc_id) = &scene_context.location_id {
    if let Some(snap) = world_registry.by_id(loc_id) {
        if snap.segment_slug == "locations" {
            let main = snap.data.get("main").and_then(|v| v.as_object());
            let sensory = main
                .and_then(|m| m.get("sensory_detail"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let ty = main
                .and_then(|m| m.get("type"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            (snap.name.clone(), sensory, ty)
        } else {
            (String::new(), String::new(), String::new())
        }
    } else {
        (String::new(), String::new(), String::new())
    }
} else {
    (String::new(), String::new(), String::new())
};
tokens.insert("world.location_name", loc_name);
tokens.insert("world.location_sensory_detail", loc_sensory);
tokens.insert("world.location_type", loc_type);
```

- [ ] **Step 3: Add the tokens to the template TOML**

Find the character voice template (likely `prompts/speakers/character/template.toml` or `crates/water-core/src/voice/character_template.rs` consts). Append a setting/context block:

```toml
# Where the character template's [user.template] string is — append:
# Setting (only present when scene has a location):
# {{world.location_name}} — {{world.location_type}}
# {{world.location_sensory_detail}}
```

The M3 line-based omission rule (drop lines that resolve to only-whitespace) means these lines disappear cleanly when tokens are empty.

If the character voice template doesn't yet use a TOML file (only Rust consts), embed the additional lines directly in the const string with a `\n{{world.location_name}} — {{world.location_type}}\n{{world.location_sensory_detail}}` suffix.

- [ ] **Step 4: Write the failing test**

```rust
#[test]
fn character_voice_prompt_includes_location_sensory_when_set() {
    // Setup: create a location entry with a distinctive sensory_detail,
    // build a world_reg, build a scene context with location_id set,
    // render the character voice prompt, assert the sensory_detail string appears.
    let dir = tempfile::tempdir().unwrap();
    let db = crate::Db::open_in_memory().unwrap();
    let p = crate::ProjectStore::new(&db).insert("P").unwrap();
    let store = crate::world::WorldStore::new(&db, dir.path().to_path_buf());
    store.seed_builtins(&p.id).unwrap();
    let loc = store.find_segment_by_slug(&p.id, "locations").unwrap().unwrap();
    let entry_id = store.create_entry(&loc.id, "Pell").unwrap();
    store.update_entry_field(
        &entry_id,
        "main.sensory_detail",
        &serde_json::json!("Dust thick enough to read fingertips in"),
    ).unwrap();

    let world_reg = crate::world::WorldRegistry::from_db(&db, &p.id, dir.path().to_path_buf()).unwrap();
    let char_reg = crate::character::CharacterRegistry::default();
    let row = mk_test_character_row("Aren");
    let scene_ctx = SceneContext {
        scene_id: crate::Id::new(),
        location_id: Some(entry_id),
        pov_character_id: None,
        characters_present: vec![],
    };
    let speaker = CharacterSpeaker::from_row(&row, &char_reg, &world_reg, &scene_ctx);
    let render = speaker.render(/* trigger context */);
    assert!(
        render.user.contains("Dust thick enough to read fingertips in"),
        "expected sensory_detail in prompt; got:\n{}",
        render.user,
    );
}

#[test]
fn character_voice_prompt_omits_location_lines_when_location_id_none() {
    let dir = tempfile::tempdir().unwrap();
    let db = crate::Db::open_in_memory().unwrap();
    let p = crate::ProjectStore::new(&db).insert("P").unwrap();
    crate::world::WorldStore::new(&db, dir.path().to_path_buf()).seed_builtins(&p.id).unwrap();
    let world_reg = crate::world::WorldRegistry::from_db(&db, &p.id, dir.path().to_path_buf()).unwrap();
    let char_reg = crate::character::CharacterRegistry::default();
    let row = mk_test_character_row("Aren");
    let scene_ctx = SceneContext {
        scene_id: crate::Id::new(),
        location_id: None,
        pov_character_id: None,
        characters_present: vec![],
    };
    let speaker = CharacterSpeaker::from_row(&row, &char_reg, &world_reg, &scene_ctx);
    let render = speaker.render(/* trigger context */);
    // No setting-line residue: no token name should appear as a literal.
    assert!(!render.user.contains("{{world."), "token left unrendered");
    // The phrase that always appears around the location line (if any) should
    // not survive when tokens are empty; check that for the chosen wording.
    // Concrete test depends on the actual template surface. Inspect render
    // output during implementation and refine assertions.
}
```

- [ ] **Step 5: Run + commit**

```powershell
cargo test -p water-core voice::character_template
git add crates/water-core/src/voice/ prompts/speakers/character/
git commit -m "feat(voice): CharacterSpeaker injects {{world.location_*}} tokens from WorldRegistry"
```

**Phase F close-out.** Spec § 10 exit criterion #6 met: setting `scene.location_id` causes the next character-voice dispatch to include the location's sensory detail.

---

## Phase G — Cross-feature wiring

### Task 27: `SceneMetadataSheet` location selector + `sceneSetLocation` UI

Adds the "Location" row to the scene metadata sheet (M3 T21).

**Files:**
- Modify: `app/src/scenes/SceneMetadataSheet.tsx`
- Modify: `app/src/scenes/SceneMetadataSheet.test.tsx`

- [ ] **Step 1: Inspect current SceneMetadataSheet**

```powershell
Get-Content app\src\scenes\SceneMetadataSheet.tsx
```

Note where POV is rendered (it's the closest parallel to what we're adding).

- [ ] **Step 2: Add location state + selector**

```tsx
const [locationOptions, setLocationOptions] = useState<WorldEntryIndexEntry[]>([]);

useEffect(() => {
  let cancelled = false;
  ipc.worldSegmentList().then(async (segs) => {
    const loc = segs.find((s) => s.slug === "locations");
    if (!loc) return;
    const entries = await ipc.worldEntryList(loc.id);
    if (cancelled) return;
    setLocationOptions(entries);
  });
  return () => {
    cancelled = true;
  };
}, []);

// In the render:
<div className="scene-metadata-row">
  <label>Location</label>
  <select
    value={metadata.location?.id ?? ""}
    onChange={async (e) => {
      const newId = e.target.value === "" ? null : e.target.value;
      await ipc.sceneSetLocation({ sceneId, locationId: newId });
      onMetadataChange();
    }}
    data-testid="scene-location-select"
  >
    <option value="">— none —</option>
    {locationOptions.map((opt) => (
      <option key={opt.id} value={opt.id}>
        {opt.name || "(unnamed)"}
      </option>
    ))}
  </select>
  {metadata.location && (
    <button
      onClick={async () => {
        await ipc.sceneSetLocation({ sceneId, locationId: null });
        onMetadataChange();
      }}
      data-testid="scene-location-clear"
    >
      ×
    </button>
  )}
</div>
```

- [ ] **Step 3: Tests**

Extend `SceneMetadataSheet.test.tsx`:

```tsx
it("renders the location dropdown populated from locations segment", async () => {
  (ipc.worldSegmentList as ReturnType<typeof vi.fn>).mockResolvedValue([
    { id: "seg-l", slug: "locations", name: "Locations", ordering: 1, is_collection: true, hue_token: "x", hidden: false, has_template_override: false },
  ]);
  (ipc.worldEntryList as ReturnType<typeof vi.fn>).mockResolvedValue([
    { id: "e1", segment_id: "seg-l", name: "The Pell Library", preview: "" },
  ]);
  // ... render the sheet; await dropdown; assert option visible ...
});

it("selecting a location calls sceneSetLocation with the id", async () => {
  // ... similar setup, then userEvent.selectOptions(select, "e1") ...
  await waitFor(() =>
    expect(ipc.sceneSetLocation).toHaveBeenCalledWith({ sceneId: "scn-1", locationId: "e1" }),
  );
});

it("clear button unsets location", async () => {
  // ... setup with location pre-set, click × button, assert sceneSetLocation called with null ...
});
```

- [ ] **Step 4: Commit**

```powershell
pnpm --filter @water/app test src/scenes/SceneMetadataSheet.test.tsx
git add app/src/scenes/
git commit -m "feat(scenes): SceneMetadataSheet gains Location selector"
```

---

### Task 28: `ChipSuggestion` discriminated payload + `SceneAutosuggestChips` extension

Extends the M3 autosuggest channel to carry world-entry suggestions alongside character suggestions, differentiated by hue.

**Files:**
- Modify: `app/src/scenes/sceneMetadataChannel.ts`
- Modify: `app/src/scenes/SceneAutosuggestChips.tsx`
- Modify: tests

- [ ] **Step 1: Inspect existing channel**

```powershell
Get-Content app\src\scenes\sceneMetadataChannel.ts
```

The M3 channel pushes character suggestions to subscribers.

- [ ] **Step 2: Extend the payload type**

```typescript
// In sceneMetadataChannel.ts (or wherever the type lives):
export type ChipSuggestion =
  | {
      kind: "character";
      characterId: string;
      characterName: string;
      matched: string;
    }
  | {
      kind: "world_entry";
      entryId: string;
      entryName: string;
      segmentSlug: string;
      matched: string;
    };
```

Update the channel's publish/subscribe to use `ChipSuggestion[]`.

- [ ] **Step 3: Extend the chip renderer**

In `SceneAutosuggestChips.tsx`:

```tsx
import type { ChipSuggestion } from "./sceneMetadataChannel";

function chipForSuggestion(s: ChipSuggestion, hueLookup: (slug: string) => string) {
  if (s.kind === "character") {
    return (
      <button
        key={`char-${s.characterId}`}
        className="autosuggest-chip"
        style={{ ["--chip-hue" as string]: "var(--water-hue-character-default)" }}
        data-testid={`chip-char-${s.characterId}`}
        onClick={() => /* existing add-character flow */}
      >
        + {s.characterName}
      </button>
    );
  }
  return (
    <button
      key={`world-${s.entryId}`}
      className="autosuggest-chip"
      style={{ ["--chip-hue" as string]: `var(${hueLookup(s.segmentSlug)})` }}
      data-testid={`chip-world-${s.entryId}`}
      onClick={() => /* set scene location to this entry */}
    >
      📍 {s.entryName}
    </button>
  );
}
```

The world-chip click handler calls `ipc.sceneSetLocation` with the chip's `entryId`.

- [ ] **Step 4: Wire the producer side**

The component (or scene editor) that produces chip suggestions today (character-autosuggest) needs a companion path for world. Concretely: when the editor's recent paragraph changes, call `ipc.worldAutosuggest({ sceneId, paragraph })` in parallel with the existing character autosuggest, then publish a merged `ChipSuggestion[]` to the channel.

Inspect existing producer:

```powershell
Select-String -Path "app\src\scenes","app\src\editor" -Pattern "autosuggest|publishToSceneMetadataChannel" -Recurse
```

Patch the producer to publish both:

```tsx
const [charHits, worldHits] = await Promise.all([
  ipc.characterAutosuggest({ sceneId, paragraph }),
  ipc.worldAutosuggest({ sceneId, paragraph }),
]);
const suggestions: ChipSuggestion[] = [
  ...charHits.map((h) => ({ kind: "character" as const, characterId: h.id, characterName: h.name, matched: h.matched })),
  ...worldHits.map((h) => ({ kind: "world_entry" as const, entryId: h.id, entryName: h.name, segmentSlug: "locations", matched: "" })),
];
sceneMetadataChannel.publish({ sceneId, suggestions });
```

If the existing channel publishes a different shape (single suggestion at a time), adapt accordingly.

- [ ] **Step 5: Test + commit**

Append tests covering:
- Mixed character + world suggestions render both chip kinds.
- Clicking a world chip calls `sceneSetLocation` with the entry id.
- Only `locations`-segment entries appear (other slugs filtered upstream by `world_autosuggest_core` in Task 10).

```powershell
pnpm --filter @water/app test src/scenes
git add app/src/scenes/
git commit -m "feat(scenes): ChipSuggestion discriminated payload + world-entry chips"
```

---

### Task 29: `pinned_pill.origin_trigger` plumbing + Chorus-stub pin handler

Threads `origin_trigger` through pin emission and adds the Chorus-stub creation branch to the pin handler.

**Files:**
- Modify: `app/src-tauri/src/commands/pill.rs` (or wherever `pill_pin` lives)
- Modify: orchestrator pill emission (set `origin_trigger` on emitted pills)

- [ ] **Step 1: Find the pin handler**

```powershell
Select-String -Path "app\src-tauri\src","crates\water-core\src" -Pattern "pill_pin|pinned_pill" -Recurse
```

Identify the request struct (`PinPillRequest`) and the insert statement.

- [ ] **Step 2: Add origin_trigger to PinPillRequest + response**

```rust
#[derive(Debug, Deserialize)]
pub struct PinPillRequest {
    pub scene_id: String,
    pub block_id: String,
    pub snippet: String,
    pub speaker_kind: String,
    pub speaker_id: String,
    pub message: String,
    pub hue: String,
    pub rabbit_hole_path: Option<String>,
    pub origin_trigger: Option<String>,   // NEW
}

#[derive(Debug, Serialize)]
pub struct PinPillResponse {
    pub pin_id: String,
    pub stub_entry_id: Option<String>,   // NEW
}
```

- [ ] **Step 3: Update the insert SQL**

```rust
db.conn().execute(
    "INSERT INTO pinned_pill (id, scene_id, block_id, snippet, speaker_kind, speaker_id, message, hue, rabbit_hole_path, origin_trigger, created_at)
     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
    (
        pin_id.as_str(),
        &req.scene_id,
        &req.block_id,
        &req.snippet,
        &req.speaker_kind,
        &req.speaker_id,
        &req.message,
        &req.hue,
        req.rabbit_hole_path.as_deref(),
        req.origin_trigger.as_deref(),
        &now,
    ),
)?;
```

- [ ] **Step 4: Add the stub-creation branch**

After the insert, before returning:

```rust
let stub_entry_id = if req.speaker_kind == "persona"
    && req.speaker_id == "chorus"
    && req.origin_trigger.as_deref() == Some("no_universe_yet")
{
    let project_id = read_project_id(&db).map_err(|e| e.to_string())?;
    let store = water_core::world::WorldStore::new(&db, project_root.clone());
    let loc_seg = store
        .find_segment_by_slug(&project_id, "locations")
        .map_err(|e| e.to_string())?
        .ok_or("locations segment missing")?;
    let entry_id = store
        .create_entry_seeded(&loc_seg.id, "", "main.sensory_detail", &req.snippet)
        .map_err(|e| e.to_string())?;
    Some(entry_id.to_string())
} else {
    None
};

Ok(PinPillResponse {
    pin_id: pin_id.to_string(),
    stub_entry_id,
})
```

- [ ] **Step 5: Ensure pill emission sets origin_trigger**

In the orchestrator's pill-emission path (where `TriggerCandidate`s are converted to outgoing pills), pass `trigger_id` through to the pill payload. Specifically, the renderer-facing `PillEvent` (or similar) needs an `origin_trigger: String` field that the renderer forwards when the user pins:

```rust
pub struct PillEvent {
    // ... existing fields ...
    pub origin_trigger: String,
}
```

And the renderer's pin call (Bouquet.tsx, Task 30) sends `origin_trigger: pill.origin_trigger`.

- [ ] **Step 6: Write the failing test**

```rust
#[tokio::test]
async fn pill_pin_with_chorus_no_universe_yet_creates_locations_stub() {
    use water_core::ProjectStore;
    let dir = tempfile::tempdir().unwrap();
    let db = water_core::Db::open(dir.path().join("project.db")).unwrap();
    let p = ProjectStore::new(&db).insert("P").unwrap();
    water_core::world::WorldStore::new(&db, dir.path().to_path_buf())
        .seed_builtins(&p.id)
        .unwrap();
    // Set up a scene (depends on existing scene-store helper).
    let manuscript = water_core::ManuscriptStore::new(&db).insert(&p.id, "M").unwrap();
    let chapter = water_core::ChapterStore::new(&db).insert(&manuscript, "C").unwrap();
    let scene = water_core::SceneStore::new(&db).insert(&manuscript, Some(&chapter), "S", 0).unwrap();

    let db = std::sync::Arc::new(tokio::sync::Mutex::new(db));
    let req = PinPillRequest {
        scene_id: scene.to_string(),
        block_id: "blk-1".to_string(),
        snippet: "A library that remembers the dust on your fingertips".to_string(),
        speaker_kind: "persona".to_string(),
        speaker_id: "chorus".to_string(),
        message: "—".to_string(),
        hue: "--water-hue-persona-chorus".to_string(),
        rabbit_hole_path: None,
        origin_trigger: Some("no_universe_yet".to_string()),
    };
    let resp = pill_pin_core(&db, dir.path(), req).await.unwrap();
    assert!(resp.stub_entry_id.is_some(), "stub should be created");

    let stub_id: water_core::Id = resp.stub_entry_id.unwrap().parse().unwrap();
    let store_db = db.lock().await;
    let store = water_core::world::WorldStore::new(&store_db, dir.path().to_path_buf());
    let entry = store.read_entry(&stub_id).unwrap();
    let main = entry.data.get("main").unwrap().as_object().unwrap();
    assert!(main.get("sensory_detail").unwrap().as_str().unwrap().contains("library that remembers"));
}

#[tokio::test]
async fn pill_pin_with_other_origin_does_not_create_stub() {
    // ... similar setup, origin_trigger = Some("character_dissonance") ...
    let resp = pill_pin_core(&db, dir.path(), req).await.unwrap();
    assert!(resp.stub_entry_id.is_none());
}
```

- [ ] **Step 7: Commit**

```powershell
cargo test -p water-app commands::pill
git add app/src-tauri/src/commands/pill.rs crates/water-core/src/orchestrator/
git commit -m "feat(pill): origin_trigger plumbed; Chorus + no_universe_yet creates locations stub"
```

---

### Task 30: Bouquet pin-context fix (M2 carry-over) + stub-creation UI route

Fixes the M2 debt where `Bouquet.tsx` passes empty `sceneId`/`blockId`/`snippet` to `ipc.pillPin`. Adds the renderer-side handling for `PinPillResponse.stub_entry_id` → route to new entry sheet.

**Files:**
- Modify: `app/src/pills/Bouquet.tsx` (or wherever pin is invoked)
- Modify: `app/src/App.tsx` (or routing root) to surface the stub-created entry view

- [ ] **Step 1: Find the pin call**

```powershell
Select-String -Path "app\src" -Pattern "pillPin|ipc\.pillPin" -Recurse
```

- [ ] **Step 2: Fix the pin call to pass real context**

The pill object received in `Bouquet` must already carry `sceneId`, `blockId`, `snippet`, `originTrigger`. If it doesn't (M2 lesson), trace the prop chain back to the source and thread these fields through. The pill emission event from the orchestrator already carries this data (Task 29 Step 5).

```tsx
async function handlePin(pill: PillEvent) {
  const resp = await ipc.pillPin({
    sceneId: pill.sceneId,
    blockId: pill.blockId,
    snippet: pill.snippet,
    speakerKind: pill.speakerKind,
    speakerId: pill.speakerId,
    message: pill.message,
    hue: pill.hue,
    rabbitHolePath: null,
    originTrigger: pill.originTrigger,
  });

  if (resp.stub_entry_id) {
    // Route to the new entry sheet so the writer sees the stub immediately.
    // Use whatever global routing primitive exists (likely a state setter
    // on App or a custom nav event).
    navToWorldEntry(/* locations segment id */, resp.stub_entry_id);
  }
}
```

`navToWorldEntry` is the route-jump primitive. If `App.tsx` doesn't yet have a way for `Bouquet.tsx` to push a route change, add one:

```tsx
// In App.tsx — expose a setter for the world view via context:
const WorldNavContext = createContext<(segmentId: string, entryId: string) => void>(() => {});

// In WorldsSurface wrapper, use the context's setter.

// In Bouquet.tsx:
const navToWorldEntry = useContext(WorldNavContext);
```

A simpler alternative: emit a custom event `window.dispatchEvent(new CustomEvent("water:nav-world-entry", { detail: { entryId } }))` and have `WorldsSurface` listen. Either approach works.

- [ ] **Step 3: Tests**

```tsx
it("pinning a Chorus + no_universe_yet pill creates a stub and routes to it", async () => {
  (ipc.pillPin as ReturnType<typeof vi.fn>).mockResolvedValue({
    pin_id: "p1",
    stub_entry_id: "stub-1",
  });
  const navSpy = vi.fn();
  // ... render Bouquet with WorldNavContext.Provider value={navSpy} ...
  // ... invoke pin on a Chorus+no_universe_yet pill ...
  await waitFor(() => expect(navSpy).toHaveBeenCalledWith(expect.anything(), "stub-1"));
});

it("pinning a non-stub pill does not navigate", async () => {
  (ipc.pillPin as ReturnType<typeof vi.fn>).mockResolvedValue({ pin_id: "p1", stub_entry_id: null });
  const navSpy = vi.fn();
  // ... render + pin a character_dissonance pill ...
  await waitFor(() => expect(ipc.pillPin).toHaveBeenCalled());
  expect(navSpy).not.toHaveBeenCalled();
});
```

- [ ] **Step 4: Commit**

```powershell
pnpm --filter @water/app test src/pills
git add app/src/pills/ app/src/App.tsx
git commit -m "fix(pill): Bouquet passes real pin context; route to stub entry on Chorus stub creation"
```

**Phase G close-out.** All cross-feature wiring in place. Phase H now ships the template editor.

---

## Phase H — Template editor + settings

### Task 31: `SegmentTemplateEditor` modal (creation + edit modes)

Minimal new-segment modal per spec § 4.6. v1 cuts: no drag, no kind-edit, no choice authoring, append-only on built-ins.

**Files:**
- Create: `app/src/worlds/SegmentTemplateEditor.tsx`
- Create: `app/src/worlds/SegmentTemplateEditor.test.tsx`
- Modify: `app/src/worlds/WorldsSurface.tsx` (mount in `new-segment` view)

- [ ] **Step 1: Author the editor**

```tsx
import { useState } from "react";
import { ipc, type IntakeField, type IntakeSchemaSection } from "../ipc/commands";

type EditableField = {
  label: string;
  promptQuestion: string;
  kind: "short_text" | "long_text" | "string_list";
  optional: boolean;
};

function deriveFieldId(label: string, kind: EditableField["kind"]): string {
  const slug = label.trim().toLowerCase().replace(/[^a-z0-9]+/g, "_").replace(/^_|_$/g, "");
  const section = kind === "string_list" ? "lists" : "main";
  return `${section}.${slug}`;
}

export function SegmentTemplateEditor({
  mode,
  initial,
  onSave,
  onClose,
}: {
  mode: "create" | "edit";
  initial?: { name: string; isCollection: boolean; fields: IntakeField[]; isBuiltin: boolean; segmentId: string };
  onSave: (id: string) => void;
  onClose: () => void;
}) {
  const [name, setName] = useState(initial?.name ?? "");
  const [isCollection, setIsCollection] = useState(initial?.isCollection ?? false);
  const [fields, setFields] = useState<EditableField[]>(
    initial?.fields.map((f) => ({
      label: f.label,
      promptQuestion: f.prompt_question,
      kind: f.kind.type === "choice" ? "short_text" : (f.kind.type as EditableField["kind"]),
      optional: f.optional_skip,
    })) ?? [],
  );

  const isAppendOnly = mode === "edit" && (initial?.isBuiltin ?? false);
  const lockedFieldCount = isAppendOnly ? (initial?.fields.length ?? 0) : 0;

  function addField() {
    setFields([...fields, { label: "", promptQuestion: "", kind: "short_text", optional: false }]);
  }

  function removeField(i: number) {
    if (i < lockedFieldCount) return;   // built-in fields are not removable
    setFields(fields.filter((_, idx) => idx !== i));
  }

  async function handleSave() {
    const intakeFields: IntakeField[] = fields.map((f) => ({
      id: deriveFieldId(f.label, f.kind),
      label: f.label,
      prompt_question: f.promptQuestion,
      kind: { type: f.kind } as IntakeField["kind"],
      optional_skip: f.optional,
    }));
    const template: IntakeSchemaSection = {
      id: initial?.segmentId ?? name.trim().toLowerCase().replace(/[^a-z0-9]+/g, "_"),
      label: name,
      fields: intakeFields,
    };
    if (mode === "create") {
      const newId = await ipc.worldSegmentCreate({ name, isCollection, template });
      onSave(newId);
    } else {
      await ipc.worldSegmentUpdateTemplate({ segmentId: initial!.segmentId, template });
      onSave(initial!.segmentId);
    }
  }

  return (
    <div className="segment-template-editor" data-testid="segment-template-editor">
      <h3>{mode === "create" ? "New segment" : "Edit template"}</h3>
      <label>
        Name
        <input
          type="text"
          value={name}
          onChange={(e) => setName(e.target.value)}
          disabled={mode === "edit" && isAppendOnly}
          data-testid="segment-name-input"
        />
      </label>
      <fieldset>
        <legend>Type</legend>
        <label>
          <input
            type="radio"
            checked={!isCollection}
            onChange={() => setIsCollection(false)}
            disabled={mode === "edit"}
          />
          Single document
        </label>
        <label>
          <input
            type="radio"
            checked={isCollection}
            onChange={() => setIsCollection(true)}
            disabled={mode === "edit"}
          />
          Collection
        </label>
      </fieldset>
      <div className="fields-editor">
        <h4>Fields</h4>
        {fields.map((f, i) => {
          const locked = i < lockedFieldCount;
          return (
            <div key={i} className="field-row" data-testid={`field-row-${i}`}>
              <input
                type="text"
                value={f.label}
                onChange={(e) =>
                  setFields(fields.map((x, idx) => (idx === i ? { ...x, label: e.target.value } : x)))
                }
                placeholder="Label"
                disabled={locked}
                data-testid={`field-label-${i}`}
              />
              <select
                value={f.kind}
                onChange={(e) =>
                  setFields(
                    fields.map((x, idx) =>
                      idx === i ? { ...x, kind: e.target.value as EditableField["kind"] } : x,
                    ),
                  )
                }
                disabled={locked}
              >
                <option value="short_text">short text</option>
                <option value="long_text">long text</option>
                <option value="string_list">list</option>
              </select>
              <input
                type="text"
                value={f.promptQuestion}
                onChange={(e) =>
                  setFields(
                    fields.map((x, idx) => (idx === i ? { ...x, promptQuestion: e.target.value } : x)),
                  )
                }
                placeholder="Prompt question"
                disabled={locked}
              />
              <label>
                <input
                  type="checkbox"
                  checked={f.optional}
                  onChange={(e) =>
                    setFields(fields.map((x, idx) => (idx === i ? { ...x, optional: e.target.checked } : x)))
                  }
                  disabled={locked}
                />
                optional
              </label>
              {!locked && (
                <button onClick={() => removeField(i)} data-testid={`field-remove-${i}`}>
                  ×
                </button>
              )}
              {locked && <span className="field-locked-label">(built-in)</span>}
            </div>
          );
        })}
        <button onClick={addField} data-testid="add-field-button">
          + Add field
        </button>
      </div>
      <div className="actions">
        <button onClick={onClose}>Cancel</button>
        <button onClick={handleSave} data-testid="save-button">
          {mode === "create" ? "Create" : "Save"}
        </button>
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Mount in `WorldsSurface`**

Replace the `new-segment-placeholder`:

```tsx
{view.kind === "new-segment" && (
  <SegmentTemplateEditor
    mode="create"
    onSave={(newId) => setView({ kind: "segment", segmentId: newId })}
    onClose={() => setView({ kind: "index" })}
  />
)}
```

Import: `import { SegmentTemplateEditor } from "./SegmentTemplateEditor";`

- [ ] **Step 3: Tests**

```tsx
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { SegmentTemplateEditor } from "./SegmentTemplateEditor";

vi.mock("../ipc/commands", () => ({
  ipc: {
    worldSegmentCreate: vi.fn(),
    worldSegmentUpdateTemplate: vi.fn(),
  },
}));

import { ipc } from "../ipc/commands";

describe("SegmentTemplateEditor", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    (ipc.worldSegmentCreate as ReturnType<typeof vi.fn>).mockResolvedValue("new-seg-id");
  });

  it("create mode saves a new segment with derived field ids", async () => {
    const onSave = vi.fn();
    render(<SegmentTemplateEditor mode="create" onSave={onSave} onClose={() => {}} />);
    await userEvent.type(screen.getByTestId("segment-name-input"), "Magic Systems");
    await userEvent.click(screen.getByLabelText("Collection"));
    await userEvent.click(screen.getByTestId("add-field-button"));
    await userEvent.type(screen.getByTestId("field-label-0"), "System Name");
    await userEvent.click(screen.getByTestId("save-button"));
    await waitFor(() => expect(ipc.worldSegmentCreate).toHaveBeenCalled());
    const call = (ipc.worldSegmentCreate as ReturnType<typeof vi.fn>).mock.calls[0][0];
    expect(call.name).toBe("Magic Systems");
    expect(call.isCollection).toBe(true);
    expect(call.template.fields[0].id).toBe("main.system_name");
    expect(onSave).toHaveBeenCalledWith("new-seg-id");
  });

  it("edit mode on built-in segment locks existing fields and disables type/name", async () => {
    render(
      <SegmentTemplateEditor
        mode="edit"
        initial={{
          name: "Concept",
          isCollection: false,
          fields: [
            {
              id: "main.core_premise",
              label: "Core Premise",
              prompt_question: "?",
              kind: { type: "long_text" },
              optional_skip: false,
            },
          ],
          isBuiltin: true,
          segmentId: "seg-concept",
        }}
        onSave={() => {}}
        onClose={() => {}}
      />,
    );
    expect(screen.getByTestId("segment-name-input")).toBeDisabled();
    expect(screen.getByTestId("field-label-0")).toBeDisabled();
    expect(screen.queryByTestId("field-remove-0")).not.toBeInTheDocument();
  });

  it("edit mode allows appending new fields on built-in segments", async () => {
    render(
      <SegmentTemplateEditor
        mode="edit"
        initial={{
          name: "Concept",
          isCollection: false,
          fields: [
            {
              id: "main.core_premise",
              label: "Core Premise",
              prompt_question: "?",
              kind: { type: "long_text" },
              optional_skip: false,
            },
          ],
          isBuiltin: true,
          segmentId: "seg-concept",
        }}
        onSave={() => {}}
        onClose={() => {}}
      />,
    );
    await userEvent.click(screen.getByTestId("add-field-button"));
    expect(screen.getByTestId("field-row-1")).toBeInTheDocument();
    expect(screen.getByTestId("field-label-1")).not.toBeDisabled();
    expect(screen.getByTestId("field-remove-1")).toBeInTheDocument();
  });
});
```

- [ ] **Step 4: Commit**

```powershell
pnpm --filter @water/app test src/worlds/SegmentTemplateEditor.test.tsx
git add app/src/worlds/
git commit -m "feat(worlds): SegmentTemplateEditor (create + edit, append-only on built-ins)"
```

---

### Task 32: World settings: hide/show toggle + delete user-added

Adds the "..." menu on the World index that exposes the hide/show + delete actions per segment.

**Files:**
- Create: `app/src/worlds/WorldSettingsMenu.tsx`
- Modify: `app/src/worlds/WorldIndex.tsx`
- Create: `app/src/worlds/WorldSettingsMenu.test.tsx`

- [ ] **Step 1: Author the settings menu**

```tsx
import { useState } from "react";
import { ipc, type WorldSegment } from "../ipc/commands";

const BUILTIN_SLUGS = new Set([
  "concept", "locations", "politics_and_social", "culture", "world", "history",
]);

export function WorldSettingsMenu({
  segments,
  onChanged,
  onClose,
}: {
  segments: WorldSegment[];
  onChanged: () => void;
  onClose: () => void;
}) {
  return (
    <div className="world-settings-menu" data-testid="world-settings-menu">
      <h3>World segments</h3>
      <button onClick={onClose} className="close-x">×</button>
      <ul>
        {segments.map((s) => {
          const isBuiltin = BUILTIN_SLUGS.has(s.slug);
          return (
            <li key={s.id}>
              <label>
                <input
                  type="checkbox"
                  checked={!s.hidden}
                  onChange={async (e) => {
                    await ipc.worldSegmentSetHidden({ segmentId: s.id, hidden: !e.target.checked });
                    onChanged();
                  }}
                  data-testid={`visibility-${s.slug || s.id}`}
                />
                {s.name}
                {isBuiltin && <span className="badge-builtin">built-in</span>}
              </label>
              {!isBuiltin && (
                <button
                  onClick={async () => {
                    if (confirm(`Delete segment "${s.name}"? This cannot be undone.`)) {
                      await ipc.worldSegmentDelete(s.id);
                      onChanged();
                    }
                  }}
                  data-testid={`delete-${s.id}`}
                >
                  Delete
                </button>
              )}
            </li>
          );
        })}
      </ul>
    </div>
  );
}
```

- [ ] **Step 2: Wire into WorldIndex**

```tsx
const [showSettings, setShowSettings] = useState(false);

// In render, near the heading:
<button onClick={() => setShowSettings(true)} data-testid="world-settings-button">⋯</button>

{showSettings && (
  <WorldSettingsMenu
    segments={segments}
    onChanged={async () => {
      const rows = await ipc.worldSegmentList();
      setSegments(rows);
    }}
    onClose={() => setShowSettings(false)}
  />
)}
```

- [ ] **Step 3: Tests**

```tsx
it("toggling visibility calls worldSegmentSetHidden", async () => {
  // ... render menu with one segment visible ...
  await userEvent.click(screen.getByTestId("visibility-concept"));
  await waitFor(() =>
    expect(ipc.worldSegmentSetHidden).toHaveBeenCalledWith({ segmentId: "seg-c", hidden: true }),
  );
});

it("delete button is hidden for built-in segments", async () => {
  // ... render with only built-in slugs ...
  expect(screen.queryByTestId(/^delete-/)).not.toBeInTheDocument();
});

it("delete button is shown for user-added segments and calls worldSegmentDelete on confirm", async () => {
  vi.spyOn(window, "confirm").mockReturnValue(true);
  // ... render with one user-added segment (slug = "") ...
  await userEvent.click(screen.getByTestId("delete-seg-user"));
  await waitFor(() => expect(ipc.worldSegmentDelete).toHaveBeenCalledWith("seg-user"));
});
```

- [ ] **Step 4: Commit**

```powershell
pnpm --filter @water/app test src/worlds/WorldSettingsMenu.test.tsx
git add app/src/worlds/
git commit -m "feat(worlds): settings menu — visibility toggle + delete user-added segments"
```

**Phase H close-out.** Spec § 10 exit criterion #2 met: user can add a new segment with a custom template and intake walks it.

---

## Phase I — Audit, fixture, smoke, tag

### Task 33: `eval/m4_acceptance/pell_library.toml` + 4 test scenes

Builds the M4 acceptance fixture analog to M3's `marcus_vale.toml`. Used for the manual smoke walk in Task 34 and as the regression fixture for the `world_drift` trigger.

**Files:**
- Create: `eval/m4_acceptance/pell_library.toml`
- Create: `eval/m4_acceptance/scenes/consistent.md`
- Create: `eval/m4_acceptance/scenes/contradiction_sunlight.md`
- Create: `eval/m4_acceptance/scenes/contradiction_elevation.md`
- Create: `eval/m4_acceptance/scenes/near_miss_unrelated.md`
- Create: `eval/m4_acceptance/README.md`

- [ ] **Step 1: Author the Pell Library fixture**

Use `[System.IO.File]::WriteAllText(...)` for clean UTF-8 (PowerShell 5.1's `Set-Content -Encoding UTF8` writes a BOM):

```powershell
$pell = @'
schema_version = "locations@1"
id = "01J4PELL000000000000000000"
name = "The Pell Library"
aliases = ["Pell", "the library", "Aren's old place"]

[main]
type = "underground library"
sensory_detail = "Dust thick enough to read fingertips in. The air smells of cold stone and old paper. No natural light reaches the lower stacks; brass lanterns guttering on iron hooks."
significance = "Aren spent her childhood here under the sub-basement's east wing. She still flinches at the smell of dust."

[lists]
notable_features = ["the sub-basement", "the locked east wing", "no natural light", "brass lanterns"]
'@
[System.IO.File]::WriteAllText("eval\m4_acceptance\pell_library.toml", $pell)
```

- [ ] **Step 2: Author the four test scenes**

```powershell
$consistent = @'
She crossed into the sub-basement of Pell, lantern low. Dust webbed the brass hooks where the lamps used to hang. The locked east wing waited at the far end, its iron door cold under her palm.
'@
[System.IO.File]::WriteAllText("eval\m4_acceptance\scenes\consistent.md", $consistent)

$contra_sun = @'
She returned to The Pell Library at noon, sunlight warming the high reading desk, gold pouring through the long windows. The east wing's iron door stood ajar; she had not seen it open in years.
'@
[System.IO.File]::WriteAllText("eval\m4_acceptance\scenes\contradiction_sunlight.md", $contra_sun)

$contra_elev = @'
Pell perched on the cliff overlooking the harbor, its tower visible from the docks below. The library's locked east wing was open to the wind, salt threading through every page.
'@
[System.IO.File]::WriteAllText("eval\m4_acceptance\scenes\contradiction_elevation.md", $contra_elev)

$near_miss = @'
She had not thought of Pell in days. The merchants in the harbor square were trading silk for olive oil; her ledger ran thin. Pell would have to wait until the next quarter.
'@
[System.IO.File]::WriteAllText("eval\m4_acceptance\scenes\near_miss_unrelated.md", $near_miss)
```

Note: the `contradiction_*` scenes plant contradictions against `main.type` ("underground library", contradicted by "perched on the cliff" or "sunlight" + "no natural light") and `lists.notable_features` ("no natural light", contradicted by "sunlight"). The `near_miss_unrelated.md` mentions Pell with no overlapping content words from `[main]` — the contextual-overlap pre-check should suppress this without an LLM call.

- [ ] **Step 3: Author the README**

```powershell
$readme = @'
# M4 acceptance fixture — Pell Library

Reference world entry + four test scenes for validating the world_drift trigger.

## Expected behaviour

| Scene                              | Expected Stage 1 | Expected Stage 2 | Pill fires? |
|------------------------------------|------------------|------------------|-------------|
| consistent.md                      | match            | consistent       | NO          |
| contradiction_sunlight.md          | match            | contradicts      | YES         |
| contradiction_elevation.md         | match            | contradicts      | YES         |
| near_miss_unrelated.md             | suppressed       | (skipped)        | NO          |

The `near_miss` scene contains the string "Pell" but no content overlap with the
entry's [main] block — the contextual-overlap pre-check (≥ 2 content words)
suppresses the candidate before any LLM call.

## Usage

1. Open Water; create a fresh project.
2. Open the World nav → Locations → "+ New entry".
3. Import the values from `pell_library.toml` (name, aliases, sensory_detail, etc.).
4. Open a new scene; set `location_id` to Pell.
5. Paste each test scene's text in turn into the editor.
6. Observe pill margin.

Reference for the M3 character-side equivalent: `eval/m3_acceptance/marcus_vale.toml`.
'@
[System.IO.File]::WriteAllText("eval\m4_acceptance\README.md", $readme)
```

- [ ] **Step 4: Commit**

```powershell
git add eval/m4_acceptance/
git commit -m "test(m4): Pell Library acceptance fixture + 4 test scenes for world_drift"
```

---

### Task 34: Manual smoke walk + tag `m4` + write M5 handoff

The closing task. Walks the 12-step smoke checklist from spec § 13, records pass/fail per step, tags `m4` if clean, and writes the M5 handoff document.

**Files:**
- Modify: `KNOWN_FRAGILE.md` (append entries #18-#22 per spec § 14)
- Create: `docs/superpowers/handoffs/2026-MM-DD-m5-handoff.md` (date = tag day)
- Possibly modify: spec doc with a closing status note

- [ ] **Step 1: Final pre-tag full-suite green check**

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"
cargo test -p water-core
cargo test -p water-app
pnpm --filter @water/app test
```

Expected: all green. Note the test count delta from `m3` baseline (201 + 19 + 157 + 8 = 385 at `m3`).

- [ ] **Step 2: Walk the manual smoke checklist (spec § 13)**

Boot the dev build:

```powershell
pnpm --filter @water/app tauri dev
```

For each of the 12 steps in spec § 13, record PASS / FAIL / NOTE. Any FAIL becomes either:
- An immediate hotfix (small adaptation; commit before tag), OR
- A documented limitation in KNOWN_FRAGILE + M5 handoff (large adaptation; ship anyway).

Record results in a temporary scratch doc (do NOT commit the scratch doc).

- [ ] **Step 3: Update `KNOWN_FRAGILE.md` with #18-#22**

Append to `KNOWN_FRAGILE.md`:

```markdown
## #18 — Name-collision resolution is heuristic
Character-in-scene wins; both-fire fallback in ambiguous cases. May mis-classify
when a character is referenced but not yet added to `characters_present`.
Mitigation: M3 autosuggest chip surfaces the missing character.

Added: M4 (`world_drift` Phase D).
Owner: `crates/water-core/src/world/collision.rs::resolve_token_kind`.

## #19 — `world_drift` contextual-overlap pre-check can miss real contradictions
Heuristic ≥ 2 content-word overlap (minus stopwords) between paragraph and entry
`[main]` text. Drops paragraphs where the writer's mention and the contradiction
are stylistically distant. Future fix: lower threshold OR embedding similarity (M5+).

Added: M4.
Owner: `crates/water-core/src/orchestrator/triggers/world_drift.rs::has_contextual_overlap`.

## #20 — Cartographer pill hue follows persona-hue, not segment-hue
Pills from a Locations contradiction look identical to pills from a Concept
contradiction. v1 limitation; parent spec § 4.6 doesn't require segment-hued pills.

Added: M4.
Owner: `prompts/speakers/cartographer/template.toml` (hue_token).

## #21 — Unnamed Chorus stubs persist indefinitely
No auto-reaping. Could become hygiene debt; M5+ may add a "reap unnamed entries
> N days" pass.

Added: M4.
Owner: `app/src-tauri/src/commands/pill.rs::pill_pin_core` (stub creation branch).

## #22 — Case-sensitivity asymmetry between character autosuggest and world_drift
Character autosuggest is case-sensitive on word boundaries; `world_drift` Stage 1
is case-insensitive. Intentional (place names case-vary more than character names
in English prose) but worth a note.

Added: M4.
Owner: `crates/water-core/src/world/registry.rs::find_by_token` (case-folded);
contrast with `crates/water-core/src/character/autosuggest.rs` (case-sensitive).
```

- [ ] **Step 4: Tag `m4`**

```powershell
git add KNOWN_FRAGILE.md
git commit -m "docs(m4): KNOWN_FRAGILE #18-#22 (M4 documented limitations)"
git tag m4
```

- [ ] **Step 5: Write the M5 handoff**

Create `docs/superpowers/handoffs/2026-MM-DD-m5-handoff.md` (replace MM-DD with the tag day). Use the M3 → M4 handoff (`2026-05-19-m4-handoff.md`) as a template. Sections to include:

1. **One-paragraph product context** (carry forward + add M4's contribution).
2. **Repo state inheriting**: tag table including `m4`, branch, tree state, test counts.
3. **The M5 spec, in your hands** — M5 = Heatmap Audiovisualizer per parent spec § 4.7. Components: per-scene metric rollup, scene-summary subsystem, beat-label subsystem, heatmap cache, renderer. Exit criteria as listed.
4. **The absolutely-do-not-violate constraints** — carry forward M2/M3/M4 + flag M5-specific (e.g. local-first scene summarization is the M5 add).
5. **Recommended first steps** — same shape as M4 handoff: smoke M4, brainstorm M5, plan-write, execute via subagent-driven-development.
6. **Carried-over debt from M4** — list any FAIL steps from the smoke walk + remaining polish backlog items not bundled into M4.
7. **Hard-earned lessons from M4** — process lessons, Vitest / TypeScript gotchas, Rust gotchas, Windows / PowerShell items. Carry forward from M3 handoff with M4-specific additions.
8. **Critical files and where to find things** — same shape; update for new world/ files.
9. **Quick-resume block for a fresh session**.
10. **Open questions to bring into the M5 brainstorm** — heatmap rendering technology choice, local-LLM model selection for scene summaries, etc.

- [ ] **Step 6: Commit the handoff**

```powershell
git add docs/superpowers/handoffs/2026-MM-DD-m5-handoff.md
git commit -m "docs(m5): handoff covering M4 close-out + M5 (Heatmap Audiovisualizer) entry"
```

- [ ] **Step 7: Update plan + spec status**

Append to `docs/superpowers/plans/2026-05-19-m4-world-bible.md` at the top (under the Status line):

```markdown
**Status:** Closed at tag `m4` on YYYY-MM-DD. All 34 tasks complete. N amendments recorded. ~M tests at tag (vs. 385 at `m3`).
```

Append to `docs/superpowers/specs/2026-05-19-m4-world-bible-design.md` similarly.

```powershell
git add docs/superpowers/plans/2026-05-19-m4-world-bible.md docs/superpowers/specs/2026-05-19-m4-world-bible-design.md
git commit -m "docs(m4): close plan + spec at m4 tag"
```

- [ ] **Step 8: Final state check**

```powershell
git tag
git log --oneline -5
```

Expected: `m4` tag present. Working tree clean. Plan, spec, KNOWN_FRAGILE, and M5 handoff all committed.

**Phase I close-out — and M4 close-out.** The World/Setting Bible is shipped. All four spec § 10 exit criteria are met. The writer can build a world, populate it, link scenes to locations, watch `world_drift` catch contradictions, and pin a Chorus pill to spawn a stub entry. Next session: M5 Heatmap Audiovisualizer.

---

## Appendix: Spec coverage map

| Spec § | Requirement | Task(s) |
|---|---|---|
| § 1.1 Rust core layout | `world/store.rs`, `templates.rs`, `registry.rs`, `collision.rs`, `autosuggest.rs` | 2, 3, 4, 5, 6, 15 |
| § 1.2 Tauri commands | `world.rs`, `state.rs::WorldWriteLocks`, `pill.rs` extension | 8, 9, 10, 11, 12, 29 |
| § 1.3 React UI | `WorldsSurface`, `WorldIndex`, `WorldSegmentView`, `WorldEntrySheet`, `WorldEntryIntakeSheet`, `SegmentTemplateEditor`, `flattenSerdeFlatten` | 19, 20, 21, 22, 23, 24, 31, 32 |
| § 2.1 v4 migration | All migration columns + `pinned_pill.origin_trigger` | 1 |
| § 2.2 single-doc TOML | `world/<slug>.toml` round-trip | 4 |
| § 2.3 collection TOML | `world/<slug>/<entry-ulid>.toml` round-trip | 5 |
| § 2.4 template TOML | `world/_segments/<slug>.template.toml` | 3 (creation), 7 (rebuild scan) |
| § 2.5 `WorldWriteLocks` | `state.rs::OpenProject` extension | 12 |
| § 2.6 `rebuild.rs` | scan, seed, orphan repair | 7 |
| § 3.1 `WorldStore` | full CRUD surface | 3, 4, 5 |
| § 3.2 `BUILT_IN_TEMPLATES` | 6 segment templates | 2 |
| § 3.3 `WorldRegistry` | snapshot + case-insensitive index | 6 |
| § 3.4 Speakers | `CartographerSpeaker`, `CharacterSpeaker` extension | 14, 18, 25, 26 |
| § 3.5 voice router | `WORLD_TRACK_TRIGGERS` | 14 |
| § 3.6 OrchestratorContext | `world_registry` field | 13 |
| § 4.1 routing | `WorldsSurface` 5-view discriminant | 20 |
| § 4.2 index | `WorldIndex` + `WorldSegmentTile` | 20 |
| § 4.3 segment view | single-doc + collection branches | 21, 22 |
| § 4.4 entry sheet | inline + aliases | 23 |
| § 4.5 intake reuse | `WorldEntryIntakeSheet` + orphan reaping | 24 |
| § 4.6 template editor | `SegmentTemplateEditor` minimal v1 | 31 |
| § 4.7 IPC surface | all `ipc.world*` methods | 8, 9, 10 |
| § 5.1 Stage 1 | `WorldDriftEvaluator` + contextual-overlap + cooldown | 16 |
| § 5.2 Stage 2 | `pill_world_drift_check.toml` + confirmation handler | 17 |
| § 5.3 Cartographer template | tone clause + 3-layer audit | 18 |
| § 5.4 trigger registry | registration | 16 |
| § 6.1 scene ↔ world | `sceneSetLocation` + `sceneReadMetadata.location` | 11, 27 |
| § 6.2 autosuggest | discriminated `ChipSuggestion` payload | 28 |
| § 6.3 Chorus pin → stub | `pill_pin_core` Chorus branch + UI route | 29, 30 |
| § 6.4 settings | hide/show + delete | 32 |
| § 10 exit criteria | all four (plus M4 additions #5-#7) | 33, 34 (manual smoke) |
| § 13 manual smoke checklist | 12-step walk | 34 |
| § 14 KNOWN_FRAGILE #18-#22 | recorded at tag time | 34 |










