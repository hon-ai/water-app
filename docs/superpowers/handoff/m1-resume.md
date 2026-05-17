# M1 Foundation — Resume Handoff (v4)

**Updated:** 2026-05-17 (session 4 closed)
**Reason:** Session 4 completed Tasks 13-15 plus a fmt cleanup commit, and approached its context budget. A fresh session resumes at Task 16 (Snapshot writer — Phase D).

This document is everything a new controller needs to pick up where I left off without re-reading the prior conversation.

---

## TL;DR

- Plan: `docs/superpowers/plans/2026-05-16-m1-foundation.md`. 40 tasks. **Tasks 1-15 are done.** Resume at **Task 16** (Phase D — Autosave + Snapshots starts here).
- Spec: `docs/superpowers/specs/2026-05-16-water-design.md`. Locked.
- Workflow: **subagent-driven-development** (`superpowers:subagent-driven-development` skill).
- Branch: `master`. No worktree.

---

## Repo state

### Session 4 commit history (newest first)

| SHA | What |
|---|---|
| `8bc7bf3` | plan(T15): amend Task 15 with upsert_segment ULID-slug doc + idempotency test |
| `185159a` | fix(core): clarify and test upsert_segment ULID-slug semantics |
| `9f78650` | **feat(core): Character and World stores (M1 surface)** (Task 15) |
| `616d808` | **style: cargo fmt water-core to clean pre-existing drift** (opportunistic cleanup) |
| `21fd38e` | **feat(core): chapters.toml read/write** (Task 14) |
| `1cace83` | plan(T13): amend Task 13 to refresh file_hash on move_to |
| `1dfacb4` | fix(core): refresh scene file_hash on move_to |
| `6fb2ed9` | **feat(core): SceneStore with on-disk + DB persistence** (Task 13) |
| `fa36343` | docs: handoff v3 for M1 resume at Task 13 (session-3 handoff) |

Earlier commits documented in handoff v3 (in git history).

### Tree summary (significant files only, additions in session 4 marked ★)

```
Water/
├── Cargo.toml                       ← workspace; deps unchanged from v3 (sha2 added directly to water-core)
├── Cargo.lock                       ← committed; modified once this session (sha2 dep, +1 line)
├── rust-toolchain.toml              ← stable
├── pnpm-workspace.yaml
├── pnpm-lock.yaml
├── package.json
├── .editorconfig
├── .gitignore
├── README.md
├── KNOWN_FRAGILE.md                 ← 2 entries (char_dissonance + block-id duplicate tolerance)
├── docs/superpowers/
│   ├── specs/2026-05-16-water-design.md
│   ├── plans/2026-05-16-m1-foundation.md  ← amended 9 times (T2,T4,T5,T6,T7,T9,T10,T13,T15)
│   └── handoff/m1-resume.md         ← this file (v4)
├── crates/water-core/
│   ├── Cargo.toml                   ← + sha2 = "0.10" (T13 added directly, not workspace-style)
│   ├── sql/v1_init.sql              ← full 16-table schema (T8)
│   └── src/
│       ├── lib.rs                   ← re-exports + #![allow(clippy::missing_errors_doc)]
│       ├── error.rs                 ← 13-variant Error enum
│       ├── id.rs                    ← ULID Id newtype
│       ├── db.rs                    ← Db wrapper
│       ├── migrations.rs            ← rusqlite_migration runner
│       ├── project.rs               ← ProjectStore + ManuscriptStore (T9, 6 tests)
│       ├── water_toml.rs            ← WaterToml read/write (T10, 2 tests)
│       ├── scene_md.rs              ← SceneFile + SceneFrontmatter codec (T11, 4 tests)
│       ├── block.rs                 ← Block + ensure_block_ids/split_blocks (T12, 5 tests)
│       ★ scene.rs                   ★ SceneStore (T13, 4 tests; hash_file is pub(crate))
│       ★ chapters.rs                ★ ChaptersFile codec (T14, 2 tests)
│       ★ character.rs               ★ CharacterStore (T15, 3 tests; toml::Table flatten)
│       └── world.rs                 ★ WorldStore (T15, 2 tests; segments only, no entries)
└── app/                              ← unchanged this session
    ├── package.json, tsconfig.json, vite.config.ts, postcss.config.cjs, tailwind.config.ts
    ├── index.html
    ├── src/ (App.tsx, App.test.tsx, test-setup.ts, main.tsx, styles/tokens.css)
    └── src-tauri/ (Cargo.toml, tauri.conf.json, build.rs, icons/icon.ico, src/main.rs)
```

### Verified working at HEAD (`8bc7bf3`)

- `cargo build -p water-core` → succeeds.
- `cargo test -p water-core` → **37 passed** (was 26 at end of session 3; +11 across T13/T14/T15).
- `cargo clippy -p water-core --all-targets -- -D warnings` → clean.
- `cargo fmt -p water-core --check` → **clean** (no drift, post-`616d808` baseline).
- App-side stack — not re-verified this session (T13-T15 are core-only). Last verified end of session 2.

---

## Toolchain inventory

Unchanged from v2/v3. PowerShell PATH workaround still required:
```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"
```

---

## Plan amendments made during execution

The plan file has been amended **nine times** during execution (was 7 at end of session 3; +2 this session). Each amendment is a separate git commit, and the plan content at HEAD already reflects all of them.

| Commit | Amendment |
|---|---|
| `94e1494` | T1 prereq: pin toolchain to `stable`. |
| `8830071` | T2: drop `[lib]` block; drop log-plugin shim; require `icons/icon.ico`. |
| `bf36688` | T4: vite.config.ts imports `defineConfig` from `vitest/config`. |
| `20d1d3c` | T5: add `"noEmit": true` to `app/tsconfig.json`. |
| `b279f71` | T6: add `#[must_use]` to `Id::new` / `Id::as_str`. |
| `7d30770` | T7: `#[must_use]` on `migrations::all` / `Db::conn` / `Db::conn_mut`; backtick `` `SQLite` ``. |
| `661d571` | T9: `set_default_manuscript` enforces project↔manuscript pairing via EXISTS subquery; +2 tests. |
| `a68c17f` | T10: `WaterToml::read` wraps `toml::from_str` errors in `Error::InvalidProject` with path. |
| `1cace83` | **T13: `move_to` refreshes `scene.file_hash` after disk write; test strengthened to assert hash changes.** |
| `8bc7bf3` | **T15: `upsert_segment` gains doc comment naming ULID-slug overload contract; +1 idempotency test exercising the ON CONFLICT branch.** |

**Patterns reinforced this session:**

- The "fix the code-review finding + amend the plan" pattern continues to deliver clarity: T13 found a real DB/disk consistency gap; T15 found a documentation/coverage gap. Both got immediate fixes before moving on.
- **Combined spec+quality review** in ONE dispatch worked for all three session-4 tasks (even T13, the integration task). The combined reviewer is given a "DEEP review" framing for integration tasks and a terse framing for codec tasks; both delivered.
- **Opportunistic `style:` cleanup commits** are cheap and worth doing when an implementer surfaces fmt drift in unrelated files. `616d808` cleaned 5 files of pre-existing drift in ~10s of work. Future sessions should keep the `cargo fmt --check` baseline green.

---

## Sharp edges / known issues

Existing entries from v3 (still apply) — references 1-22 in handoff v3 carry forward verbatim.

New entries discovered in session 4:

23. **`SceneStore::move_to` now refreshes `file_hash`** (T13 fix). Future operations that mutate the file directly (e.g., external editor edits picked up by T21 repair) must also keep `file_hash` honest, or T20's rebuild will rewrite the DB row unnecessarily.

24. **`WorldStore::upsert_segment` has overloaded `slug` semantics** (T15 amendment): pass a ULID for idempotent upsert; pass anything else (e.g. `"concept"`) and you get a fresh row per call. Real-world callers will need a stable ULID slug derivation strategy — likely M2 territory when world segment ids are seeded via Conversational Intake.

25. **`CharacterStore::delete` uses `.ok()` on `fs::remove_file`** to swallow filesystem errors (intentional for idempotency). This conflates "file already gone" with "permission denied" / "I/O error." M1-acceptable; future hardening could match `ErrorKind::NotFound` explicitly and propagate other variants.

26. **`CharacterFile::data` is `toml::Table` serialized to JSON** for the `character.data_json` index column. TOML-only types (datetime) would lose precision through the conversion. M1 character data is text-only; problem deferred until schema grows date fields.

27. **No round-trip test for `CharacterFile` via TOML** (T15 minor): the 3 character tests `upsert` and inspect the DB row only; none reads back the written `.toml` and asserts the `#[serde(flatten)] data` table survives. T20 rebuild-from-truth will exercise this, but a dedicated unit test would be cheap insurance.

28. **`hash_file` lives in `scene.rs` as `pub(crate)`** and is consumed by `character.rs::upsert`. If a third caller appears, consider hoisting to a shared `crate::util` or `crate::fs_util` module.

29. **`WorldStore::project_root` is a `&PathBuf` getter** with no internal consumer at M1. Presumably wired up by T20/T21 to locate on-disk world files. Fine as-is; just a heads-up that it's currently dead-weight to the crate internals.

30. **`Cargo.lock` shrinkage on `sha2` add was only +1 line** because `sha2` and all its transitive deps (`digest`, `block-buffer`, `crypto-common`, `generic-array`, `typenum`, `cpufeatures`) were already in the lockfile via another crate (likely keyring or rusqlite_migration). Not an issue, just unusual.

31. **FK rejection negative tests are still missing** for `scene`, `character`, `world_segment`. T9 has one for manuscript; the others rely on the schema's `PRAGMA foreign_keys=ON` being correctly wired by `Db::open_in_memory`. Coverage gap; not blocking.

32. **fmt baseline is now clean** (post-`616d808`). Future implementers should run `cargo fmt -p water-core --check` as a pre-commit step and reject any drift in pre-existing files. The `--check` is now in every implementer prompt's self-review checklist.

---

## Subagent prompt conventions (refined further in session 4)

The v2/v3 conventions hold. Session 4 added:

- **For integration tasks (T13, T15)**: tell the combined reviewer it's an integration task and to "go deep on integration concerns" (orphan files, hash drift, cross-table consistency, FK enforcement). For codec tasks (T14), the terse framing is fine.
- **The 3-commit pattern for tasks with review findings** is now standard: `feat(...)` → `fix(...)` → `plan(TN):`. Reserved for IMPORTANT or cheap-MINOR findings; bare-MINOR observations get noted and skipped.
- **Pre-authorize `cargo fmt -p water-core` only on the implementer's new files.** Tell them explicitly to NOT introduce drift in unrelated pre-existing files. The session-4 fmt baseline (`616d808`) clean the previously drifted files.
- **For tasks creating multiple files in one commit (T15: character.rs + world.rs)**: list both files in the implementer prompt's pre-authorized adaptations and self-review checklist. The implementer should report file sizes for both.
- **For schema-touching tasks**: confirm the `CREATE TABLE` columns in `sql/v1_init.sql` match the implementer's INSERT before dispatch. Saves a rare but expensive "schema mismatch" bug round.

---

## How to resume — exact next-step prompt

Open a fresh session. After acknowledging the conversation start (per `using-superpowers`), issue this prompt:

> Resume execution of the Water M1 Foundation plan at Task 16.
>
> - Plan: `docs/superpowers/plans/2026-05-16-m1-foundation.md`
> - Spec: `docs/superpowers/specs/2026-05-16-water-design.md`
> - Handoff: `docs/superpowers/handoff/m1-resume.md` (this file, v4)
> - Tasks 1-15 are committed on `master`. HEAD is `8bc7bf3`.
> - 37 tests passing; clippy + fmt clean.
> - Use the `superpowers:subagent-driven-development` skill.
> - When context starts to run low, write handoff v5 and stop cleanly.
>
> Start by:
> 1. Loading the `subagent-driven-development` skill.
> 2. Reading this handoff doc and the plan's Task 16 section (line 3004).
> 3. Updating the TodoWrite (T1-T15 completed, T16 in progress, T17-T40 + final review pending).
> 4. Dispatching the Task 16 implementer with full task text inlined.
> 5. Following the subagent prompt conventions documented above.
>
> Note: Task 16 (Snapshot writer with zstd) starts Phase D — the autosave/snapshot subsystem. Likely new deps: `zstd` (already a workspace dep per session 1) and possibly `tar`. Watch for clippy on the binary IO loop.

---

## Remaining task index

25 remaining tasks + final review:

| # | Phase | Task | Notes |
|---|---|---|---|
| 16 | D | Snapshot writer (zstd) | TDD; `zstd` workspace dep exists |
| 17 | D | Snapshot retention pruner | Hourly/daily/weekly |
| 18 | D | Snapshot scheduler (tokio task) | `tokio::test(start_paused=true)` |
| 19 | D | Snapshot restore (creates pre-restore) | TDD |
| 20 | E | Rebuild-from-truth (scan folder → repopulate) | First exercise of orphan-file resolution |
| 21 | E | External-edit repair pass | Builds on Rebuild |
| 22 | F | Sidecar scaffold with `uv` | Creates `sidecar/` Python workspace |
| 23 | F | `/analyze` stub + pytest tests | Deterministic stub for M2 wiring |
| 24 | F | Shared IPC contract (`water-core/src/ipc.rs`) | TDD |
| 25 | G | `Sidecar` handle (managed + external modes) | wiremock for unit tests |
| 26 | G | `SidecarSupervisor` (watch channel) | TDD |
| 27 | H | `LlmProvider` trait + `CannedProvider` | Foundation for all adapters |
| 28 | H | Anthropic adapter | wiremock |
| 29 | H | OpenAI adapter | wiremock |
| 30 | H | Ollama adapter | wiremock |
| 31 | H | llama.cpp adapter | wiremock |
| 32 | H | MLX adapter (feature-flagged stub) | `--features mlx` |
| 33 | H | Router with secrets, rate limit, circuit breaker | TDD |
| 34 | I | Pastel-glow design tokens (CSS variables) | **Watch for:** `tailwind.config.ts borderRadius 2xl/3xl gap** flagged in T4 review |
| 35 | I | ThemeProvider (light/dark/auto) | RTL tests |
| 36 | J | AppState + project commands | Tauri commands |
| 37 | J | Scene + provider + diagnostics commands | Tauri commands |
| 38 | J | SceneList + Diagnostics UI + IPC client | First end-user surfaces |
| 39 | K | M1 exit-criteria integration tests | 4 tests + 1 `#[ignore]` |
| 40 | K | Manual acceptance checklist + final sanity | docs + build |
| — | — | Final code review of entire M1 implementation | End gate |

---

## Session statistics

### Session 4 (this one)

- **Tasks completed:** T13, T14, T15 (three tasks).
- **Commits this session:** 8 task-commits + 1 fmt-cleanup + 1 handoff = 10.
- **Implementer DONE first time, no fixes:** T14, T15 impl (2/3) — though T15 had a follow-up fix.
- **Tasks with follow-up `fix(...)` commit:** T13 (file_hash refresh), T15 (upsert_segment doc + test).
- **Plan amendments needed:** 2 (T13 hash refresh, T15 doc clarification).
- **Combined spec+quality reviews used:** all three tasks.
- **Tests grew:** 26 → 37 (+11 across three tasks).
- **No build/test regressions** at any point.

### Cumulative across sessions

- **Tasks complete:** 15 of 40 (37.5%).
- **Plan amendments total:** 9.
- **Tests in `water-core`:** 37 passing; clippy + fmt clean.

### Heuristic for next controller

For Phase D (T16-T19), expect snapshot tasks to be relatively independent but with crash-safety considerations:
- T16 likely a clean DONE task (~25k tokens).
- T17 retention pruning has time-window edge cases (60s/7d/90d/forever) — expect at least one review finding.
- T18 scheduler uses tokio paused-time tests — first async task in M1; budget extra for unfamiliar territory.
- T19 restore creates pre-restore snapshot — integration-flavored; budget like T13.

**Plan for ~30-40k average per task this phase. Could potentially do 3 tasks per fresh session.**

**If the user asks "where are we"** the answer is: **"T1-T15 done, T16 (snapshot writer) next. 37 tests passing. Plan amended 9 times. See `docs/superpowers/handoff/m1-resume.md`."**
