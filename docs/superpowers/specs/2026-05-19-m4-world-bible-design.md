# M4 — World/Setting Bible Design Spec

> **Status.** Brainstorm-locked 2026-05-19. Built on the M3 architecture (`m3` tag, `846f9a8`). All open questions in the M3-handoff § 10 are resolved below in § 11. This is the design layer; the executable plan is produced next by `superpowers:writing-plans`.

**Parent spec.** `docs/superpowers/specs/2026-05-16-water-design.md` — § 3.6 (World templates), § 3.0 (`world_segment` + `world_entry` tables), § 4.6 (M4 exit criteria), § (no-universe-yet story).

**Predecessor.** `docs/superpowers/specs/2026-05-18-m3-character-sheets.md` (closed). M4 reuses the M3 architectural shape almost wholesale.

**Handoff.** `docs/superpowers/handoffs/2026-05-19-m4-handoff.md`.

---

## 0. One-paragraph scope

M4 wires world entries into the same pill engine that M3 wired character voices into. It ships 6 default world segments (`concept`, `locations`, `politics_and_social`, `culture`, `world`, `history`) using `is_collection` to distinguish single-doc segments (one inline-editable sheet) from collection segments (CharacterIndex-shaped grid + per-entry sheets). The schema-agnostic `ConversationalIntake` component from M3 is reused unchanged by feeding it `IntakeSchemaSection[]` for world templates. A minimal segment template editor lets users add custom segments (full polish deferred to M7). The `world_drift` trigger fires on the M3 two-stage confirmation pattern — Stage 1 cheap name/alias match, Stage 2 LLM YES/NO/unclear check — surfacing a Cartographer persona pill. Voice router gains a `WORLD_TRACK_TRIGGERS` const + `Cartographer` hard-wire for `world_drift`. Cross-milestone integration: `CharacterSpeaker` voice templates gain access to a `WorldRegistry` so a scene's `location_id` injects sensory-detail excerpts into the character voice prompt. The "no-universe-yet" eliciting story is closed end-to-end: pinning a Chorus pill triggered by `no_universe_yet` creates a `world_entry` stub in the `locations` segment and routes the writer to its sheet.

---

## 1. Architecture overview

M4 mirrors M3's structural pattern. One added module surface (templates-as-data), one cross-cutting integration (Speakers accept `&WorldRegistry`), one new trigger (`world_drift`), one new persona speaker (Cartographer).

### 1.1 Rust core — `crates/water-core/src/`

```
world/
  mod.rs              -- re-exports + WorldStore wiring
  store.rs            -- segment + entry CRUD, TOML round-trip, file_hash gate
  templates.rs        -- BUILT_IN_TEMPLATES const: 6 IntakeSchemaSection records
                       -- effective_template(db, segment_id) -> resolves user override vs. built-in default
  registry.rs         -- WorldRegistry: in-memory snapshot of segments + entries, name+alias index
  autosuggest.rs      -- world_entry name scanner over scene paragraphs (mirrors character/autosuggest.rs)
  collision.rs        -- resolve_token_kind: shared character-vs-world collision resolver
voice/
  router.rs           -- (extended) WORLD_TRACK_TRIGGERS const; world_drift -> Cartographer
  cartographer_template.rs  -- (NEW) world-aware persona template loader, parallel to character_template.rs
  character_template.rs     -- (EXTENDED) accepts &WorldRegistry; adds {{world.location_*}} tokens
orchestrator/
  triggers/world_drift.rs   -- (NEW) Stage 1 evaluator; emits requires_confirmation: true
  mod.rs                     -- (extended) OrchestratorContext carries world_registry
prompts/
  pill_world_drift_check.toml          -- (NEW) Stage 2 confirmation prompt
  speakers/cartographer/template.toml  -- (NEW) Cartographer voice template
sql/
  v4_world_bible.sql  -- (NEW) migration: world_segment + world_entry column additions; pinned_pill.origin_trigger
```

### 1.2 Tauri commands — `app/src-tauri/src/commands/`

```
world.rs   -- (NEW) all world_* commands per § 4.6, _core async-fn pattern
scene.rs   -- (EXTENDED) scene_set_location + scene_read_metadata returns location
state.rs   -- (EXTENDED) OpenProject gains world_write_locks: WorldWriteLocks (DashMap<Id, Arc<Mutex<()>>>)
pill.rs    -- (EXTENDED) pin handler creates world_entry stub on Chorus+no_universe_yet pin
```

### 1.3 React UI — `app/src/`

```
worlds/
  WorldsSurface.tsx          -- 3-level routing: index | segment | entry | entry-intake | new-segment
  WorldIndex.tsx             -- glowing-tile grid of segments
  WorldSegmentTile.tsx       -- one tile (preview + entry count for collections)
  WorldSegmentView.tsx       -- single-doc -> sheet; collection -> grid + '+ New entry'
  WorldEntryCard.tsx         -- card in collection grid (parallel to CharacterCard)
  WorldEntrySheet.tsx        -- inline-edit sheet for one entry (parallel to CharacterSheet)
  WorldEntryIntakeSheet.tsx  -- wraps Sheet + ConversationalIntake for new-entry walks
  SegmentTemplateEditor.tsx  -- minimal modal: new segment + add/remove fields
intake/                      -- UNCHANGED; ConversationalIntake stays schema-agnostic
scenes/
  SceneMetadataSheet.tsx     -- (EXTENDED) gains location selector single-select
  SceneAutosuggestChips.tsx  -- (EXTENDED) handles discriminated ChipSuggestion payload
  sceneMetadataChannel.ts    -- (EXTENDED) ChipSuggestion = character | world_entry
util/
  flattenSerdeFlatten.ts     -- (NEW, refactored from characters/flattenCharacterData.ts) — generic
ipc/commands.ts              -- new types: WorldSegment, WorldEntry, WorldEntryFile, ChipSuggestion
                             -- new methods on the ipc{} singleton (M3 lesson: no per-feature ipc modules)
App.tsx                      -- mount <WorldsSurface /> on activeNav === "world"
```

### 1.4 Opportunistic M3 polish folded in

Per the M3-handoff agreed plan (sprint mode B — polish opportunistically when M4 work touches the same module):

- **M3-gotcha refactor: `flattenCharacterData.ts` → generic `flattenSerdeFlatten.ts` under `app/src/util/`.** Phase A. (Not a numbered polish item; recorded in the M3-handoff "Vitest/TypeScript gotchas" section as the trigger for a second consumer.)
- **Polish #1 — non-string `main.full_name` silent data-loss guard.** Pattern baked into `world_entry::update_entry_field` rename-cascade from day one (`main.name` for world entries; same guard shape).
- **Polish #2 — round-robin claim test seeds only 4 characters.** Picked up alongside world-hue round-robin work in Phase A (matching test seeds 7+ entries to cover wrap-around).
- **Polish #3 — `hue_token` CHECK constraint or TODO.** Mirrored on `world_segment.hue_token` from the start.
- **Polish #12 — pre-existing fmt drift in `commands/*.rs`.** Phase B `cargo fmt -p water-app` pass.
- **M2 carry-over: Bouquet pin context.** Phase G. Load-bearing for the Chorus-stub flow; mandatory (not optional polish).

Polish items #4–#11, #13–#17 from the M3-handoff § 6 are NOT explicitly bundled (handle out of band or defer to a dedicated polish PR).

---

## 2. Data model

### 2.1 v4 SQLite migration — `crates/water-core/sql/v4_world_bible.sql`

```sql
-- world_segment: gains template override + ordering hue + soft-hide flag + slug + timestamps
ALTER TABLE world_segment ADD COLUMN template_json TEXT;                       -- NULL = use built-in default; non-NULL = user override
ALTER TABLE world_segment ADD COLUMN hidden INTEGER NOT NULL DEFAULT 0;        -- built-in segments soft-hide; non-built-ins can be deleted outright
ALTER TABLE world_segment ADD COLUMN hue_token TEXT NOT NULL DEFAULT '';
ALTER TABLE world_segment ADD COLUMN slug TEXT NOT NULL DEFAULT '';            -- stable identifier for built-ins ("concept", "locations", ...); empty for user-added
ALTER TABLE world_segment ADD COLUMN created_at TEXT NOT NULL DEFAULT '';
ALTER TABLE world_segment ADD COLUMN updated_at TEXT NOT NULL DEFAULT '';

-- world_entry: aliases + timestamps + schema_version parity with character
ALTER TABLE world_entry ADD COLUMN aliases_json TEXT NOT NULL DEFAULT '[]';
ALTER TABLE world_entry ADD COLUMN schema_version TEXT NOT NULL DEFAULT '';    -- slug@version, e.g. "locations@1"
ALTER TABLE world_entry ADD COLUMN created_at TEXT NOT NULL DEFAULT '';
ALTER TABLE world_entry ADD COLUMN updated_at TEXT NOT NULL DEFAULT '';

-- pinned_pill: origin trigger lets the pin handler detect Chorus+no_universe_yet for stub-creation
ALTER TABLE pinned_pill ADD COLUMN origin_trigger TEXT;                        -- NULL for legacy rows

CREATE INDEX world_entry_by_segment ON world_entry(segment_id);

INSERT INTO schema_version (version) VALUES (4);
```

**Notes:**

- SQLite requires a `DEFAULT` when adding a `NOT NULL` column via `ALTER TABLE`. Empty-string defaults are placeholder; the migration follows with one-shot `UPDATE` statements to backfill sensible values for any pre-existing rows (unlikely to exist since nobody has used the world feature yet — the world commands surface lands in M4).
- `slug` on `world_segment` is the lookup key for built-ins. At project init, the 6 built-ins are seeded with `slug = '<the-name>'` and `template_json = NULL`. Code looks up "which built-in is this?" by `slug`, not by `id` (ULIDs differ per project).
- `hue_token` round-robin cycles through `--water-hue-world-1..N` per segment, parallel to M3's character hue work.
- `pinned_pill.origin_trigger` is added in v4 (cross-table relevance: the Chorus-pin → stub flow needs it).

### 2.2 On-disk TOML — single-doc segment

Path: `world/<slug>.toml`.

```toml
schema_version = "concept@1"
id = "01J..."          # ULID, stable across project moves
name = "Concept"       # human display name; built-ins seed from BUILT_IN_TEMPLATES; user-renamable for user-added segments

[main]
core_premise = "..."
genre = "..."
tone = "..."

[lists]
themes = ["...", "..."]
inspirations = ["...", "..."]
```

`[main]` / `[lists]` sectioning mirrors M3's character TOML shape (via `#[serde(flatten)]` on the section enum). One file per single-doc segment; 5 files for the default built-ins (concept, politics_and_social, culture, world, history).

### 2.3 On-disk TOML — collection segment

Path: `world/<slug>/` (a directory).

- `world/<slug>/_meta.toml` — segment-level metadata:

```toml
schema_version = "locations@1"
id = "01J..."
name = "Locations"
ordering = 1
hue_token = "--water-hue-world-2"
hidden = false
```

- `world/<slug>/<entry-ulid>.toml` — one file per entry:

```toml
schema_version = "locations@1"
id = "01J..."
name = "The Pell Library"
aliases = ["Pell", "the library", "Aren's old place"]

[main]
type = "library"
sensory_detail = "Dust thick enough to read fingertips in..."
significance = "..."

[lists]
notable_features = ["the sub-basement", "the locked east wing"]
```

The on-disk `aliases` array is the source of truth for `world_entry.aliases_json` (rebuilt on project open per § 3.8 of the parent design spec).

### 2.4 On-disk TOML — user-added custom segment templates

Path: `world/_segments/<slug>.template.toml`. Absence = built-in default (looked up by slug). Presence for a built-in slug = user override.

```toml
slug = "magic_systems"
name = "Magic Systems"
is_collection = true
schema_version = 1

[[fields]]
id = "main.system_name"
label = "System Name"
prompt_question = "What's this magic system called?"
kind = { type = "short_text" }
optional_skip = false

[[fields]]
id = "main.cost"
label = "Cost"
prompt_question = "What does using this magic cost the user?"
kind = { type = "long_text" }
optional_skip = false

[[fields]]
id = "lists.adherents"
label = "Notable Adherents"
prompt_question = "Who in your world uses or studies this system?"
kind = { type = "string_list" }
optional_skip = true
```

The template file is one-to-one with an `IntakeSchemaSection` record. `field.id` dotted paths follow the `[main]` / `[lists]` convention.

### 2.5 `OpenProject` extension — `app/src-tauri/src/state.rs`

```rust
pub struct OpenProject {
    pub db: Arc<Mutex<Db>>,
    pub project_root: PathBuf,
    pub scene_write_locks: SceneWriteLocks,
    pub character_write_locks: CharacterWriteLocks,
    pub world_write_locks: WorldWriteLocks,   // NEW
    // ... existing fields
}

pub type WorldWriteLocks = DashMap<Id, Arc<Mutex<()>>>;
```

Lock-ordering invariant (KNOWN_FRAGILE #6) holds: `project lock → drop → world-write-lock → db lock`.

### 2.6 `rebuild.rs` extensions — `crates/water-core/src/rebuild.rs`

Existing rebuild already seeds legacy world rows (file lines 116–124 at the `m3` tag). M4 extends to:

1. Scan `world/` for built-in single-doc files (e.g. `concept.toml`) and built-in collection directories (e.g. `locations/`).
2. Scan `world/_segments/*.template.toml` for user-added segments.
3. Seed any missing built-ins from `BUILT_IN_TEMPLATES` with empty `data_json` and the default `is_collection` flag.
4. Reattach `scene.location_id` references; orphaned refs (entry deleted out of band) become NULL with a `tracing::warn!` log.
5. Rebuild `world_entry.aliases_json` from each entry's TOML `aliases` array.

---

## 3. Core abstractions

### 3.1 `WorldStore` — `crates/water-core/src/world/store.rs`

Mirror of `CharacterStore` (M3) with the single-doc/collection bifurcation. All CRUD passes through `WorldStore`; DB and disk are kept in sync per call.

```rust
pub struct WorldStore<'a> {
    db: &'a Db,
    project_root: PathBuf,
}

impl<'a> WorldStore<'a> {
    // === Segments (built-in + user-added) ===
    pub fn list_segments(&self, project_id: &Id) -> Result<Vec<WorldSegmentRow>>;
    pub fn read_segment(&self, segment_id: &Id) -> Result<WorldSegmentRow>;
    pub fn find_segment_by_slug(&self, project_id: &Id, slug: &str) -> Result<Option<WorldSegmentRow>>;
    pub fn seed_builtins(&self, project_id: &Id) -> Result<()>;                            // idempotent; called on project open
    pub fn create_user_segment(&self, project_id: &Id, name: &str, is_collection: bool, template: &IntakeSchemaSection) -> Result<Id>;
    pub fn update_segment_template(&self, segment_id: &Id, template: &IntakeSchemaSection) -> Result<()>;
    pub fn set_segment_hidden(&self, segment_id: &Id, hidden: bool) -> Result<()>;         // built-ins: hide; user-added: delete instead
    pub fn delete_user_segment(&self, segment_id: &Id) -> Result<()>;                      // refuses if slug is a built-in

    // === Single-doc segment data ===
    pub fn read_single_doc(&self, segment_id: &Id) -> Result<WorldSingleDocFile>;
    pub fn update_single_doc_field(&self, segment_id: &Id, field_id: &str, value: &Value) -> Result<()>;

    // === Collection entries ===
    pub fn list_entries(&self, segment_id: &Id) -> Result<Vec<WorldEntryIndexRow>>;
    pub fn read_entry(&self, entry_id: &Id) -> Result<WorldEntryFile>;
    pub fn create_entry(&self, segment_id: &Id, name: &str) -> Result<Id>;                  // creates draft with empty data
    pub fn create_entry_seeded(&self, segment_id: &Id, name: &str, seed_field_id: &str, seed_value: &str) -> Result<Id>;
    pub fn update_entry_field(&self, entry_id: &Id, field_id: &str, value: &Value) -> Result<()>;
    pub fn update_entry_aliases(&self, entry_id: &Id, aliases: &[String]) -> Result<()>;
    pub fn delete_entry(&self, entry_id: &Id) -> Result<()>;
    pub fn delete_entry_if_empty(&self, entry_id: &Id) -> Result<bool>;                     // returns true if reaped
}
```

**Rename-cascade guard.** `update_entry_field` with `field_id = "main.name"` mutates `world_entry.name` and re-renders the TOML filename if the slug-derived path changes. Per M3 polish item #1, the guard:

```rust
if field_id == "main.name" && !value.is_string() {
    return Err(Error::Other("main.name must be a string".into()));
}
```

is baked in from day one.

`update_*_field` methods take JSON `Value`, do dotted-path mutation against the data blob, write the new full TOML to disk, compute `file_hash`, then update the DB row — one transaction per call. **Held under `world_write_locks[entry_id]`.**

### 3.2 Built-in templates — `crates/water-core/src/world/templates.rs`

```rust
pub struct BuiltInTemplate {
    pub slug: &'static str,
    pub display_name: &'static str,
    pub is_collection: bool,
    pub template: IntakeSchemaSection,   // built lazily via once_cell or returned by fn
}

pub fn built_in_templates() -> &'static [BuiltInTemplate] {
    // concept, locations, politics_and_social, culture, world, history
}

pub fn effective_template(db: &Db, segment_id: &Id) -> Result<IntakeSchemaSection> {
    // 1. SELECT slug, template_json FROM world_segment WHERE id = ?
    // 2. If template_json IS NOT NULL: parse and return.
    // 3. Else look up slug in built_in_templates(); if found return that; else Err.
}
```

**Concrete field-id convention** for all built-in templates: `main.<key>` for scalar fields and `lists.<key>` for `string_list` kinds. Matches M3's `[main]` / `[lists]` shape on disk and keeps `flattenSerdeFlatten.ts` symmetric.

**Built-in segment specs** (verbatim from parent design § 3.6):

| Slug | is_collection | Fields |
|---|---|---|
| `concept` | false | `main.core_premise` (long_text), `main.genre` (short_text), `main.tone` (short_text), `lists.themes` (string_list), `lists.inspirations` (string_list) |
| `locations` | true | `main.name` (short_text)*, `main.type` (short_text), `main.sensory_detail` (long_text), `lists.notable_features` (string_list), `main.significance` (long_text) |
| `world` | false | `main.era` (short_text), `main.technology_level` (short_text), `main.magic_or_speculative_rules` (long_text), `main.geography` (long_text) |
| `politics_and_social` | false | `main.governance` (long_text), `lists.factions` (string_list), `main.conflicts` (long_text), `main.hierarchies` (long_text), `lists.taboos` (string_list) |
| `culture` | false | `main.languages` (long_text), `main.religions` (long_text), `main.art_and_ritual` (long_text), `main.daily_life` (long_text) |
| `history` | false | `lists.timeline_beats` (string_list), `lists.legends` (string_list), `lists.unresolved_threads` (string_list) |

*For `locations`, `main.name` is the canonical entry name and triggers the rename-cascade guard on update. Other collection segments added by users name their entries by whatever field's `id == "main.name"`.

### 3.3 `WorldRegistry` — `crates/water-core/src/world/registry.rs`

Hot-path read-only snapshot, built once per orchestrator dispatch.

```rust
pub struct WorldRegistry {
    by_id: HashMap<Id, WorldEntrySnapshot>,
    by_name_or_alias: HashMap<String, Vec<Id>>,    // LOWERCASED token -> entries
    segments: HashMap<Id, WorldSegmentRow>,
    by_segment_slug: HashMap<String, Id>,           // "locations" -> segment_id
}

pub struct WorldEntrySnapshot {
    pub id: Id,
    pub segment_id: Id,
    pub segment_slug: String,
    pub name: String,
    pub aliases: Vec<String>,
    pub data: serde_json::Value,
}

impl WorldRegistry {
    pub fn from_db(db: &Db, project_id: &Id) -> Result<Self>;
    pub fn by_id(&self, id: &Id) -> Option<&WorldEntrySnapshot>;
    pub fn find_by_token(&self, lowercased_token: &str) -> &[Id];   // case-insensitive
    pub fn entries_by_segment_slug(&self, slug: &str) -> Vec<&WorldEntrySnapshot>;
    pub fn segments(&self) -> impl Iterator<Item = &WorldSegmentRow>;
}
```

**Case-sensitivity policy.** Character autosuggest (M3) is case-sensitive on word boundaries; `WorldRegistry::find_by_token` is **case-insensitive**. Place names are more case-variable in English prose than character names (writers reference "the library" mid-sentence; they don't write "marcus" mid-sentence). The asymmetry is intentional. Recorded in KNOWN_FRAGILE as a documented choice (see § 14).

### 3.4 Speakers — `&WorldRegistry` threading

Two speakers gain world awareness.

**`CartographerSpeaker` (NEW).** A `PersonaSpeaker` variant — does NOT need a new top-level `Speaker` impl. Lives at `crates/water-core/src/voice/cartographer_template.rs`. Loads `prompts/speakers/cartographer/template.toml`. Template token surface:

| Token | Source |
|---|---|
| `{{trigger_kind}}` | The trigger that fired (always `world_drift` in M4) |
| `{{matched_entry_name}}` | `WorldEntrySnapshot.name` |
| `{{matched_entry_segment}}` | `WorldEntrySnapshot.segment_slug` |
| `{{relevant_world_excerpt}}` | Pre-rendered server-side from `[main]` block, capped ~400 tokens |
| `{{scene_paragraph}}` | The scene text in question |
| `{{confirmation_reason}}` | The Stage 2 LLM's "reason" string (used as hint, not quoted) |

Pre-rendering `{{relevant_world_excerpt}}` server-side keeps `Speaker::render` a pure function — no live DB read inside the speaker.

**`CharacterSpeaker` (EXTENDED from M3).** Signature change:

```rust
// M3:
pub fn from_row(row: &CharacterRow, registry: &CharacterRegistry) -> CharacterSpeaker;

// M4:
pub fn from_row(
    row: &CharacterRow,
    char_registry: &CharacterRegistry,
    world_registry: &WorldRegistry,
    scene: &SceneContext,
) -> CharacterSpeaker;
```

New tokens in the character voice template, namespaced under `world.` to avoid collision with future template fields:

| Token | Resolution |
|---|---|
| `{{world.location_name}}` | Empty if `scene.location_id` is NULL |
| `{{world.location_sensory_detail}}` | Empty if NULL OR entry is not in `locations` segment |
| `{{world.location_type}}` | Same |

The M3 voice template's **line-based omission** rule (a line resolving to only-whitespace after token substitution is dropped) means empty tokens cleanly disappear from the prompt rather than producing dangling "Location: " lines.

### 3.5 Voice router — `crates/water-core/src/voice/router.rs`

```rust
pub const WORLD_TRACK_TRIGGERS: &[&str] = &["world_drift"];

pub fn select_speaker(
    trigger_id: &str,
    candidate: &TriggerCandidate,
    scene: &SceneSnapshot,
    char_registry: &CharacterRegistry,
    world_registry: &WorldRegistry,
) -> SpeakerHandle {
    if CHAR_TRACK_TRIGGERS.contains(&trigger_id) {
        return select_char_speaker_with_pov_prefer(/* M3 logic, unchanged */);
    }
    if WORLD_TRACK_TRIGGERS.contains(&trigger_id) {
        return SpeakerHandle::Persona("cartographer");   // hard-wired in M4
    }
    // ... existing persona rotation
}
```

In M4 the `WORLD_TRACK_TRIGGERS` branch is degenerate (one trigger, one persona). Const-array shape is set up so M5+ can add e.g. `architect_plot_check` without restructuring.

### 3.6 `OrchestratorContext` — `crates/water-core/src/orchestrator/mod.rs`

```rust
pub struct OrchestratorContext<'a> {
    pub scene: &'a SceneSnapshot,
    pub project: &'a ProjectSnapshot,
    pub character_registry: &'a CharacterRegistry,
    pub world_registry: &'a WorldRegistry,    // NEW
}
```

`orchestrator_service.rs` builds `WorldRegistry::from_db(&db, &project_id)` once per dispatch alongside `CharacterRegistry`. Construction cost is bounded (≤ dozens of entries in any plausible project; HashMap construction is sub-millisecond).

---

## 4. UI surface

### 4.1 `WorldsSurface.tsx` — routing

```tsx
type View =
  | { kind: "index" }
  | { kind: "segment"; segmentId: string }
  | { kind: "entry"; segmentId: string; entryId: string }
  | { kind: "entry-intake"; segmentId: string; draftEntryId: string }
  | { kind: "new-segment" };   // template editor modal overlay

// State held in WorldsSurface. Back-stack one-level (entry -> segment -> index).
// Scroll position preserved across back navigation per M3 T20 pattern.
```

Mounted by `App.tsx` when `activeNav === "world"`. Current fall-through (handoff § 8) where the World nav showed the scenes view is removed.

### 4.2 `WorldIndex.tsx` — segment tile grid

CSS grid of `WorldSegmentTile` components. Default: 6 tiles (5 single-doc + 1 collection). User-added segments append after built-ins. Hidden built-ins are filtered unless a "show hidden" toggle is on (settings-level; ships as secondary affordance, full polish M7).

Each `WorldSegmentTile` shows:

- Segment display name.
- Hue-tinted soft glow (`hue_token` from DB).
- Preview line:
  - **Single-doc**: first sentence of the first non-empty field (truncated ~80 chars).
  - **Collection**: entry count + first 2 entry names (e.g. "3 entries: The Pell Library, Aren's Atelier, ...").
- Tile bottom-right: small icon distinguishing single-doc (page) vs. collection (grid).

Trailing "+ New segment" tile opens the template editor modal (`View = { kind: "new-segment" }`).

### 4.3 `WorldSegmentView.tsx` — segment-level routing

Routes on `is_collection`:

**Single-doc branch.** One `<Sheet>` containing inline-editable fields ordered by the template's `IntakeSchemaSection.fields` order. Reuses `InlineField` from `app/src/characters/`. Top of sheet: segment name (editable for user-added segments only — built-ins are immutable label). Bottom of sheet: optional "Walk intake" affordance opens `ConversationalIntake` against the whole template for guided edit.

**Collection branch.** `<WorldCollectionGrid>`:

- Grid of `WorldEntryCard` (parallel to `CharacterCard`): name, hue glow, 1-line preview of first non-empty field, sort/search bar at top.
- Trailing "+ New entry" card. Click opens `<WorldEntryIntakeSheet>` (intake walk against the segment's effective template).
- Segment header: name + "Edit template" affordance (user-added segments only) opening `SegmentTemplateEditor` in edit mode.

### 4.4 `WorldEntrySheet.tsx` — per-entry inline edit

Direct parallel to `CharacterSheet`. Top-level: name (rename-cascade-guarded per § 3.1). Below: sections grouped by `IntakeSchemaSection.id`. Each field uses `InlineField` with the appropriate `IntakeFieldKind`.

**M4-specific addition: aliases editor.** Aliases are always present on `world_entry` (not template-driven). Near the top of the sheet, below the name, an inline string-list editor for aliases. Commits via `ipc.worldEntryUpdateAliases`. This is the writer-facing surface for `world_drift` Stage 1 alias matching.

### 4.5 `WorldEntryIntakeSheet.tsx` — intake reuse for new entries

```tsx
function WorldEntryIntakeSheet({ segmentId, draftEntryId, onComplete, onClose }) {
  const schema = useWorldIntakeSchema(segmentId);   // ipc.worldIntakeSchema(segmentId)
  const values = useWorldEntryValues(draftEntryId);

  return (
    <Sheet
      onClose={async () => {
        await ipc.worldEntryDeleteIfEmpty(draftEntryId);   // reap orphan drafts
        onClose();
      }}
    >
      <ConversationalIntake
        schema={schema}
        values={values}
        onAnswer={async (fieldId, value) => {
          await ipc.worldEntryUpdateField({ entryId: draftEntryId, fieldId, value });
        }}
        onComplete={() => onComplete(draftEntryId)}
      />
    </Sheet>
  );
}
```

**Reuses `ConversationalIntake` unchanged.** M3's lessons bake in for free:

- T15 `key={field.id}` for autoFocus across same-kind transitions.
- T16 `useRef` write-path race guard.
- T15 mount-time `useEffect` for `onComplete` if all-fields-already-answered.

**Orphan-draft reaping.** "+ New entry" calls `ipc.worldEntryCreate({ segmentId, name: "" })` which materializes a draft row. If the user closes before completing, `worldEntryDeleteIfEmpty` removes the row only if all template fields are empty. This avoids both (a) data loss on intentional partial completion and (b) detritus from cancelled intakes.

### 4.6 `SegmentTemplateEditor.tsx` — minimal template editor

Slim modal, two entry points: "+ New segment" tile (creation mode) and "Edit template" affordance on existing segments (edit mode).

**Layout (creation mode):**

```
┌─ New segment ──────────────────────────────────┐
│  Name        [____________________]            │
│  Type        ( ) Single document               │
│              (•) Collection                    │
│                                                │
│  Fields                                        │
│  ┌──────────────────────────────────────────┐  │
│  │ 1. [Label_______] kind: [short_text ▼]   │  │
│  │    Prompt: [_______________________]     │  │
│  │    □ Optional (writer can skip)          │  │
│  │                                       [×] │  │
│  └──────────────────────────────────────────┘  │
│  + Add field                                   │
│                                                │
│  [Cancel]                       [Create]       │
└────────────────────────────────────────────────┘
```

**Field-id derivation.** At creation time, `IntakeField.id` is derived from the label: lowercased + snake_cased + prefixed with `main.` (or `lists.` for `string_list` kind). Example: `"Sensory Detail" + short_text → "main.sensory_detail"`. Once created, the `id` is **immutable**. Renames change `label` but not `id` (preserves data continuity across template edits).

**v1 cuts (recorded explicitly):**

1. NO drag-reorder. Insertion order is the field order.
2. NO inline `kind` editing after creation (remove + re-add to change kind).
3. NO `id` rename.
4. NO inline editing of `choice` options after creation. v1 editor surfaces only `short_text`, `long_text`, `string_list` kinds. `choice`-kind built-in fields (none in default segments at present) still work, just not user-authorable via UI.
5. **Built-in segments are append-only in edit mode.** Can add new fields; cannot remove built-in fields; cannot rename built-in field labels. The append-only constraint keeps rebuild-from-truth stable across app upgrades.

### 4.7 IPC surface — `app/src/ipc/commands.ts`

All new methods on the `ipc{}` singleton (M3 lesson: no per-feature `ipc/<x>` modules).

```typescript
export type WorldSegment = {
  id: string;
  slug: string;                  // empty for user-added
  name: string;
  ordering: number;
  is_collection: boolean;
  hue_token: string;
  hidden: boolean;
  has_template_override: boolean;
};

export type WorldEntryIndexEntry = {
  id: string;
  segment_id: string;
  name: string;
  preview: string;               // 1-line, pre-rendered server-side
};

export type WorldEntryFile = {
  id: string;
  segment_id: string;
  schema_version: string;
  name: string;
  aliases: string[];
  // section keys at top level via #[serde(flatten)] — M3 lesson, NO `data` wrapper.
  [key: string]: unknown;
};

export type ChipSuggestion =
  | { kind: "character"; characterId: string; characterName: string; matched: string }
  | { kind: "world_entry"; entryId: string; entryName: string; segmentSlug: string; matched: string };

export const ipc = {
  // ... existing M1-M3 methods ...
  worldSegmentList: () => Promise<WorldSegment[]>,
  worldSegmentCreate: (req: { name: string; isCollection: boolean; template: IntakeSchemaSection }) => Promise<string>,
  worldSegmentUpdateTemplate: (req: { segmentId: string; template: IntakeSchemaSection }) => Promise<void>,
  worldSegmentSetHidden: (req: { segmentId: string; hidden: boolean }) => Promise<void>,
  worldSegmentDelete: (segmentId: string) => Promise<void>,                                  // refuses built-ins
  worldIntakeSchema: (segmentId: string) => Promise<IntakeSchemaSection>,
  worldSingleDocRead: (segmentId: string) => Promise<WorldEntryFile>,
  worldSingleDocUpdateField: (req: { segmentId: string; fieldId: string; value: unknown }) => Promise<void>,
  worldEntryList: (segmentId: string) => Promise<WorldEntryIndexEntry[]>,
  worldEntryRead: (entryId: string) => Promise<WorldEntryFile>,
  worldEntryCreate: (req: { segmentId: string; name: string }) => Promise<string>,
  worldEntryUpdateField: (req: { entryId: string; fieldId: string; value: unknown }) => Promise<void>,
  worldEntryUpdateAliases: (req: { entryId: string; aliases: string[] }) => Promise<void>,
  worldEntryDeleteIfEmpty: (entryId: string) => Promise<boolean>,
  worldEntryDelete: (entryId: string) => Promise<void>,
  worldAutosuggest: (req: { sceneId: string; paragraph: string }) => Promise<WorldEntryIndexEntry[]>,
  sceneSetLocation: (req: { sceneId: string; locationId: string | null }) => Promise<void>,
};
```

---

## 5. `world_drift` trigger

### 5.1 Stage 1 — name + alias scan (cheap, no LLM)

New trigger at `crates/water-core/src/orchestrator/triggers/world_drift.rs`. Patterns on M3 `character_dissonance` (T7): cheap text scan + `requires_confirmation: true` to gate the expensive LLM check.

```rust
pub struct WorldDriftEvaluator;

const MIN_PARAGRAPH_WORDS: usize = 12;
const MIN_CONTEXT_OVERLAP_WORDS: usize = 2;
pub const WORLD_DRIFT_COOLDOWN_MS: i64 = 180_000;   // 3 min per (entry, scene)

impl TriggerEvaluator for WorldDriftEvaluator {
    fn id(&self) -> &'static str { "world_drift" }

    fn evaluate(&self, ctx: &OrchestratorContext) -> Vec<TriggerCandidate> {
        let paragraph = ctx.scene.recent_paragraph_text();
        if paragraph.split_whitespace().count() < MIN_PARAGRAPH_WORDS {
            return vec![];
        }

        let paragraph_tokens = tokenize_words(paragraph);   // M3 lemma-gate tokenizer (English, \b-bounded)
        let mut candidates = vec![];

        for (_offset, token) in &paragraph_tokens {
            let lower = token.to_lowercase();
            for entry_id in ctx.world_registry.find_by_token(&lower) {
                let entry = ctx.world_registry.by_id(entry_id).unwrap();

                // Collision resolution (§ 6.2): character-in-scene wins.
                if let Some(c) = ctx.character_registry.find_by_name(token) {
                    if ctx.scene.characters_present.contains(&c.id) {
                        continue;
                    }
                }

                // Contextual-overlap pre-check (KNOWN_FRAGILE #19).
                if !has_contextual_overlap(&paragraph_tokens, entry, MIN_CONTEXT_OVERLAP_WORDS) {
                    continue;
                }

                // Per-(entry,scene) cooldown gate.
                if ctx.is_cooled_down("world_drift", &entry.id, ctx.scene.id, WORLD_DRIFT_COOLDOWN_MS) {
                    continue;
                }

                candidates.push(TriggerCandidate {
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

        candidates
    }
}
```

**Three-stage gate, summarized:**

1. **Token match.** Lowercased `\b`-bounded word match against `world_entry.name` + aliases.
2. **Contextual-overlap pre-check.** ≥ 2 content-word overlap between paragraph tokens and the entry's `[main]` text (minus stopwords). Drops obvious false positives without LLM call. Heuristic — recorded as KNOWN_FRAGILE #19.
3. **LLM confirmation (Stage 2 below).**

**Tokenizer reuse.** `tokenize_words` is the M3 lemma-gate tokenizer. Shares KNOWN_FRAGILE #17 (English-only) limitation transitively.

**Cooldown.** `WORLD_DRIFT_COOLDOWN_MS = 180_000` (3 min, longer than M3 `CHARACTER_DEFAULT_COOLDOWN_MS = 60_000`). Per-(entry, scene) cooldown key — same entry can re-fire in a different scene immediately; different entry in same scene can fire immediately.

### 5.2 Stage 2 — LLM confirmation prompt

`prompts/pill_world_drift_check.toml`:

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

**`{{entry_excerpt}}` rendering.** Server-side pre-render of the entry's `[main]` block as a compact `key: value` multiline string. Lists become comma-joined. Capped at ~400 tokens. Deterministic given (entry data, prompt version).

**Verdict handling:**

| Verdict | Action |
|---|---|
| `contradicts` | Promote to pill. Cartographer message rendered via cartographer template, using `reason` as `{{confirmation_reason}}` hint (NOT quoted verbatim). |
| `consistent` | Drop, no pill. |
| `unclear` | Drop, no pill. `tracing::debug!` log for eval. |

`unclear` suppression is a deliberate signal-vs-noise tradeoff: the writer never sees a pill that hedges. Recorded as a design choice.

### 5.3 Cartographer speaker template — `prompts/speakers/cartographer/template.toml`

Reactive/observational tone per M2 constraints; line-based omission per M3 character_template pattern.

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

Three-layer tone enforcement (unchanged from M2/M3): prompt clause → PASS (post-generation lemma blacklist scan) → post-hoc regex. Cartographer inherits the existing pipeline.

### 5.4 Trigger registry — `crates/water-core/src/orchestrator/registry.rs`

`WorldDriftEvaluator` registered alongside `CharacterDissonanceEvaluator` et al. Tone-audit run picks up Cartographer template automatically via its persona registration.

---

## 6. Cross-feature wiring

### 6.1 Scene ↔ World linking (`scene.location_id`)

The schema column `scene.location_id TEXT REFERENCES world_entry(id) ON DELETE SET NULL` has existed since v1. M4 lights it up.

**Setting it.** `SceneMetadataSheet.tsx` gains a "Location" row beside POV:

```
POV:       [ Marcus Vale         ▼ ]
Location:  [ The Pell Library    ▼ ]    (×) clear
Present:   [ Marcus ] [ + add ]
```

Single-select against the project's `locations` segment entries. Reuses the existing select component from M3 T13. Commits via `ipc.sceneSetLocation({ sceneId, locationId })`. The "(×) clear" affordance unsets to NULL.

**Reading it.** `sceneReadMetadata` (M3 T21) returns:

```typescript
type SceneMetadata = {
  povCharacter: { id: string; name: string } | null;
  charactersPresent: { id: string; name: string }[];
  location: { id: string; name: string; segmentSlug: string } | null;   // NEW
};
```

Voice prompt enrichment is immediate: once `location_id` is set, the next dispatch's `CharacterSpeaker::from_row` resolves `{{world.location_*}}` tokens against the matching `WorldRegistry` entry.

### 6.2 Autosuggest — character + world chip coexistence

M3 ships `SceneAutosuggestChips.tsx` for character-name detection. M4 extends the existing channel with a discriminated payload rather than creating a parallel surface:

```typescript
type ChipSuggestion =
  | { kind: "character"; characterId: string; characterName: string; matched: string }
  | { kind: "world_entry"; entryId: string; entryName: string; segmentSlug: string; matched: string };
```

`SceneAutosuggestChips.tsx` renders both kinds with the same chip primitive but differentiates by hue (character hue vs. world segment hue). The user-action on a `world_entry` chip is "Set as location" — sets `scene.location_id`. **Only `locations`-segment entries autosuggest in M4** (other segments have no corresponding scene field to bind to).

**Collision resolution policy (Q8).** Both autosuggest and `world_drift` Stage 1 use a shared helper `crates/water-core/src/world/collision.rs::resolve_token_kind(...)`:

```rust
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
    scene: &SceneSnapshot,
) -> TokenKind {
    let char_match = char_registry.find_by_name(token);
    let world_matches: Vec<Id> = world_registry.find_by_token(&token.to_lowercase()).to_vec();
    match (char_match, world_matches.is_empty()) {
        (Some(c), false) if scene.characters_present.contains(&c.id) => TokenKind::CharacterOnly(c.id),
        (Some(c), false) => TokenKind::BothFire { character_id: c.id, world_ids: world_matches },
        (Some(c), true) => TokenKind::CharacterOnly(c.id),
        (None, false) => TokenKind::WorldOnly(world_matches),
        (None, true) => TokenKind::Neither,
    }
}
```

Policy lives in one place. Recorded as KNOWN_FRAGILE #18 (heuristic; may mis-classify when a character is referenced but not yet added to `characters_present` — mitigated by the autosuggest chip surfacing the missing character).

### 6.3 Chorus pin → `world_entry` stub

When a pill from `no_universe_yet` (Chorus speaker) is pinned, the pin handler creates a `world_entry` stub seeded with the pinned snippet.

**Detection signal.** `pinned_pill.speaker_kind = "persona"` AND `speaker_id = "chorus"` AND `origin_trigger = "no_universe_yet"`. The `origin_trigger` column is added in v4 (§ 2.1).

**Handler — `app/src-tauri/src/commands/pill.rs`:**

```rust
async fn pill_pin_core(
    db: &Arc<Mutex<Db>>,
    project_root: &Path,
    world_locks: &WorldWriteLocks,
    pin: PinPillRequest,
) -> Result<PinPillResponse> {
    // 1. Insert into pinned_pill as today.
    let pin_id = insert_pinned_pill(db, &pin).await?;

    // 2. M4 stub-creation branch.
    let stub_entry_id = if pin.speaker_kind == "persona"
        && pin.speaker_id == "chorus"
        && pin.origin_trigger.as_deref() == Some("no_universe_yet")
    {
        let project_id = read_project_id(db).await?;
        let store = WorldStore::new(/* db lock guard */, project_root.to_path_buf());
        let locations_segment_id = store
            .find_segment_by_slug(&project_id, "locations")?
            .ok_or_else(|| Error::Other("locations segment missing".into()))?
            .id;

        let entry_id = store.create_entry_seeded(
            &locations_segment_id,
            "",                                       // empty name -> renders as "(unnamed location)" in UI
            "main.sensory_detail",
            &pin.snippet,
        )?;
        Some(entry_id)
    } else {
        None
    };

    Ok(PinPillResponse { pin_id, stub_entry_id })
}
```

**UI side.** `Bouquet.tsx` (or wherever pin-click lives) inspects `PinPillResponse.stub_entry_id`. If non-null, routes the user to `WorldsSurface` with `View = { kind: "entry", segmentId, entryId }`. Writer lands in an entry sheet pre-populated with their snippet and an empty name field.

**Bouquet pin context fix (M2 carry-over).** Bouquet today passes empty `sceneId` / `blockId` / `snippet` to `ipc.pillPin`. The stub flow needs `snippet` non-empty. Fixing this is load-bearing M4 work (Phase G), not optional polish.

**No auto-reaping of unnamed stubs.** The snippet has value as a fragment even without a name. UI displays unnamed entries as "(unnamed location)" in the Locations grid. Recorded as KNOWN_FRAGILE #21 (potential hygiene debt; M5+ may add stale-stub reaping).

### 6.4 Settings surface

A "World" section reachable from the World index "..." menu (or settings panel — exact entry point M7's job to polish). Surfaces:

- List of all segments with checkbox: visible/hidden.
- "+ New segment" button → `SegmentTemplateEditor` modal.
- For user-added segments: "Edit template", "Delete segment" buttons.
- For built-in segments: "Edit template" opens the modal in append-only mode.

Intentionally lo-fi for v1. M7 owns the polished settings experience.

---

## 7. Build sequence — Phases A–I

Phases are mostly sequential; Phase E has parallel-friendly sub-tasks. Each phase produces a green-tested commit. Total: **22–28 tasks** (vs. handoff § 5 expectation of ~16-20 — the additional surface is Chorus-stub wiring + the explicit M3-polish opportunism bundling).

**Phase A — Engine foundations (4–5 tasks).** v4 migration; `world/` module skeleton with `WorldStore` + `templates.rs::BUILT_IN_TEMPLATES` (all 6) + `effective_template`; `seed_builtins` idempotent on project open; on-disk TOML round-trip for both shapes; `WorldRegistry::from_db`. Opportunistic M3 polish: `flattenSerdeFlatten` extraction; hue round-robin test seed expansion; hue CHECK constraint or TODO.

**Phase B — Tauri commands (3–4 tasks).** All `world*` commands per § 4.7 via `_core` async-fn pattern. `WorldWriteLocks` added to `OpenProject`. `sceneReadMetadata` extended with `location`. `sceneSetLocation` added. Opportunistic M3 polish: `commands/*.rs` fmt-pass.

**Phase C — Orchestrator wiring (2–3 tasks).** `WorldRegistry` threaded into `OrchestratorContext`; `orchestrator_service.rs` builds it once per dispatch. `WORLD_TRACK_TRIGGERS` const + voice router branch. `CartographerSpeaker` `PersonaSpeaker` variant registered.

**Phase D — `world_drift` trigger (3 tasks).** Stage 1 evaluator + collision-resolver helper; `pill_world_drift_check.toml` + confirmation handler integration; Cartographer template + tone-audit pass.

**Phase E — World UI surface (5–6 tasks; parallelizable).** `WorldsSurface` routing → `WorldIndex` + `WorldSegmentTile` → `WorldSegmentView` (single-doc + collection branches) → `WorldEntryCard` + `WorldEntrySheet` → `WorldEntryIntakeSheet` with reused `ConversationalIntake`. Sub-tasks across components can be parallelized once routing is in.

**Phase F — Character speaker extension (2 tasks).** `CharacterSpeaker::from_row` gains `&WorldRegistry` + `&SceneContext`; `{{world.location_*}}` tokens land in `character_template.rs`. Tests: scene with `location_id` set → voice prompt contains sensory detail; without → tokens render empty and lines drop via line-based omission.

**Phase G — Cross-feature wiring (3–4 tasks).** SceneMetadataSheet location selector; discriminated `ChipSuggestion` payload + collision resolver shared with `world_drift`; pin pipeline Chorus-stub branch; Bouquet pin-context fix.

**Phase H — Template editor + settings (2–3 tasks).** `SegmentTemplateEditor` modal in both creation and edit modes. Hide/show toggle for built-ins. Delete for user-added.

**Phase I — Audit, fixture, smoke, tag (2 tasks).** Build `eval/m4_acceptance/pell_library.toml` + planted-contradiction test scene. Walk the manual smoke checklist (§ 13). Tag `m4`. Write next handoff doc.

---

## 8. Test strategy

### 8.1 Rust core unit tests

- `world::store` — round-trip TOML both shapes; rename-cascade guard for entry name; alias index rebuild after edit; ON DELETE SET NULL cascade to `scene.location_id`.
- `world::registry` — case-insensitive name + alias lookup; segment slug routing; snapshot completeness after `from_db`.
- `world::collision::resolve_token_kind` — `CharacterOnly` when character-in-scene; `BothFire` fallback; `WorldOnly` when no character match; deterministic ordering.
- `world_drift::evaluate` — Stage 1 emits candidates for clean matches; suppresses below `MIN_PARAGRAPH_WORDS`; suppresses when contextual-overlap fails; per-(entry, scene) cooldown gates re-fire.
- `voice::router` — `world_drift` → Cartographer; `CHAR_TRACK_TRIGGERS` and `WORLD_TRACK_TRIGGERS` don't collide.
- Cartographer template render — tone clause present; PASS blacklist applied; post-hoc regex strips.

### 8.2 Tauri command tests (via `_core` extraction)

- `world_segment_create` — refuses duplicate slug; builds `template_json` correctly; writes `_segments/<slug>.template.toml`.
- `world_entry_create` + `world_entry_update_field` — disk + DB stay in sync; `file_hash` updates.
- `world_entry_delete_if_empty` — `true` when all fields blank; `false` when any field present.
- `pill_pin_core` — Chorus + `no_universe_yet` produces stub; other pin signatures produce no stub; `PinPillResponse.stub_entry_id` correctly populated.

### 8.3 TypeScript / Vitest

- `WorldsSurface` routing — back navigation preserves scroll (M3 T20 pattern).
- `WorldEntryIntakeSheet` — uses `ConversationalIntake` unchanged; orphan-draft reaping fires on close; M3 T15 `key={field.id}` regression test.
- `SegmentTemplateEditor` — built-in mode is append-only; field id derived from label is stable across re-renders; cannot remove built-in fields.
- `SceneAutosuggestChips` — discriminated payload renders correct hue per kind; world chips appear for `locations`-segment matches only.

### 8.4 Eval fixture

`eval/m4_acceptance/pell_library.toml` — a `locations` entry analog to M3's `marcus_vale.toml`. Planted attributes:

```toml
name = "The Pell Library"
aliases = ["Pell", "the library"]
[main]
type = "underground library"
sensory_detail = "Dust thick enough to read fingertips in. The air smells of cold stone and old paper."
significance = "Aren spent her childhood here. She still flinches at the smell of dust."

[lists]
notable_features = ["the sub-basement", "the locked east wing", "no natural light"]
```

Companion test scenes (in `eval/m4_acceptance/scenes/`):

- `consistent.md` — paragraph mentions Pell with reference to dust and the sub-basement. Expected: no `world_drift` pill.
- `contradiction_sunlight.md` — paragraph describes Aren reading in Pell "by the warm afternoon sunlight." Expected: `world_drift` pill (contradicts "no natural light").
- `contradiction_elevation.md` — paragraph places Pell "on the cliff overlooking the harbor." Expected: `world_drift` pill (contradicts "underground").
- `near_miss_unrelated.md` — paragraph mentions Pell in passing with no contextual overlap. Expected: contextual-overlap pre-check suppresses; no LLM call.

---

## 9. Out-of-scope (explicit)

- Embedding-based `world_drift` upgrade (KNOWN_FRAGILE escape hatch lives in M5+).
- Heatmap audiovisualizer integration with world entries (M5).
- Macro spatial canvas surface for world entries (M6).
- Polished template editor: drag-reorder, inline kind editing, `choice`-kind authoring, field `id` renames (M7).
- Rich-text world-entry content (long_text with bold/italic/links). v1 uses plain text; M2.5's mark system stays scene-only.
- Cross-project segment template export/import.
- Internationalization of the tokenizer (KNOWN_FRAGILE #17 propagates transitively to `world_drift`).
- Structured telemetry on `world_drift` false-positive rate. Only basic `tracing` logs.
- Segment-hued Cartographer pills (KNOWN_FRAGILE #20).
- Auto-reaping of unnamed Chorus-stub entries (KNOWN_FRAGILE #21).

---

## 10. Exit criteria (verbatim from parent spec § 4.6, + M4 additions)

From parent spec § 4.6:

1. All built-in segments work with intake.
2. User can add a new segment with a custom template and intake walks it.
3. Character voice prompts can pull relevant `world_entry` excerpts.
4. `world_drift` correctly identifies a planted contradiction in test scenes.

M4 additions:

5. Pinning a Chorus pill in a `no_universe_yet` context creates a `locations` `world_entry` stub seeded with the pinned snippet and routes the writer to its sheet.
6. Setting `scene.location_id` via the metadata sheet causes the next character-voice dispatch to include the location's sensory detail in the prompt.
7. The manual smoke checklist (§ 13) walks cleanly.

---

## 11. Resolved open questions (from M3-handoff § 10)

| # | Question | Resolution |
|---|---|---|
| 1 | Segment template descriptor format | **Reuse `IntakeField[]` as-is** (§ 3.2). Documented adaptation of parent spec § 3.6 "JSON-Schema-shaped" wording. |
| 2 | Where templates live | **Built-ins as Rust consts in `templates.rs`; user customizations in `world_segment.template_json` + `world/_segments/<slug>.template.toml`** (§ 2.1, § 2.4, § 3.2). |
| 3 | `is_collection` routing UX | **3-level routing: Index → Segment (sheet for single-doc, grid for collection) → Entry sheet (collection-only)** (§ 4.1, § 4.3). |
| 4 | "+ New entry" placement | **Tile inside the segment grid; walks ConversationalIntake; draft entry only persisted after first answer, orphan-reaped on cancel** (§ 4.3, § 4.5). |
| 5 | `world_drift` Stage 2 prompt | **Dedicated `pill_world_drift_check.toml`; reuses M3 two-stage confirmation dispatch; YES/NO/unclear verdict with `unclear` suppressed** (§ 5.2). |
| 6 | Cartographer ↔ world data | **World-aware `PersonaSpeaker` (CartographerSpeaker); excerpts injected via speaker template tokens; `CharacterSpeaker` also gains `&WorldRegistry`** (§ 3.4, § 5.3). |
| 7 | Template editor scope | **Minimal modal: new-segment + per-field add/remove inline. NO drag-reorder, NO inline kind edit, NO id rename, NO choice-kind authoring. Built-in segments append-only in edit mode** (§ 4.6). |
| 8 | Naming collision policy | **Context-aware: character wins if scene also has the character in `characters_present`; otherwise both fire. Shared `resolve_token_kind` helper** (§ 6.2). Recorded as KNOWN_FRAGILE #18. |
| 9 | Chorus pin → stub flow | **In M4. Minimal version: defaults to `locations` segment, no segment-picker, routes user to the new entry's edit sheet** (§ 6.3). |
| 10 | M3 polish bundling | **Opportunistic.** Items #1, #2, #3, #11, #12 + Bouquet pin-context bundled into M4 phases A, B, G. Others deferred (§ 1.4). |

---

## 12. Adaptations from locked parent spec

- **Parent spec § 3.6** says world templates are "JSON-Schema-shaped." **Adaptation:** they are `IntakeField[]`-shaped, the same descriptor M3 uses for character schemas. Rationale: keeps `ConversationalIntake` single-consumer, reuses M3's renderer/validator/test harness, avoids forking the intake walker. The on-disk template TOML is human-readable and semantically equivalent.
- **Parent spec § 3.6** lists default-segment field bundles inline. **No adaptation:** the field bundles in § 3.2 above match the parent spec verbatim. `world` (single doc) was confirmed as a single-doc segment per the parent's default list.
- **Parent spec § 4.6** says "Segment template editor — add/remove fields, add segments." **Adaptation:** v1 scope is restricted per Q7 (no drag, no kind edit, append-only built-ins). The full template editor is M7's responsibility. Exit criterion is still met because users CAN add a new segment with a custom template and intake CAN walk it.

---

## 13. Manual smoke checklist (parallel to M3 § 18)

Run on a freshly-created project at tag time. Walked by the implementing agent before tagging `m4`.

1. Open a fresh project — World nav-rail icon takes you to `WorldIndex`. 6 built-in segment tiles visible, none hidden.
2. Open Concept — single-doc sheet renders; all 5 fields inline-editable; values persist across re-open.
3. Open Locations — empty grid + a "+ New entry" tile only.
4. Click "+ New entry" — intake walks template fields in order; completion lands on the entry's sheet; entry appears in the grid on back.
5. Edit aliases on the entry — list-editor commits; re-open shows them.
6. Open a scene; set Location to the new entry — `SceneMetadataSheet` reflects it; `scene.location_id` persists.
7. Write a paragraph mentioning the entry by name with a contradictory attribute (use a `contradiction_*.md` fixture) — Cartographer pill fires in margin.
8. Write a paragraph mentioning it consistently — no pill.
9. Add a custom segment "Magic Systems" (collection, 3 fields); walk intake on a new entry — works.
10. Hide a built-in segment via settings — disappears from index; un-hide restores it.
11. Open a project with no characters and no world entries; observe Chorus pill; pin it — `(unnamed location)` stub appears in `Locations`; UI routes to its sheet pre-populated with the snippet as sensory_detail.
12. Set `scene.location_id` to an entry whose `sensory_detail` has a distinctive phrase; trigger a character-voice dispatch; inspect `WATER_REPLAY_LOG` (with `WATER_REPLAY_LOG=1`) — the rendered character-voice prompt contains the sensory-detail text.

Each step records pass/fail. Any failure becomes either a hotfix before tag OR a known limitation recorded in KNOWN_FRAGILE and the M5 handoff.

---

## 14. KNOWN_FRAGILE additions (to be recorded at `m4` tag time)

- **#18 — Name-collision resolution is heuristic.** Character-in-scene wins; both-fire fallback in ambiguous cases. May mis-classify when a character is referenced but not yet added to `characters_present`. Mitigation: M3 autosuggest chip surfaces the missing character.
- **#19 — `world_drift` contextual-overlap pre-check can miss real contradictions.** Heuristic 2-content-word overlap drops paragraphs where the writer's mention and the contradiction are stylistically distant. Future fix: lower threshold or replace with embedding similarity in M5+.
- **#20 — Cartographer pill hue follows persona-hue, not segment-hue.** Pills from a Locations contradiction look identical to pills from a Concept contradiction. v1 limitation; parent spec § 4.6 doesn't require segment-hued pills.
- **#21 — Unnamed Chorus stubs persist indefinitely.** No auto-reaping. Could become hygiene debt; M5+ may add a "reap unnamed entries > N days" pass.
- **#22 — Case-sensitivity asymmetry between character autosuggest and world_drift.** Character autosuggest is case-sensitive on word boundaries; `world_drift` Stage 1 is case-insensitive. Intentional (place names case-vary more than character names in English prose) but worth a note.

---

## 15. Pre-authorized adaptations during implementation

Per the M3 lesson: "Plan code is always stale by the time the implementer reads it." Adaptations pre-authorized for M4:

- **Rust API signatures may shift** to satisfy borrow checker / lifetimes / async fn requirements (e.g. `_core` extraction).
- **TS prop shapes may shift** to match actual `IntakeField` ergonomics in the React tree.
- **TOML field ordering** on disk may differ from the spec's example blocks (serde's default order).
- **CSS class names and component sub-structure** are unconstrained by this spec.
- **Test-helper extraction** (e.g. `mk_world_registry`, `mk_world_entry_file`) is encouraged whenever a test pattern repeats 3+ times.
- **`tracing::warn!` / `tracing::debug!` logs** at silent-failure points are encouraged (carries forward the M3 polish #7, #13 lesson).
- **Const extraction** for magic numbers (`MIN_PARAGRAPH_WORDS`, `MIN_CONTEXT_OVERLAP_WORDS`, `WORLD_DRIFT_COOLDOWN_MS`) is required.

Adaptations NOT pre-authorized — surface as a question if any of these come up:

- Changing the on-disk TOML directory layout (`world/<slug>.toml` vs. `world/<slug>/<entry>.toml`).
- Changing the v4 migration column set without coordinating.
- Changing the template descriptor format away from `IntakeField[]`.
- Removing the orphan-draft reaping behavior.
- Changing the verdict-handling rules in Stage 2 (e.g. surfacing `unclear` as a pill).

---

End of spec.

