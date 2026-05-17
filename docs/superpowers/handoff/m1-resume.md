# M1 Foundation — Resume Handoff (v2)

**Updated:** 2026-05-16 (session 2 closed)
**Reason:** Session 2 completed Tasks 3-8 and approached its context budget. A fresh session resumes at Task 9.

This document is everything a new controller needs to pick up where I left off without re-reading the prior conversation.

---

## TL;DR

- Plan lives at `docs/superpowers/plans/2026-05-16-m1-foundation.md`. 40 tasks. **Tasks 1-8 are done.** Resume at **Task 9**.
- Spec lives at `docs/superpowers/specs/2026-05-16-water-design.md`. Locked. Do not change without explicit user approval.
- The user chose **subagent-driven development** (`superpowers:subagent-driven-development` skill). Continue that workflow.
- Toolchain is installed and verified working. No fresh installs needed.
- Branch: `master`. No worktree. Plan executes directly on master with frequent commits.

---

## Repo state

### Commit history (newest first)

| SHA | What |
|---|---|
| `f880dc6` | **feat(core): full v1 schema (16 tables)** (Task 8) |
| `7d30770` | plan(T7): amend Task 7 to add `#[must_use]` / pedantic accommodations |
| `b305c2a` | **feat(core): SQLite connection + migration runner** (Task 7) |
| `b279f71` | plan(T6): amend Task 6 to add `#[must_use]` for clippy::pedantic |
| `ca7e8e5` | fix(core): add `#[must_use]` to `Id::new` and `Id::as_str` for clippy::pedantic |
| `a2ea103` | **feat(core): ULID-backed Id type** (Task 6) |
| `20d1d3c` | plan(T5): amend Task 5 to include tsconfig noEmit fix |
| `88d5890` | **test: renderer + core sanity tests** (Task 5) |
| `d15133d` | fix(app): set noEmit so `tsc -b` stops shadowing TS sources with `.js` |
| `bf36688` | plan(T4): amend Task 4 to include vite.config.ts import fix |
| `e147d28` | fix(app): import `defineConfig` from `vitest/config` so `tsc -b` passes |
| `fbd57f8` | **feat(app): wire Tailwind 4 with placeholder tokens** (Task 4) |
| `4429902` | **chore: pnpm workspace + root scripts + editorconfig** (Task 3) |
| `318fbec` | docs: handoff for M1 resume at Task 3 (session-1 handoff doc) |
| `8830071` | plan(T2): drop `[lib]` block, drop log-plugin shim, require `icon.ico` |
| `ac88d9c` | **feat(app): scaffold Tauri 2 + Vite + React shell** (Task 2) |
| `d226aa9` | chore: commit `Cargo.lock` and document workspace-member deviation |
| `5a06322` | **feat(core): initialize water-core crate and Cargo workspace** (Task 1) |
| `94e1494` | plan: pin rust-toolchain to `stable` (verified locally on 1.95) |
| `6875267` | Add M1 Foundation implementation plan |
| `5310f7e` | Add Water v1 design spec and repo skeleton |

### Tree summary (significant files only)

```
Water/
├── Cargo.toml                       ← workspace; members ["app/src-tauri", "crates/water-core"]
├── Cargo.lock                       ← committed; unchanged since T1 housekeeping
├── rust-toolchain.toml              ← stable
├── pnpm-workspace.yaml              ← packages: ["app"]
├── pnpm-lock.yaml                   ← generated, committed (Task 3)
├── package.json                     ← 7 root scripts (dev/build/test/lint/fmt)
├── .editorconfig
├── .gitignore                       ← includes pnpm, Tauri build, app/dist/
├── README.md
├── KNOWN_FRAGILE.md                 ← initial char_dissonance entry only
├── docs/superpowers/
│   ├── specs/2026-05-16-water-design.md
│   ├── plans/2026-05-16-m1-foundation.md  ← amended 5 times (T2, T4, T5, T6, T7)
│   └── handoff/m1-resume.md         ← this file (v2)
├── crates/water-core/
│   ├── Cargo.toml                   ← all workspace deps wired; ulid + rusqlite + rusqlite_migration in use
│   ├── sql/v1_init.sql              ← full 16-table schema (Task 8)
│   └── src/
│       ├── lib.rs                   ← re-exports Error, Result, Id, Db
│       ├── error.rs                 ← 13-variant Error enum (from T1)
│       ├── id.rs                    ← Id newtype + #[must_use] (Task 6)
│       ├── db.rs                    ← Db wrapper, open/open_in_memory/conn/conn_mut (Task 7)
│       └── migrations.rs            ← rusqlite_migration runner (Task 7)
└── app/
    ├── package.json
    ├── tsconfig.json                ← has "noEmit": true (Task 5 amendment)
    ├── vite.config.ts               ← imports defineConfig from "vitest/config" (Task 4 amendment)
    ├── postcss.config.cjs           ← @tailwindcss/postcss + autoprefixer (Task 4)
    ├── tailwind.config.ts           ← borderRadius + fontFamily (Task 4)
    ├── index.html
    ├── src/
    │   ├── main.tsx
    │   ├── App.tsx                  ← renders <h1>Water</h1>; has unused className="water-shell"
    │   ├── test-setup.ts            ← imports @testing-library/jest-dom/vitest (Task 5)
    │   ├── App.test.tsx             ← 1 test (Task 5)
    │   └── styles/tokens.css        ← @import "tailwindcss" + placeholder vars (Task 4)
    └── src-tauri/
        ├── Cargo.toml
        ├── tauri.conf.json
        ├── build.rs
        ├── icons/icon.ico           ← 766-byte placeholder; replace in M7
        ├── gen/                     ← gitignored
        └── src/main.rs
```

### Verified working at HEAD (`f880dc6`)

- `cargo build -p water-core` → succeeds.
- `cargo build -p water-app` → succeeds.
- `cargo test -p water-core` → **9 passed** (version + 4 ULID + 4 db/migration).
- `cargo clippy -p water-core --all-targets -- -D warnings` → clean.
- `cargo clippy --all-targets -- -D warnings` (full workspace) → clean (confirmed after T6 fix; not re-confirmed every task).
- `pnpm --filter @water/app test` → 1 passed (vitest).
- `pnpm --filter @water/app build` → succeeds end-to-end (tsc -b + vite build).

---

## Toolchain inventory

All confirmed working as of handoff:

| Tool | Version | Path |
|---|---|---|
| rustc | 1.95.0 | `%USERPROFILE%\.cargo\bin\rustc.exe` |
| cargo | 1.95.0 | `%USERPROFILE%\.cargo\bin\cargo.exe` |
| rustup | 1.29.0 | `%USERPROFILE%\.cargo\bin\rustup.exe` |
| node | v22.14.0 | `C:\Program Files\nodejs\node.exe` |
| pnpm | 9.15.9 | `%APPDATA%\npm\pnpm.cmd` |
| uv | (latest) | `C:\Users\H BLAUNTE\.local\bin\uv.exe` |
| git | latest | `C:\Program Files\Git\cmd\git.exe` |

**PowerShell PATH note:** Each `bash` tool invocation is a fresh PowerShell. `cargo` is normally on user PATH but fresh sessions may not see it. Workaround used in subagent prompts:

```powershell
$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"
```

`pnpm` is reliably on PATH via `%APPDATA%\npm`.

MSVC linker / Visual Studio Build Tools: verified present (Tauri 2 build succeeded multiple times).

---

## Plan amendments made during execution

The plan file has been amended **five times** during execution. Each amendment is a separate git commit and is reflected in the plan file content at HEAD — re-read the plan and you'll see all amendments already applied.

| Commit | Amendment |
|---|---|
| `94e1494` | T1 prerequisites: pin `rust-toolchain.toml` to `stable` (not `1.78`). |
| `8830071` | T2: drop `[lib]` block from `app/src-tauri/Cargo.toml`; drop `tauri_plugin_log_init` shim from `main.rs`; require `"icons/icon.ico"` in `tauri.conf.json` bundle.icon. |
| `bf36688` | T4: add Step 0 — change `app/vite.config.ts` `defineConfig` import from `"vite"` to `"vitest/config"` so `tsc -b` types the `test` block. |
| `20d1d3c` | T5: add Step 0 — add `"noEmit": true` to `app/tsconfig.json` so `tsc -b` doesn't shadow TS sources. |
| `b279f71` | T6: add `#[must_use]` to `Id::new` and `Id::as_str` (clippy::pedantic `must_use_candidate`). |
| `7d30770` | T7: add `#[must_use]` to `migrations::all`, `Db::conn`, `Db::conn_mut`; backtick `` `SQLite` `` in db.rs module doc (clippy::pedantic `doc_markdown`). |

**Pattern observed:** Plan listings that were written without running `cargo clippy --all-targets -- -D warnings` repeatedly trip pedantic lints on new code. The next controller should **proactively add `#[must_use]` to plan-listed methods returning non-`Result` owned values or references** when dispatching implementers, to avoid round-trips.

---

## Sharp edges / known issues

1. **CRLF/LF git warnings** on every `git add` — environmental, ignore.
2. **Cargo.lock is committed.** New deps modify it; include in `git add` if you add deps. Tasks T1-T8 haven't needed new deps; all workspace deps were pre-declared in `Cargo.toml`.
3. **`app/src-tauri/icons/icon.ico`** is a 766-byte placeholder; replace in M7.
4. **`Error::Other(String)` variant** in `crates/water-core/src/error.rs` is a known footgun (T1 review). T6's `Id::FromStr` uses it; future tasks should prefer `Error::InvalidId` or similar named variants. Acceptable for v1; revisit at M7.
5. **`clippy::pedantic` is enabled** at crate level in `water-core/src/lib.rs`. Frequent fires: `must_use_candidate`, `doc_markdown`. Test code is exempt from many; library code is not. Pattern: add `#[must_use]` to plan-listed pub fns proactively.
6. **Plan invocation command bug:** plan T7 Step 5 says `cargo test -p water-core db::tests migrations` — cargo test accepts ONE filter, not multiple. The implementer ran without args; the test names were specific enough. Future plan-invocations may have similar quirks; trust the test result, not the invocation text.
7. **Commit message wording:** plan T8 Step 5 prescribed `feat(core): v1 schema migration with all spec tables`. Controller dispatched `feat(core): full v1 schema (16 tables)` instead — accepted as defensible. **Lesson for next controller: quote plan's exact commit-message string in dispatch prompts.**
8. **Unused className "water-shell"** in `app/src/App.tsx` (line 3). No CSS rule targets it. T34/T35 (design tokens + ThemeProvider) will likely replace `App.tsx` or wire styles; not blocking.
9. **stale `.js` artifacts** were a problem prior to T5's `noEmit:true` fix. They no longer appear; if any reappear, something regressed.
10. **WAL sidecars** in tempfile tests: `Db::open` enables WAL mode, which creates `<path>-wal` and `<path>-shm` siblings. The test `file_db_persists_across_opens` deletes only the main `.db` file; siblings leak to tmp. Plan-acknowledged, harmless.
11. **`schema_version` table** is hand-managed (T8 SQL inserts row `(1)`), independent from `rusqlite_migration`'s `PRAGMA user_version` tracking. **V2+ migrations must `INSERT INTO schema_version` manually**, or the library won't do it. Worth a one-line comment in `migrations.rs` when V2 lands.
12. **Cascade index gaps**: `scene_character_presence(character_id)`, `world_entry(segment_id)`, `pinned_pill(scene_id)`, `block_metrics(scene_id)` all rely on full scans for FK CASCADE on the parent side. Acceptable at M1 volumes. Add indexes if cascade latency surfaces.
13. **`schema_version_row_is_one` test** uses `query_row` which returns the first arbitrary row; would silently pass if a future migration inserts `version=2`. When V2 lands, retighten to `MAX(version)` or `COUNT(*)=1 AND version=1`.

---

## Subagent prompt conventions established this session

The controller settled on a stable shape for prompts; the next controller should reuse it:

**Implementer prompt:**
- Inline the full task text verbatim from the plan.
- Inline the "Context" block: working directory + workdir reminder, branch, current commit, prior-state file pointers, sharp edges relevant to the task.
- Inline proactive clippy guidance for any plan-listed `pub fn` returning non-`Result` owned values or references — tell them to add `#[must_use]` to specific named functions to avoid the lint round-trip.
- Inline an explicit "Self-Review Checklist."
- Ask for a concise report (DONE status + summary + files + SHA + test/clippy tails + concerns).
- Pre-authorize obvious follow-on changes that flow naturally from the task (like T8's update to `in_memory_db_runs_migrations` — the placeholder test from T7 had to change because T7's placeholder table no longer exists).

**Spec reviewer prompt:**
- Inline the full task spec.
- Inline the implementer's claims (commit SHA + file count + test count).
- Tell them to read files at HEAD and compare line-by-line.
- Tell them to confirm commits match (split, message, file list).
- Tell them to re-run tests + clippy themselves.
- Acknowledge any controller-induced known divergences (like T8's commit message) up front so the reviewer flags but doesn't churn.

**Code quality reviewer prompt:**
- Base SHA + Head SHA of the task's commits.
- Categorize concerns: Critical / Important / Minor.
- "Don't flag" list to keep the review focused on this task's contributions.

---

## How to resume — exact next-step prompt

Open a fresh session. After acknowledging the conversation start (per `using-superpowers`), the next controller should issue this prompt to itself or to a fresh subagent:

> Resume execution of the Water M1 Foundation plan at Task 9.
>
> - Plan: `docs/superpowers/plans/2026-05-16-m1-foundation.md`
> - Spec: `docs/superpowers/specs/2026-05-16-water-design.md`
> - Handoff context: `docs/superpowers/handoff/m1-resume.md` (this file, v2)
> - Tasks 1-8 are committed on `master`. HEAD is `f880dc6`.
> - Use the `superpowers:subagent-driven-development` skill — fresh implementer subagent per task with full task text inlined; spec reviewer; code quality reviewer; mark complete; next task.
> - Continuous execution per the skill's rule. Stop only for BLOCKED, ambiguity, or context exhaustion.
> - When context starts to run low, write another handoff doc and stop cleanly.
>
> Start by:
> 1. Loading the `subagent-driven-development` skill.
> 2. Reading this handoff doc and the plan's Task 9 section (around line 1375).
> 3. Updating the TodoWrite (T1-T8 completed, T9 in progress, T10-T40 + final review pending).
> 4. Dispatching the Task 9 implementer with full task text inlined.
> 5. Following the subagent prompt conventions documented above (proactive `#[must_use]` guidance for any new pub fns).

---

## Remaining task index

32 remaining tasks + final review:

| # | Phase | Task | Notes |
|---|---|---|---|
| 9 | B | ProjectStore + ManuscriptStore CRUD | TDD; first real CRUD against the schema |
| 10 | C | `water.toml` read/write | TDD |
| 11 | C | Scene Markdown codec (frontmatter + body) | YAML frontmatter via serde_yaml |
| 12 | C | `^bk-XXXX` block-ID maintenance | Will append entry 2 to KNOWN_FRAGILE.md |
| 13 | C | SceneStore (create/read/write_body/move_to/list) | Composes blocks + scene_md + DB |
| 14 | C | `chapters.toml` read/write | TDD |
| 15 | C | CharacterStore + WorldStore | M1 thin TOML-only surface |
| 16 | D | Snapshot writer (zstd) | TDD |
| 17 | D | Snapshot retention pruner | Hourly/daily/weekly policy |
| 18 | D | Snapshot scheduler (tokio task) | `tokio::test(start_paused=true)` |
| 19 | D | Snapshot restore (creates pre-restore) | TDD |
| 20 | E | Rebuild-from-truth (scan folder → repopulate) | Integration-shaped test |
| 21 | E | External-edit repair pass | Builds on Rebuild |
| 22 | F | Sidecar scaffold with `uv` | Creates `sidecar/` workspace |
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
| 34 | I | Pastel-glow design tokens (CSS variables) | **Watch for:** add `2xl: '40px'` and `3xl: '64px'` (or appropriate) to tailwind.config.ts borderRadius — T4 code review flagged that extend.borderRadius doesn't override Tailwind defaults for `2xl`/`3xl` |
| 35 | I | ThemeProvider (light/dark/auto) | React Testing Library tests |
| 36 | J | AppState + project commands | Tauri commands |
| 37 | J | Scene + provider + diagnostics commands | Tauri commands |
| 38 | J | SceneList + Diagnostics UI + IPC client | First end-user surfaces |
| 39 | K | M1 exit-criteria integration tests | 4 tests + 1 `#[ignore]` |
| 40 | K | Manual acceptance checklist + final sanity | docs + build |
| — | — | Final code review of entire M1 implementation | Per skill end gate |

---

## Session 2 statistics

For calibration of the next controller's budget planning:

- **Tasks completed:** T3, T4, T5, T6, T7, T8 (six tasks).
- **Mid-execution plan amendments required:** five (T4, T5, T6, T7, plus T8 had a controller-only commit message divergence).
- **Implementer "DONE" first time, no fixes:** T3, T5, T8 (3/6).
- **Implementer "DONE_WITH_CONCERNS" requiring a follow-on fix:** T4 (vite.config typing), T6 (clippy must_use), T7 (3 pedantic fires) — 3/6.
- **Average commits per task:** 2 (one feat + one plan amendment).
- **Build/test pipeline health:** consistently green at end of each task. Tests grew from 1 → 9. Clippy clean.

**Heuristic:** A task with a 200-line plan listing + tests consumes roughly 12-20k tokens of controller context end-to-end (implementer + 2 reviewers + my reasoning). Five-task budgets per session are realistic.

If the user asks "where are we" the answer is: **"T1-T8 done, T9 next. Six tasks completed in session 2 + two in session 1 = 8 of 40. Plan amended five times. See `docs/superpowers/handoff/m1-resume.md`."**
