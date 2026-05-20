# Known Fragile Heuristics

A living catalogue of intentionally-shallow heuristics in Water. If a feature listed here is misbehaving, **start your investigation here** — these are the most likely culprits.

Each entry has:

- **What it is** — a one-line description.
- **Where it lives** — pointers into the spec and (later) code paths.
- **Why it's fragile** — the reason we accepted the fragility.
- **What success looks like** — when it's working as intended.
- **First-look mitigations** — quick checks before a deeper rewrite.

---

## 1. `character_dissonance` trigger — lemma-overlap heuristic

**What it is.** A pill trigger that fires when the most recent paragraph contains language that contradicts a present character's stated `values`, `fears`, or `lie_they_believe` field on their LSM v2.1 sheet. The check is a shallow lemma-overlap between the paragraph and those character-sheet fields.

**Where it lives.**

- Design spec: `docs/superpowers/specs/2026-05-16-water-design.md` § 6.1 (trigger taxonomy), § 7.1 R3.
- (Future) Rust code: `crates/water-core/src/orchestrator/triggers/character_dissonance.rs`.
- (Future) Eval task family: `eval/tasks/character_dissonance/*.json`.

**Why it's fragile.**

- Lemma overlap is a bag-of-words approximation. It cannot detect semantic contradictions phrased without keyword overlap ("she walked away from the help she'd asked for" contradicts `values = [honesty, kept_promises]` without sharing any lemmas).
- It produces false positives when a character is *internally negotiating* a value rather than violating it (a meaningful narrative moment, not dissonance).
- Sensitivity is hard to tune across genres and registers.

**What success looks like.**

- Fires when a character with a clear `lie_they_believe` acts *as if the lie were true* in a moment where the writer's text shows it's costing them.
- Does not fire when a character is simply experiencing tension or change.
- Eval-harness scorecard `character_dissonance.precision` and `.recall` both > 0.7.

**First-look mitigations.**

1. Check the per-character `dissonance_sensitivity` slider in settings — a single user may have it too high.
2. Inspect `replay logs` (settings → diagnostics → enable replay) for the offending trigger; the log records the paragraph text, character sheet snapshot, and the lemma-overlap score.
3. Compare the firing paragraph to the character's `lie_they_believe`, `values`, `fears` fields directly — if they don't *feel* related, the heuristic is doing what it does.
4. If the false-positive rate is high in the field, the next step is to replace the lemma-overlap with a cheap embedding-similarity check (MiniLM cosine between paragraph and sheet excerpts), or to gate the trigger behind a secondary LLM confirmation pass.

---

## 2. Block-id duplicate tolerance

**What it is.** `ensure_block_ids` (in `crates/water-core/src/block.rs`) does not
de-duplicate colliding `^bk-XXXX` tokens within a single scene body. If the
writer manually edits the file and creates two blocks with the same id, both
ids are preserved.

**Where it lives.** `crates/water-core/src/block.rs::ensure_block_ids`.

**Why it's fragile.** Pill anchoring uses the snippet (Section 3.3 of the spec)
as canonical, so duplicate ids do not actually break pill resolution. But
duplicate ids would confuse a future block-id-keyed feature (e.g., paragraph-
level metric pinning).

**What success looks like.** No false positives reported by writers; the
duplicate case is rare because Water never *introduces* duplicates.

**First-look mitigations.**
1. If duplicates cause downstream confusion, upgrade `ensure_block_ids` to
   rename the second occurrence on next save.
2. Add a one-shot repair option to renumber all duplicates.

---

## 3. `serde_yaml` is unmaintained

**What it is.** `crates/water-core/Cargo.toml` depends on `serde_yaml` for scene
frontmatter parsing. The crate has been advisory-flagged as unmaintained
(RUSTSEC-2024-0320) since mid-2024.

**Where it lives.** `crates/water-core/src/scene_md.rs` (YAML frontmatter
serialize/deserialize). No other module uses YAML.

**Why it's fragile.** No active maintainer means no security fixes. Surface is
small (the deserialization input is trusted writer-authored TOML+YAML in their
own project folder), so exposure is low.

**What success looks like.** Migrating to a maintained fork (`serde_norway` or
`serde_yml`) without changing the on-disk frontmatter format. Round-trip tests
should pass before and after.

**First-look mitigations.** None needed for M1. Plan the migration for M2 if
the lint trips a downstream consumer.

---

## 4. `Secrets::load` warns-but-doesn't-fail on malformed dev-keys.toml

**What it is.** After M1.1 fix A4, a parse failure in `~/.water/dev-keys.toml`
emits a `tracing::warn!` and falls back to an empty map. The user gets a
downstream "no secret for provider" rather than a hard parse error.

**Where it lives.** `crates/water-core/src/llm/secrets.rs::Secrets::load`.

**Why it's fragile.** A user with a typo gets a confusing two-step diagnostic:
first the warning (often missed in console output), then the downstream
NotFound. We prefer "don't crash on a typo" to "crash on every minor issue"
but the friction is real.

**What success looks like.** Tracing subscriber that surfaces warnings to the
user via a Diagnostics page toast in M2+. For now, manual log inspection.

**First-look mitigations.**
1. Document the parse-warn behavior in the developer setup notes.
2. Add a "View logs" Tauri command in M1.x or M2 to expose tracing output to
   the renderer.

---

## 5. IPC schema drift between Rust `water_core::ipc` and Python sidecar

**What it is.** The Pydantic models in `sidecar/src/water_sidecar/routes/analyze.py`
and the serde structs in `crates/water-core/src/ipc.rs` are kept in sync by
hand. The only tripwire is a single hand-pinned JSON literal in
`ipc.rs::tests::analyze_response_matches_sidecar_schema`.

**Where it lives.** `crates/water-core/src/ipc.rs` (Rust side),
`sidecar/src/water_sidecar/routes/analyze.py` (Python side).

**Why it's fragile.** A new field added to one side and forgotten on the other
will silently drop on deserialization. A renamed field will fail loudly the
first time the renderer requests it.

**What success looks like.** M2 adds a CI step that posts a known-bad payload
to the running sidecar and asserts both ends reject it. Eventually a generated
schema (`ts-rs` or similar) so the contract has one source of truth.

**First-look mitigations.**
1. Keep the test fixture in `ipc.rs::tests` exhaustive (every field, every
   variant of `status`).
2. When changing the contract on one side, search-and-update the other before
   committing.

---

## 6. `SnapshotScheduler` and `SidecarSupervisor` are stop-on-close-only

**What it is.** After M1.1 wires both subsystems into `open_project`, neither
auto-restarts on crash. `SnapshotScheduler`'s loop has no failure mode that
warrants restart (it's tokio-only). `SidecarSupervisor`'s 3-consecutive-failure
arm sends a final Error event and breaks the loop — no respawn.

**Where it lives.** `crates/water-core/src/sidecar_supervisor.rs` (3-strike
break at line ~85). `app/src-tauri/src/commands/project.rs::boot_sidecar_for_project`
(no restart logic).

**Why it's fragile.** If the sidecar crashes (panics, OOM, killed by user)
the user sees `sidecar:status: error` and the diagnostics page shows `error`
until they close + reopen the project. No automatic recovery.

**What success looks like.** M2 adds respawn-with-backoff to the supervisor,
or wraps the boot helper in a higher-level loop that retries on the Error
final event.

**First-look mitigations.**
1. The user can `close` + `open` to manually respawn.
2. The diagnostics page surfaces the error detail (T10 added
   `last_status_detail`).

**Resolved in M2 Task 3:** Replaced 3-strike-break with respawn-with-backoff
(1s, 2s, 5s, 10s, 30s, 30s, ...; reset on success). Supervisor never gives
up. `sidecar:status` Tauri events fire on every state change.

---

## 7. `SceneStore::rename` / `write_body` whole-file write race

**What it is.** Both `SceneStore::rename` and `SceneStore::write_body` perform
a read → mutate frontmatter / body → write the whole scene file under no
per-file lock. If a user blurs the title input while the body autosave timer
is mid-flush — both operations happening within the same ~50 ms window —
whichever finishes second clobbers the other's frontmatter mutation, because
each method reads, mutates a different field, and writes back the entire
serialized `SceneFile`.

**Where it lives.**

- `crates/water-core/src/scene.rs` — `SceneStore::rename` (~line 167) and
  `SceneStore::write_body` (the older method, same file).
- M1.5 spec § 5 R4 explicitly accepts this as M1-acceptable.

**Why it's fragile.** No per-scene mutex. The race window is small in
practice (autosave debounce is 2 s; title blur is event-driven) but it
exists. Last-writer-wins on the WHOLE file means the loser's mutation is
silently dropped, not merged.

**What success looks like.** The user's title edit and body edit both
persist after a blur-then-keep-typing flow. Currently true unless both
writes interleave their read/write windows.

**First-look mitigations.**

1. If a "scene title is wrong" or "body lost a sentence" bug surfaces, check
   `manuscript/scenes/<ulid>.md` against `project.db`'s scene row directly.
   Out-of-sync frontmatter is the smoking gun.
2. M1.5's `EditorCanvas` mitigates the most common race window by flushing
   the title rename on unmount and the body write on the same unmount (see
   `app/src/chrome/EditorCanvas.tsx` cleanup functions). This narrows the
   user-visible race surface but does not eliminate it.
3. M2 plan: introduce a `SceneWriteLock` (per-scene `tokio::Mutex<()>`) that
   `rename` and `write_body` both acquire before touching disk. Drop the
   whole-file rewrite in favor of frontmatter-only rewrites where possible.

**Resolved in M2 Task 2 (commit pending):** Per-scene `SceneWriteLocks` registry on `OpenProject`. Both `rename` and `write_body` acquire the lock before disk I/O. Concurrent 50-iteration property test in `scene.rs::tests` proves serialization.

**Scope note:** `SceneStore::move_to` follows the same read-modify-write
pattern and is NOT yet gated. Drag-reorder + body autosave concurrent on
the same scene remains a theoretical race (low-likelihood in practice
because move_to is user-initiated, autosave is timer-initiated, and the
overlap window is small). A FIXME in `crates/water-core/src/scene.rs`
points at this gap.

---

## 8. Anti-loop Jaccard suffix-stripper is English-only

**What it is.** Anti-loop overlap normalizes tokens via a fixed `-s/-es/-ed/-ing/-ly` suffix-stripper. Non-English words don't share these suffixes; their stems collapse incorrectly (e.g., German `geben` and `gibt` won't collide). Spec § 17.

**Where it lives.** `crates/water-core/src/orchestrator/anti_loop.rs::strip_suffix`.

**Why it's fragile.** v1 ships English-only manuscripts. Non-English is M7+.

**First-look mitigations.** Inspect replay logs (`WATER_REPLAY_LOG=1`) for the affected scene; check anti-loop overlap distribution per speaker.

---

## 9. Pill block-anchor stability under decoration churn

**What it is.** When `pill:emerged` fires while the user is mid-edit at the anchored block, the editor's transaction filter may re-apply decoration after a partial write. Snippet-as-canonical (master spec § 3.3) is the fallback.

**Where it lives.** `app/src/pill/PillLayer.tsx` (anchor rect computation); `app/src/editor/blockIdPlugin.ts` (block-id stability).

**Why it's fragile.** Selection + decoration coexistence is hard. Race window is small but visible during rapid typing.

**First-look mitigations.** Check `bouquet.block_target_id` against the live scene's current block IDs; if mismatched, the snippet still resolves via text-content scan.

---

## 10. Structural-inflection detection is shallow

**What it is.** The editor emits `new_scene` / `new_chapter` on block insert (cheap and accurate, user-initiated). `pov_change` / `location_change` are stubbed in M2 — sidecar heuristics are M3+. The current `descendants` scan in `Editor.tsx` is O(N blocks) per transaction.

**Where it lives.** `app/src/editor/Editor.tsx` (inflection detection); `crates/water-core/src/orchestrator/triggers/structural_inflection.rs` (priority scaling).

**Why it's fragile.** Heuristic; nightly tone-audit scorecard will track leak rate once M3 wires real pov/location detection.

**First-look mitigations.** Inspect the renderer's last `typing:telemetry` event for the affected scene; lower the priority multiplier in `structural_inflection.rs` if false-positive rate is too high.

---

## 11. Single-shot LLM paths bypass rate-limit + circuit-breaker

**What it is.** `LlmRouter::generate_raw_with_default` and `generate_structured_with_default` (added in T26 for the M2 orchestrator) look up the primary provider via `Arc::clone` and call its trait method directly. They do NOT route through `ProviderState`'s rate-limiter (`bucket.try_take()`) or circuit-breaker (`breaker.allow()`), unlike `generate_bouquet` which does.

**Where it lives.** `crates/water-core/src/llm/router.rs::generate_raw_with_default` and `generate_structured_with_default` (around lines 178-222).

**Why it's fragile.** The orchestrator (`app/src-tauri/src/orchestrator_service.rs`) drives one LLM call per telemetry tick that passes the trigger gate. On a chatty trigger + a typing flurry, the primary provider gets called many times per minute with no rate limiting, and a flaky primary won't trip the breaker.

**What success looks like.** Both single-shot paths route through the same `ProviderState`-based dispatch `generate_bouquet` uses. Likely fold them into one core method with two adapters (n=1 plain prose, n=3 structured), or wrap them in the rate-limit/breaker pipeline.

**First-look mitigations.** If a user reports rate-limit errors from their cloud provider during heavy typing, suspect this. The M2 dispatch path is less protected than M1's `provider_test`. Manual workaround: configure a different primary provider.

**Surfaced by the M2 milestone review (Task 30). Targeted for `m2.0.1` patch or M3 follow-up.**

---

## 12. CommonMark emphasis tokenizer is pragmatic, not strict

**What it is.** The M2.5 inline mark parser at `app/src/editor/serialize.ts::markdownToInlineNodes` recognizes `**`, `*`, and `[text](url)` with simplified left-/right-flanking rules. Three specific deviations from strict CommonMark:

1. **Expanded flank-char class.** `isFlankCharOutside` includes `*`, `_`, `` ` ``, `~`, `\` (strict CommonMark would compute delimiter-run flanking differently).
2. **Em-before-strong heuristic.** When encountering `***` and the innermost active mark is `em`, the parser prefers closing em (single `*`) first. This is coupled to T2's serializer `MARK_PRIORITY = ["link", "strong", "em"]` — if MARK_PRIORITY changes, this heuristic breaks.
3. **Recursive mark parsing inside link text.** The spec § 6.2 says "Link span (atomic; doesn't nest)" but the parser DOES tokenize `*`/`**` inside `[...]` text. Required because T2's serializer emits marks inside link wrappers (e.g., a text run marked both link+strong serializes as `[**text**](url)`).

Edge cases like `a*b*c` (intra-word emphasis without surrounding whitespace) still parse as literal asterisks rather than `<em>b</em>`.

**Where it lives.** `app/src/editor/serialize.ts::tokenizeInline` and `::markdownToInlineNodes`. 50-iteration round-trip property test at `app/src/editor/serialize.test.ts`.

**Why it's fragile.** Strict CommonMark spec is complex; the pragmatic subset shipped in M2.5 handles 99% of literary use. The remaining 1% are corner cases mostly seen in code/technical writing, which isn't Water's target.

**What success looks like.** Writers don't encounter parse surprises in normal literary prose. The property test passes consistently. Round-trip from PM doc → markdown → PM doc → markdown produces identical second-pass output.

**First-look mitigations.**

1. If a writer hits an edge case, suggest wrapping with whitespace or using bold (`**`) instead.
2. Upgrade path: replace `markdownToInlineNodes` with `markdown-it`-based tokenizer or adopt `prosemirror-markdown` library wholesale. ~80 LoC swap.

---

## 13. React 18 deleted-tree useEffect cleanup runs parent-first, not child-first

**What it is.** In React 18, when a component subtree is unmounted as part of a deletion (e.g. parent re-renders without rendering the subtree), passive-effect cleanups inside the deleted subtree run **parent-first**, not child-first as commonly assumed. The path is `commitPassiveUnmountEffectsInsideOfDeletedTree_begin` (top-down) rather than the normal unmount path (bottom-up).

**Where it lives.** Encountered during M2.5 T6 when `<SelectionToolbar>` (child of `<Editor>`) wraps the PM `EditorView`'s `dispatchTransaction` and tries to restore the original on cleanup. If `<Editor>` calls `view.destroy()` synchronously in its own effect cleanup, the toolbar's `setProps` call lands on an already-destroyed view → `view.docView` is null → crash.

**Why it's fragile.** Standard React idioms ("clean up resources you own in your useEffect cleanup") implicitly assume children clean up first. Composing children that capture refs to imperative resources owned by the parent silently breaks under React 18's deleted-tree path.

**Where the workarounds live.**

1. `app/src/editor/Editor.tsx` defers `view.destroy()` to a `queueMicrotask` while immediately nulling `viewRef.current` and `setViewReady(false)`. Child cleanups (still synchronous) run first against a still-alive view; then the microtask destroys it.
2. `app/src/editor/SelectionToolbar.tsx` has an `if (editorView.isDestroyed) return;` defense-in-depth guard at the start of its cleanup.

**What success looks like.** Tests don't crash on Editor unmount. Both `Editor.test.tsx` and `SelectionToolbar.test.tsx` exercise the unmount path without errors.

**First-look mitigations.**

1. If a future refactor removes the microtask defer in Editor.tsx, the `isDestroyed` guard in SelectionToolbar.tsx catches the crash.
2. If you add another child component that captures the PM view via wrapping (e.g., a future plugin manager or a debug overlay), copy the same guard pattern: check `view.isDestroyed` at the start of any cleanup that calls into the view.
3. Future React versions may change the deleted-tree cleanup order; revisit assumptions when upgrading.

---

## 14. Link mark `toDOM` override drops user-supplied `title` attribute

**What it is.** M2.5 T7 overrides the link mark's `toDOM` in `app/src/editor/schema.ts` to add a "Cmd-click to open" / "Ctrl-click to open" tooltip hint. The override unconditionally sets `title` to the hint text, so any user-supplied `title` attribute on a link mark would be silently replaced.

**Where it lives.** `app/src/editor/schema.ts`'s link mark `toDOM`.

**Why it's fragile.** The current LinkPopup UI doesn't accept a title input, so no user-supplied titles exist today. Benign in M2.5. Will bite if any feature adds title support (M7 settings, character voice prompts, world bible entries) without also updating the override.

**What success looks like.** Either the override preserves user titles (`title: mark.attrs.title ?? hint`), OR the title input is never added without updating the override.

**First-look mitigations.**

1. Cheap fix: `title: mark.attrs.title ?? (isMac ? "Cmd-click to open" : "Ctrl-click to open")`. Land alongside any future feature that introduces title input.

---

## 15. Scene-character autosuggest is name-string-matching, not co-reference resolution

**What it is.** M3 T14 added `crates/water-core/src/character/autosuggest.rs::suggest_for_scene_body`. It scans a scene body using case-sensitive word-boundary regex (`\b<name>\b`) over `full_name` + each `alias`. Pronouns ("he", "her", "they"), possessives, and descriptions ("the archivist") don't match. Characters referenced only by pronoun in a scene won't be suggested.

**Where it lives.** `crates/water-core/src/character/autosuggest.rs` (the scanner) + `app/src/scenes/SceneAutosuggestChips.tsx` (the renderer).

**Why it's fragile.** Writers naturally use pronouns after the first mention. A scene where Marcus is named once at the top and then referred to as "he" for the next 600 words will only register `mention_count = 1` for Marcus. If the suggestion threshold ever changes from "any mention > 0" to "≥ K mentions" (currently it surfaces everyone with at least one match, top 5), pronoun-heavy scenes will hide their actual cast.

`\b` is Unicode-aware in Rust's `regex` crate, so a CJK name embedded inside a longer run of word characters (`李` inside `李明`) won't match either. Same family of limitation.

**What success looks like.** The chip surface for `SceneMetadataSheet` is an *assist*, not an *authority* — the writer always retains the multi-select checkbox row underneath. Manual multi-select bridges the gap until M5+'s sidecar co-reference resolution lands.

**First-look mitigations.**

1. If a writer reports "Marcus isn't being suggested for scenes he's clearly in," check whether the scene mentions Marcus by name (case-sensitive, word-bounded) at least once. If not, the chip absence is expected.
2. Until co-ref lands, document the chip strip as "characters detected by name in the current draft" rather than "characters present."
3. Don't introduce a `min_mention_count > 0` threshold without re-evaluating against pronoun-heavy real-world scenes.
4. Upgrade path: M5+ sidecar co-reference model annotates spans → `suggest_for_scene_body` consumes them alongside the regex pass.

---

## 16. Character-voice prompt template injects up to 11 LSM fields per pill

**What it is.** `prompts/speakers/character/template.toml` (M3 T4) substitutes `full_name`, `role_in_story`, `want`, `need`, `lie_they_believe`, `truth`, `fatal_flaw`, `voice`, `speech_patterns`, `fears`, `values` into the prompt fragment per character pill. Empty fields are omitted line-by-line (`render_with_omission`), so the cost varies with how much of the sheet a writer has filled out.

**Where it lives.** `prompts/speakers/character/template.toml` + `crates/water-core/src/voice/character_template.rs`.

**Why it's fragile.** Token cost grows with character complexity. A fully-filled Marcus Vale fixture (see `eval/m3_acceptance/marcus_vale.toml`) produces a sizable system prompt — perfectly tolerable for the current single-character flow, but it scales linearly per pill, and the eval harness has not yet established a token-budget assertion.

If a future writer authors a 200-trait OC and then drops them into every scene, the per-pill cost compounds; the orchestrator currently has no per-pill budget cap.

**What success looks like.** Eval harness (`eval/`) should track per-character pill token usage. If it crosses a threshold (~600 tokens system per pill), M7 settings would add a "concise mode" that omits the `bonus_traits` aux fields and keeps only main + arc + perspectives.

**First-look mitigations.**

1. Pin the token cost via `eval/` snapshots once the harness is wired up (target: M5 evaluation pass).
2. If pill cost surprises a writer, the writer can leave optional LSM fields empty — the template's line-by-line omission already cuts dead lines.
3. Concise-mode flag in M7 settings is the documented escape hatch.

---

## 17. `character_dissonance` Stage 1 uses Jaccard lemma overlap (English-only)

**What it is.** `crates/water-core/src/orchestrator/lemma_overlap.rs` re-exports M2's `anti_loop::{tokenize, jaccard}`. Both are English-only: whitespace-split + ASCII lowercase + a small English stopword set. There is no stemming, no language-aware tokenization, no morphological awareness.

**Where it lives.** `crates/water-core/src/orchestrator/lemma_overlap.rs` (the wrapper) + `crates/water-core/src/orchestrator/anti_loop.rs` (the actual tokenizer).

**Why it's fragile.** Inherits the full caveat of KNOWN_FRAGILE #8 (anti-loop is English-only). Specifically for `character_dissonance` Stage 1, the cheap gate that decides whether to spend an LLM call on a Stage-2 confirmation:

- **German/Dutch/Slavic etc.** — agglutinative compounds (`Eichendorffstraße`) tokenize as one word; the gate misses overlapping concepts that an English equivalent would catch. The gate will under-fire.
- **CJK** — no whitespace tokens at all. Every paragraph hashes to a single one-token "word"; the gate is meaningless. Will dramatically over-fire OR under-fire depending on punctuation.
- **Romance languages** — works passably (whitespace-separated) but loses inflectional matches ("escribiría" vs. "escribir" both contribute different tokens).

**What success looks like.** For English manuscripts, the gate's behavior is well-defined and tested (see `lemma_overlap::overlap` and `character_dissonance::evaluate` tests).

**First-look mitigations.**

1. If a writer reports `character_dissonance` firing implausibly (way too often or never), check the manuscript language. If non-English: confirmed limitation, document as a known gap in the writer's setup notes.
2. Don't lower the gate threshold globally to compensate — it would over-fire on English manuscripts.
3. Upgrade path: M5+'s semantic-embedding gate (planned; superscedes both #8 and #17).

---

## 18. Character-vs-world name-collision resolution is heuristic

**What it is.** When a token in a scene paragraph could resolve to either a character (`character.full_name`) or a world entry (`world_entry.name` / aliases), `crate::orchestrator::collision::resolve_token_kind` applies a deterministic policy: if the scene has the character in `characters_present`, the character wins; otherwise both speakers can fire. The character-in-scene rule is a heuristic — it assumes the writer adds characters to the scene before mentioning them.

**Where it lives.** `crates/water-core/src/orchestrator/collision.rs` (the shared helper) + `crates/water-core/src/orchestrator/triggers/world_drift.rs` (consumer).

**Why it's fragile.** Writers often type the character's name first and add the character to `characters_present` after via the autosuggest chip. During that window — a few seconds at most — the resolver will treat the token as a world hit if a world entry with the same name exists. In practice the autosuggest chip surfaces the missing character within the same debounce cycle, so the window is small. But the asymmetry is real: a `Cartographer` pill could land for "Aren" the location before the writer accepts the "Aren" character chip.

**What success looks like.** When the writer adds characters to scenes before/while mentioning them (the common flow per spec § 20), the resolver routes correctly every time.

**First-look mitigations.**

1. Check the scene's `characters_present` set — if the colliding character isn't there yet, the chip should be visible above the editor.
2. Accept the autosuggest chip; the next paragraph save will route correctly.
3. If both pills fire repeatedly even with the chip accepted, file a bug — that's a real defect in the resolver.

---

## 19. `world_drift` contextual-overlap pre-check can miss real contradictions

**What it is.** Before spending an LLM call on Stage 2 confirmation, `WorldDriftEvaluator` requires the scene paragraph and the world entry's `[main]` text to share at least 2 content words (stopwords stripped). The gate exists to suppress incidental name mentions (the `near_miss` case in the Pell Library fixture) from burning LLM budget.

**Where it lives.** `crates/water-core/src/orchestrator/triggers/world_drift.rs::contextual_overlap`.

**Why it's fragile.** A 2-word threshold drops paragraphs where the writer's contradiction is stylistically distant from the entry's source text. If an entry's `sensory_detail` says "Dust thick enough to read fingertips in" and the writer's contradiction is "She crossed the marble atrium, gleaming white in the afternoon light," the resolver may see no content overlap and suppress — even though "atrium gleaming white" contradicts "dust thick" semantically.

**What success looks like.** The fixture scenes (`eval/m4_acceptance/scenes/*`) all route correctly: contradictions trip the gate; the near-miss is suppressed.

**First-look mitigations.**

1. If a writer reports a missing `world_drift` pill, check the paragraph + entry overlap manually.
2. Don't lower the threshold globally — it would over-fire on near-miss mentions.
3. Upgrade path: M5+'s embedding-similarity pre-check (planned).

---

## 20. Cartographer pill hue follows persona-hue, not segment-hue

**What it is.** Pills emitted by the Cartographer speaker carry `--water-hue-persona-cartographer`, identical regardless of which world segment triggered the drift. A contradiction against a `Locations` entry looks the same as one against a `Concept` entry.

**Where it lives.** `crates/water-core/src/voice/speaker.rs::CartographerSpeaker` + the persona registration in `voice/router.rs`.

**Why it's fragile.** Cosmetic only. Spec § 4.6 doesn't require segment-hued pills; v1 ships persona-hued.

**What success looks like.** The Cartographer pill is recognizably itself; the writer can tell it apart from Echo/Architect/Editor at a glance. Segment-source is conveyed via the pill message text rather than hue.

**First-look mitigations.**

1. If you wanted segment-hued pills (`--water-hue-world-N` round-robin per `world_segment.hue_token`), the path is to extend `CartographerSpeaker::from_row` with an optional `segment_hue` and have the voice router pass it from the trigger snapshot. Deferred to M7 polish unless field-tested writers ask for it.

---

## 21. Unnamed Chorus stubs persist indefinitely

**What it is.** When the writer pins a Chorus + `no_universe_yet` pill, a `locations` `world_entry` is created with `name = ""` and `main.sensory_detail` seeded from the pill snippet. No auto-reaping of these stubs ever runs — they stay in the index as "(unnamed location)" entries until the writer either names them or deletes them.

**Where it lives.** `app/src-tauri/src/commands/pill.rs::pill_pin_core` (creates the stub) + `crates/water-core/src/world/store.rs::create_entry_seeded` (the underlying creator).

**Why it's fragile.** The snippet has value as a fragment even without a name (the whole point of the no-universe-yet flow), but a writer who pins many Chorus pills without elaborating ends up with a Locations grid full of unnamed entries. Could become hygiene debt.

**What success looks like.** Writers typically name a stub within minutes of pinning (the WorldEntrySheet is the first thing they see after pinning, via T30's nav-to-stub event). The unnamed-stub backlog stays small in practice.

**First-look mitigations.**

1. The Locations grid renders unnamed entries as `(unnamed location)` — they're visible.
2. The writer can manually delete via the entry sheet's delete affordance.
3. Upgrade path: M5+ may add an optional "reap unnamed entries > N days" pass.

---

## 22. Case-sensitivity asymmetry between character autosuggest and world_drift

**What it is.** Character autosuggest (M3) is case-sensitive on word boundaries; `WorldRegistry::find_by_token` (and therefore `world_drift` Stage 1) is **case-insensitive**. A character named "Marcus" requires the writer to type "Marcus" (not "marcus"); a location named "The Pell Library" matches "the pell library" too.

**Where it lives.** `crates/water-core/src/character/autosuggest.rs` (case-sensitive) vs. `crates/water-core/src/world/registry.rs::find_by_token` (case-insensitive).

**Why it's fragile.** The asymmetry is intentional: place names are more case-variable in English prose than character names. Writers don't write "marcus" mid-sentence, but they routinely write "the library" mid-sentence to refer to "The Pell Library." However, the asymmetry is non-obvious — a reader of the autosuggest tests + the world tests may assume one or the other applies everywhere.

**What success looks like.** Writers see character chips for proper-case character mentions and world chips for either-case place mentions. Both subsystems behave as designed.

**First-look mitigations.**

1. If a character chip isn't surfacing for a known-character mention, check the casing — the M3 tokenizer requires proper case.
2. If a world chip is surfacing for a stray lowercased place reference, that's expected: the contextual-overlap pre-check (#19) is what suppresses noise, not the case sensitivity.
3. The asymmetry is documented in `world/registry.rs` (module docstring) and the M4 spec § 6.1 (case-sensitivity policy).

---

*(More entries will be added as fragile heuristics are introduced. Keep this file in repo root.)*
