# M3 — Character Sheets Spec

**Status:** Closed at tag `m3` on 2026-05-19. See `docs/superpowers/plans/2026-05-18-m3-character-sheets.md` for the executed plan and amendments 1-18 + final closure.
**Date:** 2026-05-18
**Base:** tag `m2.5` (commit `d13f484`)
**Master spec:** `docs/superpowers/specs/2026-05-16-water-design.md` § 4.5 + § 3.5 + § 6.2
**Handoff:** `docs/superpowers/handoffs/2026-05-18-m3-handoff.md`
**Predecessor:** `docs/superpowers/specs/2026-05-18-m2.5-rich-text.md`

---

## 1. Background

M1 through M2.5 shipped the manuscript engine: a ProseMirror block editor with inline rich-text marks; a deterministic pill orchestrator with 10 triggers, state machine, FIFO eviction, and Jaccard anti-loop; a voice router + 5 persona speakers; a TOML prompt library with structured-JSON LLM routing and three-layer tone enforcement; pill UI with bouquets, rabbit holes, pinned column, and Mod-click-to-open-external; 200-fixture tone audit.

Two character-track trigger stubs (`character_dissonance` and `idle_pause_with_present_character`) returned `None` because M2 had no characters to point at. The voice router's character branch was a placeholder (`let _ = candidate.preferred_track`). The `CharacterRegistry` from M2 T15 was empty by design.

M3 fills these in. Writers create characters via a Conversational Intake popup that walks the LSM v2.1 schema one question at a time. The resulting `characters/<ulid>.toml` files are the on-disk truth; SQLite indexes them. The voice router learns to prefer POV characters when present in a scene, with cooldown-driven LRU fallback. The two trigger stubs ship real implementations. A character voice prompt template renders per character via the LSM sheet fields.

M3 is sized for ~1.5 weeks of work (master spec estimate); the plan is ~22 tasks across 7 phases.

---

## 2. Goals & non-goals

### 2.1 Goals

1. **LSM v2.1 schema descriptors** for Conversational Intake, encoded as Rust constants and exposed to the renderer via `ipc.intakeSchema(kind)`.
2. **Conversational Intake popup** — reusable component that walks any `IntakeField[]` descriptor, writing answers to disk per question (no batch save).
3. **Character sheet view** — inline-editable document view of a character; autosave per field; "Continue intake" CTA when incomplete.
4. **Character index** — soft glow grid of character cards, sortable, searchable.
5. **Scene ↔ character linking** — manual multi-select in scene metadata + advisory autosuggest from name-string scan.
6. **Character voice prompt template** — `prompts/speakers/character/template.toml` rendered per character via the LSM sheet (voice + identity + arc fields).
7. **CharacterRegistry implementation** — populated from the `character` table on project open; produces `CharacterSpeaker` instances usable by the M2 voice router.
8. **Trigger stub fillings:**
   - `idle_pause_with_present_character` — rotates among present characters on idle pauses.
   - `character_dissonance` — two-stage: lemma overlap gate (cheap, sync) + LLM confirmation pass (semantic, async).
9. **Voice router POV-prefer rule** — `default_speaker_for_trigger` returns POV character id when set; cooldown defers to LRU among present characters; finally falls back to persona default.

### 2.2 Non-goals (explicit)

- **World Bible** (M4 — reuses the Conversational Intake component against world segment schemas).
- **Conversational Intake for scene goals** (master spec § 4.5 mentions; deferred to M4 alongside world).
- **Embedding-based semantic similarity** for `character_dissonance` Stage 1 (M5+ when sidecar exposes `/embed`).
- **Character relationship graph / arc visualization** (M5+).
- **Character avatar imagery / file uploads** — placeholder hue chip only.
- **Find-and-replace dialog on rename** — M7 polish if requested.
- **Character voice manual hue override UI** — M7 settings sheet.
- **Co-reference resolution** for autosuggest (pronouns) — M5+ sidecar work.
- **Per-trigger character field selection** — defer to M5+ once eval harness shows which fields correlate with quality voice generation.

---

## 3. Hard constraints (inherited from master spec)

1. **No conversational LLM input.** The Intake popup writes the writer's typed answers to disk; those answers are READ INTO prompts when the character speaks, but the LLM never receives the writer's intake answer as a "message."
2. **Reactive / observational pill tone.** Character voices obey the same three-layer tone enforcement (prompt clause, `PASS` self-rejection, post-hoc regex blacklist).
3. **Universe-first / characters-second-but-above-personas.** Now real: when scene has characters present, voice router prefers them over personas.
4. **Local-first.** No new cloud dependencies.
5. **Determinism on orchestrator side.** `(trigger, scene_state, project_state, character_state)` produces the same prompt assembly + speaker selection. Variation lives only in LLM sampling. (The `character_dissonance` Stage 2 LLM call introduces variance ONLY in the firing decision; the eventual pill prompt assembly is still deterministic given the firing was confirmed.)
6. **Human-readable on disk.** Character TOML is the truth; SQLite `character` row is a rebuildable index. Per-answer writes mean a partial character is just a TOML with fewer fields.
7. **Plugin contract seams.** `CharacterSpeaker` impl alongside `PersonaSpeaker` per M2's `Speaker` trait. Schema descriptors are versioned (LSM v2.1). M4 plugs in world segment schemas against the same `ConversationalIntake` component.
8. **Pastel-glow visual identity.** Intake popup, sheet view, character index all use existing tokens in `app/src/styles/tokens.css`. No new hex codes outside the new character-hue tokens (§ 11).
9. **Notion/Apple-fluid motion.** Intake question transitions glide ~240 ms (`--water-dur-small`). Reduced-motion → 1 ms.

---

## 4. Architecture

```
┌──────────────────────────── water-core (Rust) ─────────────────────────────┐
│  character.rs (M1)              CharacterStore + CharacterFile + Row      │
│  character/intake.rs            NEW: IntakeField type + LSM v2.1 schemas  │
│  character/registry.rs          NEW: CharacterRegistry::from_db impl     │
│  character/autosuggest.rs       NEW: name-string scan for scene linking  │
│  voice/speaker.rs               CharacterSpeaker impl (sibling Persona)   │
│  voice/router.rs                POV-prefer rule (Q3) + char track flow   │
│  orchestrator/triggers/                                                    │
│    character_dissonance.rs      NEW: lemma gate + confirmation flag       │
│    idle_pause_with_present_character.rs  NEW: rotate present characters  │
│  orchestrator/mod.rs            TriggerCandidate.requires_confirmation    │
│  orchestrator/lemma_overlap.rs  NEW: stopword-stripped lemma Jaccard       │
│                                  (reuses anti_loop tokens; new threshold) │
│  sql/v3_character_hue.sql       NEW: hue_token column + backfill          │
└────────────────────────────────────────────────────────────────────────────┘

┌──────────────────────────── app/src-tauri (Tauri) ─────────────────────────┐
│  orchestrator_service.rs                                                   │
│    on_telemetry                 handles requires_confirmation by running  │
│                                  pill_dissonance_check task BEFORE level-0 │
│  commands/character.rs          NEW: character_create / read / list /     │
│                                       update_field / delete / set_pov /   │
│                                       link_to_scene / unlink_from_scene / │
│                                       intake_schema / autosuggest_for_scene │
└────────────────────────────────────────────────────────────────────────────┘

┌──────────────────────────── prompts/ ──────────────────────────────────────┐
│  speakers/character/template.toml      NEW: voice template (§ 10)         │
│  tasks/pill_dissonance_check.toml      NEW: yes/no confirmation task      │
└────────────────────────────────────────────────────────────────────────────┘

┌──────────────────────────── app/src (renderer) ────────────────────────────┐
│  intake/                                                                   │
│    ConversationalIntake.tsx     NEW: reusable schema-walker component     │
│    IntakeField.tsx              NEW: one-question-at-a-time renderer      │
│    types.ts                     NEW: TS mirror of Rust IntakeField        │
│  characters/                                                               │
│    CharacterSheet.tsx           NEW: inline-editable sheet view           │
│    CharacterIndex.tsx           NEW: glow-grid of cards                   │
│    CharacterCard.tsx            NEW: one card in the index                │
│    SceneAutosuggestChips.tsx    NEW: advisory chips in scene metadata     │
│  sheets/                                                                   │
│    CharacterIntakeSheet.tsx     NEW: wraps ConversationalIntake in Sheet  │
│    SceneMetadataSheet.tsx       NEW: scene-level metadata editor +        │
│                                       characters_present + POV selectors  │
│  chrome/                                                                   │
│    IconRail.tsx                 wire Characters icon → CharacterIndex     │
│    ScenesPanel.tsx              row hover reveals "Details" → opens        │
│                                  SceneMetadataSheet                       │
│  ipc/commands.ts                extend with character verbs               │
│  ipc/events.ts                  no new events; character changes are      │
│                                  command-response not push                │
└────────────────────────────────────────────────────────────────────────────┘
```

No new Tauri events. Character mutations happen via IPC commands with synchronous response; renderer refreshes optimistically.

---

## 5. Conversational Intake — descriptor format

Rust-side at `crates/water-core/src/character/intake.rs`:

```rust
#[derive(Debug, Clone, Copy, Serialize)]
pub struct IntakeField {
    pub id: &'static str,             // dotted path, e.g. "main.full_name"
    pub section: &'static str,        // "main" | "bonus_traits" | "arc" | "perspectives"
    pub label: &'static str,          // short title shown in sheet view
    pub prompt_question: &'static str,// shown in Intake popup
    pub helper: Option<&'static str>, // optional explanation under question
    pub examples: &'static [&'static str], // optional pre-fill suggestions
    pub kind: IntakeFieldKind,
    pub optional_skip: bool,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case", tag = "type", content = "options")]
pub enum IntakeFieldKind {
    ShortText,
    LongText,
    StringList,
    Choice(&'static [&'static str]),
}

pub const LSM_MAIN: &[IntakeField] = &[ /* 12 fields */ ];
pub const LSM_BONUS_TRAITS: &[IntakeField] = &[ /* 8 fields */ ];
pub const LSM_ARC: &[IntakeField] = &[ /* 5 fields */ ];
pub const LSM_PERSPECTIVES: &[IntakeField] = &[ /* 4 fields */ ];

pub const LSM_V2_1: &[(&str, &[IntakeField])] = &[
    ("main", LSM_MAIN),
    ("bonus_traits", LSM_BONUS_TRAITS),
    ("arc", LSM_ARC),
    ("perspectives", LSM_PERSPECTIVES),
];
```

Total: 29 fields per character. The Intake popup walks them in order: all `main` first, then `bonus_traits`, then `arc`, then `perspectives`. The writer can stop at any point; later sessions resume at the first unanswered field.

### 5.1 LSM v2.1 field copy (verbatim — prompt_question text)

Drafted by the spec author per § 18.4. Each field's `prompt_question` is below; the implementer transcribes verbatim into the Rust constants. Adjust copy in a single follow-up commit if any reads stiffly during manual smoke.

**Main:**
- `full_name`: "What is this character's full name?" — `ShortText`, examples: `["Marcus Vale", "Ada Thorne"]`
- `aliases`: "What other names is this character known by? (Nicknames, titles, pen names.)" — `StringList`, optional_skip: true
- `age`: "How old are they at the start of the story?" — `ShortText`, examples: `["32", "early 40s", "ageless"]`, optional_skip: true
- `pronouns`: "What pronouns?" — `ShortText`, examples: `["she/her", "they/them", "he/him"]`, optional_skip: true
- `role_in_story`: "What role does this character play in the story?" — `Choice(["protagonist", "antagonist", "supporting", "mentor", "foil", "other"])`
- `want`: "What do they want? What are they consciously pursuing?" — `LongText`
- `need`: "What do they actually need? What would heal them, even if they don't see it?" — `LongText`
- `ghost_wound`: "What past event still haunts them? What unhealed thing shapes who they are today?" — `LongText`, optional_skip: true
- `lie_they_believe`: "What false belief do they hold about themselves or the world? What story do they tell themselves that isn't quite true?" — `LongText`
- `truth`: "What truth would set them free if they could see it?" — `LongText`, optional_skip: true
- `fatal_flaw`: "What character trait will most likely undo them in this story?" — `LongText`, optional_skip: true
- `strength`: "What is their greatest virtue or capacity?" — `LongText`, optional_skip: true

**Bonus traits:**
- `voice`: "How would you describe their voice? (Cadence, register, tone — not what they say but how they sound.)" — `LongText`, examples: `["spare, weather-worn, with quiet warmth", "clipped and precise, like a lawyer"]`
- `tells`: "What do they do without realizing it? (Physical or verbal tells.)" — `StringList`, optional_skip: true
- `habits`: "What recurring small actions or rituals shape their day?" — `StringList`, optional_skip: true
- `speech_patterns`: "What phrases, fillers, or quirks of speech recur in their dialogue?" — `StringList`, optional_skip: true
- `physicality`: "How do they move? How do they hold themselves in a room?" — `LongText`, optional_skip: true
- `preferences`: "Any strong likes, dislikes, or aesthetic preferences? (One per line: `coffee: bitter, no sugar`.)" — `StringList`, optional_skip: true
- `fears`: "What are they most afraid of? (Not phobias — the real fears.)" — `StringList`
- `values`: "What do they hold sacred? What would they refuse to compromise on?" — `StringList`

**Arc:**
- `starting_state`: "Where is this character emotionally / morally / situationally when the story begins?" — `LongText`, optional_skip: true
- `ending_state`: "Where are they by the end?" — `LongText`, optional_skip: true
- `inciting_change`: "What event in the early story knocks them out of equilibrium?" — `LongText`, optional_skip: true
- `midpoint_shift`: "What changes at the midpoint? What do they finally see, or refuse?" — `LongText`, optional_skip: true
- `climax_choice`: "What choice defines them at the climax?" — `LongText`, optional_skip: true

**Perspectives:**
- `self_view`: "How do they see themselves?" — `LongText`, optional_skip: true
- `others_view`: "How do other characters in the story see them?" — `LongText`, optional_skip: true
- `narrator_view`: "How does the narrative voice (whether explicit or implicit) frame them?" — `LongText`, optional_skip: true
- `antagonist_view`: "How would their antagonist describe them?" — `LongText`, optional_skip: true

Only 5 fields are required (no `optional_skip`): `full_name`, `role_in_story`, `want`, `need`, `lie_they_believe`, plus `voice`, `fears`, `values` in bonus_traits — 8 total. A writer can produce a usable character in ~90 seconds by answering only these.

---

## 6. Conversational Intake popup — UX

- **Trigger:** "+ New character" button in CharacterIndex; or "Continue intake (8 fields remaining)" button on an incomplete sheet.
- **Container:** wraps the existing M1.5 `Sheet` primitive (right-edge slide-in, 420 px wide). Slide-in transform inherited from M2 T23.
- **One question at a time:** the current field's `prompt_question` is the headline (Plex Serif, 20px); `helper` text in muted color (Plex Sans, 14px) below; input field sized for `kind`; "Examples:" chips below if any.
- **Navigation:**
  - `Enter` (ShortText / Choice) → advance to next field.
  - `Cmd-Enter` / `Ctrl-Enter` (LongText) → advance.
  - `Shift-Enter` (LongText) → newline.
  - `Esc` → close popup (state preserved per § 7).
  - "← Back" link above input → previous question.
  - "Skip →" link (only if `optional_skip`) → advance without writing.
  - "Examples: spare, weather-worn, with quiet warmth" → click an example to insert it into the input (StringList: appends as a new item).
- **Per-answer write:** every advance triggers `ipc.characterUpdateField(characterId, fieldId, value)` which writes the TOML and re-indexes the row. The current `characterId` is owned by the popup; "+ New character" calls `ipc.characterCreate()` first which returns a fresh `characterId` for an otherwise-empty character.
- **Visual progress:** a small "M.1 / 29" counter top-right of the popup so the writer sees where they are.
- **Transition:** soft 240 ms vertical glide between questions (`--water-dur-small`, `--water-ease-out-soft`). Reduced-motion → 1 ms.
- **Section headers:** each section ("Main", "Bonus traits", "Arc", "Perspectives") shows a one-frame banner with the section title before the first question in that section. Helps the writer pace.

---

## 7. Save-resume behavior (per Q2 decision)

- Each `Enter` / `Skip` / `Back` writes the current state to disk:
  - The character TOML at `characters/<ulid>.toml` is updated atomically.
  - The SQLite `character` row's `data_json` + `updated_at` + `file_hash` are refreshed.
- If the writer closes the popup mid-flow:
  - The character row exists with whatever fields were written so far.
  - Empty fields are simply absent from the TOML (omitted on serialize when value is `null` or the empty string for ShortText/LongText, or the empty array for StringList).
  - The sheet view shows "Continue intake (N fields remaining)" CTA that reopens the popup at the first unanswered required field (then first unanswered optional field).
- A character is "complete" iff every non-`optional_skip` field has a value. Reflected in a small dot indicator in the index card (filled = complete; ring = partial).

---

## 8. Character sheet view

`app/src/characters/CharacterSheet.tsx`:

- Four sections, each a soft glow card (matching the M2 pill capsule visual): "Main", "Bonus traits", "Arc", "Perspectives".
- Each field renders as `<label>: <value>` with the label in Plex Sans Small (12px, muted) and the value in Plex Serif Body (16px).
- Click a value → it becomes an `<input>` (ShortText) or `<textarea>` (LongText) or comma-separated text field (StringList) or `<select>` (Choice). Auto-focus + select.
- Blur or `Enter` (short) / `Cmd-Enter` (long) → save via `ipc.characterUpdateField(id, fieldId, value)`. Same write path as Intake.
- "saved at HH:MM" chip at top-right (same pattern as M1.5 EditorCanvas).
- "Continue intake" CTA at the top of the sheet if any required fields are empty.
- Right-rail: the character's hue chip (16px circle, `var(<hue_token>)` background, soft glow). Also their full_name (for header).
- Bottom: "Delete character" button (small, muted) → confirmation dialog → `ipc.characterDelete(id)`.

The sheet view is reached by clicking a CharacterCard in the index, or by clicking a character's name in a future scene metadata sheet.

---

## 9. Character index

`app/src/characters/CharacterIndex.tsx`:

- Soft glow grid of `CharacterCard` tiles. CSS Grid, `auto-fill` with min 240px column width.
- Each `CharacterCard`:
  - 16px hue chip (top-left).
  - Full name (Plex Serif, 18px).
  - Role badge (Plex Sans Small, 12px, muted: "protagonist", "antagonist", etc.).
  - Want teaser: first 12 words of `main.want`, ellipsis if longer (Plex Sans 14px, 2 lines max).
  - Completion dot (top-right): filled = all required fields present, ring = partial.
  - Click → opens sheet view.
- "+ New character" button at top-left of the grid, styled as a tile with a `Plus` icon.
- Top toolbar:
  - Sort: by name (alphabetical, default) | by recently edited.
  - Search: text input. Substring case-insensitive match against `main.full_name` + `main.aliases`.
- Empty state: when no characters exist, render a soft "No characters yet. Click + to create one." message instead of an empty grid.

Reached from the IconRail (M1.5) by clicking the Characters icon — which currently no-ops; M3 wires it.

---

## 10. Character voice prompt template

`prompts/speakers/character/template.toml`:

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

Substitutions (Q6 — full voice + arc context):
- `{{full_name}}` ← `main.full_name`
- `{{role_descriptor}}` ← derived: "You are the protagonist of this story." / "You are an antagonist." / etc.
- `{{want}}` ← `main.want`
- `{{need}}` ← `main.need`
- `{{lie_they_believe}}` ← `main.lie_they_believe`
- `{{voice}}` ← `bonus_traits.voice`
- `{{speech_patterns}}` ← `bonus_traits.speech_patterns` joined with ", "
- `{{fears}}` ← `bonus_traits.fears` joined with ", "
- `{{values}}` ← `bonus_traits.values` joined with ", "
- `{{ghost_wound}}` ← `main.ghost_wound`
- `{{fatal_flaw}}` ← `main.fatal_flaw`

**Missing-field policy.** If any substitution source is empty / absent, the entire sentence containing that placeholder is omitted from the rendered prompt. So a character with only `full_name` and `voice` filled gets a shorter, still-coherent prompt; no "Your fears: ." artifacts.

Token cost estimate (all fields populated): 120–200 tokens. Target ~150. Well within the M2 § 6.3 600-token total budget for a level-0 pill.

---

## 11. Character hue palette + schema v3

Add to `app/src/styles/tokens.css`:

```css
--water-hue-character-1: #c5d4e8;  /* soft periwinkle */
--water-hue-character-2: #e8c5d4;  /* soft rose */
--water-hue-character-3: #d4e8c5;  /* soft sage */
--water-hue-character-4: #e8d4c5;  /* soft sand */
--water-hue-character-5: #c5e8d4;  /* soft mint */
--water-hue-character-6: #d4c5e8;  /* soft lilac */
```

(Final palette values pinned in M7. Placeholders chosen to be distinct from the existing persona hues.)

Schema migration v3 at `crates/water-core/sql/v3_character_hue.sql`:

```sql
ALTER TABLE character ADD COLUMN hue_token TEXT NOT NULL DEFAULT '';

-- Backfill: round-robin assign hues to existing characters by created_at order.
-- M1 only allowed character rows via CharacterStore; rows exist but hue_token is empty.
UPDATE character SET hue_token = (
  SELECT '--water-hue-character-' || (((rownum - 1) % 6) + 1)
  FROM (
    SELECT id, ROW_NUMBER() OVER (ORDER BY created_at) AS rownum
    FROM character AS c
    WHERE c.id = character.id
  )
);

UPDATE schema_version SET version = 3;
```

(SQLite SQL syntax; the implementer should adapt the window function or fall back to a procedural UPDATE if the SQLite version in `rusqlite` doesn't support `ROW_NUMBER()` — it does in 3.25+.)

New characters created post-v3 auto-assign hue via `(SELECT COUNT(*) FROM character) % 6 + 1` at `CharacterStore::insert` time.

Manual override: not in M3 (M7 settings).

---

## 12. `character_dissonance` two-stage design

### 12.1 Stage 1 — lemma overlap gate (sync, in trigger evaluate)

`crates/water-core/src/orchestrator/triggers/character_dissonance.rs::evaluate`:

```rust
fn evaluate(&self, ctx: &TriggerContext<'_>) -> Option<TriggerCandidate> {
    if ctx.telemetry.cursor_classification == CursorClassification::MidSentence {
        return None;
    }
    // Pull the just-finished paragraph's text from analysis snapshot.
    let paragraph_text = ctx.analysis.last_block_text.as_deref()?;
    // Iterate present characters; for each, check overlap against fields.
    for char_id in &ctx.scene.characters_present {
        let character = ctx.project.character_by_id(char_id)?;
        for field_source in [
            character.values_joined(),
            character.fears_joined(),
            character.lie_they_believe().to_string(),
        ] {
            let overlap = jaccard(&tokenize(&paragraph_text), &tokenize(&field_source));
            if overlap >= 0.30 {
                return Some(TriggerCandidate {
                    trigger_id: self.id(),
                    priority: 5.5,
                    preferred_track: SpeakerTrack::Character,
                    reason: format!("dissonance_gate char={} field=<...> overlap={overlap:.2}"),
                    block_target_id: Some(ctx.telemetry.block_id.clone()),
                    requires_confirmation: Some(ConfirmationRequest {
                        task_id: "pill_dissonance_check",
                        character_id: char_id.clone(),
                        field_source,
                    }),
                });
            }
        }
    }
    None
}
```

`jaccard` and `tokenize` reused from `crates/water-core/src/orchestrator/anti_loop.rs`. The 0.30 threshold is below anti-loop's 0.70 — dissonance is "any meaningful overlap"; anti-loop is "too much overlap."

`ctx.analysis.last_block_text` is a new field on `AnalysisSnapshot` (the sidecar adds it when it identifies the just-finished block; until M5 sidecar adds it, the orchestrator can derive it from `scene_text + block_id` — but the simpler M3 path is to have the renderer include the block text in the `typing:telemetry` payload when `idle_for_ms >= 3000`).

`ctx.project.character_by_id` is a new helper on `ProjectSnapshot` — the orchestrator gains a reference to the populated `CharacterRegistry`, which lookup-by-id.

### 12.2 Stage 2 — LLM confirmation (async, in orchestrator service)

`TriggerCandidate.requires_confirmation: Option<ConfirmationRequest>` is added in M3. When `on_telemetry` picks a candidate with `requires_confirmation = Some(req)`:

1. Build a `pill_dissonance_check` prompt request: small system+user pair asking yes/no.
2. Call `router.generate_raw_with_default(req.system, req.user)` (reused from M2 T26).
3. Parse the response: trim, lowercase, check for `^yes` or `^y\b`. Anything else → drop the candidate.
4. If yes: proceed with normal level-0 pill generation using the character speaker + trigger framing.

Prompt at `prompts/tasks/pill_dissonance_check.toml`:

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

Token cost: ~100–150 tokens in (paragraph + character context), 1 token out. Cheap.

### 12.3 Failure modes

- LLM call times out / errors → drop the candidate (same as level-0 pill error path).
- LLM responds with malformed text (not "yes" / "no") → treat as "no", drop.
- LLM responds "yes" but the eventual level-0 pill fails PostFilter → drop, log to replay log.

---

## 13. Trigger stub fillings — `idle_pause_with_present_character`

`crates/water-core/src/orchestrator/triggers/idle_pause_with_present_character.rs`:

```rust
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
```

The voice router (per § 14) picks among present characters using POV + cooldown LRU. So even though the trigger doesn't specify a particular character, the router's deterministic selection produces one.

---

## 14. Voice router — POV-prefer rule

Modify `crates/water-core/src/voice/router.rs::default_speaker_for_trigger`. Today the function returns `&'static str` (a persona id). M3 extends it to return a richer choice:

```rust
pub enum DefaultSpeaker {
    Character(CharacterId),
    Persona(&'static str),
}

pub fn default_speaker_for_trigger(
    trigger_id: &str,
    scene: &SceneSnapshot,
    char_registry: &CharacterRegistry,
) -> DefaultSpeaker {
    // Character-track triggers prefer POV character when scene has one.
    const CHAR_TRACK_TRIGGERS: &[&str] = &[
        "block_anchored_drift",
        "topic_drift",
        "valence_spike",
        "idle_pause_with_present_character",
        "character_dissonance",
    ];
    if CHAR_TRACK_TRIGGERS.contains(&trigger_id) {
        if let Some(pov_id) = scene.pov_character_id.as_ref() {
            if scene.characters_present.contains(pov_id) && char_registry.by_id(pov_id).is_some() {
                return DefaultSpeaker::Character(pov_id.clone());
            }
        }
        // Falls through to LRU among present characters when no POV is set.
        if let Some(c) = char_registry.pick_lru_present(&scene.characters_present, /*cooldowns*/) {
            return DefaultSpeaker::Character(c);
        }
    }
    // Persona fallback (existing M2 behavior).
    DefaultSpeaker::Persona(persona_for_trigger(trigger_id))
}
```

The `route()` function then takes the `DefaultSpeaker` and resolves it via the appropriate registry (character or persona).

Cooldown-defer-to-LRU: existing M2 machinery applies to both character and persona speakers via shared `CooldownState`. No new logic needed.

---

## 15. Scene-character autosuggest

`crates/water-core/src/character/autosuggest.rs`:

```rust
pub struct AutosuggestResult {
    pub character_id: Id,
    pub mention_count: u32,
}

pub fn suggest_for_scene_body(
    body_text: &str,
    characters: &[CharacterRow],
) -> Vec<AutosuggestResult> {
    let mut results: Vec<AutosuggestResult> = Vec::new();
    for character in characters {
        // Match \b<full_name>\b case-sensitive.
        let mut count = count_word_boundary_matches(body_text, &character.full_name);
        // Plus each alias.
        for alias in &character.aliases {
            count += count_word_boundary_matches(body_text, alias);
        }
        if count > 0 {
            results.push(AutosuggestResult {
                character_id: character.id.clone(),
                mention_count: count,
            });
        }
    }
    results.sort_by(|a, b| b.mention_count.cmp(&a.mention_count));
    results.truncate(5);
    results
}

fn count_word_boundary_matches(haystack: &str, needle: &str) -> u32 {
    // Construct a regex \b<escaped-needle>\b; count matches.
}
```

Exposed via `ipc.characterAutosuggestForScene(sceneId, bodyText)` which returns `AutosuggestResult[]`. Renderer's `SceneAutosuggestChips` shows up to 5 chips: "Suggested present: Marcus (×4), Talia (×2)". Click → `ipc.characterLinkToScene(sceneId, characterId)`. Dismiss → no action; chip disappears for this session (re-fetched on scene re-open).

Trigger: scene body autosave (every 2s debounced; reuses EditorCanvas's existing autosave). After save → call `characterAutosuggestForScene(sceneId, bodyText)`; update chips.

---

## 16. Tauri command surface

New commands in `app/src-tauri/src/commands/character.rs`:

- `character_create(state) -> CharacterRow` — creates empty character with auto-assigned hue, returns id + assigned hue.
- `character_read(state, character_id) -> CharacterFile` — full TOML.
- `character_list(state) -> Vec<CharacterRow>` — index data only (id, full_name, role, hue, completion).
- `character_update_field(state, character_id, field_id, value: Value) -> CharacterRow` — writes one LSM field via dotted path; re-indexes.
- `character_delete(state, character_id) -> ()` — deletes TOML + row + cascades scene_character_presence + nulls scene.pov_character_id.
- `character_link_to_scene(state, scene_id, character_id) -> ()` — adds to scene_character_presence.
- `character_unlink_from_scene(state, scene_id, character_id) -> ()` — removes.
- `character_set_pov(state, scene_id, character_id: Option<Id>) -> ()` — sets / clears scene.pov_character_id (character must be in characters_present).
- `intake_schema(state, schema_id: "lsm-v2.1") -> Vec<(section, Vec<IntakeField>)>` — returns the LSM v2.1 schema as JSON.
- `character_autosuggest_for_scene(state, scene_id, body_text) -> Vec<AutosuggestResult>` — per § 15.

All commands acquire scene write-lock (M2 T2) where they touch scene metadata. Character writes use a per-character write-lock (new — `CharacterWriteLocks` paralleling `SceneWriteLocks`) to prevent torn writes during rapid Intake key-presses.

---

## 17. Phase / build order

**Total: 22 tasks across 7 phases.**

### Phase A — Schema + storage (3 tasks)

- **A1.** `IntakeField` Rust type + LSM v2.1 schema constants (`character/intake.rs`). One test asserting `LSM_V2_1` contains 29 fields and required-field count is 8.
- **A2.** Schema v3 migration — `hue_token` column + backfill SQL + migration test.
- **A3.** `CharacterRegistry::from_db` implementation — populates from `character` table; `by_id` + `pick_lru_present`. Tests with synthetic character rows.

### Phase B — Character voice + trigger fills (5 tasks)

- **B1.** `CharacterSpeaker` impl (sibling of `PersonaSpeaker`). Tests for display_name, hue_token, cooldown defaults.
- **B2.** Character voice template TOML + substitution helper at `prompts/loader::render_character_template(file)`. Tests verifying missing-field policy.
- **B3.** Voice router POV-prefer rule (§ 14). `DefaultSpeaker` enum + extended `default_speaker_for_trigger`. Tests for: POV present → character; POV absent but present chars → LRU; no chars → persona.
- **B4.** Fill `idle_pause_with_present_character` per § 13. Tests for: fires on idle+chars; doesn't fire mid-sentence; doesn't fire when no chars.
- **B5.** Fill `character_dissonance` Stage 1 (lemma gate). Tests for: fires above threshold; doesn't fire below; respects mid-sentence gate.

### Phase C — Orchestrator confirmation-pass extension (3 tasks)

- **C1.** Add `requires_confirmation: Option<ConfirmationRequest>` to `TriggerCandidate` + the `ConfirmationRequest` struct. Tests verifying serialization + backward compat with M2 candidates (default to `None`).
- **C2.** `pill_dissonance_check` task TOML + parser glue.
- **C3.** `OrchestratorService::on_telemetry` dispatches confirmation before level-0. Tests with `CannedProvider::with_response("yes")` proceeding; `with_response("no")` dropping.

### Phase D — Tauri commands + autosuggest (3 tasks)

- **D1.** Character CRUD commands per § 16: create / read / list / update_field / delete. Tests use existing `tempfile` pattern.
- **D2.** Scene linkage commands: link / unlink / set_pov.
- **D3.** `intake_schema` + `character_autosuggest_for_scene` commands. Tests for autosuggest correctness (word-boundary, aliases, count ranking).

### Phase E — Renderer: Intake (3 tasks)

- **E1.** `ConversationalIntake.tsx` reusable component — takes `schema: IntakeSchema` prop, renders one field at a time, calls `onAnswer(fieldId, value)` per advance. Component tests: render question, advance on Enter, skip on optional.
- **E2.** `CharacterIntakeSheet.tsx` — wraps `ConversationalIntake` in the M1.5 Sheet primitive; loads LSM v2.1 schema via `ipc.intakeSchema`; calls `ipc.characterUpdateField` per advance. Tests for the open/close lifecycle.
- **E3.** "+ New character" + "Continue intake" entry points wired in CharacterIndex + CharacterSheet.

### Phase F — Renderer: Sheet + Index + scene linking (4 tasks)

- **F1.** `CharacterSheet.tsx` — inline-editable sheet view per § 8. Tests for field-edit save + autosave chip.
- **F2.** `CharacterIndex.tsx` + `CharacterCard.tsx` — grid + sort + search per § 9. Tests for empty state, sort toggle, search filter.
- **F3.** Wire IconRail's Characters icon to mount CharacterIndex; tear down EditorCanvas when on Characters surface (preserve scroll on return via reloadToken pattern from M2 T24).
- **F4.** `SceneMetadataSheet.tsx` + `SceneAutosuggestChips.tsx` — scene metadata edit (characters_present multi-select + POV single-select + autosuggest chips). Wire ScenesPanel row-hover "Details" affordance.

### Phase G — Audit + tag (1 task)

- **G1.** Final review (dispatch `superpowers:requesting-code-review` over `m2.5..HEAD`); manual smoke against the Marcus Vale fixture; KNOWN_FRAGILE updates; tag `m3`.

---

## 18. Exit criteria

1. Create a new character via "+ New character" → walk full LSM v2.1 intake → result matches the schema (29 fields, 8 required). Time to complete ≤ 3 minutes for a writer with the reference Marcus Vale fixture answers prepared.
2. Edit any field via the sheet view; `characters/<ulid>.toml` reflects the change within 2 seconds (autosave debounce).
3. Open the `.toml` in any text editor; it is human-readable and matches the LSM v2.1 schema shape from master spec § 3.5.
4. Add character to a scene's `characters_present`; set them as POV; trigger `block_anchored_drift` on a paragraph → pill emerges with the character as speaker, voice profile reflects the LSM fields per § 10.
5. Scene autosuggest detects character name mentions in scene body and surfaces them as chips. Click to confirm → character is linked. Dismiss → chip disappears.
6. `idle_pause_with_present_character` fires when configured (idle 8s+ with present characters + no recent pill).
7. `character_dissonance` Stage 1 + Stage 2 fires correctly in the manual smoke (planted paragraph that contradicts a character's stated fear → pill emerges; planted paragraph unrelated → no pill).
8. KNOWN_FRAGILE updated with new entries.

Plus all gates:
- `cargo test -p water-core` → all pass (target +12 from m2.5).
- `pnpm --filter @water/app test` → all pass (target +18 from m2.5).
- `cargo clippy -p water-core --all-targets -- -D warnings` clean.
- `cargo clippy -p water-app -- -D warnings` clean.
- `cargo fmt -p water-core --check` clean.
- `cargo build -p water-app` clean.
- `pnpm --filter @water/app build` clean.

---

## 19. Risks & mitigations

| # | Risk | Mitigation |
|---|---|---|
| 1 | `character_dissonance` confirmation prompt doubles per-trigger LLM cost | Cheap prompt (~150 in / 1 out); fires only when lemma gate ≥ 0.30; cooldown prevents storm |
| 2 | Intake popup interrupts flow if accidentally opened | Sheet has Esc-to-close; per-answer writes mean no destructive interruption |
| 3 | Scene autosuggest false positives on common names | Word-boundary matching, case-sensitive, advisory chips only; manual multi-select primary |
| 4 | LSM v2.1 prompt-question copy reads stiff in practice | All copy lives in Rust constants; iterate via one follow-up commit if manual smoke surfaces awkward phrasing |
| 5 | Character hue palette runs out at >6 characters | Round-robin recycling acceptable for v1; M7 settings will allow manual override |
| 6 | Rename cascade: writer renames character but old name still appears in scene text | Auto-add old name to `aliases` on rename (§ 18.1 below); autosuggest still detects pre-existing mentions |
| 7 | `requires_confirmation` field on `TriggerCandidate` may break M2 deserialization of replay logs | New field is `Option<_>` with `#[serde(default)]`; M2 replay-log entries lack it; deserialization defaults to `None`. |
| 8 | Character voice prompt template + arc fields produce overly long prompts | Missing-field policy (§ 10); fields are joined comma-separated to keep token cost bounded; M7 settings can let writer choose "concise" vs "full" mode |

---

## 20. Open detail decisions (recommendations; flag if you want different)

- **Rename cascade:** when writer renames a character (changes `main.full_name`), Water adds the old name to `main.aliases` automatically so autosuggest still detects pre-existing scene-text mentions. No find-and-replace dialog in M3.
- **Sheet edit interaction model:** inline edit per field (click → input → blur saves). NOT a modal/Sheet for each edit. NOT "re-enter intake" for editing.
- **Intake popup placement:** existing M1.5 `Sheet` primitive (right-edge slide-in). Reuses the slide-in transform fix from M2 T23.
- **Sample character for benchmark:** "Marcus Vale" with full LSM v2.1 fields, committed to `eval/m3_acceptance/marcus_vale.toml` as the reference for the "walk full intake in ≤ 3 minutes" exit criterion. Used by Phase G's manual smoke.
- **POV character constraint:** the POV character must be in `characters_present`. If a writer removes a character from `characters_present` while they are still POV, the POV is auto-cleared.
- **Character deletion:** cascades to `scene_character_presence` (rows removed) and `scene.pov_character_id` (nulled). The character `.toml` is moved to `characters/.trash/<ulid>-<timestamp>.toml` rather than hard-deleted, so the writer can recover by hand within the session. (M7 settings will add a "permanently delete" affordance.)

---

## 21. Expected new KNOWN_FRAGILE entries

- **#15** — Scene-character autosuggest is name-string-matching, not co-reference resolution. Pronouns ("he", "her") don't link; characters referenced only by pronoun in a scene won't be suggested. Manual multi-select bridges the gap. Upgrade path: M5 sidecar co-reference resolution.
- **#16** — Character-voice prompt template injects up to 11 LSM fields per pill. Token cost grows with character complexity. The eval harness (M1.5, ongoing) should track per-character pill token usage; if it crosses a threshold, omit `bonus_traits` aux fields ("concise mode") via M7 settings.
- **#17** — `character_dissonance` Stage 1 uses Jaccard lemma overlap (English-only, KNOWN_FRAGILE #8 caveat applies). Non-English manuscripts will produce poor gate behavior. M5+ semantic embedding closes this.

---

## 22. Plan-writing notes

This spec produces a ~22-task plan across 7 phases:
- Phase A: Rust schema + storage (3 tasks)
- Phase B: Rust voice + triggers (5 tasks)
- Phase C: Rust orchestrator extension (3 tasks)
- Phase D: Tauri commands (3 tasks)
- Phase E: Renderer Intake (3 tasks)
- Phase F: Renderer Sheet + Index + linking (4 tasks)
- Phase G: Audit + tag (1 task)

The implementer can crib heavily from existing M2 patterns:
- M2 T2's `SceneWriteLocks` → new `CharacterWriteLocks` parallels exactly.
- M2 T15's `PersonaRegistry::from_db` → new `CharacterRegistry::from_db` mirror.
- M2 T17's prompt library loader extends with one new task TOML + one new template TOML.
- M2 T19-T22 React patterns (event subscriptions, portal mounts, cancellation race fix) directly apply.
- M2 T23's Sheet primitive is the Intake popup's container.
- M2 T26's orchestrator service `on_telemetry` extends with the confirmation dispatch.

The Marcus Vale reference fixture should be committed as part of Phase G; the spec author drafts the content based on the LSM v2.1 schema and field-question copy in § 5.1.

---

*Spec ends. Implementation plan to follow.*
