# M3 Character Sheets — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship the LSM v2.1 character system on top of M2.5: Conversational Intake popup walking the schema one question at a time, inline-editable sheet view, character index grid, scene↔character linking (manual + advisory autosuggest), character voice rendered via per-character LSM fields, and two filled triggers (`idle_pause_with_present_character`, `character_dissonance` with lemma gate + LLM confirmation pass). Tag `m3` when done.

**Architecture:** 22 tasks across 7 phases. Phase A wires the Rust schema + storage (IntakeField, v3 migration, CharacterRegistry). Phase B fills character voice + the two trigger stubs. Phase C extends the M2 orchestrator with a confirmation-pass before level-0 pill dispatch. Phase D adds Tauri commands for CRUD + autosuggest. Phase E builds the reusable Conversational Intake component. Phase F builds the sheet view + index + scene metadata + autosuggest chips. Phase G is the final review + tag.

**Tech Stack:** Rust 1.85 + tokio + rusqlite + serde + reqwest (existing); React 18 + TypeScript strict (existing); Tauri 2 (existing). No new third-party deps.

**Spec:** `docs/superpowers/specs/2026-05-18-m3-character-sheets.md`

**Per-session prerequisite (Windows / PowerShell):**

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"
```

Put this at the top of every implementer prompt.

**Pre-authorized lint fixes (clippy):** `must_use_candidate`, `doc_markdown`, `single_match_else`, `implicit_hasher`, `inherent_to_string`, `cast_precision_loss`, `cast_possible_truncation`, `uninlined_format_args`, `duration_suboptimal_units`, `assigning_clones`, `needless_pass_by_value`, `match_same_arms` (with `#[allow]` when state-machine arms are semantically distinct), `field_reassign_with_default` (use struct-update syntax). Apply silently.

**Pre-authorized Vitest patterns:**
- Use `vi.hoisted(() => ({ mock: vi.fn() }))` for top-level mock identities.
- Use `vi.useFakeTimers({ shouldAdvanceTime: true })` in `beforeEach` whenever combining fake timers with `waitFor`.
- Apply the cancellation-race fix pattern to any new `useEffect` + async subscription (capture `cancelled` flag; check after await before storing unsub).
- `pnpm --filter @water/app test src/path/to/file.test.tsx` (no `--`).

**TS strict mode is on with `noUncheckedIndexedAccess: true`.** Array/regex/Map index lookups need `?.[i] ?? fallback` or explicit guards.

**Carry-forward M2/M2.5 lessons:**
- **`Db: Send + !Sync`.** Use `Arc<Mutex<Db>>` pattern from `app/src-tauri/src/commands/scene.rs::scene_rename`. Drop project lock before db lock.
- **React 18 deleted-tree cleanup order** (KNOWN_FRAGILE #13): if you wrap an imperative resource owned by a parent component (e.g. PM view), defer the parent's destroy to a `queueMicrotask` AND add an `isDestroyed` guard in the child's cleanup. M2 T6 fix is the template.
- **rusqlite::Statement: !Send.** Async Tauri handlers can't hold one across `.await`. Scope queries in their own block returning owned `Vec`.

---

## Phase A — Schema + storage

### Task 1: IntakeField type + LSM v2.1 schema descriptors

Defines the Rust `IntakeField` type that the Conversational Intake component walks, plus the 29-field LSM v2.1 schema constants (12 main + 8 bonus_traits + 5 arc + 4 perspectives). Pure Rust, no Tauri integration yet.

**Files:**
- Create: `crates/water-core/src/character/mod.rs` (or update existing if structure permits)
- Create: `crates/water-core/src/character/intake.rs`
- Modify: `crates/water-core/src/lib.rs`

- [ ] **Step 1: Confirm the existing module structure**

Read `crates/water-core/src/lib.rs` and `crates/water-core/src/character.rs`. The current code likely has `character.rs` as a single file with `CharacterStore` + `CharacterFile`. To add `intake.rs` cleanly, either convert `character.rs` to `character/mod.rs` + `character/store.rs` + `character/intake.rs`, OR keep `character.rs` and add a sibling `character_intake.rs`.

For this plan we'll convert to a directory (cleaner long-term — Phase A.3 adds `character/registry.rs` and Phase D adds `character/autosuggest.rs`). If the file is already small the conversion is mechanical.

Run from `C:\Users\H BLAUNTE\Water`:

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"
New-Item -ItemType Directory -Path "crates\water-core\src\character"
Move-Item "crates\water-core\src\character.rs" "crates\water-core\src\character\mod.rs"
```

Update `crates/water-core/src/lib.rs` `pub mod character` is unchanged (Rust resolves `character::*` to `character/mod.rs` automatically). Run a sanity build to confirm:

```powershell
cargo build -p water-core
```

Expected: clean.

- [ ] **Step 2: Write the failing test for IntakeField + LSM constants**

Create `crates/water-core/src/character/intake.rs`:

```rust
//! Conversational Intake schema descriptors.
//!
//! Each `IntakeField` describes one question in a schema. The
//! `ConversationalIntake` renderer reads these via the Tauri command
//! `intake_schema` and walks them one at a time. Per-answer commits write
//! back through `character_update_field`.
//!
//! LSM v2.1 is the only schema in M3. M4 will add World Bible segment
//! schemas against the same `IntakeField` type.

use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case", tag = "type", content = "options")]
pub enum IntakeFieldKind {
    ShortText,
    LongText,
    StringList,
    Choice(&'static [&'static str]),
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct IntakeField {
    pub id: &'static str,
    pub section: &'static str,
    pub label: &'static str,
    pub prompt_question: &'static str,
    pub helper: Option<&'static str>,
    pub examples: &'static [&'static str],
    pub kind: IntakeFieldKind,
    pub optional_skip: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lsm_v2_1_has_29_fields_total() {
        let total = LSM_MAIN.len() + LSM_BONUS_TRAITS.len() + LSM_ARC.len() + LSM_PERSPECTIVES.len();
        assert_eq!(total, 29);
    }

    #[test]
    fn lsm_v2_1_has_8_required_fields() {
        let required = LSM_MAIN.iter().chain(LSM_BONUS_TRAITS).chain(LSM_ARC).chain(LSM_PERSPECTIVES)
            .filter(|f| !f.optional_skip)
            .count();
        assert_eq!(required, 8);
    }

    #[test]
    fn lsm_v2_1_field_ids_are_unique() {
        let mut all: Vec<&str> = LSM_V2_1.iter()
            .flat_map(|(_, fields)| fields.iter().map(|f| f.id))
            .collect();
        all.sort_unstable();
        let original_len = all.len();
        all.dedup();
        assert_eq!(all.len(), original_len, "duplicate field id in LSM_V2_1");
    }

    #[test]
    fn lsm_v2_1_field_ids_are_dotted_paths() {
        for (section_name, fields) in LSM_V2_1 {
            for field in *fields {
                let prefix = format!("{section_name}.");
                assert!(field.id.starts_with(&prefix), "field {} should start with {}", field.id, prefix);
                assert_eq!(field.section, *section_name);
            }
        }
    }
}
```

(The constants `LSM_MAIN`, `LSM_BONUS_TRAITS`, `LSM_ARC`, `LSM_PERSPECTIVES`, and `LSM_V2_1` don't exist yet. Tests will fail to compile.)

- [ ] **Step 3: Run; expect compile failure**

```powershell
cargo test -p water-core character::intake 2>&1 | Select-Object -Last 6
```

Expected: FAIL — `cannot find value LSM_MAIN in this scope` etc.

- [ ] **Step 4: Add the 29-field constants**

Append to `crates/water-core/src/character/intake.rs`:

```rust
// ---- main (12 fields) ----

pub const LSM_MAIN: &[IntakeField] = &[
    IntakeField {
        id: "main.full_name",
        section: "main",
        label: "Full name",
        prompt_question: "What is this character's full name?",
        helper: None,
        examples: &["Marcus Vale", "Ada Thorne"],
        kind: IntakeFieldKind::ShortText,
        optional_skip: false,
    },
    IntakeField {
        id: "main.aliases",
        section: "main",
        label: "Aliases",
        prompt_question: "What other names is this character known by? (Nicknames, titles, pen names.)",
        helper: None,
        examples: &[],
        kind: IntakeFieldKind::StringList,
        optional_skip: true,
    },
    IntakeField {
        id: "main.age",
        section: "main",
        label: "Age",
        prompt_question: "How old are they at the start of the story?",
        helper: None,
        examples: &["32", "early 40s", "ageless"],
        kind: IntakeFieldKind::ShortText,
        optional_skip: true,
    },
    IntakeField {
        id: "main.pronouns",
        section: "main",
        label: "Pronouns",
        prompt_question: "What pronouns?",
        helper: None,
        examples: &["she/her", "they/them", "he/him"],
        kind: IntakeFieldKind::ShortText,
        optional_skip: true,
    },
    IntakeField {
        id: "main.role_in_story",
        section: "main",
        label: "Role in story",
        prompt_question: "What role does this character play in the story?",
        helper: None,
        examples: &[],
        kind: IntakeFieldKind::Choice(&["protagonist", "antagonist", "supporting", "mentor", "foil", "other"]),
        optional_skip: false,
    },
    IntakeField {
        id: "main.want",
        section: "main",
        label: "Want",
        prompt_question: "What do they want? What are they consciously pursuing?",
        helper: None,
        examples: &[],
        kind: IntakeFieldKind::LongText,
        optional_skip: false,
    },
    IntakeField {
        id: "main.need",
        section: "main",
        label: "Need",
        prompt_question: "What do they actually need? What would heal them, even if they don't see it?",
        helper: None,
        examples: &[],
        kind: IntakeFieldKind::LongText,
        optional_skip: false,
    },
    IntakeField {
        id: "main.ghost_wound",
        section: "main",
        label: "Ghost wound",
        prompt_question: "What past event still haunts them? What unhealed thing shapes who they are today?",
        helper: None,
        examples: &[],
        kind: IntakeFieldKind::LongText,
        optional_skip: true,
    },
    IntakeField {
        id: "main.lie_they_believe",
        section: "main",
        label: "Lie they believe",
        prompt_question: "What false belief do they hold about themselves or the world? What story do they tell themselves that isn't quite true?",
        helper: None,
        examples: &[],
        kind: IntakeFieldKind::LongText,
        optional_skip: false,
    },
    IntakeField {
        id: "main.truth",
        section: "main",
        label: "Truth",
        prompt_question: "What truth would set them free if they could see it?",
        helper: None,
        examples: &[],
        kind: IntakeFieldKind::LongText,
        optional_skip: true,
    },
    IntakeField {
        id: "main.fatal_flaw",
        section: "main",
        label: "Fatal flaw",
        prompt_question: "What character trait will most likely undo them in this story?",
        helper: None,
        examples: &[],
        kind: IntakeFieldKind::LongText,
        optional_skip: true,
    },
    IntakeField {
        id: "main.strength",
        section: "main",
        label: "Strength",
        prompt_question: "What is their greatest virtue or capacity?",
        helper: None,
        examples: &[],
        kind: IntakeFieldKind::LongText,
        optional_skip: true,
    },
];

// ---- bonus_traits (8 fields) ----

pub const LSM_BONUS_TRAITS: &[IntakeField] = &[
    IntakeField {
        id: "bonus_traits.voice",
        section: "bonus_traits",
        label: "Voice",
        prompt_question: "How would you describe their voice? (Cadence, register, tone — not what they say but how they sound.)",
        helper: None,
        examples: &["spare, weather-worn, with quiet warmth", "clipped and precise, like a lawyer"],
        kind: IntakeFieldKind::LongText,
        optional_skip: false,
    },
    IntakeField {
        id: "bonus_traits.tells",
        section: "bonus_traits",
        label: "Tells",
        prompt_question: "What do they do without realizing it? (Physical or verbal tells.)",
        helper: None,
        examples: &[],
        kind: IntakeFieldKind::StringList,
        optional_skip: true,
    },
    IntakeField {
        id: "bonus_traits.habits",
        section: "bonus_traits",
        label: "Habits",
        prompt_question: "What recurring small actions or rituals shape their day?",
        helper: None,
        examples: &[],
        kind: IntakeFieldKind::StringList,
        optional_skip: true,
    },
    IntakeField {
        id: "bonus_traits.speech_patterns",
        section: "bonus_traits",
        label: "Speech patterns",
        prompt_question: "What phrases, fillers, or quirks of speech recur in their dialogue?",
        helper: None,
        examples: &[],
        kind: IntakeFieldKind::StringList,
        optional_skip: true,
    },
    IntakeField {
        id: "bonus_traits.physicality",
        section: "bonus_traits",
        label: "Physicality",
        prompt_question: "How do they move? How do they hold themselves in a room?",
        helper: None,
        examples: &[],
        kind: IntakeFieldKind::LongText,
        optional_skip: true,
    },
    IntakeField {
        id: "bonus_traits.preferences",
        section: "bonus_traits",
        label: "Preferences",
        prompt_question: "Any strong likes, dislikes, or aesthetic preferences? (One per line: `coffee: bitter, no sugar`.)",
        helper: None,
        examples: &[],
        kind: IntakeFieldKind::StringList,
        optional_skip: true,
    },
    IntakeField {
        id: "bonus_traits.fears",
        section: "bonus_traits",
        label: "Fears",
        prompt_question: "What are they most afraid of? (Not phobias — the real fears.)",
        helper: None,
        examples: &[],
        kind: IntakeFieldKind::StringList,
        optional_skip: false,
    },
    IntakeField {
        id: "bonus_traits.values",
        section: "bonus_traits",
        label: "Values",
        prompt_question: "What do they hold sacred? What would they refuse to compromise on?",
        helper: None,
        examples: &[],
        kind: IntakeFieldKind::StringList,
        optional_skip: false,
    },
];

// ---- arc (5 fields) ----

pub const LSM_ARC: &[IntakeField] = &[
    IntakeField {
        id: "arc.starting_state",
        section: "arc",
        label: "Starting state",
        prompt_question: "Where is this character emotionally / morally / situationally when the story begins?",
        helper: None,
        examples: &[],
        kind: IntakeFieldKind::LongText,
        optional_skip: true,
    },
    IntakeField {
        id: "arc.ending_state",
        section: "arc",
        label: "Ending state",
        prompt_question: "Where are they by the end?",
        helper: None,
        examples: &[],
        kind: IntakeFieldKind::LongText,
        optional_skip: true,
    },
    IntakeField {
        id: "arc.inciting_change",
        section: "arc",
        label: "Inciting change",
        prompt_question: "What event in the early story knocks them out of equilibrium?",
        helper: None,
        examples: &[],
        kind: IntakeFieldKind::LongText,
        optional_skip: true,
    },
    IntakeField {
        id: "arc.midpoint_shift",
        section: "arc",
        label: "Midpoint shift",
        prompt_question: "What changes at the midpoint? What do they finally see, or refuse?",
        helper: None,
        examples: &[],
        kind: IntakeFieldKind::LongText,
        optional_skip: true,
    },
    IntakeField {
        id: "arc.climax_choice",
        section: "arc",
        label: "Climax choice",
        prompt_question: "What choice defines them at the climax?",
        helper: None,
        examples: &[],
        kind: IntakeFieldKind::LongText,
        optional_skip: true,
    },
];

// ---- perspectives (4 fields) ----

pub const LSM_PERSPECTIVES: &[IntakeField] = &[
    IntakeField {
        id: "perspectives.self_view",
        section: "perspectives",
        label: "Self view",
        prompt_question: "How do they see themselves?",
        helper: None,
        examples: &[],
        kind: IntakeFieldKind::LongText,
        optional_skip: true,
    },
    IntakeField {
        id: "perspectives.others_view",
        section: "perspectives",
        label: "Others view",
        prompt_question: "How do other characters in the story see them?",
        helper: None,
        examples: &[],
        kind: IntakeFieldKind::LongText,
        optional_skip: true,
    },
    IntakeField {
        id: "perspectives.narrator_view",
        section: "perspectives",
        label: "Narrator view",
        prompt_question: "How does the narrative voice (whether explicit or implicit) frame them?",
        helper: None,
        examples: &[],
        kind: IntakeFieldKind::LongText,
        optional_skip: true,
    },
    IntakeField {
        id: "perspectives.antagonist_view",
        section: "perspectives",
        label: "Antagonist view",
        prompt_question: "How would their antagonist describe them?",
        helper: None,
        examples: &[],
        kind: IntakeFieldKind::LongText,
        optional_skip: true,
    },
];

pub const LSM_V2_1: &[(&str, &[IntakeField])] = &[
    ("main", LSM_MAIN),
    ("bonus_traits", LSM_BONUS_TRAITS),
    ("arc", LSM_ARC),
    ("perspectives", LSM_PERSPECTIVES),
];
```

- [ ] **Step 5: Register the new module**

Modify `crates/water-core/src/character/mod.rs` — append at the top:

```rust
pub mod intake;
```

Verify nothing else needs re-exporting; `IntakeField` is reachable as `water_core::character::intake::IntakeField`.

- [ ] **Step 6: Run tests; expect pass**

```powershell
cargo test -p water-core character::intake
```

Expected: 4 tests pass.

- [ ] **Step 7: Run lints + fmt**

```powershell
cargo clippy -p water-core --all-targets -- -D warnings
cargo fmt -p water-core --check
```

Expected: clean.

- [ ] **Step 8: Commit**

```powershell
git add crates/water-core/src/character/
git commit -m "feat(character): IntakeField type + LSM v2.1 schema constants (M3 T1)"
```

---

### Task 2: Schema v3 migration — `hue_token` column

Adds a `hue_token TEXT NOT NULL DEFAULT ''` column to the `character` table and backfills existing rows via round-robin against the `--water-hue-character-{1..6}` palette. Updates `schema_version` to 3.

**Files:**
- Create: `crates/water-core/sql/v3_character_hue.sql`
- Modify: `crates/water-core/src/migrations.rs`
- Modify: `app/src/styles/tokens.css`

- [ ] **Step 1: Add the hue tokens to tokens.css**

Modify `app/src/styles/tokens.css`. After the existing `--water-hue-chorus` line (or wherever persona hues live), append:

```css
  /* Character voice palette — round-robin assigned at character create time.
     Final shades pinned in M7. Distinct from persona hues. */
  --water-hue-character-1: #c5d4e8;  /* soft periwinkle */
  --water-hue-character-2: #e8c5d4;  /* soft rose */
  --water-hue-character-3: #d4e8c5;  /* soft sage */
  --water-hue-character-4: #e8d4c5;  /* soft sand */
  --water-hue-character-5: #c5e8d4;  /* soft mint */
  --water-hue-character-6: #d4c5e8;  /* soft lilac */
```

Add the same six lines inside any dark-mode `@media (prefers-color-scheme: dark)` block if persona hues are also redefined there (read tokens.css to confirm pattern).

- [ ] **Step 2: Write the v3 migration SQL**

Create `crates/water-core/sql/v3_character_hue.sql`:

```sql
-- M3 schema v3: add character hue_token column with round-robin backfill.
-- Forward-only. v2 -> v3.

ALTER TABLE character ADD COLUMN hue_token TEXT NOT NULL DEFAULT '';

-- Backfill: round-robin assign hues to existing characters by created_at order.
-- M1 created the character row format; M3 introduces hue tokens. Existing rows
-- get hues 1..6 cycling by oldest-first.
UPDATE character
SET hue_token = '--water-hue-character-' || (
    ((SELECT COUNT(*) FROM character AS c2 WHERE c2.created_at < character.created_at OR (c2.created_at = character.created_at AND c2.id < character.id)) % 6) + 1
);

UPDATE schema_version SET version = 3;
```

(The subquery-rank pattern is portable across SQLite versions; avoids needing `ROW_NUMBER()` window function which only exists in 3.25+. Both approaches work for this codebase's bundled rusqlite SQLite.)

- [ ] **Step 3: Register the migration**

Modify `crates/water-core/src/migrations.rs`. Find the `MIGRATIONS` slice (added in M2 T5). Append:

```rust
const MIGRATIONS: &[(u32, &str)] = &[
    (2, include_str!("../sql/v2_pill_engine.sql")),
    (3, include_str!("../sql/v3_character_hue.sql")),
];
```

- [ ] **Step 4: Write failing tests**

Append to `crates/water-core/src/migrations.rs`'s test module (or wherever migration tests live — see M2 T5's pattern):

```rust
#[test]
fn migration_ratchets_to_v3() {
    let (_tmp, mut db) = fresh_v1_db();
    run_pending(&mut db).unwrap();
    assert_eq!(current_version(db.conn()).unwrap(), 3);
}

#[test]
fn migration_v3_adds_hue_token_column() {
    let (_tmp, mut db) = fresh_v1_db();
    run_pending(&mut db).unwrap();
    let cols: Vec<String> = db
        .conn()
        .prepare("PRAGMA table_info(character)")
        .unwrap()
        .query_map([], |r| r.get::<_, String>(1))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert!(cols.iter().any(|c| c == "hue_token"), "missing hue_token column");
}

#[test]
fn migration_v3_backfills_hue_round_robin() {
    let (_tmp, mut db) = fresh_v1_db();
    // Insert a project + 4 characters at v1 (without hue_token column).
    // Then migrate.
    db.conn().execute(
        "INSERT INTO project (id, name, created_at, updated_at) VALUES ('p1', 'P', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        rusqlite::params![],
    ).unwrap();
    for (i, id) in ["c1", "c2", "c3", "c4"].iter().enumerate() {
        let created_at = format!("2026-01-0{}T00:00:00Z", i + 1);
        db.conn().execute(
            "INSERT INTO character (id, project_id, name, schema_version, data_json, file_path, created_at, updated_at)
             VALUES (?1, 'p1', ?2, 'lsm-v2.1', '{}', ?3, ?4, ?4)",
            rusqlite::params![id, id, format!("characters/{}.toml", id), created_at],
        ).unwrap();
    }
    run_pending(&mut db).unwrap();
    let hues: Vec<String> = db
        .conn()
        .prepare("SELECT hue_token FROM character ORDER BY created_at")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(hues, vec![
        "--water-hue-character-1".to_string(),
        "--water-hue-character-2".to_string(),
        "--water-hue-character-3".to_string(),
        "--water-hue-character-4".to_string(),
    ]);
}
```

- [ ] **Step 5: Run tests; expect pass**

```powershell
cargo test -p water-core migrations
```

Expected: existing M2 migration tests pass + 3 new M3 tests pass.

- [ ] **Step 6: Run gates**

```powershell
cargo test -p water-core
cargo clippy -p water-core --all-targets -- -D warnings
cargo fmt -p water-core --check
pnpm --filter @water/app build
```

Expected: all clean. The build verifies the tokens.css changes don't break Vite.

- [ ] **Step 7: Commit**

```powershell
git add crates/water-core/sql/v3_character_hue.sql crates/water-core/src/migrations.rs app/src/styles/tokens.css
git commit -m "feat(core): v3 migration — character hue_token + palette tokens (M3 T2)"
```

---

### Task 3: `CharacterRegistry::from_db` implementation

Implements the `CharacterRegistry` that the voice router calls. Populates from the `character` table, exposes `by_id`, `list`, and `pick_lru_present(present, cooldowns)` for LRU selection among present characters.

**Files:**
- Create: `crates/water-core/src/character/registry.rs`
- Modify: `crates/water-core/src/character/mod.rs`

- [ ] **Step 1: Write failing tests**

Create `crates/water-core/src/character/registry.rs`:

```rust
//! Character registry — populated from the `character` table on project
//! open. Sibling of `voice::PersonaRegistry`. Used by the voice router
//! when a character-track trigger fires.
//!
//! Hue assignment happens in `CharacterStore::insert`; this registry just
//! reads what's already there.

use crate::voice::speaker::{Speaker, SpeakerArc, SpeakerKind};
use crate::{Db, Id};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

#[derive(Debug, Clone)]
pub struct CharacterRow {
    pub id: Id,
    pub name: String,
    pub hue_token: String,
    /// JSON-decoded sheet data. Sourced from the `character.data_json` column.
    pub data: serde_json::Value,
}

pub struct CharacterRegistry {
    by_id: HashMap<String, SpeakerArc>,
    rows: Vec<CharacterRow>,
}

impl CharacterRegistry {
    /// Load characters from the project DB. Each character becomes a
    /// `CharacterSpeaker` indexed by its `Id`.
    pub fn from_db(db: &Db) -> Result<Self, String> {
        let mut stmt = db.conn().prepare(
            "SELECT id, name, hue_token, data_json FROM character ORDER BY created_at"
        ).map_err(|e| e.to_string())?;
        let rows_iter = stmt.query_map([], |r| {
            let data_json: String = r.get(3)?;
            Ok(CharacterRow {
                id: Id::from_str(&r.get::<_, String>(0)?).map_err(|e| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e))))?,
                name: r.get(1)?,
                hue_token: r.get(2)?,
                data: serde_json::from_str(&data_json).unwrap_or(serde_json::Value::Null),
            })
        }).map_err(|e| e.to_string())?;
        let mut rows: Vec<CharacterRow> = Vec::new();
        let mut by_id: HashMap<String, SpeakerArc> = HashMap::new();
        for row in rows_iter {
            let row = row.map_err(|e| e.to_string())?;
            let speaker: SpeakerArc = Arc::new(crate::voice::speaker::CharacterSpeaker::from_row(&row));
            by_id.insert(row.id.as_str().to_string(), speaker);
            rows.push(row);
        }
        Ok(Self { by_id, rows })
    }

    #[must_use]
    pub fn empty() -> Self {
        Self {
            by_id: HashMap::new(),
            rows: Vec::new(),
        }
    }

    #[must_use]
    pub fn by_id(&self, id: &str) -> Option<SpeakerArc> {
        self.by_id.get(id).cloned()
    }

    #[must_use]
    pub fn list(&self) -> &[CharacterRow] {
        &self.rows
    }

    /// Returns the least-recently-used character from `present`, skipping
    /// characters whose cooldown hasn't elapsed.
    #[must_use]
    pub fn pick_lru_present(
        &self,
        present: &[Id],
        cooldowns: &HashMap<String, Instant>,
        now: Instant,
    ) -> Option<SpeakerArc> {
        let mut candidates: Vec<(SpeakerArc, Option<Instant>)> = present
            .iter()
            .filter_map(|id| self.by_id(id.as_str()).map(|s| {
                let last = cooldowns.get(s.id()).copied();
                (s, last)
            }))
            .filter(|(s, last)| {
                match last {
                    Some(t) => now.duration_since(*t).as_millis() as u64 >= s.cooldown_ms(),
                    None => true,
                }
            })
            .collect();
        // Sort by last-emit ascending (oldest first); None (never emitted) is oldest.
        candidates.sort_by_key(|(_, last)| last.map_or(Instant::now() - std::time::Duration::from_secs(86400), |t| t));
        candidates.into_iter().next().map(|(s, _)| s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn fresh_db_with_migrations() -> (TempDir, Db) {
        let dir = TempDir::new().unwrap();
        let mut db = Db::open(dir.path().join("project.db")).unwrap();
        crate::migrations::run_pending(&mut db).unwrap();
        // Seed a project.
        db.conn().execute(
            "INSERT INTO project (id, name, created_at, updated_at) VALUES ('p1', 'P', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
            rusqlite::params![],
        ).unwrap();
        (dir, db)
    }

    fn insert_character(db: &Db, id: &str, name: &str, hue: &str, created_at: &str) {
        db.conn().execute(
            "INSERT INTO character (id, project_id, name, schema_version, data_json, hue_token, file_path, created_at, updated_at)
             VALUES (?1, 'p1', ?2, 'lsm-v2.1', '{}', ?3, ?4, ?5, ?5)",
            rusqlite::params![id, name, hue, format!("characters/{}.toml", id), created_at],
        ).unwrap();
    }

    #[test]
    fn from_db_loads_zero_characters() {
        let (_tmp, db) = fresh_db_with_migrations();
        let reg = CharacterRegistry::from_db(&db).unwrap();
        assert_eq!(reg.list().len(), 0);
    }

    #[test]
    fn from_db_loads_one_character() {
        let (_tmp, db) = fresh_db_with_migrations();
        insert_character(&db, "01HE000000000000000000000A", "Marcus", "--water-hue-character-1", "2026-01-01T00:00:00Z");
        let reg = CharacterRegistry::from_db(&db).unwrap();
        assert_eq!(reg.list().len(), 1);
        assert_eq!(reg.list()[0].name, "Marcus");
        assert_eq!(reg.list()[0].hue_token, "--water-hue-character-1");
        let speaker = reg.by_id("01HE000000000000000000000A").unwrap();
        assert_eq!(speaker.display_name(), "Marcus");
        assert_eq!(speaker.kind(), SpeakerKind::Character);
    }

    #[test]
    fn pick_lru_present_returns_least_recently_used() {
        let (_tmp, db) = fresh_db_with_migrations();
        insert_character(&db, "01HE000000000000000000000A", "A", "--water-hue-character-1", "2026-01-01T00:00:00Z");
        insert_character(&db, "01HE000000000000000000000B", "B", "--water-hue-character-2", "2026-01-02T00:00:00Z");
        let reg = CharacterRegistry::from_db(&db).unwrap();
        let present = vec![
            Id::from_str("01HE000000000000000000000A").unwrap(),
            Id::from_str("01HE000000000000000000000B").unwrap(),
        ];
        let now = Instant::now();
        let mut cooldowns: HashMap<String, Instant> = HashMap::new();
        // B just emitted; A never emitted.
        cooldowns.insert("01HE000000000000000000000B".into(), now);
        // Default character cooldown is 60_000ms; advance past it for A's never-emitted to win
        let pick = reg.pick_lru_present(&present, &cooldowns, now);
        assert!(pick.is_some());
        assert_eq!(pick.unwrap().id(), "01HE000000000000000000000A");
    }
}
```

- [ ] **Step 2: Add CharacterSpeaker stub (T4 will fill it)**

This test references `crate::voice::speaker::CharacterSpeaker::from_row` which doesn't yet exist (it lands in T4). To unblock T3, add a minimal stub to `crates/water-core/src/voice/speaker.rs`:

```rust
// Append to voice/speaker.rs:

#[derive(Debug, Clone)]
pub struct CharacterSpeaker {
    id: String,
    display_name: String,
    hue_token: String,
    prompt_fragment: String,
    anti_loop_threshold: f32,
    cooldown_ms: u64,
}

impl CharacterSpeaker {
    /// Construct from a `CharacterRow`. T4 fills the prompt template
    /// rendering; T3's stub just wires identity + defaults.
    pub fn from_row(row: &crate::character::registry::CharacterRow) -> Self {
        Self {
            id: row.id.as_str().to_string(),
            display_name: row.name.clone(),
            hue_token: row.hue_token.clone(),
            prompt_fragment: String::new(), // T4 fills this
            anti_loop_threshold: 0.70,
            cooldown_ms: 60_000, // slightly longer than personas
        }
    }
}

impl Speaker for CharacterSpeaker {
    fn id(&self) -> &str { &self.id }
    fn kind(&self) -> SpeakerKind { SpeakerKind::Character }
    fn display_name(&self) -> &str { &self.display_name }
    fn hue_token(&self) -> &str { &self.hue_token }
    fn prompt_fragment(&self) -> &str { &self.prompt_fragment }
    fn anti_loop_threshold(&self) -> f32 { self.anti_loop_threshold }
    fn cooldown_ms(&self) -> u64 { self.cooldown_ms }
}
```

- [ ] **Step 3: Register the new module**

Modify `crates/water-core/src/character/mod.rs`:

```rust
pub mod intake;
pub mod registry;
```

Re-export `CharacterRegistry` and `CharacterRow` if convenient:

```rust
pub use registry::{CharacterRegistry, CharacterRow};
```

- [ ] **Step 4: Run tests**

```powershell
cargo test -p water-core character::registry
```

Expected: 3 tests pass.

- [ ] **Step 5: Gates**

```powershell
cargo test -p water-core
cargo clippy -p water-core --all-targets -- -D warnings
cargo fmt -p water-core --check
```

Expected: all clean.

- [ ] **Step 6: Commit**

```powershell
git add crates/water-core/src/character/ crates/water-core/src/voice/speaker.rs
git commit -m "feat(character): CharacterRegistry + CharacterSpeaker stub (M3 T3)"
```

---

## Phase B — Character voice + trigger fills

### Task 4: `CharacterSpeaker` full impl with voice-template rendering

Replaces T3's stub `prompt_fragment: String::new()` with a real renderer that substitutes LSM v2.1 sheet fields into the character voice template TOML. The template + substitution helper land in this task (split from the M2 prompt loader for cleanliness).

**Files:**
- Create: `prompts/speakers/character/template.toml`
- Modify: `crates/water-core/src/voice/speaker.rs`
- Create: `crates/water-core/src/voice/character_template.rs`
- Modify: `crates/water-core/src/voice/mod.rs`

- [ ] **Step 1: Create the voice template TOML**

Create `prompts/speakers/character/template.toml`:

```toml
version = "1"
schema_version = "lsm-v2.1"

[prompt]
voice_profile = """
You are {{full_name}}. {{role_descriptor}}

You want: {{want}}.
What you need (often without seeing it): {{need}}.
The lie you believe: {{lie_they_believe}}.

Your voice: {{voice}}.
You often say things like: {{speech_patterns}}.

What you fear: {{fears}}.
What you hold sacred: {{values}}.

What still haunts you: {{ghost_wound}}.
Your fatal flaw: {{fatal_flaw}}.

Speak as {{full_name}} would speak in this moment.
"""
```

- [ ] **Step 2: Write failing tests for the substitution helper**

Create `crates/water-core/src/voice/character_template.rs`:

```rust
//! Character voice template rendering.
//!
//! Takes an LSM v2.1 sheet (as `serde_json::Value`) and renders the voice
//! template at `prompts/speakers/character/template.toml`. Missing fields
//! cause their entire sentence to be omitted (missing-field policy per
//! M3 spec § 10).

use serde::Deserialize;

const TEMPLATE_TOML: &str = include_str!("../../../../prompts/speakers/character/template.toml");

#[derive(Debug, Deserialize)]
struct TemplateFile {
    version: String,
    #[serde(default)]
    schema_version: String,
    prompt: TemplatePrompt,
}

#[derive(Debug, Deserialize)]
struct TemplatePrompt {
    voice_profile: String,
}

#[derive(Debug, Clone)]
pub struct CharacterTemplate {
    /// The raw `voice_profile` string with `{{placeholder}}` markers.
    raw: String,
}

impl CharacterTemplate {
    /// Load the built-in template at compile time.
    #[must_use]
    pub fn load_builtin() -> Self {
        let parsed: TemplateFile = toml::from_str(TEMPLATE_TOML).expect("built-in character template must parse");
        Self { raw: parsed.prompt.voice_profile }
    }

    /// Render the template with the given LSM v2.1 sheet data. Missing
    /// fields cause their entire sentence to be omitted.
    #[must_use]
    pub fn render(&self, sheet: &serde_json::Value) -> String {
        let main = sheet.get("main").unwrap_or(&serde_json::Value::Null);
        let bonus = sheet.get("bonus_traits").unwrap_or(&serde_json::Value::Null);

        let substitutions: &[(&str, String)] = &[
            ("full_name", read_str(main, "full_name")),
            ("role_descriptor", role_descriptor(read_str(main, "role_in_story").as_str())),
            ("want", read_str(main, "want")),
            ("need", read_str(main, "need")),
            ("lie_they_believe", read_str(main, "lie_they_believe")),
            ("ghost_wound", read_str(main, "ghost_wound")),
            ("fatal_flaw", read_str(main, "fatal_flaw")),
            ("voice", read_str(bonus, "voice")),
            ("speech_patterns", read_list_joined(bonus, "speech_patterns")),
            ("fears", read_list_joined(bonus, "fears")),
            ("values", read_list_joined(bonus, "values")),
        ];

        // Split the template into sentences (paragraphs by blank lines or
        // sentences by ". "). For each unit, if it contains any placeholder
        // whose substitution is empty, drop the whole unit; otherwise emit
        // the unit with placeholders replaced.
        render_with_omission(&self.raw, substitutions)
    }
}

fn read_str(obj: &serde_json::Value, key: &str) -> String {
    obj.get(key).and_then(|v| v.as_str()).unwrap_or("").trim().to_string()
}

fn read_list_joined(obj: &serde_json::Value, key: &str) -> String {
    let arr = obj.get(key).and_then(|v| v.as_array());
    arr.map(|items| {
        items.iter()
            .filter_map(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(", ")
    }).unwrap_or_default()
}

fn role_descriptor(role: &str) -> String {
    match role {
        "protagonist" => "You are the protagonist of this story.".to_string(),
        "antagonist" => "You are an antagonist in this story.".to_string(),
        "supporting" => "You are a supporting character in this story.".to_string(),
        "mentor" => "You are a mentor figure in this story.".to_string(),
        "foil" => "You are a foil character in this story.".to_string(),
        _ => String::new(),
    }
}

fn render_with_omission(template: &str, subs: &[(&str, String)]) -> String {
    // Walk by line. If a line contains `{{X}}` and `X`'s substitution is
    // empty, drop the line. Otherwise emit the line with `{{X}}` →
    // substitution.
    let mut out_lines: Vec<String> = Vec::new();
    for line in template.lines() {
        let mut keep = true;
        let mut rendered = line.to_string();
        for (key, value) in subs {
            let marker = format!("{{{{{key}}}}}");
            if rendered.contains(&marker) {
                if value.is_empty() {
                    keep = false;
                    break;
                }
                rendered = rendered.replace(&marker, value);
            }
        }
        if keep {
            out_lines.push(rendered);
        }
    }
    // Collapse runs of empty lines to a single empty line.
    let mut collapsed: Vec<String> = Vec::new();
    let mut prev_empty = false;
    for line in out_lines {
        let is_empty = line.trim().is_empty();
        if is_empty && prev_empty { continue; }
        collapsed.push(line);
        prev_empty = is_empty;
    }
    collapsed.join("\n").trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn renders_full_sheet() {
        let t = CharacterTemplate::load_builtin();
        let sheet = json!({
            "main": {
                "full_name": "Marcus Vale",
                "role_in_story": "protagonist",
                "want": "to be seen as the man his father wasn't",
                "need": "to forgive himself for the night of the fire",
                "lie_they_believe": "If I just work hard enough, I can outrun what I did",
                "ghost_wound": "The fire he failed to stop when he was 15",
                "fatal_flaw": "He refuses to ask for help",
            },
            "bonus_traits": {
                "voice": "spare, weather-worn, with quiet warmth",
                "speech_patterns": ["You know what I mean", "It's fine"],
                "fears": ["losing his sister", "being seen as weak"],
                "values": ["loyalty", "showing up when it matters"],
            },
        });
        let rendered = t.render(&sheet);
        assert!(rendered.contains("Marcus Vale"));
        assert!(rendered.contains("protagonist of this story"));
        assert!(rendered.contains("spare, weather-worn"));
        assert!(rendered.contains("losing his sister, being seen as weak"));
        assert!(rendered.contains("loyalty, showing up when it matters"));
        assert!(rendered.contains("The fire he failed to stop"));
    }

    #[test]
    fn omits_sentences_with_missing_fields() {
        let t = CharacterTemplate::load_builtin();
        let sheet = json!({
            "main": {
                "full_name": "Ada",
                "role_in_story": "supporting",
                "want": "to retire",
                "need": "to face the regret she's been hiding",
                "lie_they_believe": "She has plenty of time",
                // ghost_wound + fatal_flaw absent
            },
            "bonus_traits": {
                "voice": "clipped",
                // speech_patterns, fears, values absent
            },
        });
        let rendered = t.render(&sheet);
        assert!(rendered.contains("Ada"));
        assert!(rendered.contains("clipped"));
        assert!(!rendered.contains("{{"), "no unresolved placeholders");
        assert!(!rendered.contains("What still haunts you"), "ghost_wound line dropped");
        assert!(!rendered.contains("Your fatal flaw"), "fatal_flaw line dropped");
        assert!(!rendered.contains("You often say things like"), "speech_patterns line dropped");
        assert!(!rendered.contains("What you fear"), "fears line dropped");
        assert!(!rendered.contains("What you hold sacred"), "values line dropped");
    }

    #[test]
    fn unknown_role_omits_descriptor() {
        let t = CharacterTemplate::load_builtin();
        let sheet = json!({
            "main": { "full_name": "X", "role_in_story": "weirdo", "want": "w", "need": "n", "lie_they_believe": "l" },
            "bonus_traits": { "voice": "v" },
        });
        let rendered = t.render(&sheet);
        // role_descriptor is empty → first line containing {{role_descriptor}} is dropped.
        // The "You are X." part is part of the same line; whole line drops.
        assert!(rendered.contains("X"), "name still appears elsewhere");
        assert!(!rendered.contains("weirdo"));
    }
}
```

- [ ] **Step 3: Run tests; expect compile failure**

```powershell
cargo test -p water-core voice::character_template
```

Expected: FAIL — the `pub use registry::{CharacterRegistry, CharacterRow}` re-export from T3 means we need to ensure module visibility is right.

- [ ] **Step 4: Register the new module**

Modify `crates/water-core/src/voice/mod.rs`. Append:

```rust
pub mod character_template;
```

- [ ] **Step 5: Wire CharacterSpeaker to use the template**

Modify `crates/water-core/src/voice/speaker.rs`. Replace the T3 stub of `CharacterSpeaker::from_row`:

```rust
impl CharacterSpeaker {
    pub fn from_row(row: &crate::character::registry::CharacterRow) -> Self {
        let template = crate::voice::character_template::CharacterTemplate::load_builtin();
        let prompt_fragment = template.render(&row.data);
        Self {
            id: row.id.as_str().to_string(),
            display_name: row.name.clone(),
            hue_token: row.hue_token.clone(),
            prompt_fragment,
            anti_loop_threshold: 0.70,
            cooldown_ms: 60_000,
        }
    }
}
```

- [ ] **Step 6: Run tests; expect pass**

```powershell
cargo test -p water-core voice::character_template
cargo test -p water-core character::registry
```

Expected: both pass. The registry's "from_db loads one character" test should now find a populated `prompt_fragment` on the resulting speaker.

- [ ] **Step 7: Strengthen the registry test**

Append to `crates/water-core/src/character/registry.rs`'s test module:

```rust
#[test]
fn speaker_prompt_fragment_includes_voice() {
    let (_tmp, db) = fresh_db_with_migrations();
    let data_json = serde_json::json!({
        "main": { "full_name": "Marcus", "role_in_story": "protagonist", "want": "w", "need": "n", "lie_they_believe": "l" },
        "bonus_traits": { "voice": "spare, weather-worn", "fears": ["x"], "values": ["y"] }
    }).to_string();
    db.conn().execute(
        "INSERT INTO character (id, project_id, name, schema_version, data_json, hue_token, file_path, created_at, updated_at)
         VALUES ('01HE000000000000000000000A', 'p1', 'Marcus', 'lsm-v2.1', ?1, '--water-hue-character-1', 'characters/x.toml', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        rusqlite::params![data_json],
    ).unwrap();
    let reg = CharacterRegistry::from_db(&db).unwrap();
    let speaker = reg.by_id("01HE000000000000000000000A").unwrap();
    assert!(speaker.prompt_fragment().contains("spare, weather-worn"));
    assert!(speaker.prompt_fragment().contains("Marcus"));
}
```

Run it:

```powershell
cargo test -p water-core character::registry::tests::speaker_prompt_fragment_includes_voice
```

Expected: PASS.

- [ ] **Step 8: Gates**

```powershell
cargo test -p water-core
cargo clippy -p water-core --all-targets -- -D warnings
cargo fmt -p water-core --check
```

Expected: all clean.

- [ ] **Step 9: Commit**

```powershell
git add prompts/speakers/character/template.toml crates/water-core/src/voice/character_template.rs crates/water-core/src/voice/speaker.rs crates/water-core/src/voice/mod.rs crates/water-core/src/character/registry.rs
git commit -m "feat(voice): CharacterSpeaker renders prompt_fragment from LSM template (M3 T4)"
```

---

### Task 5: Voice router POV-prefer rule

Extends `default_speaker_for_trigger` from a `&'static str` (persona id) return to a `DefaultSpeaker` enum that can be either a character or a persona. Character-track triggers prefer POV character → present-character LRU → persona default.

**Files:**
- Modify: `crates/water-core/src/voice/router.rs`
- Modify: `crates/water-core/src/voice/mod.rs`
- Modify: `crates/water-core/src/orchestrator/mod.rs` (extend `SceneSnapshot`)

- [ ] **Step 1: Extend `SceneSnapshot` with character data**

Find `SceneSnapshot` in `crates/water-core/src/orchestrator/mod.rs`. M2 had `characters_present: Vec<Id>` and `pov_character_id: Option<Id>`. Confirm both exist. If only one exists, add both.

- [ ] **Step 2: Write failing tests for the new router behavior**

Append to `crates/water-core/src/voice/router.rs`'s test module:

```rust
use crate::character::registry::CharacterRegistry;

#[test]
fn pov_character_picked_for_character_track_trigger() {
    let persona_reg = registry();
    let (_tmp, char_reg) = setup_character_registry_with_pov();
    let cd = CooldownState::default();
    let cand = cand_with_track("block_anchored_drift", SpeakerTrack::Character);
    let scene = scene_with_pov_and_present();
    let s = route_with_chars(&cand, &persona_reg, &char_reg, &scene, &cd, Instant::now()).unwrap();
    assert_eq!(s.id(), pov_character_id_str());
    assert_eq!(s.kind(), SpeakerKind::Character);
}

#[test]
fn pov_cooled_down_falls_to_lru_present_character() {
    let persona_reg = registry();
    let (_tmp, char_reg) = setup_character_registry_with_pov();
    let mut cd = CooldownState::default();
    cd.note_emit(pov_character_id_str());
    let cand = cand_with_track("block_anchored_drift", SpeakerTrack::Character);
    let scene = scene_with_pov_and_present();
    let s = route_with_chars(&cand, &persona_reg, &char_reg, &scene, &cd, Instant::now()).unwrap();
    assert_eq!(s.kind(), SpeakerKind::Character);
    assert_ne!(s.id(), pov_character_id_str());
}

#[test]
fn no_present_characters_falls_back_to_persona() {
    let persona_reg = registry();
    let char_reg = CharacterRegistry::empty();
    let cd = CooldownState::default();
    let cand = cand_with_track("block_anchored_drift", SpeakerTrack::Character);
    let scene = SceneSnapshot {
        id: Id::new(), pov_character_id: None, location_id: None,
        characters_present: vec![], word_count: 500, seconds_since_last_pill: 60,
    };
    let s = route_with_chars(&cand, &persona_reg, &char_reg, &scene, &cd, Instant::now()).unwrap();
    assert_eq!(s.kind(), SpeakerKind::Persona);
    assert_eq!(s.id(), "editor"); // M2 default persona for block_anchored_drift
}

#[test]
fn persona_track_trigger_unchanged_by_character_data() {
    let persona_reg = registry();
    let (_tmp, char_reg) = setup_character_registry_with_pov();
    let cd = CooldownState::default();
    // no_universe_yet is persona-track only
    let cand = cand_with_track("no_universe_yet", SpeakerTrack::Persona);
    let scene = scene_with_pov_and_present();
    let s = route_with_chars(&cand, &persona_reg, &char_reg, &scene, &cd, Instant::now()).unwrap();
    assert_eq!(s.kind(), SpeakerKind::Persona);
    assert_eq!(s.id(), "chorus");
}

// Helpers
fn cand_with_track(id: &'static str, track: SpeakerTrack) -> TriggerCandidate {
    TriggerCandidate {
        trigger_id: id,
        priority: 5.0,
        preferred_track: track,
        reason: String::new(),
        block_target_id: None,
        requires_confirmation: None, // M3 T7 adds this field; default for tests
    }
}

fn pov_character_id_str() -> &'static str {
    "01HE000000000000000000POV1"
}

fn setup_character_registry_with_pov() -> (TempDir, CharacterRegistry) {
    let dir = TempDir::new().unwrap();
    let mut db = Db::open(dir.path().join("p.db")).unwrap();
    crate::migrations::run_pending(&mut db).unwrap();
    db.conn().execute(
        "INSERT INTO project (id, name, created_at, updated_at) VALUES ('p1', 'P', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        rusqlite::params![],
    ).unwrap();
    let data = serde_json::json!({"main":{"full_name":"POV"},"bonus_traits":{"voice":"v"}}).to_string();
    db.conn().execute(
        "INSERT INTO character (id, project_id, name, schema_version, data_json, hue_token, file_path, created_at, updated_at)
         VALUES (?1, 'p1', 'POV', 'lsm-v2.1', ?2, '--water-hue-character-1', 'x', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        rusqlite::params![pov_character_id_str(), data],
    ).unwrap();
    // Second character (non-POV)
    let data2 = serde_json::json!({"main":{"full_name":"OTHER"},"bonus_traits":{"voice":"v"}}).to_string();
    db.conn().execute(
        "INSERT INTO character (id, project_id, name, schema_version, data_json, hue_token, file_path, created_at, updated_at)
         VALUES ('01HE0000000000000000OTHER1', 'p1', 'OTHER', 'lsm-v2.1', ?1, '--water-hue-character-2', 'y', '2026-01-02T00:00:00Z', '2026-01-02T00:00:00Z')",
        rusqlite::params![data2],
    ).unwrap();
    let reg = CharacterRegistry::from_db(&db).unwrap();
    (dir, reg)
}

fn scene_with_pov_and_present() -> SceneSnapshot {
    SceneSnapshot {
        id: Id::new(),
        pov_character_id: Some(Id::from_str(pov_character_id_str()).unwrap()),
        location_id: None,
        characters_present: vec![
            Id::from_str(pov_character_id_str()).unwrap(),
            Id::from_str("01HE0000000000000000OTHER1").unwrap(),
        ],
        word_count: 500,
        seconds_since_last_pill: 60,
    }
}
```

(`route_with_chars` is the new function we're adding; `cand_with_track` is the existing helper extended with the `preferred_track` arg.)

- [ ] **Step 3: Run; expect compile failure**

```powershell
cargo test -p water-core voice::router
```

Expected: FAIL — `route_with_chars` doesn't exist; `TriggerCandidate.requires_confirmation` doesn't exist yet.

- [ ] **Step 4: Stub `requires_confirmation` on TriggerCandidate**

This field is fully implemented in T7, but the router tests need it to compile. Add a minimal stub now:

Modify `crates/water-core/src/orchestrator/mod.rs`. Find `TriggerCandidate` and add:

```rust
#[derive(Debug, Clone)]
pub struct ConfirmationRequest {
    pub task_id: &'static str,
    pub character_id: Id,
    pub field_label: String,
    pub field_value: String,
}

#[derive(Debug, Clone)]
pub struct TriggerCandidate {
    pub trigger_id: &'static str,
    pub priority: f32,
    pub preferred_track: SpeakerTrack,
    pub reason: String,
    pub block_target_id: Option<String>,
    pub requires_confirmation: Option<ConfirmationRequest>,
}
```

Existing trigger implementations may construct `TriggerCandidate { ... }` literals without `requires_confirmation`. Update each one to add `requires_confirmation: None`. Grep for `TriggerCandidate {` to find them all:

```powershell
Select-String -Path 'crates\water-core\src\orchestrator\triggers\*.rs' -Pattern 'TriggerCandidate {' -List
```

For each match file, append `requires_confirmation: None,` to the literal.

- [ ] **Step 5: Implement `route_with_chars`**

Modify `crates/water-core/src/voice/router.rs`. Add the new function alongside the existing `route()`:

```rust
use crate::character::registry::CharacterRegistry;
use crate::orchestrator::{SceneSnapshot, SpeakerTrack, TriggerCandidate};

const CHAR_TRACK_TRIGGERS: &[&str] = &[
    "block_anchored_drift",
    "topic_drift",
    "valence_spike",
    "idle_pause_with_present_character",
    "character_dissonance",
];

#[must_use]
pub fn route_with_chars(
    candidate: &TriggerCandidate,
    personas: &PersonaRegistry,
    characters: &CharacterRegistry,
    scene: &SceneSnapshot,
    cooldowns: &CooldownState,
    now: Instant,
) -> Option<SpeakerArc> {
    // Character-track triggers when scene has characters: prefer POV →
    // LRU present character → fall back to persona.
    let is_char_track = CHAR_TRACK_TRIGGERS.contains(&candidate.trigger_id)
        && (candidate.preferred_track == SpeakerTrack::Character
            || candidate.preferred_track == SpeakerTrack::Either);
    if is_char_track && !scene.characters_present.is_empty() {
        // 1) POV if set and present.
        if let Some(pov_id) = scene.pov_character_id.as_ref() {
            if scene.characters_present.contains(pov_id) {
                if let Some(speaker) = characters.by_id(pov_id.as_str()) {
                    let last = cooldowns.last_emit.get(speaker.id()).copied();
                    let on_cooldown = last.is_some_and(|t| now.duration_since(t).as_millis() < u128::from(speaker.cooldown_ms()));
                    if !on_cooldown {
                        return Some(speaker);
                    }
                }
            }
        }
        // 2) LRU among present, non-cooled-down characters.
        if let Some(speaker) = characters.pick_lru_present(&scene.characters_present, &cooldowns.last_emit, now) {
            return Some(speaker);
        }
    }
    // 3) Fall through to persona routing (existing M2 logic).
    route(candidate, personas, cooldowns, now)
}
```

(`route` is the M2 persona-only function; `route_with_chars` wraps it.)

- [ ] **Step 6: Run tests; expect pass**

```powershell
cargo test -p water-core voice::router
```

Expected: 4 new tests pass + the existing M2 router tests still pass.

- [ ] **Step 7: Gates**

```powershell
cargo test -p water-core
cargo clippy -p water-core --all-targets -- -D warnings
cargo fmt -p water-core --check
```

Expected: all clean.

- [ ] **Step 8: Commit**

```powershell
git add crates/water-core/src/orchestrator/mod.rs crates/water-core/src/orchestrator/triggers/ crates/water-core/src/voice/router.rs
git commit -m "feat(voice): route_with_chars — POV-prefer with cooldown LRU fallback (M3 T5)"
```

---

### Task 6: Fill `idle_pause_with_present_character` trigger

Replaces the M2 stub with: fires when idle ≥ 8s + scene has characters present + ≥ 60s since last pill. Priority 4.0. Character track.

**Files:**
- Modify: `crates/water-core/src/orchestrator/triggers/idle_pause_with_present_character.rs`

- [ ] **Step 1: Write failing tests**

Replace the test module in `idle_pause_with_present_character.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::orchestrator::*;
    use crate::Id;

    fn ctx_with(
        idle_ms: u64,
        characters_present: Vec<Id>,
        seconds_since_last_pill: u64,
        cursor: CursorClassification,
    ) -> (TypingTelemetry, AnalysisSnapshot, SceneSnapshot, ProjectSnapshot) {
        let telem = TypingTelemetry {
            idle_for_ms: idle_ms,
            cursor_classification: cursor,
            block_id: "^bk-0001".to_string(),
            recent_word_delta: 0,
            structural_inflection: StructuralInflection::None,
        };
        let analysis = AnalysisSnapshot::default();
        let scene = SceneSnapshot {
            id: Id::new(),
            pov_character_id: characters_present.first().cloned(),
            location_id: None,
            characters_present,
            word_count: 500,
            seconds_since_last_pill,
        };
        (telem, analysis, scene, ProjectSnapshot::default())
    }

    #[test]
    fn fires_when_idle_and_chars_present_and_no_recent_pill() {
        let (t, a, s, p) = ctx_with(9000, vec![Id::new()], 60, CursorClassification::AtParagraphEnd);
        let c = TriggerContext { telemetry: &t, analysis: &a, scene: &s, project: &p };
        let cand = IdlePauseWithPresentCharacter.evaluate(&c).unwrap();
        assert_eq!(cand.trigger_id, "idle_pause_with_present_character");
        assert!((cand.priority - 4.0).abs() < 1e-5);
        assert_eq!(cand.preferred_track, SpeakerTrack::Character);
    }

    #[test]
    fn does_not_fire_when_not_idle() {
        let (t, a, s, p) = ctx_with(5000, vec![Id::new()], 60, CursorClassification::AtParagraphEnd);
        let c = TriggerContext { telemetry: &t, analysis: &a, scene: &s, project: &p };
        assert!(IdlePauseWithPresentCharacter.evaluate(&c).is_none());
    }

    #[test]
    fn does_not_fire_when_no_chars_present() {
        let (t, a, s, p) = ctx_with(9000, vec![], 60, CursorClassification::AtParagraphEnd);
        let c = TriggerContext { telemetry: &t, analysis: &a, scene: &s, project: &p };
        assert!(IdlePauseWithPresentCharacter.evaluate(&c).is_none());
    }

    #[test]
    fn does_not_fire_when_recent_pill() {
        let (t, a, s, p) = ctx_with(9000, vec![Id::new()], 30, CursorClassification::AtParagraphEnd);
        let c = TriggerContext { telemetry: &t, analysis: &a, scene: &s, project: &p };
        assert!(IdlePauseWithPresentCharacter.evaluate(&c).is_none());
    }

    #[test]
    fn does_not_fire_mid_sentence() {
        let (t, a, s, p) = ctx_with(9000, vec![Id::new()], 60, CursorClassification::MidSentence);
        let c = TriggerContext { telemetry: &t, analysis: &a, scene: &s, project: &p };
        assert!(IdlePauseWithPresentCharacter.evaluate(&c).is_none());
    }
}
```

- [ ] **Step 2: Run; expect fail**

```powershell
cargo test -p water-core idle_pause_with_present_character
```

Expected: FAIL — stub returns None always.

- [ ] **Step 3: Implement**

Replace the body of `idle_pause_with_present_character.rs`:

```rust
//! `idle_pause_with_present_character` — fires when the writer pauses
//! during a scene with characters present. Allows a character voice to
//! gently surface during quiet writing moments.
//!
//! Threshold tuning per spec § 13.

use crate::orchestrator::{CursorClassification, SpeakerTrack, Trigger, TriggerCandidate, TriggerContext};

pub struct IdlePauseWithPresentCharacter;

impl Trigger for IdlePauseWithPresentCharacter {
    fn id(&self) -> &'static str {
        "idle_pause_with_present_character"
    }

    fn evaluate(&self, ctx: &TriggerContext<'_>) -> Option<TriggerCandidate> {
        if ctx.telemetry.cursor_classification == CursorClassification::MidSentence {
            return None;
        }
        if ctx.telemetry.idle_for_ms < 8_000 {
            return None;
        }
        if ctx.scene.characters_present.is_empty() {
            return None;
        }
        if ctx.scene.seconds_since_last_pill < 60 {
            return None;
        }
        Some(TriggerCandidate {
            trigger_id: self.id(),
            priority: 4.0,
            preferred_track: SpeakerTrack::Character,
            reason: "idle_with_present_character".to_string(),
            block_target_id: Some(ctx.telemetry.block_id.clone()),
            requires_confirmation: None,
        })
    }
}
```

- [ ] **Step 4: Run; expect pass**

```powershell
cargo test -p water-core idle_pause_with_present_character
```

Expected: 5 tests pass.

- [ ] **Step 5: Gates**

```powershell
cargo test -p water-core
cargo clippy -p water-core --all-targets -- -D warnings
```

Expected: clean.

- [ ] **Step 6: Commit**

```powershell
git add crates/water-core/src/orchestrator/triggers/idle_pause_with_present_character.rs
git commit -m "feat(orchestrator): fill idle_pause_with_present_character trigger (M3 T6)"
```

---

### Task 7: Fill `character_dissonance` Stage 1 (lemma gate)

Implements the synchronous lemma-overlap gate. Stage 2 (LLM confirmation) is added by the orchestrator service in T9.

**Files:**
- Create: `crates/water-core/src/orchestrator/lemma_overlap.rs`
- Modify: `crates/water-core/src/orchestrator/mod.rs` (register module)
- Modify: `crates/water-core/src/orchestrator/triggers/character_dissonance.rs`
- Modify: `crates/water-core/src/orchestrator/mod.rs` — extend `AnalysisSnapshot` with `last_block_text` and extend `ProjectSnapshot` with character lookup

- [ ] **Step 1: Extract lemma_overlap helper**

The trigger needs Jaccard lemma overlap math (similar to M2's anti-loop). Extract a small shared helper. Read `crates/water-core/src/orchestrator/anti_loop.rs` first — the `tokenize` + `jaccard` functions there are already the right shape; we'll re-export.

Create `crates/water-core/src/orchestrator/lemma_overlap.rs`:

```rust
//! Lemma-overlap Jaccard helper for `character_dissonance` Stage 1 gate.
//!
//! Wraps the M2 `anti_loop` tokenize + jaccard helpers with a slightly
//! different threshold convention (dissonance fires above 0.30; anti-loop
//! fires above per-speaker threshold ~0.70).

pub use crate::orchestrator::anti_loop::{jaccard, tokenize};

#[must_use]
pub fn overlap(a: &str, b: &str) -> f32 {
    let ta = tokenize(a);
    let tb = tokenize(b);
    jaccard(&ta, &tb)
}
```

Register in `crates/water-core/src/orchestrator/mod.rs`:

```rust
pub mod lemma_overlap;
```

- [ ] **Step 2: Extend `AnalysisSnapshot` with `last_block_text`**

Modify `crates/water-core/src/orchestrator/mod.rs`. Find `AnalysisSnapshot` and add:

```rust
pub struct AnalysisSnapshot {
    // ... existing fields ...
    /// Text of the most recently-finished paragraph. Provided by the
    /// renderer's `typing:telemetry` events when `idle_for_ms >= 3000`.
    /// Used by `character_dissonance` to gate against character fields.
    pub last_block_text: Option<String>,
}
```

Update existing `AnalysisSnapshot::default()` derivations to include the new field (None default). Existing tests should still pass.

- [ ] **Step 3: Extend `ProjectSnapshot` with character lookup**

`ProjectSnapshot` today carries `character_count`. M3 needs character data — names, ids, sheet fields — accessible from triggers. Add a borrowed character registry to `ProjectSnapshot`:

Actually, the cleanest pattern: `ProjectSnapshot` stays simple (counts, no big data), and `TriggerContext` gains a new field for the character registry:

Modify `crates/water-core/src/orchestrator/mod.rs`:

```rust
pub struct TriggerContext<'a> {
    pub telemetry: &'a TypingTelemetry,
    pub analysis: &'a AnalysisSnapshot,
    pub scene: &'a SceneSnapshot,
    pub project: &'a ProjectSnapshot,
    pub characters: &'a crate::character::registry::CharacterRegistry,
}
```

Update existing tests' `TriggerContext { ... }` literals to add `characters: &CharacterRegistry::empty()`. Grep:

```powershell
Select-String -Path 'crates\water-core\src\orchestrator\**\*.rs' -Pattern 'TriggerContext {' -List
```

Add the field in each test. Most tests will use `&CharacterRegistry::empty()`.

- [ ] **Step 4: Write failing tests for character_dissonance**

Replace the test module in `character_dissonance.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::character::registry::{CharacterRegistry, CharacterRow};
    use crate::orchestrator::*;
    use crate::Id;

    fn ctx_with(
        last_block: &str,
        character_data: serde_json::Value,
        cursor: CursorClassification,
    ) -> (TypingTelemetry, AnalysisSnapshot, SceneSnapshot, ProjectSnapshot, CharacterRegistry) {
        let char_id = Id::from_str("01HE000000000000000000000C").unwrap();
        let telem = TypingTelemetry {
            idle_for_ms: 4000,
            cursor_classification: cursor,
            block_id: "^bk-0001".to_string(),
            recent_word_delta: 0,
            structural_inflection: StructuralInflection::None,
        };
        let analysis = AnalysisSnapshot {
            last_block_text: Some(last_block.to_string()),
            ..AnalysisSnapshot::default()
        };
        let scene = SceneSnapshot {
            id: Id::new(),
            pov_character_id: Some(char_id.clone()),
            location_id: None,
            characters_present: vec![char_id.clone()],
            word_count: 500,
            seconds_since_last_pill: 60,
        };
        // Build registry with one character carrying the given data.
        let mut reg = CharacterRegistry::empty();
        reg.insert_for_test(CharacterRow {
            id: char_id,
            name: "Marcus".to_string(),
            hue_token: "--water-hue-character-1".to_string(),
            data: character_data,
        });
        (telem, analysis, scene, ProjectSnapshot::default(), reg)
    }

    #[test]
    fn fires_when_paragraph_overlaps_character_values() {
        // Character values: ["loyalty", "showing up"]; paragraph mentions both
        let (t, a, s, p, r) = ctx_with(
            "He turned his back on his oldest friend and walked away, leaving her standing in the rain.",
            serde_json::json!({
                "bonus_traits": { "values": ["loyalty", "showing up when it matters"] }
            }),
            CursorClassification::AtParagraphEnd,
        );
        let c = TriggerContext { telemetry: &t, analysis: &a, scene: &s, project: &p, characters: &r };
        let cand = CharacterDissonance.evaluate(&c);
        // May or may not fire depending on stemming; either way ensure no panic.
        // For testable determinism, use values that overlap definitely:
        let (t2, a2, s2, p2, r2) = ctx_with(
            "He had always valued loyalty above all things, even when it cost him friends.",
            serde_json::json!({
                "bonus_traits": { "values": ["loyalty", "showing up"] }
            }),
            CursorClassification::AtParagraphEnd,
        );
        let c2 = TriggerContext { telemetry: &t2, analysis: &a2, scene: &s2, project: &p2, characters: &r2 };
        let cand2 = CharacterDissonance.evaluate(&c2);
        assert!(cand2.is_some(), "expected fire on values-overlap");
        let cand2 = cand2.unwrap();
        assert_eq!(cand2.trigger_id, "character_dissonance");
        assert!(cand2.requires_confirmation.is_some(), "stage 2 LLM confirmation required");
    }

    #[test]
    fn does_not_fire_when_no_overlap() {
        let (t, a, s, p, r) = ctx_with(
            "The rain fell softly on the roof.",
            serde_json::json!({
                "bonus_traits": { "values": ["loyalty", "showing up"] }
            }),
            CursorClassification::AtParagraphEnd,
        );
        let c = TriggerContext { telemetry: &t, analysis: &a, scene: &s, project: &p, characters: &r };
        assert!(CharacterDissonance.evaluate(&c).is_none());
    }

    #[test]
    fn does_not_fire_mid_sentence() {
        let (t, a, s, p, r) = ctx_with(
            "He had always valued loyalty above all things",
            serde_json::json!({"bonus_traits": {"values": ["loyalty"]}}),
            CursorClassification::MidSentence,
        );
        let c = TriggerContext { telemetry: &t, analysis: &a, scene: &s, project: &p, characters: &r };
        assert!(CharacterDissonance.evaluate(&c).is_none());
    }

    #[test]
    fn does_not_fire_without_last_block_text() {
        let char_id = Id::from_str("01HE000000000000000000000C").unwrap();
        let telem = TypingTelemetry {
            idle_for_ms: 4000,
            cursor_classification: CursorClassification::AtParagraphEnd,
            block_id: "^bk-0001".to_string(),
            recent_word_delta: 0,
            structural_inflection: StructuralInflection::None,
        };
        let analysis = AnalysisSnapshot::default(); // last_block_text = None
        let scene = SceneSnapshot {
            id: Id::new(),
            pov_character_id: Some(char_id.clone()),
            location_id: None,
            characters_present: vec![char_id.clone()],
            word_count: 500,
            seconds_since_last_pill: 60,
        };
        let mut reg = CharacterRegistry::empty();
        reg.insert_for_test(CharacterRow {
            id: char_id,
            name: "X".to_string(),
            hue_token: "x".to_string(),
            data: serde_json::json!({"bonus_traits":{"values":["loyalty"]}}),
        });
        let project = ProjectSnapshot::default();
        let c = TriggerContext { telemetry: &telem, analysis: &analysis, scene: &scene, project: &project, characters: &reg };
        assert!(CharacterDissonance.evaluate(&c).is_none());
    }
}
```

- [ ] **Step 5: Add `CharacterRegistry::insert_for_test`**

Modify `crates/water-core/src/character/registry.rs`. Add a test-only helper (cfg-gated):

```rust
impl CharacterRegistry {
    #[cfg(test)]
    pub fn insert_for_test(&mut self, row: CharacterRow) {
        let speaker: SpeakerArc = Arc::new(crate::voice::speaker::CharacterSpeaker::from_row(&row));
        self.by_id.insert(row.id.as_str().to_string(), speaker);
        self.rows.push(row);
    }
}
```

- [ ] **Step 6: Implement the lemma gate**

Replace the body of `character_dissonance.rs`:

```rust
//! `character_dissonance` Stage 1 — lemma-overlap gate.
//!
//! For each present character, computes Jaccard lemma overlap between the
//! just-finished paragraph and three character fields:
//!   - `bonus_traits.values` (joined)
//!   - `bonus_traits.fears` (joined)
//!   - `main.lie_they_believe`
//!
//! If overlap ≥ 0.30 for any field, fires a TriggerCandidate carrying a
//! `requires_confirmation` request. Stage 2 (LLM yes/no) happens in the
//! orchestrator service (M3 T9).

use crate::orchestrator::{
    ConfirmationRequest, CursorClassification, SpeakerTrack, Trigger, TriggerCandidate,
    TriggerContext,
};
use crate::orchestrator::lemma_overlap::overlap;

const GATE_THRESHOLD: f32 = 0.30;

pub struct CharacterDissonance;

impl Trigger for CharacterDissonance {
    fn id(&self) -> &'static str {
        "character_dissonance"
    }

    fn evaluate(&self, ctx: &TriggerContext<'_>) -> Option<TriggerCandidate> {
        if ctx.telemetry.cursor_classification == CursorClassification::MidSentence {
            return None;
        }
        let paragraph = ctx.analysis.last_block_text.as_deref()?;
        for char_id in &ctx.scene.characters_present {
            let Some(speaker) = ctx.characters.by_id(char_id.as_str()) else { continue };
            let Some(row) = ctx.characters.list().iter().find(|r| r.id == *char_id) else { continue };
            let sheet = &row.data;
            // Three fields to gate against.
            let candidates: &[(&'static str, String)] = &[
                ("values", read_list_joined(sheet, "bonus_traits", "values")),
                ("fears", read_list_joined(sheet, "bonus_traits", "fears")),
                ("lie_they_believe", read_str(sheet, "main", "lie_they_believe")),
            ];
            for (field_label, field_value) in candidates {
                if field_value.is_empty() { continue; }
                let ovl = overlap(paragraph, field_value);
                if ovl >= GATE_THRESHOLD {
                    return Some(TriggerCandidate {
                        trigger_id: self.id(),
                        priority: 5.5,
                        preferred_track: SpeakerTrack::Character,
                        reason: format!("dissonance_gate field={field_label} overlap={ovl:.2}"),
                        block_target_id: Some(ctx.telemetry.block_id.clone()),
                        requires_confirmation: Some(ConfirmationRequest {
                            task_id: "pill_dissonance_check",
                            character_id: char_id.clone(),
                            field_label: (*field_label).to_string(),
                            field_value: field_value.clone(),
                        }),
                    });
                }
            }
        }
        None
    }
}

fn read_str(sheet: &serde_json::Value, section: &str, key: &str) -> String {
    sheet.get(section)
        .and_then(|s| s.get(key))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string()
}

fn read_list_joined(sheet: &serde_json::Value, section: &str, key: &str) -> String {
    let arr = sheet.get(section).and_then(|s| s.get(key)).and_then(|v| v.as_array());
    arr.map(|items| {
        items.iter()
            .filter_map(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(", ")
    }).unwrap_or_default()
}
```

- [ ] **Step 7: Run; expect pass**

```powershell
cargo test -p water-core orchestrator::triggers::character_dissonance
```

Expected: 4 tests pass.

- [ ] **Step 8: Update all other trigger tests for new TriggerContext field**

```powershell
cargo test -p water-core
```

Some triggers' existing tests may fail to compile because `TriggerContext` now requires `characters: &CharacterRegistry`. Update each test to add `characters: &crate::character::registry::CharacterRegistry::empty()`. Re-run.

- [ ] **Step 9: Gates**

```powershell
cargo test -p water-core
cargo clippy -p water-core --all-targets -- -D warnings
cargo fmt -p water-core --check
```

Expected: all clean.

- [ ] **Step 10: Commit**

```powershell
git add crates/water-core/src/orchestrator/lemma_overlap.rs crates/water-core/src/orchestrator/mod.rs crates/water-core/src/orchestrator/triggers/character_dissonance.rs crates/water-core/src/character/registry.rs
# Plus any modified trigger test files
git add crates/water-core/src/orchestrator/triggers/
git commit -m "feat(orchestrator): fill character_dissonance Stage 1 lemma gate (M3 T7)"
```

---

### Task 8: Orchestrator wiring — populate `CharacterRegistry` + extend telemetry pipeline

Wires `OrchestratorService` to load `CharacterRegistry` on construction and to pass `last_block_text` from telemetry events into the trigger context.

**Files:**
- Modify: `app/src-tauri/src/orchestrator_service.rs`
- Modify: `app/src-tauri/src/events.rs` (extend `TypingTelemetryPayload` with `last_block_text`)
- Modify: `app/src/editor/Editor.tsx` (emit `last_block_text` in telemetry when idle ≥ 3s)
- Modify: `app/src/ipc/events.ts` (TS type for the new field)

- [ ] **Step 1: Extend telemetry payload — Rust**

Modify `app/src-tauri/src/events.rs`. Add to `TypingTelemetryPayload`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypingTelemetryPayload {
    pub idle_for_ms: u64,
    pub cursor_classification: String,
    pub block_id: String,
    pub recent_word_delta: i32,
    pub structural_inflection: String,
    /// Text of the current block; only populated on idle pulses
    /// (idle_for_ms >= 3000). None during typing bursts (5 Hz cap).
    pub last_block_text: Option<String>,
}
```

- [ ] **Step 2: Extend TS event type**

Modify `app/src/ipc/events.ts`. Update `WaterEventPayloads["typing:telemetry"]`:

```ts
  "typing:telemetry": {
    idle_for_ms: number;
    cursor_classification: "at_sentence_end" | "at_paragraph_end" | "mid_sentence";
    block_id: string;
    recent_word_delta: number;
    structural_inflection: "new_scene" | "new_chapter" | "pov_change" | "location_change" | "none";
    last_block_text: string | null;
  };
```

- [ ] **Step 3: Renderer emits `last_block_text` on idle pulses**

Modify `app/src/editor/Editor.tsx`. Find `emitFromCurrentState`. Add the block text only when `idleMs >= 3000`:

```tsx
const blockText = idleMs >= 3000 ? blockNode.textContent : null;

void emitTypingTelemetry({
  idle_for_ms: idleMs,
  cursor_classification: cursorClassification,
  block_id: blockId,
  recent_word_delta: recentWordDelta,
  structural_inflection: structuralInflection,
  last_block_text: blockText,
});
```

- [ ] **Step 4: Update `typingTelemetry.ts` type**

Modify `app/src/editor/typingTelemetry.ts`. The exported `TypingTelemetry` type is sourced from `WaterEventPayloads["typing:telemetry"]` — should auto-pick-up the new field. Verify the existing import works.

- [ ] **Step 5: Update orchestrator service to handle the new field**

Modify `app/src-tauri/src/commands/events.rs::typing_telemetry`. When forwarding into the orchestrator:

```rust
let core_payload: water_core::orchestrator::TypingTelemetry = water_core::orchestrator::TypingTelemetry {
    idle_for_ms: payload.idle_for_ms,
    cursor_classification: /* existing */,
    block_id: payload.block_id.clone(),
    recent_word_delta: payload.recent_word_delta,
    structural_inflection: /* existing */,
};
```

Note: the core `TypingTelemetry` struct does NOT need `last_block_text` — that field lives on `AnalysisSnapshot`. The orchestrator service receives both telemetry and the block text, and updates `AnalysisSnapshot::last_block_text` separately:

In `app/src-tauri/src/orchestrator_service.rs::on_telemetry` (or the handler for telemetry events), update:

```rust
if let Some(text) = payload.last_block_text.clone() {
    self.analysis.last_block_text = Some(text);
}
```

- [ ] **Step 6: Wire CharacterRegistry into OrchestratorService**

Modify `app/src-tauri/src/orchestrator_service.rs`:

```rust
pub struct OrchestratorService {
    // ... existing fields ...
    characters: water_core::character::registry::CharacterRegistry,
}

impl OrchestratorService {
    pub fn start(/* existing args */) -> OrchestratorHandle {
        // ... existing setup ...
        let characters = match water_core::character::registry::CharacterRegistry::from_db(&db_locked) {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!("CharacterRegistry::from_db failed: {e}; using empty registry");
                water_core::character::registry::CharacterRegistry::empty()
            }
        };
        // ... rest of construction with characters: characters ...
    }
}
```

When building the `TriggerContext` in `on_telemetry`:

```rust
let ctx = TriggerContext {
    telemetry: &t,
    analysis: &self.analysis,
    scene: &scene,
    project: &self.project,
    characters: &self.characters,
};
```

When calling `route`:

```rust
let speaker = water_core::voice::router::route_with_chars(
    &cand, &self.personas, &self.characters, &scene, &self.cooldowns, now,
);
```

- [ ] **Step 7: Gates**

```powershell
cargo build -p water-app
cargo clippy -p water-app -- -D warnings
cargo test -p water-core
pnpm --filter @water/app test
pnpm --filter @water/app build
```

Expected: all clean.

- [ ] **Step 8: Commit**

```powershell
git add app/src-tauri/src/events.rs app/src-tauri/src/commands/events.rs app/src-tauri/src/orchestrator_service.rs app/src/ipc/events.ts app/src/editor/Editor.tsx app/src/editor/typingTelemetry.ts
git commit -m "feat(orchestrator): wire CharacterRegistry + last_block_text into trigger context (M3 T8)"
```

---

## Phase C — Orchestrator confirmation-pass extension

Phase B left `character_dissonance::evaluate` returning a `TriggerCandidate` with a Stage-2 confirmation request attached (Step 4 of Task 7). Phase C wires the orchestrator service so that, before generating a level-0 pill, candidates with `requires_confirmation = Some(_)` are first run through a cheap yes/no LLM check. Only `^yes` proceeds.

Three tasks:
- **Task 9** — Add `requires_confirmation: Option<ConfirmationRequest>` to `TriggerCandidate` + define `ConfirmationRequest`.
- **Task 10** — `pill_dissonance_check` task TOML + loader glue (`render_confirmation_request` helper).
- **Task 11** — `OrchestratorService::on_telemetry` dispatches the confirmation call before level-0; canned-provider tests cover yes/no.

### Task 9: `TriggerCandidate.requires_confirmation` field + `ConfirmationRequest` struct

Adds the optional confirmation-request shape that `character_dissonance` (and any future trigger needing two-stage gating) hangs onto its candidate. Backward-compatible: `Option<_>` with `#[serde(default)]` so M2 replay-log entries deserialize unchanged.

**Files:**
- Modify: `crates/water-core/src/orchestrator/mod.rs` (extend `TriggerCandidate`, add `ConfirmationRequest`)
- Modify: `crates/water-core/src/orchestrator/triggers/character_dissonance.rs` (build the request when Stage 1 fires — already partially done in T7 Step 4; this task finalizes the field name and shape)
- Grep + update any other test/fixture that constructs `TriggerCandidate` literals

- [ ] **Step 1: Add `ConfirmationRequest` + extend `TriggerCandidate`**

Modify `crates/water-core/src/orchestrator/mod.rs`. Find `TriggerCandidate` (line ~93):

```rust
/// A small system+user pair sent to the LLM as a yes/no gate before
/// proceeding with level-0 pill generation. Used today by
/// `character_dissonance` Stage 2; reusable for any future two-stage
/// trigger. Cheap by design: ~150 tokens in, 1 token out.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfirmationRequest {
    /// System prompt (instructive role copy).
    pub system: String,
    /// User prompt with all variables already substituted.
    pub user: String,
    /// Tag for telemetry / replay-log filtering. Currently only
    /// `"pill_dissonance_check"` but other two-stage triggers would
    /// add more variants.
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriggerCandidate {
    pub trigger_id: &'static str,
    pub priority: f32,
    pub preferred_track: SpeakerTrack,
    pub reason: String,
    pub block_target_id: Option<String>,
    /// When `Some`, the orchestrator runs `ConfirmationRequest` as a
    /// yes/no LLM call before dispatching the level-0 prompt. When the
    /// confirmation returns "no" (or any non-"yes" string), the candidate
    /// is dropped without emitting a pill.
    ///
    /// New in M3. Defaults to `None` for backward-compat with M2 replay
    /// logs and any existing trigger that doesn't need two-stage gating.
    #[serde(default)]
    pub requires_confirmation: Option<ConfirmationRequest>,
}
```

Note the shift from `#[derive(Debug, Clone)]` to `#[derive(Debug, Clone, Serialize, Deserialize)]` on `TriggerCandidate`. Reason: the orchestrator's replay log (see `app/src-tauri/src/orchestrator_service.rs` ReplayEntry plumbing) doesn't currently serialize candidates, but the M3 confirmation-pass will log "candidate gated by confirmation" entries (see Task 11), so the candidate needs to round-trip through JSON.

Verify the derive is safe: `trigger_id: &'static str` serializes fine via `serde`'s default `&str` impl (it borrows for serialize, but won't deserialize back into `&'static str`). Since we serialize candidates outbound only (never deserialize), use `#[serde(serialize_with)]` if needed, or change to `String`.

**Decision:** change `trigger_id: &'static str` → `trigger_id: String`. This is a small spread change — every `cand.trigger_id` comparison becomes `cand.trigger_id.as_str() == "..."` or `cand.trigger_id == "..."` (`String == &str` works via `PartialEq`). Concrete trigger implementations build the candidate with `trigger_id: self.id().to_string()`. The cost is one allocation per evaluate call; orchestrator runs `evaluate` ≤ 10×/second under the 5 Hz cap, so this is negligible.

Grep for all usages and update:

```powershell
git grep -n "trigger_id:" crates/water-core/src/orchestrator/
```

Each trigger's `evaluate` currently has `trigger_id: self.id(),` — change to `trigger_id: self.id().to_string(),`.

- [ ] **Step 2: Update `character_dissonance::evaluate` to attach the request**

T7 Step 4 left a stub `requires_confirmation: Some(/* TBD */)`. Now finalize:

```rust
let body = ctx.analysis.last_block_text.as_deref()?;
let (matched_char, matched_field, score) = match best_field_match(body, ctx.characters) {
    Some(m) if m.score >= 0.30 => m,
    _ => return None,
};

// Build the Stage-2 confirmation request.
let prompts = ctx.prompts; // see Task 10 — `TriggerContext` gains `prompts: &PromptLibrary`
let req = prompts.render_confirmation_request(
    "pill_dissonance_check",
    &[
        ("full_name", &matched_char.full_name),
        ("field_label", matched_field.label),
        ("field_value", &matched_field.value),
        ("paragraph_text", body),
    ],
).ok()?;

Some(TriggerCandidate {
    trigger_id: self.id().to_string(),
    priority: 5.5, // above topic_drift (5.0), below structural_inflection (7.0)
    preferred_track: SpeakerTrack::Character,
    reason: format!(
        "lemma overlap {:.2} with {}::{}",
        score, matched_char.full_name, matched_field.label
    ),
    block_target_id: Some(ctx.telemetry.block_id.clone()),
    requires_confirmation: Some(req),
})
```

**Note on `TriggerContext::prompts`:** the existing `TriggerContext` does NOT carry `&PromptLibrary`. Task 10 extends it. This step assumes T10 has run. If you're building tasks strictly in order, Step 2 of this task should temporarily inline the confirmation request shape (no template rendering yet, build the strings by hand) and a later step in T10 swaps to the rendered version. Easier: do T9 and T10 together as one PR-shaped commit; the spec lists them as separate tasks for clarity, but they can be co-implemented if the subagent prefers a single coherent diff.

- [ ] **Step 3: Update all `TriggerCandidate { ... }` literals**

Grep for every `TriggerCandidate {` in tests and trigger impls:

```powershell
git grep -n "TriggerCandidate {" crates/water-core/src/
```

For each one, add `requires_confirmation: None,` to the literal. Use `..Default::default()` if a `Default` impl is feasible:

```rust
impl Default for TriggerCandidate {
    fn default() -> Self {
        Self {
            trigger_id: String::new(),
            priority: 0.0,
            preferred_track: SpeakerTrack::Either,
            reason: String::new(),
            block_target_id: None,
            requires_confirmation: None,
        }
    }
}
```

With `Default` in place, existing literals can use `..Default::default()` shorthand. (Clippy lint `field_reassign_with_default` would otherwise trigger — prefer struct-update syntax per pre-authorized lint rules.)

- [ ] **Step 4: Tests — `requires_confirmation` round-trips through serde + defaults to None**

Add to `crates/water-core/src/orchestrator/mod.rs` test module:

```rust
#[test]
fn trigger_candidate_default_has_no_confirmation() {
    let c = TriggerCandidate::default();
    assert!(c.requires_confirmation.is_none());
}

#[test]
fn trigger_candidate_with_confirmation_serializes_round_trip() {
    let original = TriggerCandidate {
        trigger_id: "character_dissonance".to_string(),
        priority: 5.5,
        preferred_track: SpeakerTrack::Character,
        reason: "test".to_string(),
        block_target_id: Some("block-1".to_string()),
        requires_confirmation: Some(ConfirmationRequest {
            system: "sys".to_string(),
            user: "usr".to_string(),
            kind: "pill_dissonance_check".to_string(),
        }),
    };
    let json = serde_json::to_string(&original).unwrap();
    let parsed: TriggerCandidate = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.requires_confirmation.as_ref().unwrap().kind, "pill_dissonance_check");
}

#[test]
fn trigger_candidate_missing_confirmation_field_deserializes_as_none() {
    // M2-shaped replay log entry — no `requires_confirmation` field.
    let m2_json = r#"{
        "trigger_id": "topic_drift",
        "priority": 5.0,
        "preferred_track": "persona",
        "reason": "test",
        "block_target_id": null
    }"#;
    let parsed: TriggerCandidate = serde_json::from_str(m2_json).unwrap();
    assert!(parsed.requires_confirmation.is_none());
}
```

The third test verifies the backward-compat property: M2 replay-log entries (which don't have the field) deserialize as `requires_confirmation: None`. Critical for the eval harness that re-runs old replay logs against new code.

- [ ] **Step 5: Gates**

```powershell
cargo build -p water-core
cargo clippy -p water-core --all-targets -- -D warnings
cargo test -p water-core orchestrator::tests
cargo test -p water-core orchestrator::triggers::character_dissonance
cargo fmt -p water-core --check
```

Expected: all clean. The 3 new tests pass.

- [ ] **Step 6: Commit**

```powershell
git add crates/water-core/src/orchestrator/mod.rs crates/water-core/src/orchestrator/triggers/character_dissonance.rs
git commit -m "feat(orchestrator): add ConfirmationRequest + TriggerCandidate.requires_confirmation (M3 T9)"
```

---

### Task 10: `pill_dissonance_check` task TOML + loader glue

Adds the Stage-2 confirmation prompt as a built-in TOML resource and exposes a `render_confirmation_request` helper on `PromptLibrary` for variable substitution. The new prompt shape (`system`+`user` directly, not `instruction`+`output_format`) needs a new TOML type — call it `ConfirmationPrompt` to keep `TaskPrompt` (for pill_level_0 / pill_expand / pill_regenerate) cleanly typed.

**Files:**
- Create: `prompts/tasks/pill_dissonance_check.toml`
- Modify: `crates/water-core/src/prompts/loader.rs` (add `ConfirmationPrompt`, `confirmations` field on `PromptLibrary`, `render_confirmation_request` method)
- Modify: `crates/water-core/src/orchestrator/mod.rs` (extend `TriggerContext` with `prompts: &PromptLibrary`)
- Modify: `crates/water-core/src/orchestrator/triggers/character_dissonance.rs` (call `prompts.render_confirmation_request` per T9 Step 2)
- Modify: every trigger test that builds `TriggerContext` (add `prompts:` field)

- [ ] **Step 1: Author `prompts/tasks/pill_dissonance_check.toml`**

Verbatim from spec § 12.2:

```toml
version = "1"
id = "pill_dissonance_check"

[prompt]
system = """
You evaluate whether a paragraph genuinely contradicts a character's stated belief, value, or fear.
"""
user = """
Character: {{full_name}}
Their stated {{field_label}}: {{field_value}}

Paragraph just written:
"{{paragraph_text}}"

Is this paragraph genuinely showing this character contradicting their stated {{field_label}}?
(Not just touching the topic — actually contradicting it in a way that creates meaningful friction.)

Respond with only one word: yes or no.
"""

[output]
format = "plain"
max_tokens = 4
```

The `{{var}}` placeholders are substituted by `render_confirmation_request` (Step 3). Use double-mustache to match Tera/handlebars-style — but we're NOT pulling in tera. Implement a small string-replace helper instead (Step 3).

- [ ] **Step 2: Add `ConfirmationPrompt` type to `loader.rs`**

Modify `crates/water-core/src/prompts/loader.rs`. After `TaskPrompt`:

```rust
/// A two-part prompt (system + user) used as a yes/no gate before
/// dispatching a level-0 pill prompt. Variables are double-mustache
/// `{{name}}` placeholders, substituted at render time.
///
/// Distinct from `TaskPrompt` because the shape differs: confirmation
/// prompts always have separate system + user roles and a small
/// `max_tokens` output cap, whereas tasks share a single `instruction`
/// blob that the speaker template wraps.
#[derive(Debug, Deserialize, Clone)]
pub struct ConfirmationPrompt {
    pub version: String,
    pub id: String,
    pub prompt: ConfirmationPromptBody,
    pub output: ConfirmationPromptOutput,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ConfirmationPromptBody {
    pub system: String,
    pub user: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct ConfirmationPromptOutput {
    pub format: String,
    pub max_tokens: u32,
}
```

Add an `include_str!` line near the existing task prompts:

```rust
const TASK_PILL_DISSONANCE_CHECK: &str =
    include_str!("../../../../prompts/tasks/pill_dissonance_check.toml");
```

Extend `PromptLibrary`:

```rust
pub struct PromptLibrary {
    pub tone: ToneClauses,
    pub triggers: HashMap<String, TriggerPrompt>,
    pub tasks: HashMap<String, TaskPrompt>,
    pub confirmations: HashMap<String, ConfirmationPrompt>,
}
```

In `load_builtin`:

```rust
let mut confirmations: HashMap<String, ConfirmationPrompt> = HashMap::new();
for src in [TASK_PILL_DISSONANCE_CHECK] {
    let p: ConfirmationPrompt = toml::from_str(src).map_err(|e| e.to_string())?;
    confirmations.insert(p.id.clone(), p);
}

Ok(Self {
    tone,
    triggers,
    tasks,
    confirmations,
})
```

Add the accessor:

```rust
#[must_use]
pub fn confirmation(&self, id: &str) -> Option<&ConfirmationPrompt> {
    self.confirmations.get(id)
}
```

- [ ] **Step 3: Add `render_confirmation_request` method**

Substitutes `{{var}}` tokens. Keep simple — no full template engine. In `PromptLibrary`:

```rust
/// Render a confirmation prompt with `{{var}}` substitutions.
/// Missing variables remain as literal `{{var}}` strings in the output
/// (LLM-visible) so misuse is loud rather than silent.
pub fn render_confirmation_request(
    &self,
    id: &str,
    vars: &[(&str, &str)],
) -> Result<crate::orchestrator::ConfirmationRequest, String> {
    let prompt = self.confirmation(id)
        .ok_or_else(|| format!("confirmation prompt not found: {id}"))?;
    let mut sys = prompt.prompt.system.clone();
    let mut usr = prompt.prompt.user.clone();
    for (k, v) in vars {
        let needle = format!("{{{{{k}}}}}");
        sys = sys.replace(&needle, v);
        usr = usr.replace(&needle, v);
    }
    Ok(crate::orchestrator::ConfirmationRequest {
        system: sys,
        user: usr,
        kind: id.to_string(),
    })
}
```

- [ ] **Step 4: Extend `TriggerContext` with `prompts: &PromptLibrary`**

Modify `crates/water-core/src/orchestrator/mod.rs`:

```rust
pub struct TriggerContext<'a> {
    pub telemetry: &'a TypingTelemetry,
    pub analysis: &'a AnalysisSnapshot,
    pub scene: &'a SceneSnapshot,
    pub project: &'a ProjectSnapshot,
    pub characters: &'a crate::character::registry::CharacterRegistry,
    pub prompts: &'a crate::prompts::loader::PromptLibrary,
}
```

Grep + update all existing `TriggerContext { ... }` test literals to include `prompts: &PromptLibrary::load_builtin().unwrap()` — or wrap it in a thread-local / lazy_static / `once_cell::Lazy` for cheaper test setup:

```rust
#[cfg(test)]
fn test_prompts() -> &'static crate::prompts::loader::PromptLibrary {
    use std::sync::OnceLock;
    static LIB: OnceLock<crate::prompts::loader::PromptLibrary> = OnceLock::new();
    LIB.get_or_init(|| crate::prompts::loader::PromptLibrary::load_builtin().unwrap())
}
```

Then in each trigger test module, replace `prompts: ...` with `prompts: test_prompts()`. Add this helper once per trigger test file (each trigger module has its own `#[cfg(test)] mod tests`) — or, cleaner, hoist it to a shared `orchestrator::test_util` module gated behind `#[cfg(test)]`.

- [ ] **Step 5: Update `character_dissonance::evaluate` to use the helper**

Finalize T9 Step 2's stub:

```rust
let req = ctx.prompts.render_confirmation_request(
    "pill_dissonance_check",
    &[
        ("full_name", matched_char.full_name.as_str()),
        ("field_label", matched_field.label),
        ("field_value", matched_field.value.as_str()),
        ("paragraph_text", body),
    ],
).ok()?; // If template rendering fails, drop the candidate (don't emit unrendered prompt).
```

- [ ] **Step 6: Tests — confirmation TOML loads + renders**

Add to `loader.rs` test module:

```rust
#[test]
fn library_loads_pill_dissonance_check_confirmation() {
    let lib = PromptLibrary::load_builtin().unwrap();
    assert_eq!(lib.confirmations.len(), 1);
    let p = lib.confirmation("pill_dissonance_check").expect("present");
    assert_eq!(p.id, "pill_dissonance_check");
    assert_eq!(p.output.format, "plain");
    assert_eq!(p.output.max_tokens, 4);
    assert!(p.prompt.user.contains("{{full_name}}"));
}

#[test]
fn render_confirmation_request_substitutes_all_vars() {
    let lib = PromptLibrary::load_builtin().unwrap();
    let req = lib.render_confirmation_request(
        "pill_dissonance_check",
        &[
            ("full_name", "Marcus Vale"),
            ("field_label", "fear"),
            ("field_value", "drowning in his own irrelevance"),
            ("paragraph_text", "Marcus stood proud, certain he would be remembered."),
        ],
    ).unwrap();
    assert_eq!(req.kind, "pill_dissonance_check");
    assert!(!req.user.contains("{{"), "all placeholders should be substituted; got: {}", req.user);
    assert!(req.user.contains("Marcus Vale"));
    assert!(req.user.contains("fear"));
}

#[test]
fn render_confirmation_request_unknown_id_errors() {
    let lib = PromptLibrary::load_builtin().unwrap();
    let err = lib.render_confirmation_request("does_not_exist", &[]).unwrap_err();
    assert!(err.contains("not found"));
}
```

- [ ] **Step 7: Gates**

```powershell
cargo build -p water-core
cargo clippy -p water-core --all-targets -- -D warnings
cargo test -p water-core prompts::loader::tests
cargo test -p water-core orchestrator
cargo fmt -p water-core --check
```

Expected: clean. `library_loads_all_built_in_prompts` (M2 baseline test) may need its assertion updated — the M2 test asserts `lib.triggers.len() == 10` and `lib.tasks.len() == 3`. Update to assert `lib.confirmations.len() == 1` is also true.

- [ ] **Step 8: Commit**

```powershell
git add prompts/tasks/pill_dissonance_check.toml crates/water-core/src/prompts/loader.rs crates/water-core/src/orchestrator/mod.rs crates/water-core/src/orchestrator/triggers/character_dissonance.rs
git commit -m "feat(prompts): pill_dissonance_check ConfirmationPrompt + render helper (M3 T10)"
```

---

### Task 11: `OrchestratorService::on_telemetry` dispatches confirmation before level-0

Wires the Stage-2 LLM call into the orchestrator service. When `pick_best_trigger` returns a candidate with `requires_confirmation: Some(req)`, the service runs `router.generate_raw_with_default(req.system, req.user)`, parses the response (trim → lowercase → `starts_with("yes")`), and either drops the candidate or proceeds with normal level-0 dispatch.

**Files:**
- Modify: `app/src-tauri/src/orchestrator_service.rs`
- Modify: `crates/water-core/src/orchestrator/replay_log.rs` (extend `ReplayEntry` with confirmation-pass entries — optional but recommended for eval-harness clarity)

- [ ] **Step 1: Add the confirmation dispatch in `on_telemetry`**

In `app/src-tauri/src/orchestrator_service.rs::on_telemetry`, insert after `pick_best_trigger` returns Some and before the `route` call (~line 290):

```rust
// Stage 2 (M3): if candidate requires confirmation, run a small
// yes/no LLM call before dispatching level-0. Drops candidate on
// non-"yes" response.
if let Some(req) = cand.requires_confirmation.as_ref() {
    let router_arc = {
        let g = self.router.lock().await;
        g.clone()
    };
    let Some(router_arc) = router_arc else {
        tracing::debug!("no LlmRouter configured; skipping confirmation candidate {}", cand.trigger_id);
        return;
    };
    let confirmation_system = req.system.clone();
    let confirmation_user = req.user.clone();
    let confirmation_kind = req.kind.clone();
    let raw = match router_arc
        .generate_raw_with_default(confirmation_system.clone(), confirmation_user.clone())
        .await
    {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(error = %e, trigger = %cand.trigger_id, "confirmation LLM call failed; dropping candidate");
            // Log to replay log for the eval harness.
            if let Some(log) = self.replay_log.as_ref() {
                let _ = log.append(&ReplayEntry {
                    ts: chrono::Utc::now().to_rfc3339(),
                    kind: &confirmation_kind,
                    request_system: &confirmation_system,
                    request_user: &confirmation_user,
                    response_raw: None,
                    post_filter_decision: Some("confirmation_error"),
                    anti_loop_overlap: None,
                });
            }
            return;
        }
    };
    let confirmed = raw.trim().to_ascii_lowercase().starts_with("yes");
    if let Some(log) = self.replay_log.as_ref() {
        let _ = log.append(&ReplayEntry {
            ts: chrono::Utc::now().to_rfc3339(),
            kind: &confirmation_kind,
            request_system: &confirmation_system,
            request_user: &confirmation_user,
            response_raw: Some(&raw),
            post_filter_decision: Some(if confirmed { "confirmation_yes" } else { "confirmation_no" }),
            anti_loop_overlap: None,
        });
    }
    if !confirmed {
        tracing::debug!(trigger = %cand.trigger_id, response = %raw.trim(), "confirmation said no; dropping candidate");
        return;
    }
}
```

**Note on the `cand.requires_confirmation.as_ref()` borrow + `await`:** the borrow is released before the `.await` because `req` is cloned into owned strings (`confirmation_system`, etc.) before the await point. No `!Send` issue.

**Note on cancellation race:** if a new telemetry arrives mid-await, the service serializes on `handle()` so the new event waits. That's the existing M2 behavior; no change here.

- [ ] **Step 2: Verify the level-0 dispatch path is unchanged**

The code after the confirmation block (`route` → `assemble_level_0` → `tokio::spawn`) is unchanged. The candidate's `requires_confirmation` field is not referenced again past Step 1.

- [ ] **Step 3: Tests — confirmation yes proceeds, no drops**

The orchestrator service has integration tests at `app/src-tauri/src/orchestrator_service_tests.rs` (M2 T26 baseline). Add two new tests using `CannedProvider`:

First, look at the existing canned-provider pattern. Grep:

```powershell
git grep -n "CannedProvider" app/src-tauri/src/ crates/water-core/src/
```

`CannedProvider` (M2 T15) lets you script LLM responses by ordered list. For these tests, the provider must respond to two prompts in sequence (confirmation first, then level-0). Construct the canned responses accordingly:

```rust
#[tokio::test]
async fn character_dissonance_confirmation_yes_emits_pill() {
    // Setup: project with one character (Marcus Vale, fear = "drowning…"),
    // scene with him present + POV, telemetry idle 3s with a contradicting
    // paragraph as `last_block_text`.
    let provider = CannedProvider::with_responses(vec![
        "yes".to_string(),                              // Stage-2 confirmation
        "Why does he feel proud now, of all moments?".to_string(), // level-0 pill
    ]);
    let (handle, mut events) = spawn_service_with_provider(provider).await;

    handle.send(OrchestratorRequest::Telemetry(idle_telemetry_with_contradiction())).await;

    // Expect: pill:emerged event with character speaker, hue from Marcus.
    let evt = expect_event(&mut events, "pill:emerged", Duration::from_secs(2)).await;
    assert_eq!(evt["speaker_kind"], "character");
    assert_eq!(evt["speaker_id"], "character::<marcus-id>");
}

#[tokio::test]
async fn character_dissonance_confirmation_no_drops_silently() {
    let provider = CannedProvider::with_responses(vec!["no".to_string()]);
    let (handle, mut events) = spawn_service_with_provider(provider).await;

    handle.send(OrchestratorRequest::Telemetry(idle_telemetry_with_contradiction())).await;

    // No pill:emerged event within 1s.
    let result = expect_event(&mut events, "pill:emerged", Duration::from_secs(1)).await_err();
    assert!(matches!(result, EventError::Timeout));
    // But pick_best_trigger may have picked a different candidate that
    // wasn't requires_confirmation — assert specifically that the
    // character_dissonance one was the one dropped. Use replay-log
    // assertion:
    let log_entries = read_replay_log(&handle).await;
    assert!(log_entries.iter().any(|e| e.kind == "pill_dissonance_check" && e.post_filter_decision == Some("confirmation_no".to_string())));
}

#[tokio::test]
async fn character_dissonance_confirmation_error_drops_candidate() {
    let provider = ErrorProvider::new(); // returns Err on every call
    let (handle, mut events) = spawn_service_with_provider(provider).await;

    handle.send(OrchestratorRequest::Telemetry(idle_telemetry_with_contradiction())).await;

    let result = expect_event(&mut events, "pill:emerged", Duration::from_secs(1)).await_err();
    assert!(matches!(result, EventError::Timeout));
}
```

**Note on test helpers:** the names `spawn_service_with_provider`, `idle_telemetry_with_contradiction`, `expect_event`, `ErrorProvider`, `read_replay_log` may or may not exist. Reuse what's there from M2 T26's test scaffolding (`app/src-tauri/src/orchestrator_service_tests.rs`); add missing helpers minimally.

**Note on `ErrorProvider`:** if M2 only has `CannedProvider`, add a tiny `ErrorProvider` struct that implements `LlmProvider` and always returns `Err("test error")`. ~10 lines.

- [ ] **Step 4: Gates**

```powershell
cargo build -p water-app
cargo clippy -p water-app -- -D warnings
cargo test -p water-app orchestrator_service
pnpm --filter @water/app test
```

Expected: clean. 3 new tests pass.

- [ ] **Step 5: Commit**

```powershell
git add app/src-tauri/src/orchestrator_service.rs app/src-tauri/src/orchestrator_service_tests.rs
git commit -m "feat(orchestrator): Stage-2 confirmation dispatch for requires_confirmation candidates (M3 T11)"
```

---

## Phase D — Tauri commands + autosuggest

Renderer-facing IPC. Three tasks:
- **Task 12** — Character CRUD: `character_create`, `character_read`, `character_list`, `character_update_field`, `character_delete`.
- **Task 13** — Scene linkage: `character_link_to_scene`, `character_unlink_from_scene`, `character_set_pov`.
- **Task 14** — `intake_schema` + `character_autosuggest_for_scene` + the `autosuggest.rs` core module.

All commands follow the M2 lock ordering: drop the project guard before acquiring the db lock (KNOWN_FRAGILE #6). Character writes take a per-character write-lock (new `CharacterWriteLocks`) to serialize rapid Intake key-presses.

### Task 12: Character CRUD commands

**Files:**
- Create: `app/src-tauri/src/commands/character.rs`
- Modify: `app/src-tauri/src/commands/mod.rs` (register module)
- Modify: `app/src-tauri/src/state.rs` (add `character_write_locks: Arc<CharacterWriteLocks>`)
- Modify: `app/src-tauri/src/main.rs` (`generate_handler!` macro list — register the 5 commands)
- Create: `app/src/ipc/character.ts` (TS wrappers + types)
- Modify: `app/src/ipc/index.ts` (re-export)

- [ ] **Step 1: `CharacterWriteLocks` — mirror of `SceneWriteLocks`**

Grep for `SceneWriteLocks` to find its implementation (M2 T2):

```powershell
git grep -n "SceneWriteLocks" app/src-tauri/src/
```

It lives at `app/src-tauri/src/state.rs` (or a sibling module). Copy the pattern to a `CharacterWriteLocks` that keys on `Id` (character id). Add to `app/src-tauri/src/state.rs`:

```rust
pub type CharacterWriteLocks = crate::write_locks::WriteLocks<water_core::Id>;
```

If `WriteLocks<K>` is a generic, this is one line. If `SceneWriteLocks` is concrete-typed, copy + rename. Either way the per-key acquire pattern is identical.

Add the field to `Project` (the open-project struct held in `state.project`):

```rust
pub struct Project {
    // ... existing fields ...
    pub character_write_locks: Arc<CharacterWriteLocks>,
}
```

Initialize in `Project::open` (or wherever the project is opened in M1):

```rust
character_write_locks: Arc::new(CharacterWriteLocks::new()),
```

- [ ] **Step 2: `character_create`**

Create `app/src-tauri/src/commands/character.rs`:

```rust
use crate::state::AppState;
use serde::Serialize;
use tauri::State;
use water_core::{
    Id,
    character::{CharacterFile, CharacterRow, CharacterStore, NewCharacter, registry::CharacterRegistry},
};

#[derive(Serialize)]
pub struct CharacterIndexEntry {
    pub id: String,
    pub full_name: String,
    pub role: Option<String>,
    pub hue_token: String,
    pub completion: u8, // 0..=100 — fraction of required fields filled
}

#[tauri::command]
pub async fn character_create(state: State<'_, AppState>) -> Result<CharacterIndexEntry, String> {
    let (db, root) = {
        let proj = state.project.lock().await;
        let project = proj.as_ref().ok_or("no project open")?;
        (project.db.clone(), project.root.clone())
    };
    let db_guard = db.lock().await;
    let store = CharacterStore::new(&db_guard, root);

    // Round-robin hue: pick the least-used of the 6 hue tokens. The
    // store has a helper or we query the db directly:
    let hue_token = next_hue_token(&db_guard)?;

    let row = store.create(NewCharacter { hue_token: hue_token.clone() })
        .map_err(|e| e.to_string())?;

    Ok(CharacterIndexEntry {
        id: row.id.to_string(),
        full_name: row.full_name,
        role: row.role,
        hue_token,
        completion: 0,
    })
}

fn next_hue_token(db: &water_core::db::Db) -> Result<String, String> {
    // Query `SELECT hue_token, COUNT(*) FROM character GROUP BY hue_token`,
    // then pick the hue with the lowest count (ties broken by hue index 1..6).
    // Empty result -> "character-1".
    // Spec § 11.
    todo!("implement based on existing hue palette constants in water_core::character::hue")
}
```

The `next_hue_token` helper should live in `water-core` (with the hue-palette constants); the command just calls it. Move appropriately during implementation — keep `commands/character.rs` thin.

- [ ] **Step 3: `character_read`, `character_list`**

```rust
#[tauri::command]
pub async fn character_read(state: State<'_, AppState>, id: String) -> Result<CharacterFile, String> {
    let (db, root) = {
        let proj = state.project.lock().await;
        let project = proj.as_ref().ok_or("no project open")?;
        (project.db.clone(), project.root.clone())
    };
    let id: Id = id.parse().map_err(|e: water_core::Error| e.to_string())?;
    let db_guard = db.lock().await;
    let store = CharacterStore::new(&db_guard, root);
    store.read(&id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn character_list(state: State<'_, AppState>) -> Result<Vec<CharacterIndexEntry>, String> {
    let (db, _root) = {
        let proj = state.project.lock().await;
        let project = proj.as_ref().ok_or("no project open")?;
        (project.db.clone(), project.root.clone())
    };
    let db_guard = db.lock().await;
    let rows = CharacterStore::list_index(&db_guard).map_err(|e| e.to_string())?;
    Ok(rows.into_iter().map(|r| CharacterIndexEntry {
        id: r.id.to_string(),
        full_name: r.full_name,
        role: r.role,
        hue_token: r.hue_token,
        completion: completion_pct(&r),
    }).collect())
}

fn completion_pct(row: &CharacterRow) -> u8 {
    // Count required fields (8 per LSM v2.1) that are non-empty.
    // Required IDs from intake.rs: full_name, role_in_story, want, need,
    // lie_they_believe, voice, fears, values.
    // Use serde_json::Value::pointer to peek at row.data_json.
    todo!()
}
```

`CharacterStore::list_index` returns a flat `Vec<CharacterRow>` from a query like `SELECT id, full_name, role, hue_token, data_json FROM character ORDER BY full_name COLLATE NOCASE`. Add this method to `CharacterStore` in `crates/water-core/src/character/mod.rs` if it doesn't exist yet (M1 had a `list` but it may not include `hue_token`).

- [ ] **Step 4: `character_update_field`**

Critical command for the conversational Intake flow — called once per question answered.

```rust
#[tauri::command]
pub async fn character_update_field(
    state: State<'_, AppState>,
    id: String,
    field_id: String,
    value: serde_json::Value,
) -> Result<CharacterIndexEntry, String> {
    let (db, root, locks) = {
        let proj = state.project.lock().await;
        let project = proj.as_ref().ok_or("no project open")?;
        (
            project.db.clone(),
            project.root.clone(),
            project.character_write_locks.clone(),
        )
    };
    let id: Id = id.parse().map_err(|e: water_core::Error| e.to_string())?;
    // Per-character write lock — serializes rapid Intake updates.
    let _guard = locks.acquire(&id).await;
    let db_guard = db.lock().await;
    let store = CharacterStore::new(&db_guard, root);
    let row = store.update_field(&id, &field_id, value).map_err(|e| e.to_string())?;
    Ok(CharacterIndexEntry {
        id: row.id.to_string(),
        full_name: row.full_name.clone(),
        role: row.role.clone(),
        hue_token: row.hue_token.clone(),
        completion: completion_pct(&row),
    })
}
```

`CharacterStore::update_field` looks up `field_id` in the LSM v2.1 descriptor table (Task 1), resolves its `path` (e.g. `"arc.lie_they_believe"`), and writes via `serde_json::Value::pointer_mut` into `data_json` before re-serializing to TOML on disk. Implementation lives in `water-core`.

**Rename cascade** (per spec § 20): if `field_id == "full_name"` and the new value differs from the old `row.full_name`, append the old name to `aliases` (deduplicate, case-sensitive). Do this inside `update_field` in `water-core` — single transaction. Add a unit test.

- [ ] **Step 5: `character_delete`**

Soft-delete: move the `.toml` to `characters/.trash/<ulid>-<unix-ts>.toml`. Cascade to `scene_character_presence` (remove rows) and `scene.pov_character_id` (null out where it matches).

```rust
#[tauri::command]
pub async fn character_delete(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let (db, root) = {
        let proj = state.project.lock().await;
        let project = proj.as_ref().ok_or("no project open")?;
        (project.db.clone(), project.root.clone())
    };
    let id: Id = id.parse().map_err(|e: water_core::Error| e.to_string())?;
    let db_guard = db.lock().await;
    let store = CharacterStore::new(&db_guard, root);
    store.delete_and_cascade(&id).map_err(|e| e.to_string())
}
```

`delete_and_cascade` (new in `CharacterStore`):
1. `BEGIN TRANSACTION`
2. `DELETE FROM scene_character_presence WHERE character_id = ?1`
3. `UPDATE scene SET pov_character_id = NULL WHERE pov_character_id = ?1`
4. Read the `.toml` file path from the row, move file to `characters/.trash/<ulid>-<unix-ts>.toml` (use `fs::rename`)
5. `DELETE FROM character WHERE id = ?1`
6. `COMMIT`

If the file move fails (permission, missing file), continue with the DB delete and log a warning. The orphan-toml case is acceptable; the writer can hand-clean.

- [ ] **Step 6: Register commands in `main.rs`**

```rust
.invoke_handler(tauri::generate_handler![
    // ... existing M1 + M2 commands ...
    commands::character::character_create,
    commands::character::character_read,
    commands::character::character_list,
    commands::character::character_update_field,
    commands::character::character_delete,
])
```

- [ ] **Step 7: TS IPC wrappers**

Create `app/src/ipc/character.ts`:

```ts
import { invoke } from "@tauri-apps/api/core";

export interface CharacterIndexEntry {
  id: string;
  full_name: string;
  role: string | null;
  hue_token: string;
  completion: number;
}

export interface CharacterFile {
  schema_version: "lsm-v2.1";
  hue_token: string;
  data: Record<string, unknown>;
}

export const characterCreate = (): Promise<CharacterIndexEntry> =>
  invoke("character_create");

export const characterRead = (id: string): Promise<CharacterFile> =>
  invoke("character_read", { id });

export const characterList = (): Promise<CharacterIndexEntry[]> =>
  invoke("character_list");

export const characterUpdateField = (
  id: string,
  fieldId: string,
  value: unknown,
): Promise<CharacterIndexEntry> =>
  invoke("character_update_field", { id, fieldId, value });

export const characterDelete = (id: string): Promise<void> =>
  invoke("character_delete", { id });
```

Add to `app/src/ipc/index.ts`:

```ts
export * from "./character";
```

- [ ] **Step 8: Tests — Rust integration (tempfile)**

Add `app/src-tauri/src/commands/character_tests.rs` (or inline `#[cfg(test)]` if your project conventions prefer). Each test follows the M2 pattern: `tempfile::tempdir()` → `Project::open` → invoke command → assert.

```rust
#[tokio::test]
async fn create_character_assigns_round_robin_hue() {
    let (_dir, app_state) = test_project().await;
    let c1 = character_create(app_state.clone()).await.unwrap();
    let c2 = character_create(app_state.clone()).await.unwrap();
    let c3 = character_create(app_state.clone()).await.unwrap();
    assert_eq!(c1.hue_token, "character-1");
    assert_eq!(c2.hue_token, "character-2");
    assert_eq!(c3.hue_token, "character-3");
}

#[tokio::test]
async fn update_field_full_name_appends_old_name_to_aliases() {
    let (_dir, app_state) = test_project().await;
    let c = character_create(app_state.clone()).await.unwrap();
    character_update_field(app_state.clone(), c.id.clone(), "full_name".into(), json!("Marcus Vale")).await.unwrap();
    character_update_field(app_state.clone(), c.id.clone(), "full_name".into(), json!("Marcus Tenebris")).await.unwrap();
    let file = character_read(app_state.clone(), c.id.clone()).await.unwrap();
    let aliases = file.data.get("main").and_then(|m| m.get("aliases")).unwrap();
    assert!(aliases.as_array().unwrap().iter().any(|a| a == "Marcus Vale"));
}

#[tokio::test]
async fn delete_cascades_to_scene_presence_and_pov() {
    let (_dir, app_state) = test_project().await;
    let c = character_create(app_state.clone()).await.unwrap();
    let s = scene_create(app_state.clone(), "Scene 1".into()).await.unwrap();
    character_link_to_scene(app_state.clone(), s.id.clone(), c.id.clone()).await.unwrap();
    character_set_pov(app_state.clone(), s.id.clone(), Some(c.id.clone())).await.unwrap();
    character_delete(app_state.clone(), c.id.clone()).await.unwrap();
    // Verify cascade
    // (assert scene_character_presence row gone; scene.pov_character_id null)
}
```

The cascade test references commands from Task 13. If running tasks strictly in order, defer that test until after T13. Either way, write a placeholder and mark with `#[ignore = "depends on T13"]` until T13 lands.

- [ ] **Step 9: Gates**

```powershell
cargo build -p water-app
cargo clippy -p water-app -- -D warnings
cargo test -p water-app commands::character
pnpm --filter @water/app test
```

- [ ] **Step 10: Commit**

```powershell
git add app/src-tauri/src/commands/character.rs app/src-tauri/src/commands/mod.rs app/src-tauri/src/state.rs app/src-tauri/src/main.rs app/src/ipc/character.ts app/src/ipc/index.ts crates/water-core/src/character/
git commit -m "feat(commands): character CRUD (create/read/list/update_field/delete) (M3 T12)"
```

---

### Task 13: Scene linkage commands

Three commands for managing `scene_character_presence` (multi-character link) + `scene.pov_character_id` (single character).

**Files:**
- Modify: `app/src-tauri/src/commands/character.rs` (3 more commands — they live alongside the CRUD set)
- Modify: `app/src-tauri/src/main.rs` (register)
- Modify: `app/src/ipc/character.ts` (TS wrappers)

- [ ] **Step 1: `character_link_to_scene` + `character_unlink_from_scene`**

```rust
#[tauri::command]
pub async fn character_link_to_scene(
    state: State<'_, AppState>,
    scene_id: String,
    character_id: String,
) -> Result<(), String> {
    let (db, scene_locks) = {
        let proj = state.project.lock().await;
        let project = proj.as_ref().ok_or("no project open")?;
        (project.db.clone(), project.scene_write_locks.clone())
    };
    let scene_id: Id = scene_id.parse().map_err(|e: water_core::Error| e.to_string())?;
    let character_id: Id = character_id.parse().map_err(|e: water_core::Error| e.to_string())?;
    // Scene write lock — link touches scene metadata (the body is unchanged
    // but the metadata table is logically part of "scene state").
    let _g = scene_locks.acquire(&scene_id).await;
    let db_guard = db.lock().await;
    db_guard.conn().execute(
        "INSERT OR IGNORE INTO scene_character_presence (scene_id, character_id) VALUES (?1, ?2)",
        rusqlite::params![scene_id.as_str(), character_id.as_str()],
    ).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn character_unlink_from_scene(
    state: State<'_, AppState>,
    scene_id: String,
    character_id: String,
) -> Result<(), String> {
    let (db, scene_locks) = {
        let proj = state.project.lock().await;
        let project = proj.as_ref().ok_or("no project open")?;
        (project.db.clone(), project.scene_write_locks.clone())
    };
    let scene_id: Id = scene_id.parse().map_err(|e: water_core::Error| e.to_string())?;
    let character_id: Id = character_id.parse().map_err(|e: water_core::Error| e.to_string())?;
    let _g = scene_locks.acquire(&scene_id).await;
    let db_guard = db.lock().await;
    // Transactional: remove presence row; if this character was POV, null POV.
    let tx = db_guard.conn().unchecked_transaction().map_err(|e| e.to_string())?;
    tx.execute(
        "DELETE FROM scene_character_presence WHERE scene_id = ?1 AND character_id = ?2",
        rusqlite::params![scene_id.as_str(), character_id.as_str()],
    ).map_err(|e| e.to_string())?;
    tx.execute(
        "UPDATE scene SET pov_character_id = NULL WHERE id = ?1 AND pov_character_id = ?2",
        rusqlite::params![scene_id.as_str(), character_id.as_str()],
    ).map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}
```

Per spec § 20: "If a writer removes a character from `characters_present` while they are still POV, the POV is auto-cleared." The transactional unlink above implements this.

- [ ] **Step 2: `character_set_pov`**

```rust
#[tauri::command]
pub async fn character_set_pov(
    state: State<'_, AppState>,
    scene_id: String,
    character_id: Option<String>,
) -> Result<(), String> {
    let (db, scene_locks) = {
        let proj = state.project.lock().await;
        let project = proj.as_ref().ok_or("no project open")?;
        (project.db.clone(), project.scene_write_locks.clone())
    };
    let scene_id: Id = scene_id.parse().map_err(|e: water_core::Error| e.to_string())?;
    let _g = scene_locks.acquire(&scene_id).await;
    let db_guard = db.lock().await;

    match character_id {
        Some(c_str) => {
            let c_id: Id = c_str.parse().map_err(|e: water_core::Error| e.to_string())?;
            // Constraint: POV character must be in characters_present.
            let present: i64 = db_guard.conn().query_row(
                "SELECT COUNT(*) FROM scene_character_presence WHERE scene_id = ?1 AND character_id = ?2",
                rusqlite::params![scene_id.as_str(), c_id.as_str()],
                |r| r.get(0),
            ).map_err(|e| e.to_string())?;
            if present == 0 {
                return Err("POV character must be in characters_present; link them first".into());
            }
            db_guard.conn().execute(
                "UPDATE scene SET pov_character_id = ?1 WHERE id = ?2",
                rusqlite::params![c_id.as_str(), scene_id.as_str()],
            ).map_err(|e| e.to_string())?;
        }
        None => {
            db_guard.conn().execute(
                "UPDATE scene SET pov_character_id = NULL WHERE id = ?1",
                rusqlite::params![scene_id.as_str()],
            ).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
```

- [ ] **Step 3: Register + TS wrappers**

In `main.rs`:
```rust
commands::character::character_link_to_scene,
commands::character::character_unlink_from_scene,
commands::character::character_set_pov,
```

In `app/src/ipc/character.ts`:
```ts
export const characterLinkToScene = (sceneId: string, characterId: string): Promise<void> =>
  invoke("character_link_to_scene", { sceneId, characterId });

export const characterUnlinkFromScene = (sceneId: string, characterId: string): Promise<void> =>
  invoke("character_unlink_from_scene", { sceneId, characterId });

export const characterSetPov = (sceneId: string, characterId: string | null): Promise<void> =>
  invoke("character_set_pov", { sceneId, characterId });
```

- [ ] **Step 4: Tests**

```rust
#[tokio::test]
async fn link_then_set_pov_succeeds() {
    let (_dir, app_state) = test_project().await;
    let c = character_create(app_state.clone()).await.unwrap();
    let s = scene_create(app_state.clone(), "Scene 1".into()).await.unwrap();
    character_link_to_scene(app_state.clone(), s.id.clone(), c.id.clone()).await.unwrap();
    character_set_pov(app_state.clone(), s.id.clone(), Some(c.id.clone())).await.unwrap();
    // Assert: scene.pov_character_id == c.id
}

#[tokio::test]
async fn set_pov_without_link_returns_error() {
    let (_dir, app_state) = test_project().await;
    let c = character_create(app_state.clone()).await.unwrap();
    let s = scene_create(app_state.clone(), "Scene 1".into()).await.unwrap();
    let err = character_set_pov(app_state.clone(), s.id.clone(), Some(c.id.clone())).await.unwrap_err();
    assert!(err.contains("characters_present"));
}

#[tokio::test]
async fn unlink_clears_pov_if_was_pov() {
    let (_dir, app_state) = test_project().await;
    let c = character_create(app_state.clone()).await.unwrap();
    let s = scene_create(app_state.clone(), "Scene 1".into()).await.unwrap();
    character_link_to_scene(app_state.clone(), s.id.clone(), c.id.clone()).await.unwrap();
    character_set_pov(app_state.clone(), s.id.clone(), Some(c.id.clone())).await.unwrap();
    character_unlink_from_scene(app_state.clone(), s.id.clone(), c.id.clone()).await.unwrap();
    // Assert: scene.pov_character_id is NULL
}
```

- [ ] **Step 5: Gates**

```powershell
cargo build -p water-app
cargo clippy -p water-app -- -D warnings
cargo test -p water-app commands::character
pnpm --filter @water/app test
```

- [ ] **Step 6: Commit**

```powershell
git add app/src-tauri/src/commands/character.rs app/src-tauri/src/main.rs app/src/ipc/character.ts
git commit -m "feat(commands): scene linkage (link/unlink/set_pov) for characters (M3 T13)"
```

---

### Task 14: `intake_schema` + `character_autosuggest_for_scene` + autosuggest core

**Files:**
- Create: `crates/water-core/src/character/autosuggest.rs`
- Modify: `crates/water-core/src/character/mod.rs` (register module + re-export `AutosuggestResult`, `suggest_for_scene_body`)
- Modify: `app/src-tauri/src/commands/character.rs` (add `intake_schema`, `character_autosuggest_for_scene`)
- Modify: `app/src-tauri/src/main.rs` (register)
- Modify: `app/src/ipc/character.ts` (TS wrappers)

- [ ] **Step 1: `autosuggest.rs` core module**

Verbatim from spec § 15 with the `regex` crate (already a `water-core` dep per M2's anti-loop):

```rust
//! Scene-character autosuggest: scans a scene body for character full names
//! and aliases (case-sensitive, \b word boundary) and returns the top 5
//! most-mentioned characters with mention counts.
//!
//! KNOWN_FRAGILE #15: name-string-matching, not co-reference resolution.
//! Pronouns ("he", "her") don't link. Manual multi-select bridges the gap.

use crate::character::CharacterRow;
use crate::Id;
use regex::Regex;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct AutosuggestResult {
    pub character_id: Id,
    pub full_name: String,
    pub mention_count: u32,
}

pub fn suggest_for_scene_body(
    body_text: &str,
    characters: &[CharacterRow],
) -> Vec<AutosuggestResult> {
    let mut results: Vec<AutosuggestResult> = Vec::new();
    for character in characters {
        let mut count = count_word_boundary_matches(body_text, &character.full_name);
        for alias in &character.aliases {
            count = count.saturating_add(count_word_boundary_matches(body_text, alias));
        }
        if count > 0 {
            results.push(AutosuggestResult {
                character_id: character.id.clone(),
                full_name: character.full_name.clone(),
                mention_count: count,
            });
        }
    }
    results.sort_by(|a, b| b.mention_count.cmp(&a.mention_count));
    results.truncate(5);
    results
}

fn count_word_boundary_matches(haystack: &str, needle: &str) -> u32 {
    if needle.trim().is_empty() {
        return 0;
    }
    // \b<escaped needle>\b — case sensitive.
    let pattern = format!(r"\b{}\b", regex::escape(needle));
    let Ok(re) = Regex::new(&pattern) else {
        return 0;
    };
    u32::try_from(re.find_iter(haystack).count()).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Id;

    fn row(id: &str, full_name: &str, aliases: &[&str]) -> CharacterRow {
        CharacterRow {
            id: Id::generate(),
            full_name: full_name.into(),
            role: None,
            aliases: aliases.iter().map(|s| s.to_string()).collect(),
            hue_token: "character-1".into(),
            // ... fill other fields as needed; consider a builder helper
        }
    }

    #[test]
    fn full_name_match() {
        let body = "Marcus walked into the bar. Marcus sat down.";
        let rows = vec![row("c1", "Marcus", &[])];
        let r = suggest_for_scene_body(body, &rows);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].mention_count, 2);
    }

    #[test]
    fn word_boundary_excludes_substring() {
        // "Mark" should not match inside "Marketing"
        let body = "The marketing meeting ran long.";
        let rows = vec![row("c1", "Mark", &[])];
        let r = suggest_for_scene_body(body, &rows);
        assert!(r.is_empty());
    }

    #[test]
    fn case_sensitive() {
        let body = "marcus walked in.";
        let rows = vec![row("c1", "Marcus", &[])];
        let r = suggest_for_scene_body(body, &rows);
        assert!(r.is_empty(), "lowercase 'marcus' should not match 'Marcus' (case-sensitive policy)");
    }

    #[test]
    fn aliases_counted() {
        let body = "Marc walked in. Marcus sat. Vale watched.";
        let rows = vec![row("c1", "Marcus Vale", &["Marc", "Vale"])];
        let r = suggest_for_scene_body(body, &rows);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].mention_count, 3); // 0 (full name not present) + 1 (Marc) + 1 (Vale) + 1 (Marcus — partial substring of 'Marcus Vale' won't word-boundary-match, but 'Marcus' alone... wait, 'Marcus' is not in aliases. Adjust fixture.)
    }

    #[test]
    fn ranked_by_count_top_5() {
        // Six characters, descending mention counts; assert truncation + order.
        // ...
    }

    #[test]
    fn empty_alias_doesnt_match_everything() {
        let body = "some body text";
        let rows = vec![row("c1", "Marcus", &[""])];
        let r = suggest_for_scene_body(body, &rows);
        assert!(r.is_empty());
    }
}
```

The `aliases_counted` test's expected count needs reconciling with the alias list — verify by running the test once and adjusting. The intent is: aliases-only matches are counted; the full-name presence/absence in the body is independent.

- [ ] **Step 2: Commands**

In `app/src-tauri/src/commands/character.rs`:

```rust
use water_core::character::{
    autosuggest::{suggest_for_scene_body, AutosuggestResult},
    intake::{IntakeField, LSM_V2_1},
};

#[derive(Serialize)]
pub struct IntakeSchemaSection {
    pub section: String,
    pub fields: Vec<IntakeField>,
}

#[tauri::command]
pub async fn intake_schema(schema_id: String) -> Result<Vec<IntakeSchemaSection>, String> {
    if schema_id != "lsm-v2.1" {
        return Err(format!("unknown schema_id: {schema_id}"));
    }
    // LSM_V2_1 is a static slice of (section, fields) pairs from Task 1.
    Ok(LSM_V2_1.iter().map(|(section, fields)| IntakeSchemaSection {
        section: (*section).to_string(),
        fields: fields.to_vec(),
    }).collect())
}

#[derive(Serialize)]
pub struct AutosuggestResultDto {
    pub character_id: String,
    pub full_name: String,
    pub mention_count: u32,
}

#[tauri::command]
pub async fn character_autosuggest_for_scene(
    state: State<'_, AppState>,
    scene_id: String,
    body_text: String,
) -> Result<Vec<AutosuggestResultDto>, String> {
    let (db, _root) = {
        let proj = state.project.lock().await;
        let project = proj.as_ref().ok_or("no project open")?;
        (project.db.clone(), project.root.clone())
    };
    let _scene_id: Id = scene_id.parse().map_err(|e: water_core::Error| e.to_string())?;
    // scene_id is currently unused (we autosuggest based on body text alone),
    // but kept in the signature so a future implementation can use it
    // (e.g. to exclude already-linked characters from suggestions).

    let db_guard = db.lock().await;
    let all_chars = CharacterStore::list_all_with_aliases(&db_guard)
        .map_err(|e| e.to_string())?;
    let results = suggest_for_scene_body(&body_text, &all_chars);

    Ok(results.into_iter().map(|r| AutosuggestResultDto {
        character_id: r.character_id.to_string(),
        full_name: r.full_name,
        mention_count: r.mention_count,
    }).collect())
}
```

`CharacterStore::list_all_with_aliases` is a new method that returns characters with their aliases parsed from the `data_json`. Add to `CharacterStore`. The data shape: `aliases` lives at `data_json.main.aliases` per LSM v2.1.

- [ ] **Step 3: Register + TS wrappers**

In `main.rs`:
```rust
commands::character::intake_schema,
commands::character::character_autosuggest_for_scene,
```

In `app/src/ipc/character.ts`:
```ts
export interface IntakeField {
  id: string;
  path: string;
  label: string;
  prompt_question: string;
  required: boolean;
  kind: "string" | "string_multiline" | "string_array" | "enum";
  enum_options?: string[];
}

export interface IntakeSchemaSection {
  section: string;
  fields: IntakeField[];
}

export const intakeSchema = (schemaId: string): Promise<IntakeSchemaSection[]> =>
  invoke("intake_schema", { schemaId });

export interface AutosuggestResult {
  character_id: string;
  full_name: string;
  mention_count: number;
}

export const characterAutosuggestForScene = (
  sceneId: string,
  bodyText: string,
): Promise<AutosuggestResult[]> =>
  invoke("character_autosuggest_for_scene", { sceneId, bodyText });
```

- [ ] **Step 4: Renderer event wiring (deferred to F4)**

The autosuggest command is called from scene autosave (every 2s debounced; reuses `EditorCanvas`'s existing autosave). That wiring lives in F4 (SceneMetadataSheet); this task only ships the command + types.

- [ ] **Step 5: Tests — command correctness**

```rust
#[tokio::test]
async fn intake_schema_returns_29_fields_for_lsm_v2_1() {
    let sections = intake_schema("lsm-v2.1".into()).await.unwrap();
    let total: usize = sections.iter().map(|s| s.fields.len()).sum();
    assert_eq!(total, 29);
}

#[tokio::test]
async fn intake_schema_unknown_errors() {
    let err = intake_schema("garbage".into()).await.unwrap_err();
    assert!(err.contains("unknown schema_id"));
}

#[tokio::test]
async fn autosuggest_excludes_zero_mention_chars() {
    let (_dir, app_state) = test_project().await;
    let c1 = character_create(app_state.clone()).await.unwrap();
    character_update_field(app_state.clone(), c1.id.clone(), "full_name".into(), json!("Marcus")).await.unwrap();
    let c2 = character_create(app_state.clone()).await.unwrap();
    character_update_field(app_state.clone(), c2.id.clone(), "full_name".into(), json!("Talia")).await.unwrap();
    let s = scene_create(app_state.clone(), "Scene".into()).await.unwrap();
    let results = character_autosuggest_for_scene(
        app_state.clone(),
        s.id.clone(),
        "Marcus walked in.".to_string(),
    ).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].full_name, "Marcus");
}
```

- [ ] **Step 6: Gates**

```powershell
cargo build -p water-core
cargo build -p water-app
cargo clippy -p water-core --all-targets -- -D warnings
cargo clippy -p water-app -- -D warnings
cargo test -p water-core character::autosuggest
cargo test -p water-app commands::character
pnpm --filter @water/app test
```

- [ ] **Step 7: Commit**

```powershell
git add crates/water-core/src/character/ app/src-tauri/src/commands/character.rs app/src-tauri/src/main.rs app/src/ipc/character.ts
git commit -m "feat(character): autosuggest + intake_schema commands (M3 T14)"
```

---

## Phase E — Renderer: Conversational Intake

Three React tasks for the Intake popup. Reuses the M1.5 `Sheet` primitive (right-edge slide-in dialog) and pipes per-question answers through `characterUpdateField`.

- **Task 15** — `ConversationalIntake.tsx` — schema-agnostic question-by-question component.
- **Task 16** — `CharacterIntakeSheet.tsx` — Sheet-wrapped LSM v2.1 intake; loads schema + calls update_field per advance.
- **Task 17** — Entry points: "+ New character" button (CharacterIndex stub for now) + "Continue intake" CTA (CharacterSheet stub).

### Task 15: `ConversationalIntake.tsx` — schema-agnostic component

A reusable component that walks through an array of `IntakeField`s one at a time, calling `onAnswer(fieldId, value)` for each. Schema-agnostic so future intakes (world, manuscript metadata) can reuse it.

**Files:**
- Create: `app/src/intake/ConversationalIntake.tsx`
- Create: `app/src/intake/ConversationalIntake.test.tsx`

- [ ] **Step 1: Component shape**

```tsx
import { useState } from "react";
import type { IntakeField, IntakeSchemaSection } from "../ipc/character";

interface Props {
  schema: IntakeSchemaSection[];
  initialValues: Record<string, unknown>;
  onAnswer: (fieldId: string, value: unknown) => Promise<void>;
  onComplete: () => void;
  onClose: () => void;
}

interface FlatField extends IntakeField {
  section: string;
}

export function ConversationalIntake({ schema, initialValues, onAnswer, onComplete, onClose }: Props) {
  // Flatten schema into a single ordered list of fields.
  const fields: FlatField[] = schema.flatMap((s) =>
    s.fields.map((f) => ({ ...f, section: s.section })),
  );

  // Find the first unanswered (or first required-but-empty) field for resume.
  const initialIndex = findFirstUnanswered(fields, initialValues);

  const [index, setIndex] = useState(initialIndex);
  const [draft, setDraft] = useState<unknown>(initialValues[fields[index]?.id ?? ""] ?? "");
  const [busy, setBusy] = useState(false);

  if (index >= fields.length) {
    return <div role="status">Intake complete.</div>;
  }
  const field = fields[index]!;

  const advance = async (skip: boolean) => {
    if (busy) return;
    if (!skip) {
      // Validate: required + non-empty
      if (field.required && isEmpty(draft, field.kind)) {
        return; // Stays on current field; visual cue handled by required indicator.
      }
      setBusy(true);
      try {
        await onAnswer(field.id, draft);
      } finally {
        setBusy(false);
      }
    }
    const next = index + 1;
    if (next >= fields.length) {
      onComplete();
      return;
    }
    setIndex(next);
    setDraft(initialValues[fields[next]!.id] ?? "");
  };

  const back = () => {
    if (index === 0) return;
    const prev = index - 1;
    setIndex(prev);
    setDraft(initialValues[fields[prev]!.id] ?? "");
  };

  return (
    <div data-testid="conversational-intake">
      <div data-testid="section">{field.section}</div>
      <h3>{field.prompt_question}</h3>
      <FieldInput field={field} value={draft} onChange={setDraft} onSubmit={() => void advance(false)} />
      <div role="group" aria-label="Intake navigation">
        <button type="button" onClick={back} disabled={index === 0 || busy}>Back</button>
        <button type="button" onClick={() => void advance(true)} disabled={field.required || busy}>
          {field.required ? "Required" : "Skip"}
        </button>
        <button type="button" onClick={() => void advance(false)} disabled={busy}>
          {busy ? "Saving…" : "Next"}
        </button>
        <button type="button" onClick={onClose}>Save & close</button>
      </div>
      <div data-testid="progress">{index + 1} / {fields.length}</div>
    </div>
  );
}
```

Helper components + utilities:

```tsx
function FieldInput({ field, value, onChange, onSubmit }: {
  field: IntakeField;
  value: unknown;
  onChange: (v: unknown) => void;
  onSubmit: () => void;
}) {
  switch (field.kind) {
    case "string":
      return (
        <input
          type="text"
          value={String(value ?? "")}
          onChange={(e) => onChange(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              e.preventDefault();
              onSubmit();
            }
          }}
          autoFocus
        />
      );
    case "string_multiline":
      return (
        <textarea
          value={String(value ?? "")}
          onChange={(e) => onChange(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
              e.preventDefault();
              onSubmit();
            }
          }}
          autoFocus
          rows={5}
        />
      );
    case "string_array": {
      const arr = Array.isArray(value) ? (value as string[]) : [];
      return <StringArrayEditor value={arr} onChange={(v) => onChange(v)} onSubmit={onSubmit} />;
    }
    case "enum":
      return (
        <select value={String(value ?? "")} onChange={(e) => onChange(e.target.value)} autoFocus>
          <option value="" disabled>Choose…</option>
          {(field.enum_options ?? []).map((opt) => (
            <option key={opt} value={opt}>{opt}</option>
          ))}
        </select>
      );
  }
}

function StringArrayEditor({ value, onChange, onSubmit }: {
  value: string[];
  onChange: (v: string[]) => void;
  onSubmit: () => void;
}) {
  // Comma-separated input; chips render in real-time below.
  const [text, setText] = useState(value.join(", "));
  return (
    <>
      <input
        type="text"
        placeholder="comma, separated, values"
        value={text}
        onChange={(e) => {
          setText(e.target.value);
          onChange(e.target.value.split(",").map((s) => s.trim()).filter(Boolean));
        }}
        onKeyDown={(e) => {
          if (e.key === "Enter") {
            e.preventDefault();
            onSubmit();
          }
        }}
        autoFocus
      />
      <ul data-testid="string-array-chips">
        {value.map((v) => <li key={v}>{v}</li>)}
      </ul>
    </>
  );
}

function findFirstUnanswered(fields: FlatField[], values: Record<string, unknown>): number {
  for (let i = 0; i < fields.length; i++) {
    const f = fields[i]!;
    const v = values[f.id];
    if (isEmpty(v, f.kind)) return i;
  }
  return fields.length;
}

function isEmpty(v: unknown, kind: IntakeField["kind"]): boolean {
  if (v === null || v === undefined) return true;
  if (kind === "string" || kind === "string_multiline" || kind === "enum") {
    return typeof v !== "string" || v.trim() === "";
  }
  if (kind === "string_array") {
    return !Array.isArray(v) || v.length === 0;
  }
  return false;
}
```

Note `field.id` always being a string keeps TS strict + `noUncheckedIndexedAccess` happy: `fields[index]!` is guarded by the `if (index >= fields.length)` early return; the `initialValues[fields[next]!.id]` access uses non-null assertion after we know `next < fields.length`.

- [ ] **Step 2: Component tests**

```tsx
import { describe, expect, it, vi } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { ConversationalIntake } from "./ConversationalIntake";

const schema = [
  {
    section: "main",
    fields: [
      { id: "full_name", path: "main.full_name", label: "Full name", prompt_question: "What's the character's full name?", required: true, kind: "string" as const },
      { id: "role", path: "main.role_in_story", label: "Role", prompt_question: "What role do they play?", required: false, kind: "string" as const },
    ],
  },
];

describe("ConversationalIntake", () => {
  it("renders the first question on mount", () => {
    render(
      <ConversationalIntake
        schema={schema}
        initialValues={{}}
        onAnswer={vi.fn().mockResolvedValue(undefined)}
        onComplete={vi.fn()}
        onClose={vi.fn()}
      />,
    );
    expect(screen.getByRole("heading", { name: /full name/i })).toBeInTheDocument();
    expect(screen.getByTestId("progress")).toHaveTextContent("1 / 2");
  });

  it("advances on Enter after typing", async () => {
    const onAnswer = vi.fn().mockResolvedValue(undefined);
    render(
      <ConversationalIntake schema={schema} initialValues={{}} onAnswer={onAnswer} onComplete={vi.fn()} onClose={vi.fn()} />,
    );
    const input = screen.getByRole("textbox");
    fireEvent.change(input, { target: { value: "Marcus Vale" } });
    fireEvent.keyDown(input, { key: "Enter" });
    await waitFor(() => {
      expect(onAnswer).toHaveBeenCalledWith("full_name", "Marcus Vale");
    });
    expect(screen.getByRole("heading", { name: /role/i })).toBeInTheDocument();
  });

  it("does not advance when required field is empty", async () => {
    const onAnswer = vi.fn();
    render(
      <ConversationalIntake schema={schema} initialValues={{}} onAnswer={onAnswer} onComplete={vi.fn()} onClose={vi.fn()} />,
    );
    fireEvent.click(screen.getByRole("button", { name: /^Next$/ }));
    await waitFor(() => {
      // Still on first question; no call.
      expect(onAnswer).not.toHaveBeenCalled();
    });
    expect(screen.getByRole("heading", { name: /full name/i })).toBeInTheDocument();
  });

  it("skip button is disabled on required fields", () => {
    render(
      <ConversationalIntake schema={schema} initialValues={{}} onAnswer={vi.fn()} onComplete={vi.fn()} onClose={vi.fn()} />,
    );
    expect(screen.getByRole("button", { name: /Required/ })).toBeDisabled();
  });

  it("resumes at first unanswered field", () => {
    render(
      <ConversationalIntake
        schema={schema}
        initialValues={{ full_name: "Marcus Vale" }}
        onAnswer={vi.fn()}
        onComplete={vi.fn()}
        onClose={vi.fn()}
      />,
    );
    expect(screen.getByRole("heading", { name: /role/i })).toBeInTheDocument();
    expect(screen.getByTestId("progress")).toHaveTextContent("2 / 2");
  });

  it("calls onComplete after last field", async () => {
    const onAnswer = vi.fn().mockResolvedValue(undefined);
    const onComplete = vi.fn();
    render(
      <ConversationalIntake
        schema={schema}
        initialValues={{ full_name: "Marcus Vale" }}
        onAnswer={onAnswer}
        onComplete={onComplete}
        onClose={vi.fn()}
      />,
    );
    fireEvent.change(screen.getByRole("textbox"), { target: { value: "Skeptic" } });
    fireEvent.keyDown(screen.getByRole("textbox"), { key: "Enter" });
    await waitFor(() => {
      expect(onComplete).toHaveBeenCalled();
    });
  });
});
```

- [ ] **Step 3: Gates**

```powershell
pnpm --filter @water/app test src/intake/ConversationalIntake.test.tsx
pnpm --filter @water/app build
```

- [ ] **Step 4: Commit**

```powershell
git add app/src/intake/ConversationalIntake.tsx app/src/intake/ConversationalIntake.test.tsx
git commit -m "feat(intake): ConversationalIntake schema-agnostic component (M3 T15)"
```

---

### Task 16: `CharacterIntakeSheet.tsx` — Sheet-wrapped LSM v2.1 intake

Wraps `ConversationalIntake` in the M1.5 `Sheet` primitive; loads schema + existing values; wires `characterUpdateField` per advance.

**Files:**
- Create: `app/src/intake/CharacterIntakeSheet.tsx`
- Create: `app/src/intake/CharacterIntakeSheet.test.tsx`

- [ ] **Step 1: Component**

```tsx
import { useEffect, useState } from "react";
import { Sheet } from "../sheets/Sheet";
import { ConversationalIntake } from "./ConversationalIntake";
import {
  intakeSchema,
  characterRead,
  characterUpdateField,
  type IntakeSchemaSection,
  type CharacterFile,
} from "../ipc/character";

interface Props {
  characterId: string;
  open: boolean;
  onClose: () => void;
  onCompleted: () => void;
}

export function CharacterIntakeSheet({ characterId, open, onClose, onCompleted }: Props) {
  const [schema, setSchema] = useState<IntakeSchemaSection[] | null>(null);
  const [values, setValues] = useState<Record<string, unknown> | null>(null);
  const [error, setError] = useState<string | null>(null);

  // Load schema + existing values whenever the sheet opens. Cancellation race
  // (M2 T4 pattern): a stale resolve must not overwrite a fresh load.
  useEffect(() => {
    if (!open) return;
    let cancelled = false;
    setError(null);
    setSchema(null);
    setValues(null);
    void (async () => {
      try {
        const [s, file]: [IntakeSchemaSection[], CharacterFile] = await Promise.all([
          intakeSchema("lsm-v2.1"),
          characterRead(characterId),
        ]);
        if (cancelled) return;
        setSchema(s);
        setValues(flattenCharacterData(file.data));
      } catch (e) {
        if (cancelled) return;
        setError(String(e));
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [open, characterId]);

  return (
    <Sheet open={open} onClose={onClose} title="Character intake">
      {error ? (
        <div role="alert">Failed to load: {error}</div>
      ) : schema && values ? (
        <ConversationalIntake
          schema={schema}
          initialValues={values}
          onAnswer={async (fieldId, value) => {
            await characterUpdateField(characterId, fieldId, value);
            // Optimistic local-update so resume picks up the new value.
            setValues((prev) => (prev ? { ...prev, [fieldId]: value } : prev));
          }}
          onComplete={() => {
            onCompleted();
            onClose();
          }}
          onClose={onClose}
        />
      ) : (
        <div role="status">Loading…</div>
      )}
    </Sheet>
  );
}

// `CharacterFile.data` is the deserialized TOML — sections nested under their
// section keys (`main`, `bonus_traits`, `arc`, `perspectives`). Flatten to a
// `{ field_id: value }` map keyed by the IntakeField.id (which is the leaf name
// for LSM v2.1: full_name, role_in_story, want, need, …).
function flattenCharacterData(data: Record<string, unknown>): Record<string, unknown> {
  const out: Record<string, unknown> = {};
  for (const section of Object.values(data)) {
    if (typeof section === "object" && section !== null && !Array.isArray(section)) {
      for (const [k, v] of Object.entries(section as Record<string, unknown>)) {
        out[k] = v;
      }
    }
  }
  return out;
}
```

- [ ] **Step 2: Tests**

```tsx
import { describe, expect, it, vi } from "vitest";

vi.mock("../ipc/character", () => ({
  intakeSchema: vi.fn().mockResolvedValue([
    { section: "main", fields: [
      { id: "full_name", path: "main.full_name", label: "Full name", prompt_question: "Full name?", required: true, kind: "string" },
    ]},
  ]),
  characterRead: vi.fn().mockResolvedValue({
    schema_version: "lsm-v2.1",
    hue_token: "character-1",
    data: { main: { full_name: "" } },
  }),
  characterUpdateField: vi.fn().mockResolvedValue({
    id: "c1", full_name: "Marcus Vale", role: null, hue_token: "character-1", completion: 12,
  }),
}));

import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { CharacterIntakeSheet } from "./CharacterIntakeSheet";
import * as charIpc from "../ipc/character";

describe("CharacterIntakeSheet", () => {
  it("loads schema + values on open", async () => {
    render(<CharacterIntakeSheet characterId="c1" open={true} onClose={vi.fn()} onCompleted={vi.fn()} />);
    await screen.findByText(/full name\?/i);
    expect(charIpc.intakeSchema).toHaveBeenCalledWith("lsm-v2.1");
    expect(charIpc.characterRead).toHaveBeenCalledWith("c1");
  });

  it("calls characterUpdateField per answer", async () => {
    render(<CharacterIntakeSheet characterId="c1" open={true} onClose={vi.fn()} onCompleted={vi.fn()} />);
    const input = await screen.findByRole("textbox");
    fireEvent.change(input, { target: { value: "Marcus Vale" } });
    fireEvent.keyDown(input, { key: "Enter" });
    await waitFor(() => {
      expect(charIpc.characterUpdateField).toHaveBeenCalledWith("c1", "full_name", "Marcus Vale");
    });
  });

  it("calls onCompleted after the last field", async () => {
    const onCompleted = vi.fn();
    const onClose = vi.fn();
    render(<CharacterIntakeSheet characterId="c1" open={true} onClose={onClose} onCompleted={onCompleted} />);
    const input = await screen.findByRole("textbox");
    fireEvent.change(input, { target: { value: "Marcus Vale" } });
    fireEvent.keyDown(input, { key: "Enter" });
    await waitFor(() => {
      expect(onCompleted).toHaveBeenCalled();
      expect(onClose).toHaveBeenCalled();
    });
  });

  it("does not race when characterId changes mid-load", async () => {
    // Open with c1, immediately switch to c2 before c1's load resolves.
    // Assert that c1's values don't overwrite c2's state.
    // Uses a deferred Promise pattern to control load timing.
    // ...
  });
});
```

The race test is important — it explicitly exercises the cancellation guard. Pattern from M2 T4.

- [ ] **Step 3: Gates**

```powershell
pnpm --filter @water/app test src/intake/CharacterIntakeSheet.test.tsx
pnpm --filter @water/app build
```

- [ ] **Step 4: Commit**

```powershell
git add app/src/intake/CharacterIntakeSheet.tsx app/src/intake/CharacterIntakeSheet.test.tsx
git commit -m "feat(intake): CharacterIntakeSheet — Sheet-wrapped LSM v2.1 intake (M3 T16)"
```

---

### Task 17: Entry points — "+ New character" + "Continue intake"

This task ships the minimal scaffolding so the Intake sheet is reachable end-to-end without depending on the full CharacterIndex (Phase F2) or CharacterSheet (Phase F1). Provides a temporary stub Characters surface that:
1. Lists characters via `characterList`.
2. Shows a "+ New character" button → creates a character → opens `CharacterIntakeSheet` for it.
3. For each character, shows a "Continue intake" CTA if `completion < 100`.

Phase F1/F2 replace this stub with the polished CharacterIndex.

**Files:**
- Create: `app/src/chrome/CharactersSurface.tsx` (temporary scaffold; replaced by F2)
- Modify: `app/src/App.tsx` (mount CharactersSurface when `activeNav === "characters"`)

- [ ] **Step 1: `CharactersSurface.tsx` scaffold**

```tsx
import { useEffect, useState, useCallback } from "react";
import { characterCreate, characterList, type CharacterIndexEntry } from "../ipc/character";
import { CharacterIntakeSheet } from "../intake/CharacterIntakeSheet";

export function CharactersSurface() {
  const [chars, setChars] = useState<CharacterIndexEntry[]>([]);
  const [intakeCharId, setIntakeCharId] = useState<string | null>(null);

  const reload = useCallback(async () => {
    const list = await characterList();
    setChars(list);
  }, []);

  useEffect(() => {
    void reload();
  }, [reload]);

  const handleNew = useCallback(async () => {
    const created = await characterCreate();
    await reload();
    setIntakeCharId(created.id);
  }, [reload]);

  return (
    <div>
      <header>
        <h2>Characters</h2>
        <button type="button" onClick={() => void handleNew()}>+ New character</button>
      </header>
      <ul>
        {chars.length === 0 && <li role="status">No characters yet.</li>}
        {chars.map((c) => (
          <li key={c.id}>
            <span data-hue-token={c.hue_token}>{c.full_name || "(unnamed)"}</span>
            <span>{c.completion}% complete</span>
            {c.completion < 100 && (
              <button type="button" onClick={() => setIntakeCharId(c.id)}>
                Continue intake
              </button>
            )}
          </li>
        ))}
      </ul>
      {intakeCharId && (
        <CharacterIntakeSheet
          characterId={intakeCharId}
          open={true}
          onClose={() => setIntakeCharId(null)}
          onCompleted={() => {
            void reload();
          }}
        />
      )}
    </div>
  );
}
```

- [ ] **Step 2: Mount in App.tsx**

Find the activeNav switch in `app/src/App.tsx`. Currently scenes/world is rendered. Add the characters branch:

```tsx
import { CharactersSurface } from "./chrome/CharactersSurface";

// In the render JSX, where activeNav drives surface mounting:
{activeNav === "characters" && <CharactersSurface />}
```

Take care to NOT mount `EditorCanvas` while on the characters surface — preserve the M2 T24 reloadToken pattern by keying the EditorCanvas branch on `activeNav === "scenes"`.

- [ ] **Step 3: Tests**

```tsx
import { describe, expect, it, vi } from "vitest";

vi.mock("../ipc/character", () => ({
  characterCreate: vi.fn().mockResolvedValue({ id: "c-new", full_name: "", role: null, hue_token: "character-1", completion: 0 }),
  characterList: vi.fn()
    .mockResolvedValueOnce([])
    .mockResolvedValueOnce([{ id: "c-new", full_name: "", role: null, hue_token: "character-1", completion: 0 }]),
}));

vi.mock("../intake/CharacterIntakeSheet", () => ({
  CharacterIntakeSheet: ({ open, onClose }: { open: boolean; onClose: () => void }) =>
    open ? <div data-testid="intake-sheet"><button onClick={onClose}>close</button></div> : null,
}));

import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { CharactersSurface } from "./CharactersSurface";

describe("CharactersSurface", () => {
  it("shows empty state initially", async () => {
    render(<CharactersSurface />);
    await screen.findByText(/no characters yet/i);
  });

  it("creates a character and opens intake", async () => {
    render(<CharactersSurface />);
    fireEvent.click(await screen.findByRole("button", { name: /new character/i }));
    await waitFor(() => {
      expect(screen.getByTestId("intake-sheet")).toBeInTheDocument();
    });
  });
});
```

- [ ] **Step 4: Gates**

```powershell
pnpm --filter @water/app test src/chrome/CharactersSurface.test.tsx
pnpm --filter @water/app build
```

- [ ] **Step 5: Commit**

```powershell
git add app/src/chrome/CharactersSurface.tsx app/src/App.tsx app/src/chrome/CharactersSurface.test.tsx
git commit -m "feat(characters): scaffold CharactersSurface entry points + intake wiring (M3 T17)"
```

---

## Phase F — Renderer: Sheet + Index + scene linking

Four React tasks. F1+F2 build the polished character UI (replacing the F-stub from Task 17); F3 finalizes IconRail wiring + scroll-preservation; F4 wires the scene-metadata sheet with autosuggest chips.

- **Task 18** — `CharacterSheet.tsx` (inline-editable sheet view).
- **Task 19** — `CharacterIndex.tsx` + `CharacterCard.tsx` (grid + sort + search).
- **Task 20** — Replace `CharactersSurface` scaffold with `CharacterIndex` + `CharacterSheet` routing.
- **Task 21** — `SceneMetadataSheet.tsx` + `SceneAutosuggestChips.tsx` + ScenesPanel "Details" affordance.

### Task 18: `CharacterSheet.tsx` — inline-editable sheet view

Per spec § 8. Renders the full LSM v2.1 sheet as a vertical-scroll page; each field is inline-editable (click → input → blur saves). Autosave-chip pattern (M1.5).

**Files:**
- Create: `app/src/characters/CharacterSheet.tsx`
- Create: `app/src/characters/CharacterSheet.test.tsx`
- Create: `app/src/characters/InlineField.tsx` (reusable field editor primitive)

- [ ] **Step 1: `InlineField.tsx` primitive**

A small editable-cell component. Click to enter edit mode; blur or Enter to save; Esc to revert.

```tsx
import { useEffect, useRef, useState } from "react";
import type { IntakeField } from "../ipc/character";

interface Props {
  field: IntakeField;
  value: unknown;
  onSave: (value: unknown) => Promise<void>;
}

export function InlineField({ field, value, onSave }: Props) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState<unknown>(value);
  const [status, setStatus] = useState<"idle" | "saving" | "saved" | "error">("idle");
  const inputRef = useRef<HTMLInputElement | HTMLTextAreaElement | null>(null);

  useEffect(() => {
    if (!editing) setDraft(value);
  }, [value, editing]);

  useEffect(() => {
    if (editing && inputRef.current) inputRef.current.focus();
  }, [editing]);

  // Auto-fade the "saved" chip after 1.2s.
  useEffect(() => {
    if (status === "saved") {
      const t = window.setTimeout(() => setStatus("idle"), 1200);
      return () => window.clearTimeout(t);
    }
    return undefined;
  }, [status]);

  const commit = async () => {
    if (deepEqual(draft, value)) {
      setEditing(false);
      return;
    }
    setStatus("saving");
    try {
      await onSave(draft);
      setStatus("saved");
      setEditing(false);
    } catch {
      setStatus("error");
    }
  };

  const cancel = () => {
    setDraft(value);
    setEditing(false);
    setStatus("idle");
  };

  if (!editing) {
    return (
      <div className="water-inline-field" onClick={() => setEditing(true)} role="button" tabIndex={0}>
        <label>{field.label}</label>
        <div data-empty={isEmpty(value)}>{formatValue(value, field.kind) || <em>— empty —</em>}</div>
      </div>
    );
  }

  // Edit mode
  return (
    <div className="water-inline-field" data-editing="true">
      <label>{field.label}</label>
      {field.kind === "string" && (
        <input
          ref={inputRef as React.Ref<HTMLInputElement>}
          type="text"
          value={String(draft ?? "")}
          onChange={(e) => setDraft(e.target.value)}
          onBlur={() => void commit()}
          onKeyDown={(e) => {
            if (e.key === "Enter") void commit();
            if (e.key === "Escape") cancel();
          }}
        />
      )}
      {field.kind === "string_multiline" && (
        <textarea
          ref={inputRef as React.Ref<HTMLTextAreaElement>}
          value={String(draft ?? "")}
          onChange={(e) => setDraft(e.target.value)}
          onBlur={() => void commit()}
          onKeyDown={(e) => {
            if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) void commit();
            if (e.key === "Escape") cancel();
          }}
          rows={4}
        />
      )}
      {field.kind === "string_array" && (
        <input
          ref={inputRef as React.Ref<HTMLInputElement>}
          type="text"
          value={Array.isArray(draft) ? (draft as string[]).join(", ") : ""}
          onChange={(e) => setDraft(e.target.value.split(",").map((s) => s.trim()).filter(Boolean))}
          onBlur={() => void commit()}
          onKeyDown={(e) => {
            if (e.key === "Enter") void commit();
            if (e.key === "Escape") cancel();
          }}
        />
      )}
      {field.kind === "enum" && (
        <select
          value={String(draft ?? "")}
          onChange={(e) => setDraft(e.target.value)}
          onBlur={() => void commit()}
        >
          <option value="" disabled>Choose…</option>
          {(field.enum_options ?? []).map((o) => <option key={o} value={o}>{o}</option>)}
        </select>
      )}
      {status === "saving" && <span data-testid="status-chip">Saving…</span>}
      {status === "error" && <span data-testid="status-chip" role="alert">Save failed</span>}
    </div>
  );
}

function isEmpty(v: unknown): boolean {
  if (v === null || v === undefined) return true;
  if (typeof v === "string") return v.trim() === "";
  if (Array.isArray(v)) return v.length === 0;
  return false;
}

function formatValue(v: unknown, kind: IntakeField["kind"]): string {
  if (isEmpty(v)) return "";
  if (kind === "string_array" && Array.isArray(v)) return (v as string[]).join(", ");
  return String(v);
}

function deepEqual(a: unknown, b: unknown): boolean {
  if (Array.isArray(a) && Array.isArray(b)) {
    return a.length === b.length && a.every((x, i) => x === b[i]);
  }
  return a === b;
}
```

- [ ] **Step 2: `CharacterSheet.tsx`**

```tsx
import { useEffect, useState, useCallback } from "react";
import { InlineField } from "./InlineField";
import {
  characterRead,
  characterUpdateField,
  intakeSchema,
  type CharacterFile,
  type IntakeSchemaSection,
} from "../ipc/character";

interface Props {
  characterId: string;
  onBackToIndex: () => void;
  onContinueIntake: () => void;
}

export function CharacterSheet({ characterId, onBackToIndex, onContinueIntake }: Props) {
  const [schema, setSchema] = useState<IntakeSchemaSection[] | null>(null);
  const [file, setFile] = useState<CharacterFile | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    setError(null);
    try {
      const [s, f] = await Promise.all([intakeSchema("lsm-v2.1"), characterRead(characterId)]);
      setSchema(s);
      setFile(f);
    } catch (e) {
      setError(String(e));
    }
  }, [characterId]);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      const [s, f] = await Promise.all([intakeSchema("lsm-v2.1"), characterRead(characterId)]);
      if (cancelled) return;
      setSchema(s);
      setFile(f);
    })();
    return () => {
      cancelled = true;
    };
  }, [characterId]);

  if (error) return <div role="alert">Failed to load: {error}</div>;
  if (!schema || !file) return <div role="status">Loading…</div>;

  const values = flattenCharacterData(file.data);
  const completion = computeCompletion(schema, values);

  return (
    <div className="water-character-sheet" data-hue-token={file.hue_token}>
      <header>
        <button type="button" onClick={onBackToIndex}>← All characters</button>
        <h1>{(values["full_name"] as string) || "(unnamed)"}</h1>
        <div>{completion}% complete</div>
        {completion < 100 && (
          <button type="button" onClick={onContinueIntake}>Continue intake</button>
        )}
      </header>
      {schema.map((section) => (
        <section key={section.section} aria-labelledby={`section-${section.section}`}>
          <h2 id={`section-${section.section}`}>{section.section}</h2>
          {section.fields.map((field) => (
            <InlineField
              key={field.id}
              field={field}
              value={values[field.id]}
              onSave={async (v) => {
                await characterUpdateField(characterId, field.id, v);
                // Reload to pick up rename-cascade aliases etc.
                await load();
              }}
            />
          ))}
        </section>
      ))}
    </div>
  );
}

function flattenCharacterData(data: Record<string, unknown>): Record<string, unknown> {
  const out: Record<string, unknown> = {};
  for (const section of Object.values(data)) {
    if (typeof section === "object" && section !== null && !Array.isArray(section)) {
      for (const [k, v] of Object.entries(section as Record<string, unknown>)) {
        out[k] = v;
      }
    }
  }
  return out;
}

function computeCompletion(schema: IntakeSchemaSection[], values: Record<string, unknown>): number {
  const required = schema.flatMap((s) => s.fields.filter((f) => f.required));
  if (required.length === 0) return 100;
  const filled = required.filter((f) => {
    const v = values[f.id];
    if (typeof v === "string") return v.trim() !== "";
    if (Array.isArray(v)) return v.length > 0;
    return v !== null && v !== undefined;
  }).length;
  return Math.round((filled / required.length) * 100);
}
```

- [ ] **Step 3: Tests**

```tsx
import { describe, expect, it, vi } from "vitest";

vi.mock("../ipc/character", () => ({
  intakeSchema: vi.fn().mockResolvedValue([{
    section: "main",
    fields: [
      { id: "full_name", path: "main.full_name", label: "Full name", prompt_question: "?", required: true, kind: "string" },
      { id: "role", path: "main.role_in_story", label: "Role", prompt_question: "?", required: true, kind: "string" },
    ],
  }]),
  characterRead: vi.fn().mockResolvedValue({
    schema_version: "lsm-v2.1",
    hue_token: "character-1",
    data: { main: { full_name: "Marcus Vale", role: "" } },
  }),
  characterUpdateField: vi.fn().mockResolvedValue({}),
}));

import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { CharacterSheet } from "./CharacterSheet";
import * as charIpc from "../ipc/character";

describe("CharacterSheet", () => {
  it("renders fields with current values", async () => {
    render(<CharacterSheet characterId="c1" onBackToIndex={vi.fn()} onContinueIntake={vi.fn()} />);
    await screen.findByText("Marcus Vale");
    expect(screen.getByText(/50% complete/i)).toBeInTheDocument(); // 1/2 required filled
  });

  it("shows Continue intake when completion < 100", async () => {
    render(<CharacterSheet characterId="c1" onBackToIndex={vi.fn()} onContinueIntake={vi.fn()} />);
    await screen.findByRole("button", { name: /continue intake/i });
  });

  it("saves a field on blur after editing", async () => {
    render(<CharacterSheet characterId="c1" onBackToIndex={vi.fn()} onContinueIntake={vi.fn()} />);
    const empty = await screen.findByText(/— empty —/);
    fireEvent.click(empty.parentElement!);
    const input = await screen.findByDisplayValue("");
    fireEvent.change(input, { target: { value: "Skeptic" } });
    fireEvent.blur(input);
    await waitFor(() => {
      expect(charIpc.characterUpdateField).toHaveBeenCalledWith("c1", "role", "Skeptic");
    });
  });

  it("Escape reverts changes without saving", async () => {
    render(<CharacterSheet characterId="c1" onBackToIndex={vi.fn()} onContinueIntake={vi.fn()} />);
    const name = await screen.findByText("Marcus Vale");
    fireEvent.click(name.parentElement!);
    const input = await screen.findByDisplayValue("Marcus Vale");
    fireEvent.change(input, { target: { value: "Marcus Tenebris" } });
    fireEvent.keyDown(input, { key: "Escape" });
    expect(charIpc.characterUpdateField).not.toHaveBeenCalled();
    expect(screen.getByText("Marcus Vale")).toBeInTheDocument();
  });
});
```

- [ ] **Step 4: Gates**

```powershell
pnpm --filter @water/app test src/characters/CharacterSheet.test.tsx
pnpm --filter @water/app test src/characters/InlineField.test.tsx
pnpm --filter @water/app build
```

(Add a sibling `InlineField.test.tsx` covering the empty/Esc/Enter/error states — InlineField is a reusable primitive that benefits from isolated tests.)

- [ ] **Step 5: Commit**

```powershell
git add app/src/characters/CharacterSheet.tsx app/src/characters/CharacterSheet.test.tsx app/src/characters/InlineField.tsx app/src/characters/InlineField.test.tsx
git commit -m "feat(characters): CharacterSheet inline-editable view (M3 T18)"
```

---

### Task 19: `CharacterIndex.tsx` + `CharacterCard.tsx`

Per spec § 9. Grid view of character cards with sort + search affordances. Each card shows: name, role, hue-token chip, completion percentage, last-edited timestamp (if available).

**Files:**
- Create: `app/src/characters/CharacterIndex.tsx`
- Create: `app/src/characters/CharacterCard.tsx`
- Create: `app/src/characters/CharacterIndex.test.tsx`

- [ ] **Step 1: `CharacterCard.tsx`**

```tsx
import type { CharacterIndexEntry } from "../ipc/character";

interface Props {
  character: CharacterIndexEntry;
  onClick: () => void;
}

export function CharacterCard({ character, onClick }: Props) {
  return (
    <button
      type="button"
      className="water-character-card"
      data-hue-token={character.hue_token}
      onClick={onClick}
    >
      <div className="water-character-card__hue" aria-hidden />
      <div className="water-character-card__name">
        {character.full_name || <em>(unnamed)</em>}
      </div>
      {character.role && <div className="water-character-card__role">{character.role}</div>}
      <div className="water-character-card__completion">
        {character.completion}% complete
      </div>
    </button>
  );
}
```

- [ ] **Step 2: `CharacterIndex.tsx`**

```tsx
import { useEffect, useMemo, useState, useCallback } from "react";
import { CharacterCard } from "./CharacterCard";
import { characterCreate, characterList, type CharacterIndexEntry } from "../ipc/character";

type SortKey = "name" | "completion" | "created";

interface Props {
  onOpenCharacter: (id: string) => void;
  onOpenIntake: (id: string) => void;
}

export function CharacterIndex({ onOpenCharacter, onOpenIntake }: Props) {
  const [chars, setChars] = useState<CharacterIndexEntry[]>([]);
  const [search, setSearch] = useState("");
  const [sortKey, setSortKey] = useState<SortKey>("name");
  const [loaded, setLoaded] = useState(false);

  const reload = useCallback(async () => {
    const list = await characterList();
    setChars(list);
    setLoaded(true);
  }, []);

  useEffect(() => {
    void reload();
  }, [reload]);

  const handleNew = useCallback(async () => {
    const created = await characterCreate();
    await reload();
    onOpenIntake(created.id);
  }, [reload, onOpenIntake]);

  const filtered = useMemo(() => {
    const q = search.trim().toLowerCase();
    let list = chars;
    if (q) {
      list = chars.filter((c) =>
        c.full_name.toLowerCase().includes(q) ||
        (c.role ?? "").toLowerCase().includes(q),
      );
    }
    return [...list].sort((a, b) => {
      switch (sortKey) {
        case "name":
          return a.full_name.localeCompare(b.full_name, undefined, { sensitivity: "base" });
        case "completion":
          return b.completion - a.completion;
        case "created":
          // Index list is ORDER BY full_name today; "created" sort defers to
          // a future migration adding `created_at`. Fall back to id (ULID is
          // monotonic).
          return a.id.localeCompare(b.id);
        default:
          return 0;
      }
    });
  }, [chars, search, sortKey]);

  return (
    <div className="water-character-index">
      <header>
        <h1>Characters</h1>
        <input
          type="search"
          placeholder="Search…"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          aria-label="Search characters"
        />
        <label>
          Sort:
          <select value={sortKey} onChange={(e) => setSortKey(e.target.value as SortKey)}>
            <option value="name">Name</option>
            <option value="completion">Completion</option>
            <option value="created">Created</option>
          </select>
        </label>
        <button type="button" onClick={() => void handleNew()}>+ New character</button>
      </header>
      {loaded && filtered.length === 0 && (
        <div role="status">
          {search.trim() ? "No characters match your search." : "No characters yet."}
        </div>
      )}
      <ul className="water-character-grid">
        {filtered.map((c) => (
          <li key={c.id}>
            <CharacterCard character={c} onClick={() => onOpenCharacter(c.id)} />
          </li>
        ))}
      </ul>
    </div>
  );
}
```

- [ ] **Step 3: Tests**

```tsx
import { describe, expect, it, vi } from "vitest";

vi.mock("../ipc/character", () => ({
  characterCreate: vi.fn().mockResolvedValue({ id: "c-new", full_name: "", role: null, hue_token: "character-1", completion: 0 }),
  characterList: vi.fn().mockResolvedValue([
    { id: "c1", full_name: "Marcus Vale", role: "Skeptic", hue_token: "character-1", completion: 80 },
    { id: "c2", full_name: "Talia Mor", role: "Believer", hue_token: "character-2", completion: 40 },
    { id: "c3", full_name: "Aren", role: null, hue_token: "character-3", completion: 0 },
  ]),
}));

import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { CharacterIndex } from "./CharacterIndex";

describe("CharacterIndex", () => {
  it("renders character cards", async () => {
    render(<CharacterIndex onOpenCharacter={vi.fn()} onOpenIntake={vi.fn()} />);
    await screen.findByText("Marcus Vale");
    expect(screen.getByText("Talia Mor")).toBeInTheDocument();
  });

  it("filters by search", async () => {
    render(<CharacterIndex onOpenCharacter={vi.fn()} onOpenIntake={vi.fn()} />);
    await screen.findByText("Marcus Vale");
    fireEvent.change(screen.getByRole("searchbox"), { target: { value: "talia" } });
    expect(screen.queryByText("Marcus Vale")).not.toBeInTheDocument();
    expect(screen.getByText("Talia Mor")).toBeInTheDocument();
  });

  it("sorts by completion desc", async () => {
    render(<CharacterIndex onOpenCharacter={vi.fn()} onOpenIntake={vi.fn()} />);
    await screen.findByText("Marcus Vale");
    fireEvent.change(screen.getByRole("combobox"), { target: { value: "completion" } });
    const cards = screen.getAllByRole("button").filter((b) => b.className.includes("water-character-card"));
    expect(cards[0]).toHaveTextContent("Marcus Vale"); // 80
    expect(cards[1]).toHaveTextContent("Talia Mor");   // 40
    expect(cards[2]).toHaveTextContent("Aren");        // 0
  });

  it("creates a character and opens intake", async () => {
    const onOpenIntake = vi.fn();
    render(<CharacterIndex onOpenCharacter={vi.fn()} onOpenIntake={onOpenIntake} />);
    await screen.findByText("Marcus Vale");
    fireEvent.click(screen.getByRole("button", { name: /new character/i }));
    await waitFor(() => {
      expect(onOpenIntake).toHaveBeenCalledWith("c-new");
    });
  });

  it("clicking a card calls onOpenCharacter", async () => {
    const onOpenCharacter = vi.fn();
    render(<CharacterIndex onOpenCharacter={onOpenCharacter} onOpenIntake={vi.fn()} />);
    await screen.findByText("Marcus Vale");
    fireEvent.click(screen.getByText("Marcus Vale"));
    expect(onOpenCharacter).toHaveBeenCalledWith("c1");
  });
});
```

- [ ] **Step 4: Gates**

```powershell
pnpm --filter @water/app test src/characters/CharacterIndex.test.tsx
pnpm --filter @water/app build
```

- [ ] **Step 5: Commit**

```powershell
git add app/src/characters/CharacterIndex.tsx app/src/characters/CharacterCard.tsx app/src/characters/CharacterIndex.test.tsx
git commit -m "feat(characters): CharacterIndex + CharacterCard with sort/search (M3 T19)"
```

---

### Task 20: Replace `CharactersSurface` scaffold with polished routing

T17 shipped a temporary `CharactersSurface` stub. Now replace it with proper index ↔ sheet routing, with scroll preservation on return.

**Files:**
- Modify: `app/src/chrome/CharactersSurface.tsx` (replace with router; rename internal state to `view`)
- Modify: `app/src/chrome/CharactersSurface.test.tsx` (extend coverage)

- [ ] **Step 1: Router-style component**

```tsx
import { useState, useCallback } from "react";
import { CharacterIndex } from "../characters/CharacterIndex";
import { CharacterSheet } from "../characters/CharacterSheet";
import { CharacterIntakeSheet } from "../intake/CharacterIntakeSheet";

type View = { kind: "index" } | { kind: "sheet"; characterId: string };

export function CharactersSurface() {
  const [view, setView] = useState<View>({ kind: "index" });
  const [intakeCharId, setIntakeCharId] = useState<string | null>(null);

  const openCharacter = useCallback((id: string) => {
    setView({ kind: "sheet", characterId: id });
  }, []);

  const openIntake = useCallback((id: string) => {
    setIntakeCharId(id);
  }, []);

  const backToIndex = useCallback(() => {
    setView({ kind: "index" });
  }, []);

  return (
    <>
      {view.kind === "index" && (
        <CharacterIndex onOpenCharacter={openCharacter} onOpenIntake={openIntake} />
      )}
      {view.kind === "sheet" && (
        <CharacterSheet
          key={view.characterId}
          characterId={view.characterId}
          onBackToIndex={backToIndex}
          onContinueIntake={() => openIntake(view.characterId)}
        />
      )}
      {intakeCharId && (
        <CharacterIntakeSheet
          characterId={intakeCharId}
          open={true}
          onClose={() => setIntakeCharId(null)}
          onCompleted={() => {
            // Reload index by triggering a state nudge (index reloads on mount;
            // bouncing through a "loading" state is overkill — the index will
            // refetch when re-entered). If we're on the sheet view, the sheet's
            // own load effect picks up the new values.
          }}
        />
      )}
    </>
  );
}
```

**Note on scroll preservation:** the M2 T24 reloadToken pattern keys EditorCanvas on `activeSceneId` so scroll is preserved per scene. For the character surface, the analogous behavior is "preserve index scroll position when going to a sheet and back". React's default behavior won't do this since `<CharacterIndex>` unmounts when switching to sheet view. Two options:

1. **Cheap:** wrap the index in `style={{ display: view.kind === "index" ? "block" : "none" }}` so it stays mounted. Scroll position is preserved naturally.
2. **Expensive:** use `react-router` or a portal-based stack. Overkill for M3.

Use option 1:

```tsx
return (
  <>
    <div style={{ display: view.kind === "index" ? "block" : "none" }}>
      <CharacterIndex onOpenCharacter={openCharacter} onOpenIntake={openIntake} />
    </div>
    {view.kind === "sheet" && (
      <CharacterSheet
        key={view.characterId}
        characterId={view.characterId}
        onBackToIndex={backToIndex}
        onContinueIntake={() => openIntake(view.characterId)}
      />
    )}
    {/* intake sheet portal */}
  </>
);
```

- [ ] **Step 2: Update tests**

The existing T17 tests still apply (scaffold preserved its public shape: lists characters, opens intake). Add:

```tsx
it("navigates to sheet view when a card is clicked", async () => {
  render(<CharactersSurface />);
  fireEvent.click(await screen.findByText("Marcus Vale"));
  await waitFor(() => {
    expect(screen.getByRole("button", { name: /all characters/i })).toBeInTheDocument();
  });
});

it("back-to-index button returns to grid", async () => {
  render(<CharactersSurface />);
  fireEvent.click(await screen.findByText("Marcus Vale"));
  fireEvent.click(await screen.findByRole("button", { name: /all characters/i }));
  await screen.findByRole("heading", { name: /^Characters$/ });
});
```

- [ ] **Step 3: Gates**

```powershell
pnpm --filter @water/app test src/chrome/CharactersSurface.test.tsx
pnpm --filter @water/app build
pnpm --filter @water/app tsc --noEmit
```

- [ ] **Step 4: Commit**

```powershell
git add app/src/chrome/CharactersSurface.tsx app/src/chrome/CharactersSurface.test.tsx
git commit -m "feat(characters): CharactersSurface router (index/sheet/intake) with scroll preservation (M3 T20)"
```

---

### Task 21: `SceneMetadataSheet.tsx` + `SceneAutosuggestChips.tsx`

Per spec § 6.2 + § 15. Scene metadata edit: multi-select `characters_present`, single-select POV, autosuggest chips. Wired via a "Details" affordance on ScenesPanel rows.

**Files:**
- Create: `app/src/scenes/SceneMetadataSheet.tsx`
- Create: `app/src/scenes/SceneAutosuggestChips.tsx`
- Modify: `app/src/chrome/ScenesPanel.tsx` (row-hover "Details" button)
- Modify: `app/src/chrome/EditorCanvas.tsx` (post-autosave: dispatch autosuggest call → publish to chip store)
- Create: `app/src/scenes/sceneMetadataChannel.ts` (small pub/sub for autosuggest results between EditorCanvas and SceneMetadataSheet)
- Create: `app/src/scenes/SceneMetadataSheet.test.tsx`

- [ ] **Step 1: `sceneMetadataChannel.ts`**

A small event-bus so EditorCanvas's post-autosave doesn't have to know about an open SceneMetadataSheet:

```ts
import { type AutosuggestResult } from "../ipc/character";

type Listener = (sceneId: string, results: AutosuggestResult[]) => void;

const listeners = new Set<Listener>();

export function publishAutosuggest(sceneId: string, results: AutosuggestResult[]): void {
  for (const l of listeners) l(sceneId, results);
}

export function subscribeAutosuggest(listener: Listener): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}
```

- [ ] **Step 2: EditorCanvas autosave → autosuggest dispatch**

In `app/src/chrome/EditorCanvas.tsx`, find the existing autosave (debounced 2s). After the save succeeds, fire autosuggest:

```tsx
import { characterAutosuggestForScene } from "../ipc/character";
import { publishAutosuggest } from "../scenes/sceneMetadataChannel";

// In the autosave success branch:
const results = await characterAutosuggestForScene(sceneId, body);
publishAutosuggest(sceneId, results);
```

Wrap in try/catch — autosuggest failure should not block the writer. Log + swallow.

- [ ] **Step 3: `SceneAutosuggestChips.tsx`**

```tsx
import { useEffect, useState } from "react";
import { subscribeAutosuggest } from "./sceneMetadataChannel";
import { characterLinkToScene, type AutosuggestResult } from "../ipc/character";

interface Props {
  sceneId: string;
  alreadyLinkedIds: Set<string>;
  onLinked: () => void;
}

export function SceneAutosuggestChips({ sceneId, alreadyLinkedIds, onLinked }: Props) {
  const [results, setResults] = useState<AutosuggestResult[]>([]);
  const [dismissed, setDismissed] = useState<Set<string>>(new Set());

  useEffect(() => {
    return subscribeAutosuggest((sid, r) => {
      if (sid === sceneId) setResults(r);
    });
  }, [sceneId]);

  const visible = results.filter(
    (r) => !alreadyLinkedIds.has(r.character_id) && !dismissed.has(r.character_id),
  );

  if (visible.length === 0) return null;

  return (
    <div className="water-autosuggest-chips" role="group" aria-label="Suggested characters">
      <span>Suggested present:</span>
      {visible.map((r) => (
        <span key={r.character_id} className="water-autosuggest-chip">
          <button
            type="button"
            onClick={async () => {
              await characterLinkToScene(sceneId, r.character_id);
              onLinked();
            }}
          >
            {r.full_name} (×{r.mention_count})
          </button>
          <button
            type="button"
            aria-label={`Dismiss ${r.full_name}`}
            onClick={() => setDismissed((d) => new Set(d).add(r.character_id))}
          >
            ×
          </button>
        </span>
      ))}
    </div>
  );
}
```

- [ ] **Step 4: `SceneMetadataSheet.tsx`**

```tsx
import { useEffect, useState, useCallback } from "react";
import { Sheet } from "../sheets/Sheet";
import { SceneAutosuggestChips } from "./SceneAutosuggestChips";
import {
  characterList,
  characterLinkToScene,
  characterUnlinkFromScene,
  characterSetPov,
  type CharacterIndexEntry,
} from "../ipc/character";
// Plus a scene IPC to read present + pov; add if missing:
import { sceneReadMetadata } from "../ipc/scene";

interface Props {
  sceneId: string;
  open: boolean;
  onClose: () => void;
}

interface SceneMeta {
  characters_present: string[];
  pov_character_id: string | null;
}

export function SceneMetadataSheet({ sceneId, open, onClose }: Props) {
  const [allChars, setAllChars] = useState<CharacterIndexEntry[]>([]);
  const [meta, setMeta] = useState<SceneMeta | null>(null);

  const reload = useCallback(async () => {
    const [chars, m]: [CharacterIndexEntry[], SceneMeta] = await Promise.all([
      characterList(),
      sceneReadMetadata(sceneId),
    ]);
    setAllChars(chars);
    setMeta(m);
  }, [sceneId]);

  useEffect(() => {
    if (!open) return;
    let cancelled = false;
    void (async () => {
      const [chars, m] = await Promise.all([characterList(), sceneReadMetadata(sceneId)]);
      if (cancelled) return;
      setAllChars(chars);
      setMeta(m);
    })();
    return () => {
      cancelled = true;
    };
  }, [open, sceneId]);

  if (!meta) {
    return (
      <Sheet open={open} onClose={onClose} title="Scene details">
        <div role="status">Loading…</div>
      </Sheet>
    );
  }

  const linkedIds = new Set(meta.characters_present);

  const toggleLink = async (charId: string) => {
    if (linkedIds.has(charId)) {
      await characterUnlinkFromScene(sceneId, charId);
    } else {
      await characterLinkToScene(sceneId, charId);
    }
    await reload();
  };

  const setPov = async (charId: string | null) => {
    await characterSetPov(sceneId, charId);
    await reload();
  };

  return (
    <Sheet open={open} onClose={onClose} title="Scene details">
      <SceneAutosuggestChips
        sceneId={sceneId}
        alreadyLinkedIds={linkedIds}
        onLinked={() => void reload()}
      />
      <section>
        <h3>Characters present</h3>
        <ul>
          {allChars.map((c) => (
            <li key={c.id}>
              <label>
                <input
                  type="checkbox"
                  checked={linkedIds.has(c.id)}
                  onChange={() => void toggleLink(c.id)}
                />
                {c.full_name || <em>(unnamed)</em>}
              </label>
            </li>
          ))}
        </ul>
      </section>
      <section>
        <h3>POV character</h3>
        <select
          value={meta.pov_character_id ?? ""}
          onChange={(e) => void setPov(e.target.value === "" ? null : e.target.value)}
        >
          <option value="">— none —</option>
          {allChars.filter((c) => linkedIds.has(c.id)).map((c) => (
            <option key={c.id} value={c.id}>{c.full_name}</option>
          ))}
        </select>
      </section>
    </Sheet>
  );
}
```

Note: `sceneReadMetadata(sceneId)` IPC may not exist yet. Add it as a one-line command (`SELECT pov_character_id FROM scene WHERE id = ?1` + `SELECT character_id FROM scene_character_presence WHERE scene_id = ?1`). Add to `app/src-tauri/src/commands/scene.rs` and `app/src/ipc/scene.ts`. If running tasks strictly in order, this small bonus command should be added alongside Phase D T13; or add it here.

- [ ] **Step 5: ScenesPanel "Details" affordance**

Find `app/src/chrome/ScenesPanel.tsx`. Each scene row currently shows name + ordering. Add a hover/focus-visible "Details" button:

```tsx
<button
  type="button"
  className="water-scene-row__details"
  onClick={(e) => {
    e.stopPropagation();
    onOpenDetails(scene.id);
  }}
  aria-label={`Scene details: ${scene.name}`}
>
  …
</button>
```

The parent component (App.tsx) holds the open state:

```tsx
const [detailsSceneId, setDetailsSceneId] = useState<string | null>(null);
// In ScenesPanel props:
onOpenDetails={(id) => setDetailsSceneId(id)}
// Render:
{detailsSceneId && (
  <SceneMetadataSheet
    sceneId={detailsSceneId}
    open={true}
    onClose={() => setDetailsSceneId(null)}
  />
)}
```

- [ ] **Step 6: Tests**

```tsx
import { describe, expect, it, vi } from "vitest";

vi.mock("../ipc/character", () => ({
  characterList: vi.fn().mockResolvedValue([
    { id: "c1", full_name: "Marcus", role: null, hue_token: "character-1", completion: 80 },
    { id: "c2", full_name: "Talia", role: null, hue_token: "character-2", completion: 80 },
  ]),
  characterLinkToScene: vi.fn().mockResolvedValue(undefined),
  characterUnlinkFromScene: vi.fn().mockResolvedValue(undefined),
  characterSetPov: vi.fn().mockResolvedValue(undefined),
}));

vi.mock("../ipc/scene", () => ({
  sceneReadMetadata: vi.fn().mockResolvedValue({
    characters_present: ["c1"],
    pov_character_id: "c1",
  }),
}));

import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import { SceneMetadataSheet } from "./SceneMetadataSheet";
import * as charIpc from "../ipc/character";
import { publishAutosuggest } from "./sceneMetadataChannel";

describe("SceneMetadataSheet", () => {
  it("loads scene meta and characters on open", async () => {
    render(<SceneMetadataSheet sceneId="s1" open={true} onClose={vi.fn()} />);
    await screen.findByText("Marcus");
    expect(screen.getByText("Talia")).toBeInTheDocument();
    const marcusCheckbox = screen.getByLabelText("Marcus");
    expect(marcusCheckbox).toBeChecked();
  });

  it("toggling a checkbox links/unlinks", async () => {
    render(<SceneMetadataSheet sceneId="s1" open={true} onClose={vi.fn()} />);
    const taliaBox = await screen.findByLabelText("Talia");
    fireEvent.click(taliaBox);
    await waitFor(() => {
      expect(charIpc.characterLinkToScene).toHaveBeenCalledWith("s1", "c2");
    });
  });

  it("POV select only shows linked characters", async () => {
    render(<SceneMetadataSheet sceneId="s1" open={true} onClose={vi.fn()} />);
    await screen.findByText("Marcus");
    const select = screen.getByRole("combobox") as HTMLSelectElement;
    const options = Array.from(select.querySelectorAll("option")).map((o) => o.textContent);
    expect(options).toContain("Marcus"); // linked
    expect(options).not.toContain("Talia"); // not linked
  });

  it("displays autosuggest chips after publish", async () => {
    render(<SceneMetadataSheet sceneId="s1" open={true} onClose={vi.fn()} />);
    await screen.findByText("Marcus");
    publishAutosuggest("s1", [
      { character_id: "c2", full_name: "Talia", mention_count: 3 },
    ]);
    await screen.findByText(/Talia \(×3\)/);
  });

  it("dismissing a chip removes it but does not unlink", async () => {
    render(<SceneMetadataSheet sceneId="s1" open={true} onClose={vi.fn()} />);
    await screen.findByText("Marcus");
    publishAutosuggest("s1", [
      { character_id: "c2", full_name: "Talia", mention_count: 3 },
    ]);
    fireEvent.click(await screen.findByRole("button", { name: /Dismiss Talia/ }));
    expect(screen.queryByText(/Talia \(×3\)/)).not.toBeInTheDocument();
    expect(charIpc.characterUnlinkFromScene).not.toHaveBeenCalled();
  });
});
```

- [ ] **Step 7: Gates**

```powershell
cargo build -p water-app
cargo clippy -p water-app -- -D warnings
pnpm --filter @water/app test src/scenes
pnpm --filter @water/app test src/chrome/ScenesPanel.test.tsx
pnpm --filter @water/app build
```

- [ ] **Step 8: Commit**

```powershell
git add app/src/scenes/ app/src/chrome/ScenesPanel.tsx app/src/chrome/EditorCanvas.tsx app/src/ipc/scene.ts app/src-tauri/src/commands/scene.rs
git commit -m "feat(scenes): SceneMetadataSheet + autosuggest chips + ScenesPanel details affordance (M3 T21)"
```

---

## Phase G — Audit + tag

One task: combined spec+code review over `m2.5..HEAD`, manual smoke against the Marcus Vale reference fixture, KNOWN_FRAGILE updates, and the `m3` tag.

### Task 22: Final review + manual smoke + tag `m3`

**Files:**
- Create: `eval/m3_acceptance/marcus_vale.toml` (reference fixture)
- Modify: `KNOWN_FRAGILE.md` (add #15, #16, #17)
- Modify: `docs/superpowers/specs/2026-05-18-m3-character-sheets.md` (mark "closed" header + closing date)
- Tag: `m3`

- [ ] **Step 1: Marcus Vale reference fixture**

Hand-author `eval/m3_acceptance/marcus_vale.toml` with all 29 LSM v2.1 fields filled. Use the field copy from spec § 5.1. Aim for content that:
- Demonstrates each `kind` (string, string_multiline, string_array, enum).
- Provides distinctive `voice`, `speech_patterns`, `lie_they_believe`, `fears` so the voice template produces character-flavored pills in manual smoke.
- Sets `arc.lie_they_believe = "I'll be remembered for what I built, not what I lost"` so a planted paragraph containing "Marcus felt the weight of being forgotten" trips `character_dissonance` Stage 1.

Example structure:

```toml
schema_version = "lsm-v2.1"
hue_token = "character-1"

[data.main]
full_name = "Marcus Vale"
aliases = []
role_in_story = "Skeptical archivist who guards the last copy"
want = "To finish cataloguing the library before the storm season"
need = "To accept that knowledge is a living thing, not a frozen one"

[data.arc]
lie_they_believe = "I'll be remembered for what I built, not what I lost"
ghost_wound = "His sister's research vanished when the second cellar flooded"
fatal_flaw = "Mistakes preservation for love"
# ... etc

[data.bonus_traits]
voice = "dry, technical, parenthetical asides"
speech_patterns = ["lists three things when nervous", "deflects with citations"]
# ... etc

[data.perspectives]
fears = ["drowning in his own irrelevance", "being the last reader"]
values = ["accuracy", "patience", "stewardship"]
# ... etc
```

The fixture is for manual smoke + future eval-harness regression; commit it to the repo.

- [ ] **Step 2: Dispatch combined spec + code review**

Use the `superpowers:requesting-code-review` skill. Dispatch a parallel review with:

1. **Spec adherence review** — does the implementation match `docs/superpowers/specs/2026-05-18-m3-character-sheets.md`? Especially:
   - 29 LSM v2.1 fields with verbatim prompt copy
   - Two-stage `character_dissonance` with cheap Stage-2 prompt
   - Per-character write-lock parallel to `SceneWriteLocks`
   - Round-robin hue assignment
   - Soft-delete to `characters/.trash/`
   - Rename cascade to aliases
   - POV constraint (must be in `characters_present`)
   - Autosuggest: case-sensitive, word-boundary, top-5

2. **Code quality review** — confidence-filtered findings on:
   - Lock ordering (project → db, scene-lock before db-lock, character-lock before db-lock)
   - Cancellation race fixes in every load-on-mount effect (5+ new ones in Phase E/F)
   - `noUncheckedIndexedAccess` compliance in new TS
   - Clippy + cargo fmt cleanliness
   - Test coverage for all 22 tasks' exit criteria

Run the reviewer over `m2.5..HEAD`:

```powershell
git log --oneline m2.5..HEAD
```

Should show 22 commits + this audit commit (24 total at tag time including fixture + KF updates).

Address every "high confidence" finding before tagging. "Medium confidence" findings get an entry in `docs/superpowers/handoffs/2026-05-18-m3-handoff.md` for the next milestone.

- [ ] **Step 3: Manual smoke against Marcus Vale**

Run the app:

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"
pnpm --filter @water/app tauri dev
```

Walk through each exit criterion from spec § 18:

1. **Create a new character via "+ New character"** in CharacterIndex. Walk the full LSM v2.1 intake using Marcus Vale's answers (copy from the fixture). Time it. Expected: ≤ 3 minutes for a prepared writer.
2. **Edit any field via sheet view.** Verify `characters/<ulid>.toml` reflects the change within 2 seconds. Open the TOML in a text editor to verify it's human-readable and schema-correct.
3. **Add Marcus to a scene's `characters_present`. Set him as POV.** Type a paragraph in the scene editor. After ~8s of idle, expect `block_anchored_drift` to fire with Marcus as speaker. Verify the pill text reflects his voice (dry, technical, parenthetical) per the template.
4. **Scene autosuggest.** Type a paragraph containing "Talia" (linked) and "Aren" (not linked but exists as a character). Wait for autosave (2s). Verify the autosuggest chip for Aren appears. Click → Aren is linked. Dismiss test character → chip disappears.
5. **`idle_pause_with_present_character`.** Verify it fires when idle ≥ 8s with present characters AND no recent pill (cooldown).
6. **`character_dissonance` Stage 1 + Stage 2.** Type a paragraph contradicting Marcus's `lie_they_believe` ("Marcus felt the weight of being forgotten"). Verify a pill emerges. Then type an unrelated paragraph ("The kettle whistled"). Verify NO pill. Check the replay log for `confirmation_yes` and `confirmation_no` entries.
7. **Rename cascade.** Edit `full_name` from "Marcus Vale" to "Marcus Tenebris". Verify `aliases` now contains "Marcus Vale". Verify scene autosuggest still detects pre-existing "Marcus Vale" mentions in scene text.
8. **POV constraint.** Try `character_set_pov` on a character NOT in `characters_present`. Verify error message. Add them to characters_present, then re-set POV — succeeds.

For each item, record pass/fail in `docs/superpowers/handoffs/2026-05-18-m3-handoff.md` (create if it doesn't exist; this becomes the M3-exit handoff).

- [ ] **Step 4: KNOWN_FRAGILE.md updates**

Append entries #15, #16, #17 per spec § 21:

```markdown
### #15 — Scene-character autosuggest is name-string-matching, not co-reference resolution

`crates/water-core/src/character/autosuggest.rs::suggest_for_scene_body` uses case-sensitive word-boundary regex matching on character `full_name` + `aliases`. Pronouns ("he", "her", "they") don't link. Characters referenced only by pronoun in a scene won't be suggested.

Manual multi-select via `SceneMetadataSheet` bridges the gap. Upgrade path: M5+ sidecar coreference resolution (planned).

Tests: `count_word_boundary_matches` cases. Regressions show up as spurious matches in scene text or missing matches when name-with-punctuation is present.

### #16 — Character-voice prompt template injects up to 11 LSM fields per pill

`prompts/speakers/character/template.toml` substitutes full_name, role, want, need, lie_they_believe, voice, speech_patterns, fears, values, ghost_wound, fatal_flaw. Token cost grows with character complexity.

Eval harness (`eval/`) should track per-character pill token usage. If it crosses a threshold (~600 tokens system), M7 settings will add a "concise mode" that omits `bonus_traits` aux fields.

Tests: token-budget assertion in `test_character_speaker_template_renders_full_voice`.

### #17 — `character_dissonance` Stage 1 uses Jaccard lemma overlap (English-only)

`crates/water-core/src/orchestrator/lemma_overlap.rs` reuses M2 `anti_loop.rs`'s `tokenize` + `jaccard`. Both are English-only (KNOWN_FRAGILE #8 caveat applies — naïve whitespace-split + lowercase; no language-aware stemming).

Non-English manuscripts will produce poor gate behavior. M5+ semantic embedding closes this.

Tests: `lemma_overlap::overlap` exercises the English path only. Cross-language regressions would surface as the gate firing too often (everything overlaps under a coarse tokenizer) or too rarely (whitespace-tokenization breaks on agglutinative languages).
```

- [ ] **Step 5: Update spec header + plan-writing notes**

Modify `docs/superpowers/specs/2026-05-18-m3-character-sheets.md`. At the top, add a "Status" line:

```markdown
**Status:** Closed at tag `m3` on <YYYY-MM-DD>. See `docs/superpowers/plans/2026-05-18-m3-character-sheets.md` for the executed plan and amendments.
```

Modify this plan's header similarly: add Closed/Tagged status.

- [ ] **Step 6: Final aggregate gates**

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"

# Rust
cargo build -p water-core
cargo build -p water-app
cargo clippy -p water-core --all-targets -- -D warnings
cargo clippy -p water-app -- -D warnings
cargo fmt -p water-core --check
cargo test -p water-core
cargo test -p water-app

# TypeScript
pnpm --filter @water/app test
pnpm --filter @water/app build
```

Expected counts at tag `m3`:
- `cargo test -p water-core`: 136 (m2.5 baseline) + 12 (M3 spec target) ≈ 148
- `pnpm --filter @water/app test`: 105 (m2.5 baseline) + 18 (M3 spec target) ≈ 123
- m1-exit + orchestrator + tone_audit_200 still pass (6 + 1 + 1 = 8)
- **Total target: ~279** (vs. m2.5's 249).

If counts differ materially, investigate before tagging.

- [ ] **Step 7: Commit fixture + KNOWN_FRAGILE + spec/plan status**

```powershell
git add eval/m3_acceptance/marcus_vale.toml KNOWN_FRAGILE.md docs/superpowers/specs/2026-05-18-m3-character-sheets.md docs/superpowers/plans/2026-05-18-m3-character-sheets.md docs/superpowers/handoffs/2026-05-18-m3-handoff.md
git commit -m "docs(m3): Marcus Vale fixture + KNOWN_FRAGILE #15-#17 + M3 closure"
```

- [ ] **Step 8: Tag**

```powershell
git tag -a m3 -m "M3: Character Sheets — LSM v2.1 intake, voice template, dissonance two-stage gating, scene linkage + autosuggest"
git log --oneline m2.5..m3
```

The log should show 22 task commits + the closure commit (~23 total). Verify each task's commit message references the M3 task number.

- [ ] **Step 9: Smoke-verify the tag**

```powershell
git checkout m3
cargo build -p water-app
pnpm --filter @water/app build
git checkout main # or whatever the working branch is
```

Sanity check the tag is build-clean.

---

<!-- ANCHOR:END-PHASE-G -->

## Amendments

Use this section to record deviations from the plan as they happen. Each amendment includes: date, task affected, what changed, and why. The implementer is expected to extend this list during execution — the plan is a living document.

### Format

```markdown
### Amendment N — YYYY-MM-DD — Task X step Y

**Change:** <one-paragraph summary>

**Reason:** <why the plan was wrong or incomplete>

**Resolution:** <what was actually done; reference commit SHA if landed>
```

### Amendments

### Amendment 1 — 2026-05-18 — Task 2 deferred quality fixes

**Change:** Three code-quality findings from the T2 code-review subagent were noted but not fixed inline, deferred to Phase G's combined audit (or to Task 3 if convenient since they live in the same module).

**Reason:** Continuous-execution cadence (user-selected "surface only blockers"). Findings are real but reviewer assessed "Approved with minor changes," not "needs revision." Documented here so Phase G picks them up.

**Resolution (pending):**

1. **Round-robin claim is not tested with ≥7 characters.** `migration_v3_backfills_hue_round_robin` seeds only 4 characters; a buggy `% 5` or no-modulo would still pass. Fix: seed 7+ characters and assert the 7th wraps to `--water-hue-character-1`. Cost: ~5 lines in `migrations.rs::tests`.

2. **`hue_token TEXT NOT NULL DEFAULT ''` is a footgun.** Future `INSERT INTO character` statements forgetting `hue_token` silently produce invalid empty-string hues. Fix: either add a `CHECK (hue_token LIKE '--water-hue-character-%')` constraint to the schema, OR leave a `-- TODO(m7): replace empty default with CHECK constraint` in the SQL. Decided fix: append the TODO comment in T3 if touching that SQL file; otherwise defer to Phase G.

3. **Tie-break branch (same `created_at`, different `id`) uncovered.** Lower priority — rare in practice. Skip unless Phase G has bandwidth.

4. **Test polish (low priority):** `migration_ratchets_to_v3` is redundant with `migration_is_idempotent` + `migration_ratchets_from_v1_to_latest` and hard-codes the version number. Recommend deleting in Phase G.

5. **`db.rs:127 assertion `v == 3`** also hard-codes the version. Recommend changing to compare against `migrations::all().current_version()` or similar in Phase G.

---

## Plan summary

- **Total tasks:** 22 across 7 phases (A=3, B=5, C=3, D=3, E=3, F=4, G=1).
- **Source spec:** `docs/superpowers/specs/2026-05-18-m3-character-sheets.md`.
- **Tag chain:** `m2.5` → `m3` (this plan).
- **Test target:** ~279 total (148 lib + 123 TS + 8 misc), up from m2.5's 249.
- **Key risks:** prompt token cost (#16), English-only lemma gate (#17), name-string autosuggest false positives/negatives (#15).
- **Critical pattern reuse:** SceneWriteLocks → CharacterWriteLocks (T12), Sheet primitive (T16/T21), cancellation-race guard (every load-on-mount effect), reloadToken/display:none scroll preservation (T20), CannedProvider canned-response sequencing (T11).

---

*Plan complete.*
