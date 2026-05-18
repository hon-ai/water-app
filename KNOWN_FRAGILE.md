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

*(More entries will be added as fragile heuristics are introduced. Keep this file in repo root.)*
